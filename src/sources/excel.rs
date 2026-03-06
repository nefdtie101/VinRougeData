use crate::schema::{Column, DataType, Table};
use crate::sources::DataSource;
use anyhow::{Context, Result};
use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;

#[cfg(not(target_arch = "wasm32"))]
use calamine::open_workbook;

pub struct ExcelSource {
    inner: ExcelSourceInner,
    sheet_name: Option<String>,
    has_header: bool,
}

enum ExcelSourceInner {
    #[cfg(not(target_arch = "wasm32"))]
    Path(String),
    Bytes { data: Vec<u8>, name: String },
}

impl ExcelSource {
    /// Native constructor — opens from a file path.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(file_path: String) -> Self {
        Self {
            inner: ExcelSourceInner::Path(file_path),
            sheet_name: None,
            has_header: true,
        }
    }

    /// WASM-safe constructor — reads from an in-memory byte slice.
    pub fn from_bytes(bytes: Vec<u8>, name: String) -> Self {
        Self {
            inner: ExcelSourceInner::Bytes { data: bytes, name },
            sheet_name: None,
            has_header: true,
        }
    }

    pub fn with_sheet(mut self, sheet_name: String) -> Self {
        self.sheet_name = Some(sheet_name);
        self
    }

    pub fn with_header(mut self, has_header: bool) -> Self {
        self.has_header = has_header;
        self
    }

    fn display_name(&self) -> &str {
        match &self.inner {
            #[cfg(not(target_arch = "wasm32"))]
            ExcelSourceInner::Path(p) => p.as_str(),
            ExcelSourceInner::Bytes { name, .. } => name.as_str(),
        }
    }

    fn open_workbook(&self) -> Result<Xlsx<Cursor<Vec<u8>>>> {
        match &self.inner {
            #[cfg(not(target_arch = "wasm32"))]
            ExcelSourceInner::Path(path) => {
                let bytes = std::fs::read(path).context("Failed to read Excel file")?;
                open_workbook_from_rs(Cursor::new(bytes)).context("Failed to parse Excel file")
            }
            ExcelSourceInner::Bytes { data, .. } => {
                open_workbook_from_rs(Cursor::new(data.clone()))
                    .context("Failed to parse Excel file")
            }
        }
    }

    fn excel_type_to_data_type(&self, samples: &[String]) -> DataType {
        let mut has_integer = true;
        let mut has_float = true;
        let mut has_date = true;
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
                if !matches!(
                    lower.as_str(),
                    "true" | "false" | "yes" | "no" | "1" | "0" | "t" | "f"
                ) {
                    has_boolean = false;
                }
            }

            if has_date {
                let has_date_chars = trimmed.contains('/') || trimmed.contains('-');
                if !has_date_chars {
                    has_date = false;
                }
            }
        }

        if has_boolean {
            DataType::Boolean
        } else if has_integer {
            DataType::Integer
        } else if has_float {
            DataType::Float
        } else if has_date {
            DataType::DateTime
        } else if max_length <= 255 {
            DataType::VarChar {
                max_length: Some(max_length.max(50)),
            }
        } else {
            DataType::Text
        }
    }

    fn process_sheet(&self, sheet_name: &str, range: calamine::Range<Data>) -> Result<Table> {
        let rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| {
                row.iter()
                    .map(|cell| match cell {
                        Data::Int(i) => i.to_string(),
                        Data::Float(f) => f.to_string(),
                        Data::String(s) => s.clone(),
                        Data::Bool(b) => b.to_string(),
                        Data::DateTime(dt) => dt.to_string(),
                        Data::Error(e) => format!("ERROR: {:?}", e),
                        Data::Empty => String::new(),
                        Data::DateTimeIso(dt) => dt.to_string(),
                        Data::DurationIso(d) => d.to_string(),
                    })
                    .collect()
            })
            .collect();

        let display = self.display_name().to_string();

        if rows.is_empty() {
            return Ok(Table::new(sheet_name.to_string(), "excel".to_string(), display));
        }

        let headers: Vec<String> = if self.has_header && !rows.is_empty() {
            rows[0].clone()
        } else {
            (0..rows[0].len())
                .map(|i| format!("column_{}", i + 1))
                .collect()
        };

        let data_rows = if self.has_header && rows.len() > 1 {
            &rows[1..]
        } else {
            &rows[..]
        };

        let sample_rows: Vec<Vec<String>> = data_rows.iter().take(1000).cloned().collect();

        let mut column_samples: HashMap<usize, Vec<String>> = HashMap::new();
        for row in &sample_rows {
            for (col_idx, value) in row.iter().enumerate() {
                column_samples
                    .entry(col_idx)
                    .or_insert_with(Vec::new)
                    .push(value.clone());
            }
        }

        let mut columns = Vec::new();
        for (idx, header) in headers.iter().enumerate() {
            let samples = column_samples.get(&idx).cloned().unwrap_or_default();
            let data_type = self.excel_type_to_data_type(&samples);

            let mut column = Column::new(header.clone(), data_type);

            let unique_samples: HashSet<String> = samples
                .iter()
                .filter(|s| !s.trim().is_empty())
                .cloned()
                .collect();
            column.sample_values = unique_samples.into_iter().take(5).collect();

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

        // Use just the stem of the display name for the table name
        let stem = std::path::Path::new(self.display_name())
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("workbook");
        let table_name = format!("{}_{}", stem, sheet_name);

        let mut table = Table::new(table_name, "excel".to_string(), display);
        table.row_count = Some(data_rows.len());
        table.description = Some(format!("Sheet: {}", sheet_name));

        for column in columns {
            table.add_column(column);
        }

        Ok(table)
    }
}

