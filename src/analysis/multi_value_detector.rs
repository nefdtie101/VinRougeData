use crate::schema::Column;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DetectionMethod {
    Delimited(String),    // e.g., ","
    VocabularySegmented,  // DP match against cross-column vocabulary
    PatternRepetition,    // structural shape repeat
    LengthOutlier,        // MAD-based length outlier
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiValueColumnAnalysis {
    pub column_name: String,
    pub table_name: String,
    pub detection_method: DetectionMethod,
    pub delimiter: Option<String>,
    pub confidence: f64,
    pub multi_value_cell_count: usize,
    pub total_cell_count: usize,
    pub multi_value_ratio: f64,
    pub example_raw: Vec<String>,
    pub example_split: Vec<Vec<String>>,
    pub unique_atomic_values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiValueAnalysis {
    pub table_name: String,
    pub multi_value_columns: Vec<MultiValueColumnAnalysis>,
}

pub struct MultiValueDetector {
    sample_size: usize,
}

impl MultiValueDetector {
    pub fn new(sample_size: usize) -> Self {
        Self { sample_size }
    }

    pub fn analyze_all_sources(
        &self,
        sources: &[(String, Vec<Vec<String>>, Vec<Column>)],
    ) -> Vec<MultiValueAnalysis> {
        // Pass 1: delimiter detection per column, collect vocabulary
        let mut global_vocab: HashSet<String> = HashSet::new();
        let mut per_source_delimiter_results: Vec<Vec<Option<ColumnDelimiterResult>>> = Vec::new();

        for (_table_name, data, columns) in sources {
            let mut source_results = Vec::new();
            for (col_idx, column) in columns.iter().enumerate() {
                let cell_values: Vec<&str> = data
                    .iter()
                    .take(self.sample_size)
                    .filter_map(|row| row.get(col_idx).map(|s| s.as_str()))
                    .filter(|s| !s.trim().is_empty())
                    .collect();

                if cell_values.len() < 5 {
                    source_results.push(None);
                    continue;
                }

                if self.looks_like_date_column(&cell_values) {
                    source_results.push(None);
                    continue;
                }

                let result = self.detect_delimiter_for_column(&cell_values, &column.name);
                if let Some(ref r) = result {
                    // Collect vocabulary atoms (min 3 chars)
                    for atom in &r.vocabulary {
                        if atom.len() >= 3 {
                            global_vocab.insert(atom.clone());
                        }
                    }
                }
                source_results.push(result);
            }
            per_source_delimiter_results.push(source_results);
        }

        // Pass 2: build final analyses
        let mut all_analyses = Vec::new();

        for (source_idx, (table_name, data, columns)) in sources.iter().enumerate() {
            let delimiter_results = &per_source_delimiter_results[source_idx];
            let mut multi_value_columns = Vec::new();

            for (col_idx, column) in columns.iter().enumerate() {
                let cell_values: Vec<String> = data
                    .iter()
                    .take(self.sample_size)
                    .filter_map(|row| row.get(col_idx).cloned())
                    .collect();

                let non_empty: Vec<&str> = cell_values
                    .iter()
                    .map(|s| s.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .collect();

                if non_empty.len() < 5 {
                    continue;
                }

                if let Some(delim_result) = delimiter_results.get(col_idx).and_then(|r| r.as_ref()) {
                    // Delimiter-confirmed
                    let multi_cells: Vec<&str> = non_empty
                        .iter()
                        .filter(|&&s| s.contains(delim_result.delimiter.as_str()))
                        .copied()
                        .collect();

                    let examples_raw: Vec<String> = multi_cells.iter().take(3).map(|s| s.to_string()).collect();
                    let examples_split: Vec<Vec<String>> = multi_cells
                        .iter()
                        .take(3)
                        .map(|s| {
                            s.split(&delim_result.delimiter as &str)
                                .map(|p| p.trim().to_string())
                                .collect()
                        })
                        .collect();

                    // Confidence: adjust if avg parts ≈ 2.0 (could be "Smith, John")
                    let avg_parts = if !multi_cells.is_empty() {
                        multi_cells
                            .iter()
                            .map(|s| s.split(&delim_result.delimiter as &str).count() as f64)
                            .sum::<f64>()
                            / multi_cells.len() as f64
                    } else {
                        0.0
                    };

                    // If >50% of split tokens are numeric — it's likely formatted numbers
                    let is_numeric_values = self.check_mostly_numeric(&delim_result.delimiter, &non_empty);

                    let mut confidence = delim_result.fraction;
                    if (avg_parts - 2.0).abs() < 0.5 {
                        confidence *= 0.5; // Likely "Last, First" names
                    }
                    if is_numeric_values {
                        confidence *= 0.1; // Very likely numeric formatting
                    }

                    if confidence < 0.2 {
                        continue;
                    }

                    let atoms: Vec<String> = delim_result.vocabulary.iter().take(50).cloned().collect();

                    multi_value_columns.push(MultiValueColumnAnalysis {
                        column_name: column.name.clone(),
                        table_name: table_name.clone(),
                        detection_method: DetectionMethod::Delimited(delim_result.delimiter.clone()),
                        delimiter: Some(delim_result.delimiter.clone()),
                        confidence: confidence.min(1.0),
                        multi_value_cell_count: multi_cells.len(),
                        total_cell_count: non_empty.len(),
                        multi_value_ratio: multi_cells.len() as f64 / non_empty.len() as f64,
                        example_raw: examples_raw,
                        example_split: examples_split,
                        unique_atomic_values: atoms,
                    });
                } else {
                    // Pass 2: try DP segmentation, then pattern, then length outlier
                    if let Some(analysis) = self.try_vocabulary_segmented(
                        &column.name,
                        table_name,
                        &non_empty,
                        &global_vocab,
                    ) {
                        multi_value_columns.push(analysis);
                    } else if let Some(analysis) = self.detect_pattern_repetition(
                        &column.name,
                        table_name,
                        &non_empty,
                    ) {
                        multi_value_columns.push(analysis);
                    } else if let Some(analysis) = self.detect_length_outliers(
                        &column.name,
                        table_name,
                        &non_empty,
                    ) {
                        multi_value_columns.push(analysis);
                    }
                }
            }

            if !multi_value_columns.is_empty() {
                all_analyses.push(MultiValueAnalysis {
                    table_name: table_name.clone(),
                    multi_value_columns,
                });
            }
        }

        all_analyses
    }

    fn detect_delimiter_for_column(
        &self,
        cells: &[&str],
        _column_name: &str,
    ) -> Option<ColumnDelimiterResult> {
        let candidates = [",", ";", "|", "\t", "\n"];
        let total = cells.len();

        let mut best: Option<ColumnDelimiterResult> = None;

        for &delim in &candidates {
            let count = cells.iter().filter(|&&s| s.contains(delim)).count();
            let fraction = count as f64 / total as f64;

            if fraction > 0.20 {
                // Collect vocabulary
                let mut vocab: HashSet<String> = HashSet::new();
                for cell in cells {
                    for part in cell.split(delim) {
                        let trimmed = part.trim().to_string();
                        if trimmed.len() >= 3 {
                            vocab.insert(trimmed);
                        }
                    }
                }

                let result = ColumnDelimiterResult {
                    delimiter: delim.to_string(),
                    fraction,
                    vocabulary: vocab,
                };

                // Keep best (highest fraction)
                if best.as_ref().map_or(true, |b| fraction > b.fraction) {
                    best = Some(result);
                }
            }
        }

        best
    }

    fn try_vocabulary_segmented(
        &self,
        column_name: &str,
        table_name: &str,
        cells: &[&str],
        vocab: &HashSet<String>,
    ) -> Option<MultiValueColumnAnalysis> {
        if vocab.len() < 5 {
            return None;
        }

        let mut segmented_cells: Vec<(&str, Vec<String>)> = Vec::new();

        for &cell in cells {
            if cell.len() > 500 {
                continue;
            }
            if let Some(segments) = self.dp_segment(cell, vocab) {
                if segments.len() >= 2 {
                    segmented_cells.push((cell, segments));
                }
            }
        }

        let ratio = segmented_cells.len() as f64 / cells.len() as f64;
        if ratio < 0.10 {
            return None;
        }

        let confidence = (ratio * 1.5).min(0.95);
        let example_raw: Vec<String> = segmented_cells.iter().take(3).map(|(s, _)| s.to_string()).collect();
        let example_split: Vec<Vec<String>> = segmented_cells.iter().take(3).map(|(_, parts)| parts.clone()).collect();

        // Collect unique atoms seen in splits
        let mut atoms: HashSet<String> = HashSet::new();
        for (_, parts) in &segmented_cells {
            for p in parts {
                atoms.insert(p.clone());
            }
        }
        let unique_atomic_values: Vec<String> = atoms.into_iter().take(50).collect();

        Some(MultiValueColumnAnalysis {
            column_name: column_name.to_string(),
            table_name: table_name.to_string(),
            detection_method: DetectionMethod::VocabularySegmented,
            delimiter: None,
            confidence,
            multi_value_cell_count: segmented_cells.len(),
            total_cell_count: cells.len(),
            multi_value_ratio: ratio,
            example_raw,
            example_split,
            unique_atomic_values,
        })
    }

    fn dp_segment(&self, s: &str, vocab: &HashSet<String>) -> Option<Vec<String>> {
        // Work with char boundaries for Unicode safety
        let chars: Vec<char> = s.chars().collect();
        let n = chars.len();
        if n == 0 {
            return None;
        }

        // dp[i] = true if s[0..i] can be segmented using vocab
        let mut dp = vec![false; n + 1];
        let mut prev = vec![usize::MAX; n + 1];
        dp[0] = true;

        // Build char-indexed string for slicing
        let char_starts: Vec<usize> = {
            let mut starts = vec![0usize; n + 1];
            let mut byte_pos = 0;
            for (i, c) in chars.iter().enumerate() {
                starts[i] = byte_pos;
                byte_pos += c.len_utf8();
            }
            starts[n] = byte_pos;
            starts
        };

        for i in 1..=n {
            for j in 0..i {
                if dp[j] {
                    let slice = &s[char_starts[j]..char_starts[i]];
                    if vocab.contains(slice) {
                        dp[i] = true;
                        prev[i] = j;
                        break;
                    }
                }
            }
        }

        if !dp[n] {
            return None;
        }

        // Reconstruct
        let mut segments = Vec::new();
        let mut pos = n;
        while pos > 0 {
            let start = prev[pos];
            let segment = s[char_starts[start]..char_starts[pos]].to_string();
            segments.push(segment);
            pos = start;
        }
        segments.reverse();
        Some(segments)
    }

    fn detect_pattern_repetition(
        &self,
        column_name: &str,
        table_name: &str,
        cells: &[&str],
    ) -> Option<MultiValueColumnAnalysis> {
        // Compute shape fingerprints
        let shapes: Vec<String> = cells.iter().map(|s| self.shape_fingerprint(s)).collect();

        // Find modal shape
        let mut freq: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for shape in &shapes {
            *freq.entry(shape.as_str()).or_insert(0) += 1;
        }
        let modal_shape = freq.into_iter().max_by_key(|(_, c)| *c).map(|(s, _)| s)?;
        let modal_len = modal_shape.len();

        if modal_len < 3 {
            return None;
        }

        // Flag cells where shape.len() >= modal.len() * 2 AND modal appears >=2x non-overlapping
        let mut flagged: Vec<usize> = Vec::new();
        for (i, shape) in shapes.iter().enumerate() {
            if shape.len() >= modal_len * 2 {
                let appearances = self.count_non_overlapping(shape, modal_shape);
                if appearances >= 2 {
                    flagged.push(i);
                }
            }
        }

        let ratio = flagged.len() as f64 / cells.len() as f64;
        if ratio <= 0.10 {
            return None;
        }

        let confidence = ratio * 0.6;
        let example_raw: Vec<String> = flagged.iter().take(3).map(|&i| cells[i].to_string()).collect();
        let example_split: Vec<Vec<String>> = example_raw
            .iter()
            .map(|s| vec!["[pattern-based]".to_string(), s.clone()])
            .collect();

        Some(MultiValueColumnAnalysis {
            column_name: column_name.to_string(),
            table_name: table_name.to_string(),
            detection_method: DetectionMethod::PatternRepetition,
            delimiter: None,
            confidence,
            multi_value_cell_count: flagged.len(),
            total_cell_count: cells.len(),
            multi_value_ratio: ratio,
            example_raw,
            example_split,
            unique_atomic_values: Vec::new(),
        })
    }

    fn detect_length_outliers(
        &self,
        column_name: &str,
        table_name: &str,
        cells: &[&str],
    ) -> Option<MultiValueColumnAnalysis> {
        let mut lengths: Vec<f64> = cells.iter().map(|s| s.len() as f64).collect();
        lengths.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let n = lengths.len();
        let median = if n % 2 == 0 {
            (lengths[n / 2 - 1] + lengths[n / 2]) / 2.0
        } else {
            lengths[n / 2]
        };

        let mut deviations: Vec<f64> = lengths.iter().map(|&l| (l - median).abs()).collect();
        deviations.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mad = if n % 2 == 0 {
            (deviations[n / 2 - 1] + deviations[n / 2]) / 2.0
        } else {
            deviations[n / 2]
        };

        if mad == 0.0 {
            return None;
        }

        let mut flagged: Vec<usize> = Vec::new();
        for (i, cell) in cells.iter().enumerate() {
            let len = cell.len() as f64;
            let z = 0.6745 * (len - median).abs() / mad;
            if z > 3.5 && len > median {
                flagged.push(i);
            }
        }

        if flagged.len() < 2 {
            return None;
        }

        let ratio = flagged.len() as f64 / cells.len() as f64;
        let confidence = ratio * 0.4;
        let example_raw: Vec<String> = flagged.iter().take(3).map(|&i| cells[i].to_string()).collect();
        let example_split: Vec<Vec<String>> = example_raw
            .iter()
            .map(|s| vec!["[length-outlier]".to_string(), s.clone()])
            .collect();

        Some(MultiValueColumnAnalysis {
            column_name: column_name.to_string(),
            table_name: table_name.to_string(),
            detection_method: DetectionMethod::LengthOutlier,
            delimiter: None,
            confidence,
            multi_value_cell_count: flagged.len(),
            total_cell_count: cells.len(),
            multi_value_ratio: ratio,
            example_raw,
            example_split,
            unique_atomic_values: Vec::new(),
        })
    }

    fn looks_like_date_column(&self, cells: &[&str]) -> bool {
        let date_like = cells
            .iter()
            .filter(|&&s| {
                // Simple check: has at least two '/' or '-' separating digit groups
                let slashes = s.chars().filter(|&c| c == '/').count();
                let dashes = s.chars().filter(|&c| c == '-').count();
                (slashes >= 2 || dashes >= 2)
                    && s.chars().any(|c| c.is_ascii_digit())
            })
            .count();

        date_like as f64 / cells.len() as f64 > 0.80
    }

    fn shape_fingerprint(&self, s: &str) -> String {
        s.chars()
            .map(|c| {
                if c.is_uppercase() {
                    'U'
                } else if c.is_lowercase() {
                    'l'
                } else if c.is_ascii_digit() {
                    'd'
                } else if c == ' ' {
                    ' '
                } else {
                    c
                }
            })
            .collect()
    }

    fn count_non_overlapping(&self, haystack: &str, needle: &str) -> usize {
        if needle.is_empty() {
            return 0;
        }
        let mut count = 0;
        let mut start = 0;
        while let Some(pos) = haystack[start..].find(needle) {
            count += 1;
            start += pos + needle.len();
        }
        count
    }

    fn check_mostly_numeric(&self, delimiter: &str, cells: &[&str]) -> bool {
        let mut total_tokens = 0usize;
        let mut numeric_tokens = 0usize;

        for cell in cells {
            for part in cell.split(delimiter) {
                let trimmed = part.trim();
                if trimmed.is_empty() {
                    continue;
                }
                total_tokens += 1;
                // Remove commas before parsing (e.g., "1,234.56")
                let cleaned = trimmed.replace(',', "");
                if cleaned.parse::<f64>().is_ok() {
                    numeric_tokens += 1;
                }
            }
        }

        if total_tokens == 0 {
            return false;
        }
        numeric_tokens as f64 / total_tokens as f64 > 0.50
    }
}

// Internal helper struct
struct ColumnDelimiterResult {
    delimiter: String,
    fraction: f64,
    vocabulary: HashSet<String>,
}
