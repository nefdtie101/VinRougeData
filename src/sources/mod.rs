mod csv_source;
mod excel;
mod flatfile;
mod mssql;

pub use csv_source::CsvSource;
pub use excel::ExcelSource;
pub use flatfile::FlatfileSource;
pub use mssql::MssqlSource;

use crate::schema::Table;
use anyhow::Result;

#[derive(Debug, Clone)]
pub enum SourceType {
    Mssql(String), // connection string
    Csv(String),   // file path
    Excel(String), // file path
    Flatfile {
        path: String,
        delimiter: Option<char>,
        fixed_width: Option<Vec<usize>>,
    },
}

#[async_trait::async_trait]
pub trait DataSource {
    async fn extract_schema(&mut self) -> Result<Vec<Table>>;
    async fn read_data(&mut self) -> Result<Vec<Vec<String>>> {
        // Default implementation returns empty data
        Ok(Vec::new())
    }
    fn source_type(&self) -> &str;
}
