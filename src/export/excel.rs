use super::{AnalysisResult, Exporter};
use anyhow::Result;
use rust_xlsxwriter::*;

pub struct ExcelExporter {
    path: String,
}

impl ExcelExporter {
    pub fn new(path: String) -> Self {
        Self { path }
    }

    pub fn export_to_file(&self, result: &AnalysisResult) -> Result<()> {
        let mut workbook = Workbook::new();

        // Summary sheet
        self.write_summary_sheet(&mut workbook, result)?;

        // Tables sheet
        if !result.tables.is_empty() {
            self.write_tables_sheet(&mut workbook, result)?;
        }

        // Relationships sheet
        if !result.relationships.is_empty() {
            self.write_relationships_sheet(&mut workbook, result)?;
        }

        // Workflows sheet
        if !result.workflows.is_empty() {
            self.write_workflows_sheet(&mut workbook, result)?;
        }

        // Reconciliation sheet
        if !result.reconciliation_results.is_empty() {
            self.write_reconciliation_sheet(&mut workbook, result)?;
        }

        // Multi-Value Columns sheet
        if !result.multi_value_analyses.is_empty() {
            self.write_multi_value_sheet(&mut workbook, result)?;
        }

        workbook.save(&self.path)?;
        Ok(())
    }

    /// Returns the workbook as raw bytes — used by the WASM web UI to trigger
    /// a browser download without writing to the filesystem.
    pub fn export_to_bytes(&self, result: &AnalysisResult) -> Result<Vec<u8>> {
        let mut workbook = Workbook::new();

        self.write_summary_sheet(&mut workbook, result)?;

        if !result.tables.is_empty() {
            self.write_tables_sheet(&mut workbook, result)?;
        }
        if !result.relationships.is_empty() {
            self.write_relationships_sheet(&mut workbook, result)?;
        }
        if !result.workflows.is_empty() {
            self.write_workflows_sheet(&mut workbook, result)?;
        }
        if !result.reconciliation_results.is_empty() {
            self.write_reconciliation_sheet(&mut workbook, result)?;
        }
        if !result.multi_value_analyses.is_empty() {
            self.write_multi_value_sheet(&mut workbook, result)?;
        }

        let bytes = workbook.save_to_buffer()?;
        Ok(bytes)
    }

    fn write_summary_sheet(&self, workbook: &mut Workbook, result: &AnalysisResult) -> Result<()> {
        let worksheet = workbook.add_worksheet().set_name("Summary")?;

        // Header format
        let header_format = Format::new()
            .set_bold()
            .set_font_size(14)
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White);

        // Write title
        worksheet.write_with_format(0, 0, "VinRouge Analysis Report", &header_format)?;
        worksheet.write(
            1,
            0,
            format!(
                "Generated: {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ),
        )?;

        // Summary statistics
        let bold = Format::new().set_bold();
        worksheet.write_with_format(3, 0, "Metric", &bold)?;
        worksheet.write_with_format(3, 1, "Count", &bold)?;

        worksheet.write(4, 0, "Tables")?;
        worksheet.write(4, 1, result.tables.len() as f64)?;

        worksheet.write(5, 0, "Relationships")?;
        worksheet.write(5, 1, result.relationships.len() as f64)?;

        worksheet.write(6, 0, "Workflows")?;
        worksheet.write(6, 1, result.workflows.len() as f64)?;

        worksheet.write(7, 0, "Data Profiles")?;
        worksheet.write(7, 1, result.data_profiles.len() as f64)?;

        worksheet.write(8, 0, "Grouping Analyses")?;
        worksheet.write(8, 1, result.grouping_analyses.len() as f64)?;

        worksheet.write(9, 0, "Reconciliations")?;
        worksheet.write(9, 1, result.reconciliation_results.len() as f64)?;

        let total_mv_cols: usize = result
            .multi_value_analyses
            .iter()
            .map(|a| a.multi_value_columns.len())
            .sum();
        worksheet.write(10, 0, "Multi-Value Columns")?;
        worksheet.write(10, 1, total_mv_cols as f64)?;

        // Set column widths
        worksheet.set_column_width(0, 20)?;
        worksheet.set_column_width(1, 15)?;

        Ok(())
    }

