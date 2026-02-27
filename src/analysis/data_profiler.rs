use crate::schema::Column;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataProfile {
    pub table_name: String,
    pub column_profiles: Vec<ColumnProfile>,
    pub patterns: Vec<DataPattern>,
    pub correlations: Vec<ColumnCorrelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnProfile {
    pub column_name: String,
    pub total_values: usize,
    pub unique_values: usize,
    pub null_count: usize,
    pub distinct_ratio: f64, // unique/total
    pub data_patterns: Vec<PatternType>,
    pub top_values: Vec<(String, usize)>, // value, count
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PatternType {
    Sequential,        // 1, 2, 3, 4...
    DateSequence,      // Sequential dates
    RepeatingPattern,  // A, B, C, A, B, C...
    UniqueIdentifier,  // All unique, looks like IDs
    Category,          // Low cardinality, repeating values
    Numeric,           // All numeric values
    Boolean,           // True/False, Yes/No, 0/1
    Email,             // Email addresses
    Phone,             // Phone numbers
    Url,               // URLs
    DateTime,          // Date/time values
    Currency,          // Money amounts
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataPattern {
    TimeSequence {
        column: String,
        description: String,
    },
    Hierarchy {
        parent_column: String,
        child_column: String,
        description: String,
    },
    StatusFlow {
        column: String,
        states: Vec<String>,
        description: String,
    },
    AutoIncrement {
        column: String,
        description: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnCorrelation {
    pub column_a: String,
    pub column_b: String,
    pub correlation_type: CorrelationType,
    pub strength: f64, // 0.0 to 1.0
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CorrelationType {
    OneToOne,     // Each value in A maps to exactly one value in B
    OneToMany,    // Each value in A maps to multiple values in B
    ManyToOne,    // Multiple values in A map to same value in B
    Functional,   // B seems to be derived from A (mathematical relationship)
    Temporal,     // Time-based relationship
    Hierarchical, // Parent-child relationship
}

pub struct DataProfiler {
    sample_size: usize,
}

impl DataProfiler {
    pub fn new(sample_size: usize) -> Self {
        Self { sample_size }
    }

    pub fn profile_data(&self, data: &[Vec<String>], columns: &[Column]) -> DataProfile {
        let mut column_profiles = Vec::new();
        let mut patterns = Vec::new();
        let mut correlations = Vec::new();

        // Profile each column
        for (col_idx, column) in columns.iter().enumerate() {
            let values: Vec<&str> = data
                .iter()
                .filter_map(|row| row.get(col_idx).map(|s| s.as_str()))
                .take(self.sample_size)
                .collect();

            let profile = self.profile_column(&column.name, &values);
            column_profiles.push(profile);
        }

        // Detect patterns
        patterns.extend(self.detect_patterns(data, columns));

        // Detect correlations between columns
        correlations.extend(self.detect_correlations(data, columns));

        DataProfile {
            table_name: String::new(), // Set by caller
            column_profiles,
            patterns,
            correlations,
        }
    }

    fn profile_column(&self, name: &str, values: &[&str]) -> ColumnProfile {
        let total_values = values.len();
        let null_count = values.iter().filter(|v| v.is_empty()).count();

        let non_null_values: Vec<&str> = values
            .iter()
            .filter(|v| !v.is_empty())
            .copied()
            .collect();

        let unique_set: HashSet<&str> = non_null_values.iter().copied().collect();
        let unique_values = unique_set.len();

        let distinct_ratio = if total_values > 0 {
            unique_values as f64 / total_values as f64
        } else {
            0.0
        };

        // Count value frequencies
        let mut value_counts: HashMap<&str, usize> = HashMap::new();
        for value in &non_null_values {
            *value_counts.entry(value).or_insert(0) += 1;
        }

        let mut top_values: Vec<(&str, usize)> = value_counts.into_iter().collect();
        top_values.sort_by(|a, b| b.1.cmp(&a.1));
        let top_values: Vec<(String, usize)> = top_values
            .into_iter()
            .take(10)
            .map(|(k, v)| (k.to_string(), v))
            .collect();

        let data_patterns = self.detect_column_patterns(&non_null_values);

        ColumnProfile {
            column_name: name.to_string(),
            total_values,
            unique_values,
            null_count,
            distinct_ratio,
            data_patterns,
            top_values,
        }
    }

    fn detect_column_patterns(&self, values: &[&str]) -> Vec<PatternType> {
        let mut patterns = Vec::new();

        if values.is_empty() {
            return patterns;
        }

        // Check for numeric
        let all_numeric = values
            .iter()
            .take(100)
            .all(|v| v.parse::<f64>().is_ok());
        if all_numeric {
            patterns.push(PatternType::Numeric);
        }

        // Check for boolean patterns
        let boolean_values = ["true", "false", "yes", "no", "1", "0", "t", "f", "y", "n"];
        let all_boolean = values
            .iter()
            .take(100)
            .all(|v| boolean_values.contains(&v.to_lowercase().as_str()));
        if all_boolean {
            patterns.push(PatternType::Boolean);
        }

        // Check for email pattern
        let has_emails = values
            .iter()
            .take(20)
            .any(|v| v.contains('@') && v.contains('.'));
        if has_emails {
            patterns.push(PatternType::Email);
        }

        // Check for URL pattern
        let has_urls = values
            .iter()
            .take(20)
            .any(|v| v.starts_with("http://") || v.starts_with("https://"));
        if has_urls {
            patterns.push(PatternType::Url);
        }

        // Check for sequential pattern (e.g., 1, 2, 3, 4...)
        if all_numeric && values.len() > 3 {
            let numbers: Vec<i64> = values
                .iter()
                .filter_map(|v| v.parse::<i64>().ok())
                .take(100)
                .collect();

            if !numbers.is_empty() {
                let is_sequential = numbers
                    .windows(2)
                    .all(|w| w[1] == w[0] + 1);

                if is_sequential {
                    patterns.push(PatternType::Sequential);
                    patterns.push(PatternType::UniqueIdentifier);
                }
            }
        }

        // Check for unique identifier (high cardinality, all unique)
        let unique_set: HashSet<&str> = values.iter().copied().collect();
        let uniqueness = unique_set.len() as f64 / values.len() as f64;

        if uniqueness > 0.95 && values.len() > 10 {
            patterns.push(PatternType::UniqueIdentifier);
        }

        // Check for category (low cardinality, repeating)
        if uniqueness < 0.1 && unique_set.len() > 1 && unique_set.len() < 50 {
            patterns.push(PatternType::Category);
        }

        patterns
    }

    fn detect_patterns(&self, data: &[Vec<String>], columns: &[Column]) -> Vec<DataPattern> {
        let mut patterns = Vec::new();

        // Detect auto-increment columns
        for (col_idx, column) in columns.iter().enumerate() {
            let values: Vec<i64> = data
                .iter()
                .filter_map(|row| {
                    row.get(col_idx)
                        .and_then(|v| v.parse::<i64>().ok())
                })
                .take(100)
                .collect();

            if values.len() > 3 {
                let is_auto_increment = values.windows(2).all(|w| w[1] == w[0] + 1);

                if is_auto_increment {
                    patterns.push(DataPattern::AutoIncrement {
                        column: column.name.clone(),
                        description: format!(
                            "Column '{}' appears to be auto-incrementing",
                            column.name
                        ),
                    });
                }
            }
        }

        // Detect status flow patterns
        for (col_idx, column) in columns.iter().enumerate() {
            let values: Vec<&str> = data
                .iter()
                .filter_map(|row| row.get(col_idx).map(|s| s.as_str()))
                .take(1000)
                .collect();

            let unique_values: HashSet<&str> = values.iter().copied().collect();

            // Look for status-like patterns
            let status_keywords = ["status", "state", "stage", "phase"];
            let col_lower = column.name.to_lowercase();

            if status_keywords.iter().any(|k| col_lower.contains(k))
                && unique_values.len() > 1
                && unique_values.len() < 20
            {
                let states: Vec<String> = unique_values.iter().map(|s| s.to_string()).collect();
                patterns.push(DataPattern::StatusFlow {
                    column: column.name.clone(),
                    states,
                    description: format!(
                        "Column '{}' appears to track workflow states",
                        column.name
                    ),
                });
            }
        }

        patterns
    }

    fn detect_correlations(
        &self,
        data: &[Vec<String>],
        columns: &[Column],
    ) -> Vec<ColumnCorrelation> {
        let mut correlations = Vec::new();

        // Compare each pair of columns
        for (i, col_a) in columns.iter().enumerate() {
            for (j, col_b) in columns.iter().enumerate() {
                if i >= j {
                    continue; // Skip self and already compared pairs
                }

                if let Some(corr) = self.analyze_column_pair(data, i, j, col_a, col_b) {
                    correlations.push(corr);
                }
            }
        }

        correlations
    }

    fn analyze_column_pair(
        &self,
        data: &[Vec<String>],
        idx_a: usize,
        idx_b: usize,
        col_a: &Column,
        col_b: &Column,
    ) -> Option<ColumnCorrelation> {
        // Build mapping from column A to column B
        let mut a_to_b: HashMap<String, HashSet<String>> = HashMap::new();
        let mut b_to_a: HashMap<String, HashSet<String>> = HashMap::new();

        for row in data.iter().take(self.sample_size) {
            if let (Some(val_a), Some(val_b)) = (row.get(idx_a), row.get(idx_b)) {
                if !val_a.is_empty() && !val_b.is_empty() {
                    a_to_b
                        .entry(val_a.clone())
                        .or_insert_with(HashSet::new)
                        .insert(val_b.clone());

                    b_to_a
                        .entry(val_b.clone())
                        .or_insert_with(HashSet::new)
                        .insert(val_a.clone());
                }
            }
        }

        if a_to_b.is_empty() {
            return None;
        }

        // Analyze relationship
        let a_values = a_to_b.len();
        let b_values = b_to_a.len();

        // Check one-to-one
        let one_to_one = a_to_b.values().all(|set| set.len() == 1)
            && b_to_a.values().all(|set| set.len() == 1);

        if one_to_one && a_values > 5 {
            return Some(ColumnCorrelation {
                column_a: col_a.name.clone(),
                column_b: col_b.name.clone(),
                correlation_type: CorrelationType::OneToOne,
                strength: 1.0,
                description: format!(
                    "Strong 1:1 relationship: each {} has exactly one {}",
                    col_a.name, col_b.name
                ),
            });
        }

        // Check one-to-many
        let mostly_one_to_many =
            a_to_b.values().filter(|set| set.len() > 1).count() as f64 / a_values as f64;

        if mostly_one_to_many > 0.5 && b_values > a_values {
            return Some(ColumnCorrelation {
                column_a: col_a.name.clone(),
                column_b: col_b.name.clone(),
                correlation_type: CorrelationType::OneToMany,
                strength: mostly_one_to_many,
                description: format!(
                    "1:Many relationship: each {} can have multiple {}",
                    col_a.name, col_b.name
                ),
            });
        }

        // Check many-to-one
        let mostly_many_to_one =
            b_to_a.values().filter(|set| set.len() > 1).count() as f64 / b_values as f64;

        if mostly_many_to_one > 0.5 && a_values > b_values {
            return Some(ColumnCorrelation {
                column_a: col_a.name.clone(),
                column_b: col_b.name.clone(),
                correlation_type: CorrelationType::ManyToOne,
                strength: mostly_many_to_one,
                description: format!(
                    "Many:1 relationship: multiple {} share same {}",
                    col_a.name, col_b.name
                ),
            });
        }

        None
    }
}