#[async_trait::async_trait(?Send)]
impl DataSource for ExcelSource {
    async fn extract_schema(&mut self) -> Result<Vec<Table>> {
        let mut workbook = self.open_workbook()?;
        let mut tables = Vec::new();

        if let Some(sheet_name) = &self.sheet_name.clone() {
            if let Ok(range) = workbook.worksheet_range(sheet_name) {
                let table = self.process_sheet(sheet_name, range)?;
                tables.push(table);
            } else {
                anyhow::bail!("Sheet '{}' not found in workbook", sheet_name);
            }
        } else {
            let sheet_names = workbook.sheet_names();
            for sheet_name in sheet_names {
                if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                    let table = self.process_sheet(&sheet_name, range)?;
                    tables.push(table);
                }
            }
        }

        Ok(tables)
    }

    async fn read_data(&mut self) -> Result<Vec<Vec<String>>> {
        let mut workbook = self.open_workbook()?;

        let sheet_name = if let Some(name) = &self.sheet_name {
            name.clone()
        } else {
            workbook
                .sheet_names()
                .first()
                .context("No sheets found in workbook")?
                .clone()
        };

        let range = workbook
            .worksheet_range(&sheet_name)
            .context("Failed to read sheet")?;

        let rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| {
                row.iter()
                    .map(|cell| match cell {
                        Data::Int(i) => i.to_string(),
                        Data::Float(f) => f.to_string(),
                        Data::String(s) => s.clone(),
                        Data::Bool(b) => b.to_string(),
                        Data::DateTime(dt) => dt.to_string(),
                        Data::Error(e) => format!("ERROR: {:?}", e),
                        Data::Empty => String::new(),
                        Data::DateTimeIso(dt) => dt.to_string(),
                        Data::DurationIso(d) => d.to_string(),
                    })
                    .collect()
            })
            .collect();

        let data = if self.has_header && rows.len() > 1 {
            rows[1..].to_vec()
        } else {
            rows
        };

        Ok(data)
    }

    fn source_type(&self) -> &str {
        "excel"
    }
}