    fn write_tables_sheet(&self, workbook: &mut Workbook, result: &AnalysisResult) -> Result<()> {
        let worksheet = workbook.add_worksheet().set_name("Tables")?;

        let header_format = Format::new().set_bold();

        // Headers
        worksheet.write_with_format(0, 0, "Table Name", &header_format)?;
        worksheet.write_with_format(0, 1, "Source Type", &header_format)?;
        worksheet.write_with_format(0, 2, "Source Location", &header_format)?;
        worksheet.write_with_format(0, 3, "Columns", &header_format)?;
        worksheet.write_with_format(0, 4, "Row Count", &header_format)?;

        // Data
        for (idx, table) in result.tables.iter().enumerate() {
            let row = (idx + 1) as u32;
            worksheet.write(row, 0, &table.full_name)?;
            worksheet.write(row, 1, &table.source_type)?;
            worksheet.write(row, 2, &table.source_location)?;
            worksheet.write(row, 3, table.columns.len() as f64)?;
            if let Some(count) = table.row_count {
                worksheet.write(row, 4, count as f64)?;
            }
        }

        // Set column widths
        worksheet.set_column_width(0, 30)?;
        worksheet.set_column_width(1, 15)?;
        worksheet.set_column_width(2, 40)?;
        worksheet.set_column_width(3, 10)?;
        worksheet.set_column_width(4, 12)?;

        Ok(())
    }

    fn write_relationships_sheet(
        &self,
        workbook: &mut Workbook,
        result: &AnalysisResult,
    ) -> Result<()> {
        let worksheet = workbook.add_worksheet().set_name("Relationships")?;

        let header_format = Format::new().set_bold();

        // Headers
        worksheet.write_with_format(0, 0, "From Table", &header_format)?;
        worksheet.write_with_format(0, 1, "From Column", &header_format)?;
        worksheet.write_with_format(0, 2, "To Table", &header_format)?;
        worksheet.write_with_format(0, 3, "To Column", &header_format)?;
        worksheet.write_with_format(0, 4, "Type", &header_format)?;

        // Data
        for (idx, rel) in result.relationships.iter().enumerate() {
            let row = (idx + 1) as u32;
            worksheet.write(row, 0, &rel.from_table)?;
            worksheet.write(row, 1, &rel.from_column)?;
            worksheet.write(row, 2, &rel.to_table)?;
            worksheet.write(row, 3, &rel.to_column)?;
            worksheet.write(row, 4, format!("{:?}", rel.relationship_type))?;
        }

        // Set column widths
        worksheet.set_column_width(0, 25)?;
        worksheet.set_column_width(1, 20)?;
        worksheet.set_column_width(2, 25)?;
        worksheet.set_column_width(3, 20)?;
        worksheet.set_column_width(4, 30)?;

        Ok(())
    }

    fn write_workflows_sheet(
        &self,
        workbook: &mut Workbook,
        result: &AnalysisResult,
    ) -> Result<()> {
        let worksheet = workbook.add_worksheet().set_name("Workflows")?;

        let header_format = Format::new().set_bold();

        // Headers
        worksheet.write_with_format(0, 0, "Workflow Type", &header_format)?;
        worksheet.write_with_format(0, 1, "Description", &header_format)?;
        worksheet.write_with_format(0, 2, "Confidence %", &header_format)?;
        worksheet.write_with_format(0, 3, "Steps", &header_format)?;

        // Data
        for (idx, workflow) in result.workflows.iter().enumerate() {
            let row = (idx + 1) as u32;
            worksheet.write(row, 0, format!("{:?}", workflow.workflow_type))?;
            worksheet.write(row, 1, &workflow.description)?;
            worksheet.write(row, 2, workflow.confidence as f64)?;
            worksheet.write(row, 3, workflow.steps.len() as f64)?;
        }

        // Set column widths
        worksheet.set_column_width(0, 20)?;
        worksheet.set_column_width(1, 50)?;
        worksheet.set_column_width(2, 12)?;
        worksheet.set_column_width(3, 10)?;

        Ok(())
    }

