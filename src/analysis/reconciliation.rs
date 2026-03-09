use crate::schema::Column;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationResult {
    pub source1_name: String,
    pub source2_name: String,
    pub key_columns: Vec<String>,
    pub total_source1: usize,
    pub total_source2: usize,
    pub matches: usize,
    pub only_in_source1: usize,
    pub only_in_source2: usize,
    pub duplicates_source1: usize,
    pub duplicates_source2: usize,
    pub field_mismatches: Vec<FieldMismatch>,
    pub match_percentage: f64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMismatch {
    pub key_value: String,
    pub column_name: String,
    pub source1_value: String,
    pub source2_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationConfig {
    pub key_columns: Vec<String>,
    pub compare_columns: Option<Vec<String>>, // None = compare all same-named columns
    /// Explicit cross-source column mappings: (source1_col, source2_col) with different names.
    /// When empty, VinRouge auto-detects mappings from matched row data.
    pub column_mappings: Vec<(String, String)>,
    pub case_sensitive: bool,
    pub trim_whitespace: bool,
    pub max_mismatches: usize,
}

impl Default for ReconciliationConfig {
    fn default() -> Self {
        Self {
            key_columns: Vec::new(),
            compare_columns: None,
            column_mappings: Vec::new(),
            case_sensitive: false,
            trim_whitespace: true,
            max_mismatches: 100,
        }
    }
}

pub struct Reconciliator {
    config: ReconciliationConfig,
}

impl Reconciliator {
    pub fn new(config: ReconciliationConfig) -> Self {
        Self { config }
    }

    pub fn reconcile(
        &self,
        source1_name: &str,
        source1_data: &[Vec<String>],
        source1_columns: &[Column],
        source2_name: &str,
        source2_data: &[Vec<String>],
        source2_columns: &[Column],
    ) -> ReconciliationResult {
        // Auto-detect key columns if not specified
        let key_columns = if self.config.key_columns.is_empty() {
            self.auto_detect_key_columns(source1_columns, source2_columns)
        } else {
            self.config.key_columns.clone()
        };

        if key_columns.is_empty() {
            return self.empty_result(source1_name, source2_name, "No key columns found");
        }

        // Get column indices
        let key_indices_1 = self.get_column_indices(&key_columns, source1_columns);
        let key_indices_2 = self.get_column_indices(&key_columns, source2_columns);

        if key_indices_1.is_empty() || key_indices_2.is_empty() {
            return self.empty_result(
                source1_name,
                source2_name,
                "Key columns not found in both sources",
            );
        }

        // Build key maps
        let (source1_map, duplicates_1) = self.build_key_map(source1_data, &key_indices_1);
        let (source2_map, duplicates_2) = self.build_key_map(source2_data, &key_indices_2);

        // Phase 1: collect matched row index pairs
        let mut matched_pairs: Vec<(usize, usize)> = Vec::new();
        let mut only_in_source1 = 0;

        for (key, indices1) in &source1_map {
            if let Some(indices2) = source2_map.get(key) {
                if let (Some(&idx1), Some(&idx2)) = (indices1.first(), indices2.first()) {
                    matched_pairs.push((idx1, idx2));
                }
            } else {
                only_in_source1 += 1;
            }
        }

        let matches = matched_pairs.len();

        let mut only_in_source2 = 0;
        for key in source2_map.keys() {
            if !source1_map.contains_key(key) {
                only_in_source2 += 1;
            }
        }

        // Phase 2: determine column mappings for differently-named columns
        let effective_mappings = if self.config.column_mappings.is_empty() {
            self.auto_detect_column_mappings(
                source1_data,
                source1_columns,
                source2_data,
                source2_columns,
                &matched_pairs,
            )
        } else {
            self.config.column_mappings.clone()
        };

        // Phase 3: compare matched rows
        let mut field_mismatches = Vec::new();
        for (key, indices1) in &source1_map {
            if let Some(indices2) = source2_map.get(key) {
                if let (Some(&idx1), Some(&idx2)) = (indices1.first(), indices2.first()) {
                    if let (Some(row1), Some(row2)) =
                        (source1_data.get(idx1), source2_data.get(idx2))
                    {
                        let mismatches = self.compare_rows(
                            key,
                            row1,
                            source1_columns,
                            row2,
                            source2_columns,
                            &effective_mappings,
                        );
                        field_mismatches.extend(mismatches);
                    }
                }
            }
        }

        // Calculate match percentage
        let total = source1_map.len().max(source2_map.len()) as f64;
        let match_percentage = if total > 0.0 {
            (matches as f64 / total) * 100.0
        } else {
            0.0
        };

        // Limit field mismatches
        field_mismatches.truncate(self.config.max_mismatches);

        // Generate summary
        let summary = format!(
            "Reconciled {} keys: {} matches ({:.1}%), {} only in {}, {} only in {}",
            source1_map.len().max(source2_map.len()),
            matches,
            match_percentage,
            only_in_source1,
            source1_name,
            only_in_source2,
            source2_name
        );

        ReconciliationResult {
            source1_name: source1_name.to_string(),
            source2_name: source2_name.to_string(),
            key_columns,
            total_source1: source1_data.len(),
            total_source2: source2_data.len(),
            matches,
            only_in_source1,
            only_in_source2,
            duplicates_source1: duplicates_1,
            duplicates_source2: duplicates_2,
            field_mismatches,
            match_percentage,
            summary,
        }
    }

    fn auto_detect_key_columns(&self, columns1: &[Column], columns2: &[Column]) -> Vec<String> {
        let mut common_columns = Vec::new();

        for col1 in columns1 {
            for col2 in columns2 {
                if col1.name.to_lowercase() == col2.name.to_lowercase() {
                    let lower = col1.name.to_lowercase();
                    // Only prioritise columns that are likely unique row identifiers.
                    // Intentionally exclude "code" — Sort Code, Post Code etc. are
                    // NOT unique per row and make a poor join key.
                    if lower.contains("id") || lower == "key" || lower.ends_with(" key") {
                        return vec![col1.name.clone()];
                    }
                    common_columns.push(col1.name.clone());
                    break;
                }
            }
        }

        if !common_columns.is_empty() {
            vec![common_columns[0].clone()]
        } else {
            Vec::new()
        }
    }

    /// For columns that have no same-name partner in the other source, score every
    /// (source1_col, source2_col) pair by the fraction of matched rows where their
    /// values agree.  Pairs scoring above 0.5 are returned as auto-detected mappings.
    fn auto_detect_column_mappings(
        &self,
        source1_data: &[Vec<String>],
        source1_columns: &[Column],
        source2_data: &[Vec<String>],
        source2_columns: &[Column],
        matched_pairs: &[(usize, usize)],
    ) -> Vec<(String, String)> {
        if matched_pairs.is_empty() {
            return Vec::new();
        }

        // Columns that already have a same-name partner are compared by the normal path;
        // we only need to find mappings for the remainder.
        let same_name_s1: HashSet<String> = source1_columns
            .iter()
            .filter(|c1| {
                source2_columns
                    .iter()
                    .any(|c2| c2.name.eq_ignore_ascii_case(&c1.name))
            })
            .map(|c| c.name.to_lowercase())
            .collect();

        let same_name_s2: HashSet<String> = source2_columns
            .iter()
            .filter(|c2| {
                source1_columns
                    .iter()
                    .any(|c1| c1.name.eq_ignore_ascii_case(&c2.name))
            })
            .map(|c| c.name.to_lowercase())
            .collect();

        let unmapped1: Vec<(usize, &Column)> = source1_columns
            .iter()
            .enumerate()
            .filter(|(_, c)| !same_name_s1.contains(&c.name.to_lowercase()))
            .collect();

        let unmapped2: Vec<(usize, &Column)> = source2_columns
            .iter()
            .enumerate()
            .filter(|(_, c)| !same_name_s2.contains(&c.name.to_lowercase()))
            .collect();

        if unmapped1.is_empty() || unmapped2.is_empty() {
            return Vec::new();
        }

        let mut mappings = Vec::new();
        let mut used2: HashSet<usize> = HashSet::new();

        for (idx1, col1) in &unmapped1 {
            let mut best_score = 0.5f64; // minimum threshold
            let mut best_idx2: Option<usize> = None;

            for (idx2, _col2) in &unmapped2 {
                if used2.contains(idx2) {
                    continue;
                }

                let mut agree = 0usize;
                let mut total = 0usize;

                for &(row_idx1, row_idx2) in matched_pairs {
                    if let (Some(row1), Some(row2)) =
                        (source1_data.get(row_idx1), source2_data.get(row_idx2))
                    {
                        if let (Some(v1), Some(v2)) = (row1.get(*idx1), row2.get(*idx2)) {
                            let n1 = self.normalize_value(v1);
                            let n2 = self.normalize_value(v2);
                            if !n1.is_empty() && !n2.is_empty() {
                                total += 1;
                                if n1 == n2 {
                                    agree += 1;
                                }
                            }
                        }
                    }
                }

                if total > 0 {
                    let score = agree as f64 / total as f64;
                    if score > best_score {
                        best_score = score;
                        best_idx2 = Some(*idx2);
                    }
                }
            }

            if let Some(idx2) = best_idx2 {
                let col2 = &unmapped2.iter().find(|(i, _)| *i == idx2).unwrap().1;
                mappings.push((col1.name.clone(), col2.name.clone()));
                used2.insert(idx2);
            }
        }

        mappings
    }

    fn get_column_indices(&self, column_names: &[String], columns: &[Column]) -> Vec<usize> {
        column_names
            .iter()
            .filter_map(|name| {
                columns
                    .iter()
                    .position(|col| col.name.eq_ignore_ascii_case(name))
            })
            .collect()
    }

    fn build_key_map(
        &self,
        data: &[Vec<String>],
        key_indices: &[usize],
    ) -> (HashMap<String, Vec<usize>>, usize) {
        let mut map: HashMap<String, Vec<usize>> = HashMap::new();
        let mut duplicates = 0;

        for (row_idx, row) in data.iter().enumerate() {
            let key_parts: Vec<String> = key_indices
                .iter()
                .filter_map(|&idx| row.get(idx))
                .map(|val| self.normalize_value(val))
                .collect();

            if key_parts.len() == key_indices.len() {
                let key = key_parts.join("|");
                let entry = map.entry(key).or_insert_with(Vec::new);
                if !entry.is_empty() {
                    duplicates += 1;
                }
                entry.push(row_idx);
            }
        }

        (map, duplicates)
    }

    fn normalize_value(&self, value: &str) -> String {
        let mut normalized = value.to_string();

        if self.config.trim_whitespace {
            normalized = normalized.trim().to_string();
        }

        if !self.config.case_sensitive {
            normalized = normalized.to_lowercase();
        }

        normalized
    }

    fn compare_rows(
        &self,
        key: &str,
        row1: &[String],
        columns1: &[Column],
        row2: &[String],
        columns2: &[Column],
        extra_mappings: &[(String, String)],
    ) -> Vec<FieldMismatch> {
        let mut mismatches = Vec::new();

        // Compare same-named columns
        let compare_columns: HashSet<String> = if let Some(cols) = &self.config.compare_columns {
            cols.iter().cloned().collect()
        } else {
            columns1
                .iter()
                .filter_map(|col| {
                    if columns2
                        .iter()
                        .any(|col2| col2.name.eq_ignore_ascii_case(&col.name))
                    {
                        Some(col.name.clone())
                    } else {
                        None
                    }
                })
                .collect()
        };

        for col_name in compare_columns {
            if let Some(idx1) = columns1
                .iter()
                .position(|col| col.name.eq_ignore_ascii_case(&col_name))
            {
                if let Some(idx2) = columns2
                    .iter()
                    .position(|col| col.name.eq_ignore_ascii_case(&col_name))
                {
                    if let (Some(val1), Some(val2)) = (row1.get(idx1), row2.get(idx2)) {
                        let norm1 = self.normalize_value(val1);
                        let norm2 = self.normalize_value(val2);

                        if norm1 != norm2 {
                            mismatches.push(FieldMismatch {
                                key_value: key.to_string(),
                                column_name: col_name,
                                source1_value: val1.clone(),
                                source2_value: val2.clone(),
                            });
                        }
                    }
                }
            }
        }

        // Compare mapped columns (different names across sources)
        for (col1_name, col2_name) in extra_mappings {
            if let Some(idx1) = columns1
                .iter()
                .position(|col| col.name.eq_ignore_ascii_case(col1_name))
            {
                if let Some(idx2) = columns2
                    .iter()
                    .position(|col| col.name.eq_ignore_ascii_case(col2_name))
                {
                    if let (Some(val1), Some(val2)) = (row1.get(idx1), row2.get(idx2)) {
                        let norm1 = self.normalize_value(val1);
                        let norm2 = self.normalize_value(val2);

                        if norm1 != norm2 {
                            mismatches.push(FieldMismatch {
                                key_value: key.to_string(),
                                column_name: format!("{} vs {}", col1_name, col2_name),
                                source1_value: val1.clone(),
                                source2_value: val2.clone(),
                            });
                        }
                    }
                }
            }
        }

        mismatches
    }

    fn empty_result(
        &self,
        source1_name: &str,
        source2_name: &str,
        reason: &str,
    ) -> ReconciliationResult {
        ReconciliationResult {
            source1_name: source1_name.to_string(),
            source2_name: source2_name.to_string(),
            key_columns: Vec::new(),
            total_source1: 0,
            total_source2: 0,
            matches: 0,
            only_in_source1: 0,
            only_in_source2: 0,
            duplicates_source1: 0,
            duplicates_source2: 0,
            field_mismatches: Vec::new(),
            match_percentage: 0.0,
            summary: format!("Reconciliation failed: {}", reason),
        }
    }
}
