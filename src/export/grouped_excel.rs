use crate::analysis::GroupingAnalysis;
use crate::schema::Column;
use anyhow::Result;
use rust_xlsxwriter::*;
use std::collections::HashMap;
use std::path::Path;

pub struct GroupedDataExporter {
    path: String,
}

impl GroupedDataExporter {
    pub fn new(path: String) -> Self {
        Self { path }
    }

    /// Export grouped data to separate Excel files in a directory structure
    pub fn export_grouped_data(
        &self,
        data: &[Vec<String>],
        columns: &[Column],
        grouping_analysis: &GroupingAnalysis,
    ) -> Result<()> {
        // Check if there are any grouping dimensions to export
        if grouping_analysis.grouping_dimensions.is_empty() {
            anyhow::bail!("No grouping dimensions found in the data. Cannot create grouped export.");
        }

        // Create base directory for grouped exports
        let base_path = Path::new(&self.path);
        let dir_name = base_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("grouped_export");

        let parent_dir = base_path.parent().unwrap_or_else(|| Path::new("."));
        let output_dir = parent_dir.join(dir_name);

        // Create the output directory
        std::fs::create_dir_all(&output_dir)?;

        // Animation: Show start of export
        println!("🚀 Starting grouped export...");

        // Create summary file
        print!("📊 Creating summary file... ");
        std::io::Write::flush(&mut std::io::stdout())?;
        self.create_summary_file(&output_dir, grouping_analysis)?;
        println!("✓");

        // Get all dimensions
        let valid_dimensions: Vec<_> = grouping_analysis
            .grouping_dimensions
            .iter()
            .collect();

        let total = valid_dimensions.len();

        if total == 0 {
            println!("⚠️  No grouping dimensions found");
        } else {
            println!("📁 Processing {} dimension(s):", total);
        }

        // For each grouping dimension, create separate files for each group
        for (idx, dimension) in valid_dimensions.iter().enumerate() {
            let current = idx + 1;
            let spinner = Self::get_spinner_char(idx);

            print!("  {} [{}/{}] {} ", spinner, current, total, dimension.column_name);
            std::io::Write::flush(&mut std::io::stdout())?;

            self.create_dimension_files(
                &output_dir,
                data,
                columns,
                dimension,
                &grouping_analysis.table_name,
            )?;

            println!("✓ ({} groups)", dimension.group_count);
        }

        println!("✨ Grouped data exported to: {}", output_dir.display());
        Ok(())
    }

    fn get_spinner_char(idx: usize) -> char {
        const SPINNER: [char; 4] = ['⠋', '⠙', '⠹', '⠸'];
        SPINNER[idx % SPINNER.len()]
    }

    fn create_summary_file(
        &self,
        output_dir: &Path,
        analysis: &GroupingAnalysis,
    ) -> Result<()> {
        let summary_path = output_dir.join("_summary.xlsx");
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet().set_name("Summary")?;

        let header_format = Format::new()
            .set_bold()
            .set_font_size(14)
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White);

        let bold = Format::new().set_bold();