    fn write_reconciliation_sheet(
        &self,
        workbook: &mut Workbook,
        result: &AnalysisResult,
    ) -> Result<()> {
        let worksheet = workbook.add_worksheet().set_name("Reconciliation")?;

        let header_format = Format::new().set_bold();

        // Headers
        worksheet.write_with_format(0, 0, "Source 1", &header_format)?;
        worksheet.write_with_format(0, 1, "Source 2", &header_format)?;
        worksheet.write_with_format(0, 2, "Key Columns", &header_format)?;
        worksheet.write_with_format(0, 3, "Match %", &header_format)?;
        worksheet.write_with_format(0, 4, "Matches", &header_format)?;
        worksheet.write_with_format(0, 5, "Only in Source 1", &header_format)?;
        worksheet.write_with_format(0, 6, "Only in Source 2", &header_format)?;
        worksheet.write_with_format(0, 7, "Duplicates S1", &header_format)?;
        worksheet.write_with_format(0, 8, "Duplicates S2", &header_format)?;
        worksheet.write_with_format(0, 9, "Field Mismatches", &header_format)?;

        // Data
        for (idx, recon) in result.reconciliation_results.iter().enumerate() {
            let row = (idx + 1) as u32;
            worksheet.write(row, 0, &recon.source1_name)?;
            worksheet.write(row, 1, &recon.source2_name)?;
            worksheet.write(row, 2, recon.key_columns.join(", "))?;
            worksheet.write(row, 3, recon.match_percentage)?;
            worksheet.write(row, 4, recon.matches as f64)?;
            worksheet.write(row, 5, recon.only_in_source1 as f64)?;
            worksheet.write(row, 6, recon.only_in_source2 as f64)?;
            worksheet.write(row, 7, recon.duplicates_source1 as f64)?;
            worksheet.write(row, 8, recon.duplicates_source2 as f64)?;
            worksheet.write(row, 9, recon.field_mismatches.len() as f64)?;
        }

        // Set column widths
        for col in 0..10 {
            worksheet.set_column_width(col, 18)?;
        }

        // Add field mismatches detail section if any exist
        if result
            .reconciliation_results
            .iter()
            .any(|r| !r.field_mismatches.is_empty())
        {
            let mut current_row = result.reconciliation_results.len() as u32 + 3;

            for recon in &result.reconciliation_results {
                if !recon.field_mismatches.is_empty() {
                    worksheet.write_with_format(
                        current_row,
                        0,
                        format!(
                            "Mismatches: {} vs {}",
                            recon.source1_name, recon.source2_name
                        ),
                        &header_format,
                    )?;
                    current_row += 1;

                    worksheet.write_with_format(current_row, 0, "Key Value", &header_format)?;
                    worksheet.write_with_format(current_row, 1, "Column", &header_format)?;
                    worksheet.write_with_format(
                        current_row,
                        2,
                        "Source 1 Value",
                        &header_format,
                    )?;
                    worksheet.write_with_format(
                        current_row,
                        3,
                        "Source 2 Value",
                        &header_format,
                    )?;
                    current_row += 1;

                    for mismatch in recon.field_mismatches.iter().take(50) {
                        worksheet.write(current_row, 0, &mismatch.key_value)?;
                        worksheet.write(current_row, 1, &mismatch.column_name)?;
                        worksheet.write(current_row, 2, &mismatch.source1_value)?;
                        worksheet.write(current_row, 3, &mismatch.source2_value)?;
                        current_row += 1;
                    }
                    current_row += 2;
                }
            }
        }

        Ok(())
    }

