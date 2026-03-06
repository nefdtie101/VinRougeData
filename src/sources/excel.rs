use crate::schema::{Column, DataType, Table};
use crate::sources::DataSource;
use anyhow::{Context, Result};
use calamine::{open_workbook, Data, Reader, Xlsx};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct ExcelSource {
    file_path: String,
    sheet_name: Option<String>,
    has_header: bool,
}

impl ExcelSource {
    pub fn new(file_path: String) -> Self {
        Self {
            file_path,
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

        if rows.is_empty() {
            return Ok(Table::new(
                sheet_name.to_string(),
                "excel".to_string(),
                self.file_path.clone(),
            ));
        }

        // Extract headers
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

        // Sample data for type inference (limit to 1000 rows)
        let sample_rows: Vec<Vec<String>> = data_rows.iter().take(1000).cloned().collect();

        // Build column samples
        let mut column_samples: HashMap<usize, Vec<String>> = HashMap::new();
        for row in &sample_rows {
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
            let data_type = self.excel_type_to_data_type(&samples);

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
        let table_name = format!(
            "{}_{}",
            Path::new(&self.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("workbook"),
            sheet_name
        );

        let mut table = Table::new(table_name, "excel".to_string(), self.file_path.clone());
        table.row_count = Some(data_rows.len());
        table.description = Some(format!("Sheet: {}", sheet_name));

        for column in columns {
            table.add_column(column);
        }

        Ok(table)
    }
}

#[async_trait::async_trait]
impl DataSource for ExcelSource {
    async fn extract_schema(&mut self) -> Result<Vec<Table>> {
        let mut workbook: Xlsx<_> =
            open_workbook(&self.file_path).context("Failed to open Excel file")?;

        let mut tables = Vec::new();

        if let Some(sheet_name) = &self.sheet_name {
            // Process specific sheet
            if let Ok(range) = workbook.worksheet_range(sheet_name) {
                let table = self.process_sheet(sheet_name, range)?;
                tables.push(table);
            } else {
                anyhow::bail!("Sheet '{}' not found in workbook", sheet_name);
            }
        } else {
            // Process all sheets
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
        let mut workbook: Xlsx<_> =
            open_workbook(&self.file_path).context("Failed to open Excel file")?;

        let sheet_name = if let Some(name) = &self.sheet_name {
            name.clone()
        } else {
            // Get first sheet
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

        // Skip header row if configured
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
