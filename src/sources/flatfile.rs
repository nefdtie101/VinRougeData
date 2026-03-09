use crate::schema::{Column, DataType, Table};
use crate::sources::DataSource;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct FlatfileSource {
    file_path: String,
    config: FlatfileConfig,
}

#[derive(Clone)]
pub enum FlatfileConfig {
    Delimited {
        delimiter: char,
        has_header: bool,
    },
    FixedWidth {
        column_widths: Vec<usize>,
        column_names: Vec<String>,
    },
}

impl FlatfileSource {
    pub fn new_delimited(file_path: String, delimiter: char, has_header: bool) -> Self {
        Self {
            file_path,
            config: FlatfileConfig::Delimited {
                delimiter,
                has_header,
            },
        }
    }

    pub fn new_fixed_width(
        file_path: String,
        column_widths: Vec<usize>,
        column_names: Vec<String>,
    ) -> Self {
        Self {
            file_path,
            config: FlatfileConfig::FixedWidth {
                column_widths,
                column_names,
            },
        }
    }

    fn parse_delimited_line(&self, line: &str, delimiter: char) -> Vec<String> {
        line.split(delimiter)
            .map(|s| s.trim().to_string())
            .collect()
    }

    fn parse_fixed_width_line(&self, line: &str, widths: &[usize]) -> Vec<String> {
        let mut fields = Vec::new();
        let mut start = 0;

        for &width in widths {
            let end = (start + width).min(line.len());
            let field = if start < line.len() {
                line[start..end].trim().to_string()
            } else {
                String::new()
            };
            fields.push(field);
            start = end;
        }

        fields
    }

    fn infer_data_type(&self, samples: &[String]) -> DataType {
        let mut has_integer = true;
        let mut has_float = true;
        let mut has_boolean = true;
        let mut max_length = 0;

        for value in samples {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                continue;
            }

            max_length = max_length.max(trimmed.len());

            if has_integer && trimmed.parse::<i64>().is_err() {
                has_integer = false;
            }

            if has_float && trimmed.parse::<f64>().is_err() {
                has_float = false;
            }

            if has_boolean {
                let lower = trimmed.to_lowercase();
                if !matches!(lower.as_str(), "true" | "false" | "yes" | "no" | "1" | "0") {
                    has_boolean = false;
                }
            }
        }

        if has_boolean {
            DataType::Boolean
        } else if has_integer {
            DataType::Integer
        } else if has_float {
            DataType::Float
        } else if max_length <= 255 {
            DataType::VarChar {
                max_length: Some(max_length.max(50)),
            }
        } else {
            DataType::Text
        }
    }
}

#[async_trait::async_trait(?Send)]
impl DataSource for FlatfileSource {
    async fn extract_schema(&mut self) -> Result<Vec<Table>> {
        let path = Path::new(&self.file_path);
        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let file = File::open(&self.file_path).context("Failed to open flat file")?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let (headers, data_lines) = match &self.config {
            FlatfileConfig::Delimited {
                delimiter,
                has_header,
            } => {
                let first_line = lines
                    .next()
                    .context("File is empty")?
                    .context("Failed to read first line")?;

                let headers = if *has_header {
                    self.parse_delimited_line(&first_line, *delimiter)
                } else {
                    let field_count = self.parse_delimited_line(&first_line, *delimiter).len();
                    (0..field_count)
                        .map(|i| format!("column_{}", i + 1))
                        .collect()
                };

                let mut data_lines = Vec::new();
                let start_from_second = *has_header;

                if !start_from_second {
                    data_lines.push(self.parse_delimited_line(&first_line, *delimiter));
                }

                for (idx, line) in lines.enumerate() {
                    if idx >= 1000 {
                        break;
                    }
                    let line = line.context("Failed to read line")?;
                    data_lines.push(self.parse_delimited_line(&line, *delimiter));
                }

                (headers, data_lines)
            }
            FlatfileConfig::FixedWidth {
                column_widths,
                column_names,
            } => {
                let mut data_lines = Vec::new();

                for (idx, line) in lines.enumerate() {
                    if idx >= 1000 {
                        break;
                    }
                    let line = line.context("Failed to read line")?;
                    data_lines.push(self.parse_fixed_width_line(&line, column_widths));
                }

                (column_names.clone(), data_lines)
            }
        };

        // Build column samples
        let mut column_samples: HashMap<usize, Vec<String>> = HashMap::new();
        for row in &data_lines {
            for (col_idx, value) in row.iter().enumerate() {
                column_samples
                    .entry(col_idx)
                    .or_insert_with(Vec::new)
                    .push(value.clone());
            }
        }

        // Create columns
        let mut columns = Vec::new();
        for (idx, header) in headers.iter().enumerate() {
            let samples = column_samples.get(&idx).cloned().unwrap_or_default();
            let data_type = self.infer_data_type(&samples);

            let mut column = Column::new(header.clone(), data_type);

            // Store sample values
            let unique_samples: HashSet<String> = samples
                .iter()
                .filter(|s| !s.trim().is_empty())
                .cloned()
                .collect();
            column.sample_values = unique_samples.into_iter().take(5).collect();

            // Calculate statistics
            let non_empty: Vec<_> = samples.iter().filter(|s| !s.trim().is_empty()).collect();
            column.unique_count = Some(
                non_empty
                    .iter()
                    .map(|s| s.trim())
                    .collect::<HashSet<_>>()
                    .len(),
            );
            column.null_count = Some(samples.len() - non_empty.len());

            columns.push(column);
        }

        // Create table
        let mut table = Table::new(file_name, "flatfile".to_string(), self.file_path.clone());
        table.row_count = Some(data_lines.len());

        for column in columns {
            table.add_column(column);
        }

        Ok(vec![table])
    }

    fn source_type(&self) -> &str {
        "flatfile"
    }
}