    fn write_multi_value_sheet(
        &self,
        workbook: &mut Workbook,
        result: &AnalysisResult,
    ) -> Result<()> {
        use crate::analysis::DetectionMethod;

        let worksheet = workbook.add_worksheet().set_name("Multi-Value Columns")?;

        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x70AD47))
            .set_font_color(Color::White);

        // Headers
        worksheet.write_with_format(0, 0, "Table", &header_format)?;
        worksheet.write_with_format(0, 1, "Column", &header_format)?;
        worksheet.write_with_format(0, 2, "Detection Method", &header_format)?;
        worksheet.write_with_format(0, 3, "Delimiter", &header_format)?;
        worksheet.write_with_format(0, 4, "Confidence %", &header_format)?;
        worksheet.write_with_format(0, 5, "Multi-Value Cells", &header_format)?;
        worksheet.write_with_format(0, 6, "Total Cells", &header_format)?;
        worksheet.write_with_format(0, 7, "Ratio %", &header_format)?;
        worksheet.write_with_format(0, 8, "Example (Raw)", &header_format)?;
        worksheet.write_with_format(0, 9, "Example (Split)", &header_format)?;
        worksheet.write_with_format(0, 10, "Unique Atomic Values (sample)", &header_format)?;

        let mut row = 1u32;
        for analysis in &result.multi_value_analyses {
            for col_analysis in &analysis.multi_value_columns {
                let method_str = match &col_analysis.detection_method {
                    DetectionMethod::Delimited(d) => format!("Delimited({})", d),
                    DetectionMethod::VocabularySegmented => "VocabularySegmented".to_string(),
                    DetectionMethod::PatternRepetition => "PatternRepetition".to_string(),
                    DetectionMethod::LengthOutlier => "LengthOutlier".to_string(),
                };

                worksheet.write(row, 0, &col_analysis.table_name)?;
                worksheet.write(row, 1, &col_analysis.column_name)?;
                worksheet.write(row, 2, method_str)?;
                worksheet.write(row, 3, col_analysis.delimiter.as_deref().unwrap_or(""))?;
                worksheet.write(row, 4, (col_analysis.confidence * 100.0).round())?;
                worksheet.write(row, 5, col_analysis.multi_value_cell_count as f64)?;
                worksheet.write(row, 6, col_analysis.total_cell_count as f64)?;
                worksheet.write(row, 7, (col_analysis.multi_value_ratio * 100.0).round())?;

                let raw_example = col_analysis
                    .example_raw
                    .first()
                    .map(|s| s.as_str())
                    .unwrap_or("");
                worksheet.write(row, 8, raw_example)?;

                let split_example = col_analysis
                    .example_split
                    .first()
                    .map(|parts| parts.join(" | "))
                    .unwrap_or_default();
                worksheet.write(row, 9, split_example)?;

                let atoms = col_analysis.unique_atomic_values.join(", ");
                worksheet.write(row, 10, atoms)?;

                row += 1;
            }
        }

        // Column widths
        worksheet.set_column_width(0, 20)?;
        worksheet.set_column_width(1, 25)?;
        worksheet.set_column_width(2, 22)?;
        worksheet.set_column_width(3, 12)?;
        worksheet.set_column_width(4, 14)?;
        worksheet.set_column_width(5, 18)?;
        worksheet.set_column_width(6, 13)?;
        worksheet.set_column_width(7, 10)?;
        worksheet.set_column_width(8, 50)?;
        worksheet.set_column_width(9, 50)?;
        worksheet.set_column_width(10, 60)?;

        Ok(())
    }
}

impl Exporter for ExcelExporter {
    fn export(&self, _result: &AnalysisResult) -> Result<String> {
        // This is not used for Excel since we write directly to file
        Ok(format!("Excel export saved to: {}", self.path))
    }
}