        worksheet.write_with_format(0, 0, "Grouped Data Export", &header_format)?;
        worksheet.write(
            1,
            0,
            format!("Generated: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")),
        )?;

        if !analysis.table_name.is_empty() {
            worksheet.write(2, 0, format!("Source: {}", analysis.table_name))?;
        }

        worksheet.write_with_format(4, 0, "Grouping Dimensions", &bold)?;
        worksheet.write_with_format(4, 1, "Groups", &bold)?;
        worksheet.write_with_format(4, 2, "Type", &bold)?;
        worksheet.write_with_format(4, 3, "Files", &bold)?;

        for (idx, dimension) in analysis.grouping_dimensions.iter().enumerate() {
            let row = (5 + idx) as u32;
            worksheet.write(row, 0, &dimension.column_name)?;
            worksheet.write(row, 1, dimension.group_count as f64)?;
            worksheet.write(row, 2, format!("{:?}", dimension.dimension_type))?;
            worksheet.write(row, 3, format!("{}/", dimension.column_name))?;
        }

        worksheet.set_column_width(0, 25)?;
        worksheet.set_column_width(1, 15)?;
        worksheet.set_column_width(2, 20)?;
        worksheet.set_column_width(3, 30)?;

        workbook.save(&summary_path)?;
        Ok(())
    }

    fn create_dimension_files(
        &self,
        output_dir: &Path,
        data: &[Vec<String>],
        columns: &[Column],
        dimension: &crate::analysis::GroupingDimension,
        _table_name: &str,
    ) -> Result<()> {
        // Find the column index
        let col_idx = columns
            .iter()
            .position(|col| col.name == dimension.column_name);

        if col_idx.is_none() {
            return Ok(());
        }
        let col_idx = col_idx.unwrap();

        // Build groups from actual data (case-insensitive)
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (row_idx, row) in data.iter().enumerate() {
            if let Some(value) = row.get(col_idx) {
                let cleaned = Self::clean_cell_value(value);
                if !cleaned.is_empty() {
                    // Use lowercase for grouping to handle case-insensitive matching
                    let group_key = cleaned.to_lowercase();
                    groups
                        .entry(group_key)
                        .or_insert_with(Vec::new)
                        .push(row_idx);
                }
            }
        }

        // Create subdirectory for this dimension
        let dimension_dir = output_dir.join(&dimension.column_name);
        std::fs::create_dir_all(&dimension_dir)?;

        // Create a separate file for each group
        let mut group_vec: Vec<_> = groups.iter().collect();
        group_vec.sort_by(|a, b| b.1.len().cmp(&a.1.len())); // Sort by count descending

        for (group_value, row_indices) in group_vec.iter() {
            // Create safe filename
            let safe_filename = self.sanitize_filename(group_value);
            let file_path = dimension_dir.join(format!("{}.xlsx", safe_filename));

            // Create workbook for this group
            let mut workbook = Workbook::new();
            let worksheet = workbook.add_worksheet().set_name("Data")?;

            // Write header
            let header_format = Format::new()
                .set_bold()
                .set_background_color(Color::RGB(0xD9D9D9));

            for (col_idx, column) in columns.iter().enumerate() {
                worksheet.write_with_format(0, col_idx as u16, &column.name, &header_format)?;
            }

            // Write data rows for this group (with cleaning)
            for (output_row, &data_row_idx) in row_indices.iter().enumerate() {
                if let Some(row_data) = data.get(data_row_idx) {
                    for (col_idx, value) in row_data.iter().enumerate() {
                        let cleaned_value = Self::clean_cell_value(value);
                        worksheet.write((output_row + 1) as u32, col_idx as u16, &cleaned_value)?;
                    }
                }
            }

            // Auto-fit columns
            for col_idx in 0..columns.len() {
                worksheet.set_column_width(col_idx as u16, 15)?;
            }

            workbook.save(&file_path)?;
        }

        Ok(())
    }

    fn sanitize_filename(&self, name: &str) -> String {
        // Filename restrictions (cross-platform safe):
        // - Cannot contain: : \ / ? * " < > |
        // - Max 255 characters (leaving room for extension)
        let mut sanitized = name
            .replace(':', "-")
            .replace('\\', "-")
            .replace('/', "-")
            .replace('?', "")
            .replace('*', "")
            .replace('"', "")
            .replace('<', "(")
            .replace('>', ")")
            .replace('|', "-")
            .trim()
            .to_string();

        // Replace multiple spaces/dashes with single ones
        while sanitized.contains("  ") {
            sanitized = sanitized.replace("  ", " ");
        }
        while sanitized.contains("--") {
            sanitized = sanitized.replace("--", "-");
        }

        // Truncate if too long (leave room for .xlsx extension)
        if sanitized.len() > 200 {
            sanitized.truncate(200);
        }

        // Handle empty names
        if sanitized.is_empty() {
            sanitized = "unnamed".to_string();
        }

        sanitized
    }

    /// Clean and normalize cell values for better readability
    fn clean_cell_value(value: &str) -> String {
        let trimmed = value.trim();

        // Handle empty or whitespace-only values
        if trimmed.is_empty() {
            return String::from("");
        }

        // Normalize whitespace (replace multiple spaces/tabs with single space)
        let normalized = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");

        // Clean common problematic characters
        let cleaned = normalized
            .replace('\r', "") // Remove carriage returns
            .replace('\n', " ") // Replace newlines with spaces
            .replace('\t', " "); // Replace tabs with spaces

        cleaned
    }

}
