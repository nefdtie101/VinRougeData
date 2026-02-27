use super::{AnalysisResult, Exporter};
use crate::schema::RelationshipType;
use anyhow::Result;

pub struct MarkdownExporter;

impl MarkdownExporter {
    pub fn new() -> Self {
        Self
    }
}

impl Exporter for MarkdownExporter {
    fn export(&self, result: &AnalysisResult) -> Result<String> {
        let mut output = String::new();

        // Title
        output.push_str("# Data Analysis Report\n\n");
        output.push_str(&format!(
            "Generated: {}\n\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ));

        // Summary
        output.push_str("## Summary\n\n");
        output.push_str(&format!("- **Tables**: {}\n", result.tables.len()));
        output.push_str(&format!(
            "- **Relationships**: {}\n",
            result.relationships.len()
        ));
        output.push_str(&format!("- **Workflows**: {}\n", result.workflows.len()));
        output.push_str(&format!("- **Reconciliations**: {}\n\n", result.reconciliation_results.len()));

        // Tables section
        output.push_str("## Tables\n\n");

        for table in &result.tables {
            output.push_str(&format!("### {}\n\n", table.full_name));
            output.push_str(&format!("- **Source**: {} ({})\n", table.source_type, table.source_location));

            if let Some(row_count) = table.row_count {
                output.push_str(&format!("- **Rows**: {}\n", row_count));
            }

            output.push_str(&format!("- **Columns**: {}\n\n", table.columns.len()));

            // Column table
            output.push_str("| Column | Type | Nullable | PK | FK |\n");
            output.push_str("|--------|------|----------|----|----||\n");

            for col in &table.columns {
                output.push_str(&format!(
                    "| {} | {:?} | {} | {} | {} |\n",
                    col.name,
                    col.data_type,
                    if col.nullable { "✓" } else { "" },
                    if col.is_primary_key { "✓" } else { "" },
                    if col.is_foreign_key { "✓" } else { "" }
                ));
            }

            output.push_str("\n");
        }

        // Relationships section
        output.push_str("## Relationships\n\n");

        if result.relationships.is_empty() {
            output.push_str("No relationships detected.\n\n");
        } else {
            output.push_str("| From | To | Type | Confidence |\n");
            output.push_str("|------|----|----|------------|\n");

            for rel in &result.relationships {
                let rel_type = match &rel.relationship_type {
                    RelationshipType::ForeignKey => "Foreign Key".to_string(),
                    RelationshipType::NameMatch { confidence } => {
                        format!("Name Match ({}%)", confidence)
                    }
                    RelationshipType::ValueOverlap { overlap_percent } => {
                        format!("Value Overlap ({}%)", overlap_percent)
                    }
                    RelationshipType::UniquePattern => "Unique Pattern".to_string(),
                    RelationshipType::Composite => "Composite".to_string(),
                };

                output.push_str(&format!(
                    "| {}.{} | {}.{} | {} | |\n",
                    rel.from_table, rel.from_column, rel.to_table, rel.to_column, rel_type
                ));
            }

            output.push_str("\n");
        }

        // Workflows section
        output.push_str("## Workflows\n\n");

        if result.workflows.is_empty() {
            output.push_str("No workflows detected.\n\n");
        } else {
            for workflow in &result.workflows {
                output.push_str(&format!(
                    "### {:?} Workflow ({}% confidence)\n\n",
                    workflow.workflow_type, workflow.confidence
                ));
                output.push_str(&format!("{}\n\n", workflow.description));

                output.push_str("**Steps:**\n\n");
                for (idx, step) in workflow.steps.iter().enumerate() {
                    output.push_str(&format!(
                        "{}. **{}** ({}): {}\n",
                        idx + 1,
                        step.table_name,
                        step.step_type,
                        step.description
                    ));
                }

                output.push_str("\n");
            }
        }

        // Reconciliation section
        output.push_str("## Reconciliation Results\n\n");

        if result.reconciliation_results.is_empty() {
            output.push_str("No reconciliations performed.\n\n");
        } else {
            for recon in &result.reconciliation_results {
                output.push_str(&format!("### {} vs {}\n\n", recon.source1_name, recon.source2_name));
                output.push_str(&format!("**Key Columns**: {}\n\n", recon.key_columns.join(", ")));

                output.push_str(&format!("- **Match Percentage**: {:.1}%\n", recon.match_percentage));
                output.push_str(&format!("- **Total Matches**: {}\n", recon.matches));
                output.push_str(&format!("- **Only in {}**: {}\n", recon.source1_name, recon.only_in_source1));
                output.push_str(&format!("- **Only in {}**: {}\n", recon.source2_name, recon.only_in_source2));

                if recon.duplicates_source1 > 0 || recon.duplicates_source2 > 0 {
                    output.push_str(&format!("- **Duplicates in source 1**: {}\n", recon.duplicates_source1));
                    output.push_str(&format!("- **Duplicates in source 2**: {}\n", recon.duplicates_source2));
                }

                output.push_str("\n");

                if !recon.field_mismatches.is_empty() {
                    output.push_str(&format!("**Field Mismatches** ({} found):\n\n", recon.field_mismatches.len()));
                    output.push_str("| Key | Column | Source 1 | Source 2 |\n");
                    output.push_str("|-----|--------|----------|----------|\n");

                    for mismatch in recon.field_mismatches.iter().take(20) {
                        output.push_str(&format!(
                            "| {} | {} | {} | {} |\n",
                            mismatch.key_value,
                            mismatch.column_name,
                            mismatch.source1_value,
                            mismatch.source2_value
                        ));
                    }

                    output.push_str("\n");
                }
            }
        }

        Ok(output)
    }
}
