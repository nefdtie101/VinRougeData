mod csv_source;
mod excel;
mod flatfile;

#[cfg(not(target_arch = "wasm32"))]
mod mssql;

pub use csv_source::CsvSource;
pub use excel::ExcelSource;
pub use flatfile::FlatfileSource;

#[cfg(not(target_arch = "wasm32"))]
pub use mssql::MssqlSource;

use crate::schema::Table;
use anyhow::Result;

#[derive(Debug, Clone)]
pub enum SourceType {
    #[cfg(not(target_arch = "wasm32"))]
    Mssql(String), // connection string
    Csv(String),   // file path
    Excel(String), // file path
    Flatfile {
        path: String,
        delimiter: Option<char>,
        fixed_width: Option<Vec<usize>>,
    },
}

// Use ?Send so the trait is usable in single-threaded WASM environments
#[async_trait::async_trait(?Send)]
pub trait DataSource {
    async fn extract_schema(&mut self) -> Result<Vec<Table>>;
    async fn read_data(&mut self) -> Result<Vec<Vec<String>>> {
        Ok(Vec::new())
    }
    fn source_type(&self) -> &str;
}
