use crate::schema::{Column, DataType, Table};
use crate::sources::DataSource;
use anyhow::{Context, Result};
use csv::ReaderBuilder;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct CsvSource {
    file_path: String,
    has_header: bool,
    delimiter: u8,
}

impl CsvSource {
    pub fn new(file_path: String) -> Self {
        Self {
            file_path,
            has_header: true,
            delimiter: b',',
        }
    }

    pub fn with_delimiter(mut self, delimiter: char) -> Self {
        self.delimiter = delimiter as u8;
        self
    }

    pub fn with_header(mut self, has_header: bool) -> Self {
        self.has_header = has_header;
        self
    }

    fn infer_data_type(&self, values: &[String]) -> DataType {
        let mut has_integer = true;
        let mut has_float = true;
        let mut has_date = true;
        let mut has_boolean = true;
        let mut max_length = 0;

        for value in values {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                continue;
            }

            max_length = max_length.max(trimmed.len());

            // Check integer
            if has_integer && trimmed.parse::<i64>().is_err() {
                has_integer = false;
            }

            // Check float
            if has_float && trimmed.parse::<f64>().is_err() {
                has_float = false;
            }

            // Check boolean
            if has_boolean {
                let lower = trimmed.to_lowercase();
                if !matches!(
                    lower.as_str(),
                    "true" | "false" | "yes" | "no" | "1" | "0" | "t" | "f" | "y" | "n"
                ) {
                    has_boolean = false;
                }
            }

            // Check date patterns (basic)
            if has_date {
                // Simple date pattern check
                let has_slash = trimmed.contains('/');
                let has_dash = trimmed.contains('-');
                let has_digits = trimmed.chars().any(|c| c.is_ascii_digit());

                if !has_digits || (!has_slash && !has_dash) {
                    has_date = false;
                }
            }
        }

        if has_boolean {
            DataType::Boolean
        } else if has_integer {
            if max_length <= 3 {
                DataType::SmallInt
            } else if max_length <= 10 {
                DataType::Integer
            } else {
                DataType::BigInt
            }
        } else if has_float {
            DataType::Float
        } else if has_date && max_length <= 30 {
            DataType::DateTime
        } else if max_length <= 255 {
            DataType::VarChar {
                max_length: Some(max_length.max(50)),
            }
        } else {
            DataType::Text
        }
    }

    fn detect_potential_keys(&self, columns: &mut [Column], records: &[Vec<String>]) {
        for (col_idx, column) in columns.iter_mut().enumerate() {
            let mut unique_values = HashSet::new();
            let mut null_count = 0;

            for record in records {
                if let Some(value) = record.get(col_idx) {
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        null_count += 1;
                    } else {
                        unique_values.insert(trimmed);
                    }
                }
            }

            let total_count = records.len();
            column.unique_count = Some(unique_values.len());
            column.null_count = Some(null_count);

            // Heuristic: if column has unique values for all non-null records, could be a key
            if null_count == 0 && unique_values.len() == total_count {
                // Potential primary key - mark it for analysis
                column.add_note("Potential primary key (all values unique and non-null)");
            }
        }
    }
}

#[async_trait::async_trait]
impl DataSource for CsvSource {
    async fn extract_schema(&mut self) -> Result<Vec<Table>> {
        let path = Path::new(&self.file_path);
        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut reader = ReaderBuilder::new()
            .delimiter(self.delimiter)
            .has_headers(self.has_header)
            .from_path(&self.file_path)
            .context("Failed to open CSV file")?;

        // Get headers
        let headers: Vec<String> = if self.has_header {
            reader
                .headers()
                .context("Failed to read CSV headers")?
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            // Generate column names
            let first_record = reader.records().next();
            if let Some(Ok(record)) = first_record {
                (0..record.len())
                    .map(|i| format!("column_{}", i + 1))
                    .collect()
            } else {
                return Ok(vec![]);
            }
        };

        // Sample records for type inference (max 1000 rows)
        let mut records = Vec::new();
        let mut column_samples: HashMap<usize, Vec<String>> = HashMap::new();

        for (idx, result) in reader.records().enumerate() {
            if idx >= 1000 {
                break;
            }

            let record = result.context("Failed to read CSV record")?;
            let values: Vec<String> = record.iter().map(|s| s.to_string()).collect();

            for (col_idx, value) in values.iter().enumerate() {
                column_samples
                    .entry(col_idx)
                    .or_insert_with(Vec::new)
                    .push(value.clone());
            }

            records.push(values);
        }

        // Create columns with inferred types
        let mut columns = Vec::new();
        for (idx, header) in headers.iter().enumerate() {
            let samples = column_samples.get(&idx).cloned().unwrap_or_default();
            let data_type = self.infer_data_type(&samples);

            let mut column = Column::new(header.clone(), data_type);

            // Store sample values (first 5 unique)
            let unique_samples: HashSet<String> = samples
                .iter()
                .filter(|s| !s.trim().is_empty())
                .cloned()
                .collect();
            column.sample_values = unique_samples.into_iter().take(5).collect();

            columns.push(column);
        }

        // Detect potential keys
        self.detect_potential_keys(&mut columns, &records);

        // Create table
        let mut table = Table::new(file_name, "csv".to_string(), self.file_path.clone());
        table.row_count = Some(records.len());

        for column in columns {
            table.add_column(column);
        }

        Ok(vec![table])
    }

    async fn read_data(&mut self) -> Result<Vec<Vec<String>>> {
        let mut reader = ReaderBuilder::new()
            .delimiter(self.delimiter)
            .has_headers(self.has_header)
            .from_path(&self.file_path)
            .context("Failed to open CSV file")?;

        let mut data = Vec::new();

        for result in reader.records() {
            let record = result.context("Failed to read CSV record")?;
            let values: Vec<String> = record.iter().map(|s| s.to_string()).collect();
            data.push(values);
        }

        Ok(data)
    }

    fn source_type(&self) -> &str {
        "csv"
    }
}

// Helper trait extension for adding notes to columns
trait ColumnExt {
    fn add_note(&mut self, note: &str);
}

impl ColumnExt for Column {
    fn add_note(&mut self, note: &str) {
        // For now, we'll store this in the sample_values as a marker
        // In a real implementation, you'd add a `notes` field to Column
        if !self.sample_values.contains(&format!("NOTE: {}", note)) {
            self.sample_values.insert(0, format!("NOTE: {}", note));
        }
    }
}
