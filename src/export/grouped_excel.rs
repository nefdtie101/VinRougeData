use crate::analysis::{DetectionMethod, GroupingAnalysis, MultiValueColumnAnalysis};
use crate::schema::Column;
use anyhow::Result;
use rust_xlsxwriter::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct GroupedDataExporter {
    path: String,
}

impl GroupedDataExporter {
    pub fn new(path: String) -> Self {
        Self { path }
    }

    /// Export grouped data to separate Excel files in a directory structure.
    /// `mv_columns` provides multi-value column metadata so that rows belonging
    /// to multiple atomic values are fanned out into all relevant groups.
    pub fn export_grouped_data(
        &self,
        data: &[Vec<String>],
        columns: &[Column],
        grouping_analysis: &GroupingAnalysis,
        mv_columns: &[MultiValueColumnAnalysis],
    ) -> Result<()> {
        // Check if there are any grouping dimensions to export
        if grouping_analysis.grouping_dimensions.is_empty() {
            anyhow::bail!(
                "No grouping dimensions found in the data. Cannot create grouped export."
            );
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
        let valid_dimensions: Vec<_> = grouping_analysis.grouping_dimensions.iter().collect();

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

            print!(
                "  {} [{}/{}] {} ",
                spinner, current, total, dimension.column_name
            );
            std::io::Write::flush(&mut std::io::stdout())?;

            self.create_dimension_files(
                &output_dir,
                data,
                columns,
                dimension,
                &grouping_analysis.table_name,
                mv_columns,
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

    fn create_summary_file(&self, output_dir: &Path, analysis: &GroupingAnalysis) -> Result<()> {
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
            format!(
                "Generated: {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ),
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
        mv_columns: &[MultiValueColumnAnalysis],
    ) -> Result<()> {
        // Find the column index
        let col_idx = columns
            .iter()
            .position(|col| col.name == dimension.column_name);

        if col_idx.is_none() {
            return Ok(());
        }
        let col_idx = col_idx.unwrap();

        // Check if this dimension column is a detected multi-value column
        let mv_meta = mv_columns
            .iter()
            .find(|mv| mv.column_name == dimension.column_name);

        // Pre-build vocabulary for VocabularySegmented columns so we can re-run DP
        let vocab: HashSet<String> = if let Some(mv) = mv_meta {
            if matches!(mv.detection_method, DetectionMethod::VocabularySegmented) {
                mv.unique_atomic_values.iter().cloned().collect()
            } else {
                HashSet::new()
            }
        } else {
            HashSet::new()
        };

        // Build groups from actual data (case-insensitive).
        // A row may land in MULTIPLE groups when its cell contains several atomic values.
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (row_idx, row) in data.iter().enumerate() {
            if let Some(value) = row.get(col_idx) {
                let cleaned = Self::clean_cell_value(value);
                if cleaned.is_empty() {
                    continue;
                }

                // Determine atomic group keys for this cell
                let keys: Vec<String> = match mv_meta.map(|mv| &mv.detection_method) {
                    Some(DetectionMethod::Delimited(delim)) => {
                        // Fan-out: one row → one entry per delimited value
                        cleaned
                            .split(delim.as_str())
                            .map(|s| s.trim().to_lowercase())
                            .filter(|s| !s.is_empty())
                            .collect()
                    }
                    Some(DetectionMethod::VocabularySegmented) if !vocab.is_empty() => {
                        // Re-run DP segmentation using the sampled vocabulary
                        if let Some(segments) = Self::dp_segment_static(&cleaned, &vocab) {
                            segments.into_iter().map(|s| s.to_lowercase()).collect()
                        } else {
                            vec![cleaned.to_lowercase()]
                        }
                    }
                    // PatternRepetition / LengthOutlier / no multi-value info:
                    // fall back to the full cell value (too uncertain to split)
                    _ => vec![cleaned.to_lowercase()],
                };

                for key in keys {
                    groups.entry(key).or_default().push(row_idx);
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

    /// DP string segmentation against a vocabulary set.
    /// Returns `Some(segments)` if the full string can be covered, `None` otherwise.
    fn dp_segment_static<'a>(s: &'a str, vocab: &HashSet<String>) -> Option<Vec<&'a str>> {
        let chars: Vec<char> = s.chars().collect();
        let n = chars.len();
        if n == 0 {
            return None;
        }

        // Build byte-offset lookup per char position
        let mut char_starts = vec![0usize; n + 1];
        let mut byte_pos = 0;
        for (i, c) in chars.iter().enumerate() {
            char_starts[i] = byte_pos;
            byte_pos += c.len_utf8();
        }
        char_starts[n] = byte_pos;

        let mut dp = vec![false; n + 1];
        let mut prev = vec![usize::MAX; n + 1];
        dp[0] = true;

        for i in 1..=n {
            for j in 0..i {
                if dp[j] {
                    let slice = &s[char_starts[j]..char_starts[i]];
                    if vocab.contains(slice) {
                        dp[i] = true;
                        prev[i] = j;
                        break;
                    }
                }
            }
        }

        if !dp[n] {
            return None;
        }

        let mut segments: Vec<&str> = Vec::new();
        let mut pos = n;
        while pos > 0 {
            let start = prev[pos];
            segments.push(&s[char_starts[start]..char_starts[pos]]);
            pos = start;
        }
        segments.reverse();
        Some(segments)
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

    // ── Web / in-memory export ────────────────────────────────────────────────

    /// Export all sources with grouping dimensions into a single ZIP archive
    /// returned as raw bytes.  Mirrors the TUI's `GroupedExcel` export path but
    /// writes everything in memory so it can be downloaded from the browser.
    pub fn export_all_to_zip(&self, result: &super::AnalysisResult) -> Result<Vec<u8>> {
        use std::io::{Cursor, Write as _};
        use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let opts =
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        let mut exported = 0usize;

        for (idx, (name, data, columns)) in result.source_data.iter().enumerate() {
            let Some(grouping) = result.grouping_analyses.get(idx) else {
                continue;
            };
            if grouping.grouping_dimensions.is_empty() {
                continue;
            }

            let mv_cols: &[crate::analysis::MultiValueColumnAnalysis] = result
                .multi_value_analyses
                .iter()
                .find(|a| &a.table_name == name)
                .map(|a| a.multi_value_columns.as_slice())
                .unwrap_or(&[]);

            // Path prefix so multiple sources don't collide inside the ZIP.
            let prefix = if result.source_data.len() > 1 {
                let stem = name
                    .strip_suffix(".csv")
                    .or_else(|| name.strip_suffix(".xlsx"))
                    .or_else(|| name.strip_suffix(".xls"))
                    .unwrap_or(name.as_str());
                format!("{}/", stem)
            } else {
                String::new()
            };

            // Summary sheet
            let summary_bytes = self.create_summary_workbook_bytes(grouping)?;
            zip.start_file(format!("{}_summary.xlsx", prefix), opts)?;
            zip.write_all(&summary_bytes)?;

            // One file per group per dimension
            for dimension in &grouping.grouping_dimensions {
                let groups = self.build_groups(data, columns, dimension, mv_cols);
                let mut group_vec: Vec<_> = groups.iter().collect();
                group_vec.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

                for (group_value, row_indices) in &group_vec {
                    let safe = self.sanitize_filename(group_value);
                    let path = format!("{}{}/{}.xlsx", prefix, dimension.column_name, safe);
                    let bytes = self.create_group_workbook_bytes(columns, row_indices, data)?;
                    zip.start_file(&path, opts)?;
                    zip.write_all(&bytes)?;
                }
            }

            exported += 1;
        }

        if exported == 0 {
            anyhow::bail!("No grouping dimensions found to export.");
        }

        Ok(zip.finish()?.into_inner())
    }

    /// Build a `group_value → row_indices` map for one dimension.
    /// Duplicates the fan-out logic from `create_dimension_files` so the
    /// in-memory path stays independent of the filesystem path.
    fn build_groups(
        &self,
        data: &[Vec<String>],
        columns: &[Column],
        dimension: &crate::analysis::GroupingDimension,
        mv_columns: &[MultiValueColumnAnalysis],
    ) -> HashMap<String, Vec<usize>> {
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();

        let Some(col_idx) = columns.iter().position(|c| c.name == dimension.column_name) else {
            return groups;
        };

        let mv_meta = mv_columns
            .iter()
            .find(|mv| mv.column_name == dimension.column_name);
        let vocab: HashSet<String> = match mv_meta {
            Some(mv) if matches!(mv.detection_method, DetectionMethod::VocabularySegmented) => {
                mv.unique_atomic_values.iter().cloned().collect()
            }
            _ => HashSet::new(),
        };

        for (row_idx, row) in data.iter().enumerate() {
            let Some(value) = row.get(col_idx) else {
                continue;
            };
            let cleaned = Self::clean_cell_value(value);
            if cleaned.is_empty() {
                continue;
            }

            let keys: Vec<String> = match mv_meta.map(|mv| &mv.detection_method) {
                Some(DetectionMethod::Delimited(delim)) => cleaned
                    .split(delim.as_str())
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect(),
                Some(DetectionMethod::VocabularySegmented) if !vocab.is_empty() => {
                    if let Some(segs) = Self::dp_segment_static(&cleaned, &vocab) {
                        segs.into_iter().map(|s| s.to_lowercase()).collect()
                    } else {
                        vec![cleaned.to_lowercase()]
                    }
                }
                _ => vec![cleaned.to_lowercase()],
            };

            for key in keys {
                groups.entry(key).or_default().push(row_idx);
            }
        }

        groups
    }

    fn create_summary_workbook_bytes(&self, analysis: &GroupingAnalysis) -> Result<Vec<u8>> {
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
            format!(
                "Generated: {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ),
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

        workbook.save_to_buffer().map_err(Into::into)
    }

    fn create_group_workbook_bytes(
        &self,
        columns: &[Column],
        row_indices: &[usize],
        data: &[Vec<String>],
    ) -> Result<Vec<u8>> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet().set_name("Data")?;

        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0xD9D9D9));

        for (col_i, column) in columns.iter().enumerate() {
            worksheet.write_with_format(0, col_i as u16, &column.name, &header_format)?;
        }

        for (output_row, &data_row_idx) in row_indices.iter().enumerate() {
            if let Some(row_data) = data.get(data_row_idx) {
                for (col_i, value) in row_data.iter().enumerate() {
                    worksheet
                        .write((output_row + 1) as u32, col_i as u16, Self::clean_cell_value(value))?;
                }
            }
        }

        for col_i in 0..columns.len() {
            worksheet.set_column_width(col_i as u16, 15)?;
        }

        workbook.save_to_buffer().map_err(Into::into)
    }
}
