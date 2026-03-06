use crate::schema::{Column, DataType, Table};
use crate::sources::DataSource;
use anyhow::{Context, Result};
use csv::ReaderBuilder;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;

pub struct CsvSource {
    inner: CsvSourceInner,
    has_header: bool,
    delimiter: u8,
}

enum CsvSourceInner {
    #[cfg(not(target_arch = "wasm32"))]
    Path(String),
    Content {
        data: Vec<u8>,
        name: String,
    },
}

impl CsvSource {
    /// Native constructor — opens from a file path.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(file_path: String) -> Self {
        Self {
            inner: CsvSourceInner::Path(file_path),
            has_header: true,
            delimiter: b',',
        }
    }

    /// WASM-safe constructor — reads from raw bytes (e.g. from a browser FileReader).
    pub fn from_bytes(bytes: Vec<u8>, name: String) -> Self {
        Self {
            inner: CsvSourceInner::Content { data: bytes, name },
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

    fn display_name(&self) -> &str {
        match &self.inner {
            #[cfg(not(target_arch = "wasm32"))]
            CsvSourceInner::Path(p) => p.as_str(),
            CsvSourceInner::Content { name, .. } => name.as_str(),
        }
    }

    fn make_reader(&self) -> Result<csv::Reader<Cursor<Vec<u8>>>> {
        let bytes = match &self.inner {
            #[cfg(not(target_arch = "wasm32"))]
            CsvSourceInner::Path(path) => std::fs::read(path).context("Failed to read CSV file")?,
            CsvSourceInner::Content { data, .. } => data.clone(),
        };
        Ok(ReaderBuilder::new()
            .delimiter(self.delimiter)
            .has_headers(self.has_header)
            .from_reader(Cursor::new(bytes)))
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

            if has_integer && trimmed.parse::<i64>().is_err() {
                has_integer = false;
            }

            if has_float && trimmed.parse::<f64>().is_err() {
                has_float = false;
            }

            if has_boolean {
                let lower = trimmed.to_lowercase();
                if !matches!(
                    lower.as_str(),
                    "true" | "false" | "yes" | "no" | "1" | "0" | "t" | "f" | "y" | "n"
                ) {
                    has_boolean = false;
                }
            }

            if has_date {
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

            if null_count == 0 && unique_values.len() == total_count {
                column.sample_values.insert(
                    0,
                    "NOTE: Potential primary key (all values unique and non-null)".to_string(),
                );
            }
        }
    }
}

#[async_trait::async_trait(?Send)]
impl DataSource for CsvSource {
    async fn extract_schema(&mut self) -> Result<Vec<Table>> {
        let stem = std::path::Path::new(self.display_name())
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut reader = self.make_reader()?;

        let headers: Vec<String> = if self.has_header {
            reader
                .headers()
                .context("Failed to read CSV headers")?
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            let first_record = reader.records().next();
            if let Some(Ok(record)) = first_record {
                (0..record.len())
                    .map(|i| format!("column_{}", i + 1))
                    .collect()
            } else {
                return Ok(vec![]);
            }
        };

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

        let mut columns = Vec::new();
        for (idx, header) in headers.iter().enumerate() {
            let samples = column_samples.get(&idx).cloned().unwrap_or_default();
            let data_type = self.infer_data_type(&samples);

            let mut column = Column::new(header.clone(), data_type);

            let unique_samples: HashSet<String> = samples
                .iter()
                .filter(|s| !s.trim().is_empty())
                .cloned()
                .collect();
            column.sample_values = unique_samples.into_iter().take(5).collect();

            columns.push(column);
        }

        self.detect_potential_keys(&mut columns, &records);

        let display = self.display_name().to_string();
        let mut table = Table::new(stem, "csv".to_string(), display);
        table.row_count = Some(records.len());

        for column in columns {
            table.add_column(column);
        }

        Ok(vec![table])
    }

    async fn read_data(&mut self) -> Result<Vec<Vec<String>>> {
        let mut reader = self.make_reader()?;
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
