use crate::schema::{Relationship, Table};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowType {
    Import,              // File -> Database table
    StagingToProduction, // Staging table -> Production table
    Aggregation,         // Multiple sources -> Aggregated table
    Transformation,      // Source -> Transformed -> Destination
    Lookup,              // Reference/lookup table pattern
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub table_name: String,
    pub step_type: String, // "source", "staging", "transform", "destination"
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub workflow_type: WorkflowType,
    pub steps: Vec<WorkflowStep>,
    pub confidence: u8, // 0-100
    pub description: String,
}

pub struct WorkflowDetector {
    tables: Vec<Table>,
    relationships: Vec<Relationship>,
    workflows: Vec<Workflow>,
}

impl WorkflowDetector {
    pub fn new(tables: Vec<Table>, relationships: Vec<Relationship>) -> Self {
        Self {
            tables,
            relationships,
            workflows: Vec::new(),
        }
    }

    pub fn detect_workflows(&mut self) -> Vec<Workflow> {
        self.detect_import_workflows();
        self.detect_staging_workflows();
        self.detect_aggregation_workflows();
        self.detect_lookup_tables();

        self.workflows.clone()
    }

    fn detect_import_workflows(&mut self) {
        // Detect file sources that likely feed into database tables
        let file_tables: Vec<&Table> = self
            .tables
            .iter()
            .filter(|t| {
                t.source_type == "csv" || t.source_type == "excel" || t.source_type == "flatfile"
            })
            .collect();

        let db_tables: Vec<&Table> = self
            .tables
            .iter()
            .filter(|t| t.source_type == "mssql")
            .collect();

        // Match file tables to database tables by name similarity and column overlap
        for file_table in file_tables {
            for db_table in &db_tables {
                let similarity = self.calculate_table_similarity(file_table, db_table);

                if similarity > 0.5 {
                    let confidence = (similarity * 100.0) as u8;

                    let workflow = Workflow {
                        workflow_type: WorkflowType::Import,
                        steps: vec![
                            WorkflowStep {
                                table_name: file_table.full_name.clone(),
                                step_type: "source".to_string(),
                                description: format!("File source: {}", file_table.source_location),
                            },
                            WorkflowStep {
                                table_name: db_table.full_name.clone(),
                                step_type: "destination".to_string(),
                                description: "Database table".to_string(),
                            },
                        ],
                        confidence,
                        description: format!(
                            "Import workflow: {} -> {}",
                            file_table.full_name, db_table.full_name
                        ),
                    };

                    self.workflows.push(workflow);
                }
            }
        }
    }

    fn detect_staging_workflows(&mut self) {
        // Detect staging table patterns (e.g., stg_*, staging_*, temp_*, *_staging)
        let staging_patterns = vec![
            "stg_", "staging_", "temp_", "tmp_", "_stg", "_staging", "_temp",
        ];

        for table in &self.tables {
            let lower_name = table.name.to_lowercase();
            let is_staging = staging_patterns
                .iter()
                .any(|p| lower_name.starts_with(p) || lower_name.ends_with(p));

            if is_staging {
                // Try to find corresponding production table
                let clean_name = staging_patterns
                    .iter()
                    .fold(lower_name.clone(), |acc, p| acc.replace(p, ""));

                if let Some(prod_table) = self.find_table_by_name(&clean_name) {
                    let similarity = self.calculate_column_overlap(table, prod_table);

                    if similarity > 0.5 {
                        let confidence = (similarity * 100.0) as u8;

                        let workflow = Workflow {
                            workflow_type: WorkflowType::StagingToProduction,
                            steps: vec![
                                WorkflowStep {
                                    table_name: table.full_name.clone(),
                                    step_type: "staging".to_string(),
                                    description: "Staging table".to_string(),
                                },
                                WorkflowStep {
                                    table_name: prod_table.full_name.clone(),
                                    step_type: "destination".to_string(),
                                    description: "Production table".to_string(),
                                },
                            ],
                            confidence,
                            description: format!(
                                "Staging workflow: {} -> {}",
                                table.full_name, prod_table.full_name
                            ),
                        };

                        self.workflows.push(workflow);
                    }
                }
            }
        }
    }

