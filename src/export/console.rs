use super::{AnalysisResult, Exporter};
use crate::schema::RelationshipType;
use anyhow::Result;

pub struct ConsoleExporter {
    verbose: bool,
}

impl ConsoleExporter {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl Exporter for ConsoleExporter {
    fn export(&self, result: &AnalysisResult) -> Result<String> {
        let mut output = String::new();

        // Header
        output.push_str("═══════════════════════════════════════════════════════════\n");
        output.push_str("                  DATA ANALYSIS REPORT\n");
        output.push_str("═══════════════════════════════════════════════════════════\n\n");

        // Summary
        output.push_str(&format!("Tables Found:       {}\n", result.tables.len()));
        output.push_str(&format!(
            "Relationships:      {}\n",
            result.relationships.len()
        ));
        output.push_str(&format!("Workflows Detected: {}\n", result.workflows.len()));
        output.push_str(&format!(
            "Data Profiles:      {}\n",
            result.data_profiles.len()
        ));
        output.push_str(&format!(
            "Grouping Analyses:  {}\n",
            result.grouping_analyses.len()
        ));
        output.push_str(&format!(
            "Reconciliations:    {}\n\n",
            result.reconciliation_results.len()
        ));

        // Tables
        output.push_str("───────────────────────────────────────────────────────────\n");
        output.push_str("TABLES\n");
        output.push_str("───────────────────────────────────────────────────────────\n\n");

        for table in &result.tables {
            output.push_str(&format!("📊 {}\n", table.full_name));
            output.push_str(&format!(
                "   Source: {} ({})\n",
                table.source_type, table.source_location
            ));

            if let Some(row_count) = table.row_count {
                output.push_str(&format!("   Rows: {}\n", row_count));
            }

            output.push_str(&format!("   Columns: {}\n", table.columns.len()));

            if self.verbose {
                for col in &table.columns {
                    let mut flags = Vec::new();
                    if col.is_primary_key {
                        flags.push("PK");
                    }
                    if col.is_foreign_key {
                        flags.push("FK");
                    }
                    if !col.nullable {
                        flags.push("NOT NULL");
                    }

                    let flag_str = if flags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", flags.join(", "))
                    };

                    output.push_str(&format!(
                        "      • {} : {:?}{}\n",
                        col.name, col.data_type, flag_str
                    ));
                }
            }

            output.push_str("\n");
        }

        // Relationships
        output.push_str("───────────────────────────────────────────────────────────\n");
        output.push_str("RELATIONSHIPS\n");
        output.push_str("───────────────────────────────────────────────────────────\n\n");

        if result.relationships.is_empty() {
            output.push_str("No relationships detected.\n\n");
        } else {
            for rel in &result.relationships {
                let rel_type = match &rel.relationship_type {
                    RelationshipType::ForeignKey => "🔑 FK",
                    RelationshipType::NameMatch { confidence } => {
                        &format!("📛 Name Match ({}%)", confidence)
                    }
                    RelationshipType::ValueOverlap { overlap_percent } => {
                        &format!("🔄 Value Overlap ({}%)", overlap_percent)
                    }
                    RelationshipType::UniquePattern => "🎯 Unique Pattern",
                    RelationshipType::Composite => "🧩 Composite",
                };

                output.push_str(&format!(
                    "{} {}.{} → {}.{}\n",
                    rel_type, rel.from_table, rel.from_column, rel.to_table, rel.to_column
                ));
            }
            output.push_str("\n");
        }

        // Workflows
        output.push_str("───────────────────────────────────────────────────────────\n");
        output.push_str("WORKFLOWS\n");
        output.push_str("───────────────────────────────────────────────────────────\n\n");

        if result.workflows.is_empty() {
            output.push_str("No workflows detected.\n\n");
        } else {
            for workflow in &result.workflows {
                let icon = match workflow.workflow_type {
                    crate::analysis::WorkflowType::Import => "📥",
                    crate::analysis::WorkflowType::StagingToProduction => "🔄",
                    crate::analysis::WorkflowType::Aggregation => "📊",
                    crate::analysis::WorkflowType::Transformation => "⚙️",
                    crate::analysis::WorkflowType::Lookup => "📖",
                };

                output.push_str(&format!(
                    "{} {:?} ({}% confidence)\n",
                    icon, workflow.workflow_type, workflow.confidence
                ));
                output.push_str(&format!("   {}\n", workflow.description));

                if self.verbose {
                    output.push_str("   Steps:\n");
                    for (idx, step) in workflow.steps.iter().enumerate() {
                        output.push_str(&format!(
                            "      {}. {} [{}]\n",
                            idx + 1,
                            step.table_name,
                            step.step_type
                        ));
                    }
                }

                output.push_str("\n");
            }
        }

        // Data Profiling
        output.push_str("───────────────────────────────────────────────────────────\n");
        output.push_str("DATA PROFILING\n");
        output.push_str("───────────────────────────────────────────────────────────\n\n");

