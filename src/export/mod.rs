mod console;
mod excel;
mod grouped_excel;
mod json;
mod markdown;
pub mod audit_plan;
pub mod pbc_list;

pub use console::ConsoleExporter;
pub use excel::ExcelExporter;
pub use grouped_excel::GroupedDataExporter;
pub use json::JsonExporter;
pub use markdown::MarkdownExporter;

use crate::analysis::{
    DataProfile, GroupingAnalysis, MultiValueAnalysis, ReconciliationResult, Workflow,
};
use crate::schema::{Relationship, Table};
use anyhow::Result;

#[derive(Debug, Clone)]
pub enum ExportFormat {
    Json,
    Markdown,
    Console,
    Excel,
    GroupedExcel,
}

pub struct AnalysisResult {
    pub tables: Vec<Table>,
    pub relationships: Vec<Relationship>,
    pub workflows: Vec<Workflow>,
    pub data_profiles: Vec<DataProfile>,
    pub grouping_analyses: Vec<GroupingAnalysis>,
    pub reconciliation_results: Vec<ReconciliationResult>,
    pub multi_value_analyses: Vec<MultiValueAnalysis>,
    // Store raw data for grouped exports
    pub source_data: Vec<(String, Vec<Vec<String>>, Vec<crate::schema::Column>)>, // (name, data, columns)
}

pub trait Exporter {
    fn export(&self, result: &AnalysisResult) -> Result<String>;
}