    fn detect_aggregation_workflows(&mut self) {
        // Detect aggregation patterns (e.g., *_summary, *_agg, *_total, fact_*)
        let agg_patterns = vec!["_summary", "_agg", "_total", "_aggregate", "fact_", "sum_"];

        for table in &self.tables {
            let lower_name = table.name.to_lowercase();
            let is_aggregation = agg_patterns.iter().any(|p| lower_name.contains(p));

            if is_aggregation {
                // Find potential source tables through relationships
                let source_tables: Vec<String> = self
                    .relationships
                    .iter()
                    .filter(|r| r.to_table == table.full_name)
                    .map(|r| r.from_table.clone())
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();

                if !source_tables.is_empty() {
                    let mut steps = source_tables
                        .iter()
                        .map(|t| WorkflowStep {
                            table_name: t.clone(),
                            step_type: "source".to_string(),
                            description: "Source table".to_string(),
                        })
                        .collect::<Vec<_>>();

                    steps.push(WorkflowStep {
                        table_name: table.full_name.clone(),
                        step_type: "destination".to_string(),
                        description: "Aggregation table".to_string(),
                    });

                    let workflow = Workflow {
                        workflow_type: WorkflowType::Aggregation,
                        steps,
                        confidence: 75,
                        description: format!(
                            "Aggregation workflow: {} sources -> {}",
                            source_tables.len(),
                            table.full_name
                        ),
                    };

                    self.workflows.push(workflow);
                }
            }
        }
    }

    fn detect_lookup_tables(&mut self) {
        // Detect lookup/reference tables (small tables with many incoming relationships)
        let lookup_patterns = vec!["lookup_", "ref_", "dim_", "_ref", "_lookup"];

        for table in &self.tables {
            let lower_name = table.name.to_lowercase();
            let has_lookup_pattern = lookup_patterns
                .iter()
                .any(|p| lower_name.starts_with(p) || lower_name.ends_with(p));

            // Count incoming relationships
            let incoming_count = self
                .relationships
                .iter()
                .filter(|r| r.to_table == table.full_name)
                .count();

            // Heuristic: lookup tables typically have:
            // - Small row count (< 1000)
            // - Many incoming relationships (> 2)
            // - Or matching naming pattern
            let is_small = table.row_count.map(|c| c < 1000).unwrap_or(false);
            let is_lookup = has_lookup_pattern || (is_small && incoming_count > 2);

            if is_lookup {
                let workflow = Workflow {
                    workflow_type: WorkflowType::Lookup,
                    steps: vec![WorkflowStep {
                        table_name: table.full_name.clone(),
                        step_type: "lookup".to_string(),
                        description: format!(
                            "Lookup table (referenced by {} tables)",
                            incoming_count
                        ),
                    }],
                    confidence: if has_lookup_pattern { 90 } else { 70 },
                    description: format!("Lookup/Reference table: {}", table.full_name),
                };

                self.workflows.push(workflow);
            }
        }
    }

    fn calculate_table_similarity(&self, table1: &Table, table2: &Table) -> f64 {
        // Calculate similarity based on:
        // 1. Name similarity
        // 2. Column overlap
        // 3. Column type matching

        let name_sim = self.string_similarity(&table1.name, &table2.name);
        let column_overlap = self.calculate_column_overlap(table1, table2);

        // Weighted average
        (name_sim * 0.3) + (column_overlap * 0.7)
    }

    fn calculate_column_overlap(&self, table1: &Table, table2: &Table) -> f64 {
        let cols1: HashSet<String> = table1
            .columns
            .iter()
            .map(|c| c.name.to_lowercase())
            .collect();

        let cols2: HashSet<String> = table2
            .columns
            .iter()
            .map(|c| c.name.to_lowercase())
            .collect();

        let intersection = cols1.intersection(&cols2).count();
        let union = cols1.union(&cols2).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    fn string_similarity(&self, s1: &str, s2: &str) -> f64 {
        // Simple Levenshtein-based similarity (normalized)
        let s1_lower = s1.to_lowercase();
        let s2_lower = s2.to_lowercase();

        if s1_lower == s2_lower {
            return 1.0;
        }

        let distance = levenshtein_distance(&s1_lower, &s2_lower);
        let max_len = s1_lower.len().max(s2_lower.len());

        if max_len == 0 {
            0.0
        } else {
            1.0 - (distance as f64 / max_len as f64)
        }
    }

    fn find_table_by_name(&self, name: &str) -> Option<&Table> {
        let normalized = name.to_lowercase().replace("_", "");

        self.tables.iter().find(|t| {
            let table_name = t.name.to_lowercase().replace("_", "");
            table_name == normalized || table_name.contains(&normalized)
        })
    }

    pub fn get_workflows(&self) -> &[Workflow] {
        &self.workflows
    }
}

// Simple Levenshtein distance implementation
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for (i, c1) in s1.chars().enumerate() {
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }

    matrix[len1][len2]
}