        if result.data_profiles.is_empty() {
            output.push_str("No data profiling performed.\n\n");
        } else {
            for profile in &result.data_profiles {
                output.push_str("📈 Column Patterns:\n");
                for col_profile in &profile.column_profiles {
                    if !col_profile.data_patterns.is_empty() {
                        output.push_str(&format!(
                            "   • {} - {:?}\n",
                            col_profile.column_name, col_profile.data_patterns
                        ));
                    }
                }

                if !profile.correlations.is_empty() {
                    output.push_str("\n🔗 Column Correlations:\n");
                    for corr in &profile.correlations {
                        output.push_str(&format!(
                            "   • {} ↔ {} : {:?}\n",
                            corr.column_a, corr.column_b, corr.correlation_type
                        ));
                    }
                }

                output.push_str("\n");
            }
        }

        // Grouping Analysis
        output.push_str("───────────────────────────────────────────────────────────\n");
        output.push_str("GROUPING ANALYSIS\n");
        output.push_str("───────────────────────────────────────────────────────────\n\n");

        if result.grouping_analyses.is_empty() {
            output.push_str("No grouping analysis performed.\n\n");
        } else {
            for analysis in &result.grouping_analyses {
                if !analysis.grouping_dimensions.is_empty() {
                    output.push_str(&format!(
                        "📊 Found {} grouping dimensions:\n\n",
                        analysis.grouping_dimensions.len()
                    ));

                    for dim in &analysis.grouping_dimensions {
                        let icon = match dim.dimension_type {
                            crate::analysis::DimensionType::Temporal => "⏰",
                            crate::analysis::DimensionType::Categorical => "📂",
                            crate::analysis::DimensionType::Geographic => "🌍",
                            crate::analysis::DimensionType::Hierarchical => "📊",
                            crate::analysis::DimensionType::Identifier => "👤",
                            crate::analysis::DimensionType::Numeric => "🔢",
                        };

                        output.push_str(&format!(
                            "{} {} ({:?})\n",
                            icon, dim.column_name, dim.dimension_type
                        ));
                        output.push_str(&format!(
                            "   Groups: {}, Avg records/group: {:.1}\n",
                            dim.group_count, dim.records_per_group.avg
                        ));

                        if self.verbose {
                            output.push_str("   Examples:\n");
                            for example in &dim.example_groups {
                                output.push_str(&format!(
                                    "      • {}: {} records\n",
                                    example.group_value, example.record_count
                                ));
                            }
                        }

                        if !dim.insights.is_empty() {
                            output.push_str("   Insights:\n");
                            for insight in &dim.insights {
                                output.push_str(&format!("      • {}\n", insight));
                            }
                        }

                        output.push_str("\n");
                    }
                }

                if !analysis.hierarchies.is_empty() {
                    output.push_str("🔗 Hierarchical Relationships:\n");
                    for hierarchy in &analysis.hierarchies {
                        let levels_str = hierarchy.levels.join(" → ");
                        output.push_str(&format!(
                            "   {} ({:?})\n",
                            levels_str, hierarchy.hierarchy_type
                        ));
                    }
                    output.push_str("\n");
                }

                if !analysis.suggested_analyses.is_empty() {
                    output.push_str("💡 Suggested Analyses:\n");
                    for suggestion in &analysis.suggested_analyses {
                        output.push_str(&format!("   • {}\n", suggestion));
                    }
                    output.push_str("\n");
                }
            }
        }

        // Reconciliation Results
        output.push_str("───────────────────────────────────────────────────────────\n");
        output.push_str("RECONCILIATION RESULTS\n");
        output.push_str("───────────────────────────────────────────────────────────\n\n");

        if result.reconciliation_results.is_empty() {
            output.push_str("No reconciliations performed.\n\n");
        } else {
            for recon in &result.reconciliation_results {
                output.push_str(&format!(
                    "🔄 {} vs {}\n",
                    recon.source1_name, recon.source2_name
                ));
                output.push_str(&format!(
                    "   Key Columns: {}\n",
                    recon.key_columns.join(", ")
                ));
                output.push_str(&format!("   Match Rate: {:.1}%\n", recon.match_percentage));
                output.push_str(&format!("   Matches: {}\n", recon.matches));
                output.push_str(&format!(
                    "   Only in {}: {}\n",
                    recon.source1_name, recon.only_in_source1
                ));
                output.push_str(&format!(
                    "   Only in {}: {}\n",
                    recon.source2_name, recon.only_in_source2
                ));

                if recon.duplicates_source1 > 0 || recon.duplicates_source2 > 0 {
                    output.push_str(&format!(
                        "   Duplicates: {} in source1, {} in source2\n",
                        recon.duplicates_source1, recon.duplicates_source2
                    ));
                }

                if !recon.field_mismatches.is_empty() {
                    output.push_str(&format!(
                        "   Field Mismatches: {} found\n",
                        recon.field_mismatches.len()
                    ));

                    if self.verbose {
                        for mismatch in recon.field_mismatches.iter().take(10) {
                            output.push_str(&format!(
                                "      • {} [{}]: '{}' vs '{}'\n",
                                mismatch.key_value,
                                mismatch.column_name,
                                mismatch.source1_value,
                                mismatch.source2_value
                            ));
                        }
                    }
                }

                output.push_str("\n");
            }
        }

        output.push_str("═══════════════════════════════════════════════════════════\n");

        Ok(output)
    }
}
