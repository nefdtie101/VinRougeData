use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber;

use vinrouge::analysis::{
    DataProfiler, GroupingAnalyzer, MultiValueDetector, ReconciliationConfig, Reconciliator,
    RelationshipDetector, WorkflowDetector,
};
use vinrouge::config::{AppConfig, SourceConfig};
use vinrouge::export::{
    AnalysisResult, ConsoleExporter, ExcelExporter, Exporter, GroupedDataExporter, JsonExporter,
    MarkdownExporter,
};
use vinrouge::sources::{CsvSource, DataSource, ExcelSource, FlatfileSource, MssqlSource};

#[derive(Parser)]
#[command(name = "vinrouge")]
#[command(about = "Interactive data analysis tool for databases and files", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch interactive TUI mode
    Interactive,
    /// Analyze data sources and generate reports
    Analyze {
        /// Path to configuration file
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Output format (json, markdown, console)
        #[arg(short = 'f', long, default_value = "console")]
        format: String,

        /// Output file path (prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Pretty print output
        #[arg(short, long)]
        pretty: bool,

        /// MSSQL connection string (alternative to config file)
        #[arg(long)]
        mssql: Option<String>,

        /// CSV file path (alternative to config file)
        #[arg(long)]
        csv: Option<PathBuf>,

        /// Excel file path (alternative to config file)
        #[arg(long)]
        excel: Option<PathBuf>,
    },

    /// Reconcile two data sources
    Reconcile {
        /// First CSV file path
        #[arg(long)]
        csv1: Option<PathBuf>,

        /// Second CSV file path
        #[arg(long)]
        csv2: Option<PathBuf>,

        /// First Excel file path
        #[arg(long)]
        excel1: Option<PathBuf>,

        /// Second Excel file path
        #[arg(long)]
        excel2: Option<PathBuf>,

        /// Key columns for reconciliation (comma-separated)
        #[arg(long)]
        key_columns: Option<String>,

        /// Map a column from source1 to a differently-named column in source2 for value comparison.
        /// Format: source1_col=source2_col  (repeatable, e.g. --column-mapping "Net Pay=Debit")
        #[arg(long, value_name = "COL1=COL2")]
        column_mapping: Vec<String>,

        /// Output format (json, markdown, console)
        #[arg(short = 'f', long, default_value = "console")]
        format: String,

        /// Output file path (prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Pretty print output
        #[arg(short, long)]
        pretty: bool,
    },

    /// Generate a sample configuration file
    GenerateConfig {
        /// Output path for config file
        #[arg(short, long, default_value = "vinrouge.json")]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    match cli.command {
        // If no command specified, launch TUI mode
        None => {
            vinrouge::tui::run().await?;
        }
        Some(Commands::Interactive) => {
            vinrouge::tui::run().await?;
        }
        Some(Commands::Analyze {
            config,
            format,
            output,
            pretty,
            mssql,
            csv,
            excel,
        }) => {
            handle_analyze(
                config,
                format,
                output,
                pretty,
                mssql,
                csv,
                excel,
                cli.verbose,
            )
            .await?;
        }
        Some(Commands::Reconcile {
            csv1,
            csv2,
            excel1,
            excel2,
            key_columns,
            column_mapping,
            format,
            output,
            pretty,
        }) => {
            handle_reconcile(
                csv1,
                csv2,
                excel1,
                excel2,
                key_columns,
                column_mapping,
                format,
                output,
                pretty,
                cli.verbose,
            )
            .await?;
        }
        Some(Commands::GenerateConfig { output }) => {
            handle_generate_config(output)?;
        }
    }

    Ok(())
}

async fn handle_analyze(
    config_path: Option<PathBuf>,
    format: String,
    output_path: Option<PathBuf>,
    pretty: bool,
    mssql_conn: Option<String>,
    csv_path: Option<PathBuf>,
    excel_path: Option<PathBuf>,
    verbose: bool,
) -> Result<()> {
    info!("Starting analysis...");

    // Load configuration or use command-line args
    let sources = if let Some(config_path) = config_path {
        info!("Loading configuration from {:?}", config_path);
        let config = AppConfig::from_file(config_path)?;
        config.sources
    } else {
        // Build sources from command-line arguments
        let mut sources = Vec::new();

        if let Some(conn) = mssql_conn {
            sources.push(SourceConfig::Mssql {
                connection_string: conn,
                name: None,
            });
        }

        if let Some(path) = csv_path {
            sources.push(SourceConfig::Csv {
                path: path.to_string_lossy().to_string(),
                delimiter: None,
                has_header: Some(true),
            });
        }

        if let Some(path) = excel_path {
            sources.push(SourceConfig::Excel {
                path: path.to_string_lossy().to_string(),
                sheet: None,
                has_header: Some(true),
            });
        }

        if sources.is_empty() {
            anyhow::bail!("No data sources specified. Use --config, --mssql, --csv, or --excel");
        }

        sources
    };

    // Extract schemas from all sources
    info!("Extracting schemas from {} sources", sources.len());
    let mut all_tables = Vec::new();
    let mut all_data_profiles = Vec::new();
    let mut all_grouping_analyses = Vec::new();
    let mut loaded_sources: Vec<(String, Vec<Vec<String>>, Vec<vinrouge::schema::Column>)> =
        Vec::new();

    for source_config in sources {
        let tables_and_data = match source_config {
            SourceConfig::Mssql {
                connection_string, ..
            } => {
                info!("Connecting to MSSQL...");
                let mut source = MssqlSource::new(connection_string);
                source.extract_schema().await?
            }
            SourceConfig::Csv {
                path,
                delimiter,
                has_header,
            } => {
                info!("Reading CSV file: {}", path);
                let name = path.split('/').last().unwrap_or(&path).to_string();
                let mut source = CsvSource::new(path);
                if let Some(delim) = delimiter {
                    source = source.with_delimiter(delim);
                }
                if let Some(header) = has_header {
                    source = source.with_header(header);
                }
                let tables = source.extract_schema().await?;

                // Read data for profiling and reconciliation
                if let Ok(data) = source.read_data().await {
                    if !tables.is_empty() {
                        let table = &tables[0];

                        // Store for reconciliation
                        loaded_sources.push((name, data.clone(), table.columns.clone()));

                        // Run data profiling
                        let profiler = DataProfiler::new(10000);
                        let profile = profiler.profile_data(&data, &table.columns);
                        all_data_profiles.push(profile);

                        // Run grouping analysis
                        let analyzer = GroupingAnalyzer::new(1000);
                        let grouping = analyzer.analyze_groupings(&data, &table.columns);
                        all_grouping_analyses.push(grouping);
                    }
                }

                tables
            }
            SourceConfig::Excel {
                path,
                sheet,
                has_header,
            } => {
                info!("Reading Excel file: {}", path);
                let name = path.split('/').last().unwrap_or(&path).to_string();
                let mut source = ExcelSource::new(path);
                if let Some(sheet_name) = sheet {
                    source = source.with_sheet(sheet_name);
                }
                if let Some(header) = has_header {
                    source = source.with_header(header);
                }
                let tables = source.extract_schema().await?;

                // Read data for profiling and reconciliation
                if let Ok(data) = source.read_data().await {
                    if !tables.is_empty() {
                        let table = &tables[0];

                        // Store for reconciliation
                        loaded_sources.push((name, data.clone(), table.columns.clone()));

                        // Run data profiling
                        let profiler = DataProfiler::new(10000);
                        let profile = profiler.profile_data(&data, &table.columns);
                        all_data_profiles.push(profile);

                        // Run grouping analysis
                        let analyzer = GroupingAnalyzer::new(1000);
                        let grouping = analyzer.analyze_groupings(&data, &table.columns);
                        all_grouping_analyses.push(grouping);
                    }
                }

                tables
            }
            SourceConfig::Flatfile {
                path,
                delimiter,
                column_widths,
                column_names,
                has_header,
            } => {
                info!("Reading flat file: {}", path);
                let mut source = if let (Some(widths), Some(names)) = (column_widths, column_names)
                {
                    FlatfileSource::new_fixed_width(path, widths, names)
                } else {
                    FlatfileSource::new_delimited(
                        path,
                        delimiter.unwrap_or(','),
                        has_header.unwrap_or(true),
                    )
                };
                source.extract_schema().await?
            }
        };

        all_tables.extend(tables_and_data);
    }

    info!("Extracted {} tables", all_tables.len());

    // Detect relationships
    info!("Detecting relationships...");
    let mut relationship_detector = RelationshipDetector::new(all_tables.clone());
    let relationships = relationship_detector.detect_relationships();
    info!("Found {} relationships", relationships.len());

    // Detect workflows
    info!("Detecting workflows...");
    let mut workflow_detector = WorkflowDetector::new(all_tables.clone(), relationships.clone());
    let workflows = workflow_detector.detect_workflows();
    info!("Found {} workflows", workflows.len());

    // Auto-reconcile all pairs of loaded sources
    let mut reconciliation_results = Vec::new();
    if loaded_sources.len() >= 2 {
        info!("Reconciling {} sources...", loaded_sources.len());
        let config = ReconciliationConfig::default();
        let reconciliator = Reconciliator::new(config);

        for i in 0..loaded_sources.len() {
            for j in (i + 1)..loaded_sources.len() {
                let (name1, data1, cols1) = &loaded_sources[i];
                let (name2, data2, cols2) = &loaded_sources[j];

                let result = reconciliator.reconcile(name1, data1, cols1, name2, data2, cols2);
                info!("{}", result.summary);
                reconciliation_results.push(result);
            }
        }
        info!("Completed {} reconciliations", reconciliation_results.len());
    }

    // Multi-value detection
    info!("Detecting multi-value columns...");
    let mv_detector = MultiValueDetector::new(5000);
    let all_multi_value_analyses = mv_detector.analyze_all_sources(&loaded_sources);
    info!(
        "Found {} multi-value analyses",
        all_multi_value_analyses.len()
    );

    // Create analysis result
    let result = AnalysisResult {
        tables: all_tables,
        relationships,
        workflows,
        data_profiles: all_data_profiles,
        grouping_analyses: all_grouping_analyses,
        reconciliation_results,
        multi_value_analyses: all_multi_value_analyses,
        source_data: loaded_sources,
    };

    // Export results
    info!("Exporting results in {} format", format);
    let output = match format.to_lowercase().as_str() {
        "json" => {
            let exporter = JsonExporter::new(pretty);
            exporter.export(&result)?
        }
        "markdown" | "md" => {
            let exporter = MarkdownExporter::new();
            exporter.export(&result)?
        }
        "console" => {
            let exporter = ConsoleExporter::new(verbose);
            exporter.export(&result)?
        }
        "excel" | "xlsx" => {
            let path = output_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "analysis_results.xlsx".to_string());

            let exporter = ExcelExporter::new(path.clone());
            exporter.export_to_file(&result)?;
            info!("Exported to {}", path);
            return Ok(());
        }
        "grouped" | "grouped-excel" => {
            let path = output_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "grouped_data.xlsx".to_string());

            // Export grouped data for each source
            for (idx, (name, data, columns)) in result.source_data.iter().enumerate() {
                if idx < result.grouping_analyses.len() {
                    let grouping = &result.grouping_analyses[idx];
                    if !grouping.grouping_dimensions.is_empty() {
                        let file_path = if result.source_data.len() > 1 {
                            let base = path.trim_end_matches(".xlsx");
                            format!(
                                "{}_{}.xlsx",
                                base,
                                name.replace(".csv", "").replace(".xlsx", "")
                            )
                        } else {
                            path.clone()
                        };

                        let mv_cols: &[vinrouge::analysis::MultiValueColumnAnalysis] = result
                            .multi_value_analyses
                            .iter()
                            .find(|a| &a.table_name == name)
                            .map(|a| a.multi_value_columns.as_slice())
                            .unwrap_or(&[]);

                        let exporter = GroupedDataExporter::new(file_path.clone());
                        exporter.export_grouped_data(data, columns, grouping, mv_cols)?;
                        info!("Exported grouped data to {}", file_path);
                    }
                }
            }
            return Ok(());
        }
        _ => anyhow::bail!(
            "Unknown format: {}. Use json, markdown, excel, grouped-excel, or console",
            format
        ),
    };

    // Write output
    if let Some(output_path) = output_path {
        info!("Writing output to {:?}", output_path);
        std::fs::write(output_path, output).context("Failed to write output file")?;
    } else {
        println!("{}", output);
    }

    info!("Analysis complete!");

    Ok(())
}

async fn handle_reconcile(
    csv1: Option<PathBuf>,
    csv2: Option<PathBuf>,
    excel1: Option<PathBuf>,
    excel2: Option<PathBuf>,
    key_columns: Option<String>,
    column_mappings: Vec<String>,
    format: String,
    output_path: Option<PathBuf>,
    pretty: bool,
    verbose: bool,
) -> Result<()> {
    info!("Starting reconciliation...");

    // Determine sources
    let (source1_config, source1_name) = if let Some(path) = csv1 {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        (
            SourceConfig::Csv {
                path: path.to_string_lossy().to_string(),
                delimiter: Some(','),
                has_header: Some(true),
            },
            name,
        )
    } else if let Some(path) = excel1 {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        (
            SourceConfig::Excel {
                path: path.to_string_lossy().to_string(),
                sheet: None,
                has_header: Some(true),
            },
            name,
        )
    } else {
        anyhow::bail!("Must specify --csv1 or --excel1 for first source");
    };

    let (source2_config, source2_name) = if let Some(path) = csv2 {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        (
            SourceConfig::Csv {
                path: path.to_string_lossy().to_string(),
                delimiter: Some(','),
                has_header: Some(true),
            },
            name,
        )
    } else if let Some(path) = excel2 {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        (
            SourceConfig::Excel {
                path: path.to_string_lossy().to_string(),
                sheet: None,
                has_header: Some(true),
            },
            name,
        )
    } else {
        anyhow::bail!("Must specify --csv2 or --excel2 for second source");
    };

    // Load data from both sources
    info!("Loading source 1: {}", source1_name);
    let (data1, columns1) = match source1_config {
        SourceConfig::Csv {
            path,
            delimiter,
            has_header,
        } => {
            let mut source = CsvSource::new(path);
            if let Some(delim) = delimiter {
                source = source.with_delimiter(delim);
            }
            if let Some(header) = has_header {
                source = source.with_header(header);
            }
            let tables = source.extract_schema().await?;
            let data = source.read_data().await?;
            let columns = tables
                .first()
                .map(|t| t.columns.clone())
                .unwrap_or_default();
            (data, columns)
        }
        SourceConfig::Excel {
            path,
            sheet,
            has_header,
        } => {
            let mut source = ExcelSource::new(path);
            if let Some(sheet_name) = sheet {
                source = source.with_sheet(sheet_name);
            }
            if let Some(header) = has_header {
                source = source.with_header(header);
            }
            let tables = source.extract_schema().await?;
            let data = source.read_data().await?;
            let columns = tables
                .first()
                .map(|t| t.columns.clone())
                .unwrap_or_default();
            (data, columns)
        }
        _ => anyhow::bail!("Unsupported source type"),
    };

    info!("Loading source 2: {}", source2_name);
    let (data2, columns2) = match source2_config {
        SourceConfig::Csv {
            path,
            delimiter,
            has_header,
        } => {
            let mut source = CsvSource::new(path);
            if let Some(delim) = delimiter {
                source = source.with_delimiter(delim);
            }
            if let Some(header) = has_header {
                source = source.with_header(header);
            }
            let tables = source.extract_schema().await?;
            let data = source.read_data().await?;
            let columns = tables
                .first()
                .map(|t| t.columns.clone())
                .unwrap_or_default();
            (data, columns)
        }
        SourceConfig::Excel {
            path,
            sheet,
            has_header,
        } => {
            let mut source = ExcelSource::new(path);
            if let Some(sheet_name) = sheet {
                source = source.with_sheet(sheet_name);
            }
            if let Some(header) = has_header {
                source = source.with_header(header);
            }
            let tables = source.extract_schema().await?;
            let data = source.read_data().await?;
            let columns = tables
                .first()
                .map(|t| t.columns.clone())
                .unwrap_or_default();
            (data, columns)
        }
        _ => anyhow::bail!("Unsupported source type"),
    };

    // Set up reconciliation config
    let mut config = ReconciliationConfig::default();
    if let Some(keys) = key_columns {
        config.key_columns = keys.split(',').map(|s| s.trim().to_string()).collect();
    }
    for mapping in column_mappings {
        let parts: Vec<&str> = mapping.splitn(2, '=').collect();
        if parts.len() == 2 {
            config
                .column_mappings
                .push((parts[0].trim().to_string(), parts[1].trim().to_string()));
        } else {
            anyhow::bail!(
                "Invalid --column-mapping '{}': expected format COL1=COL2",
                mapping
            );
        }
    }

    // Run reconciliation
    info!("Running reconciliation...");
    let reconciliator = Reconciliator::new(config);
    let recon_result = reconciliator.reconcile(
        &source1_name,
        &data1,
        &columns1,
        &source2_name,
        &data2,
        &columns2,
    );

    info!("{}", recon_result.summary);

    // Create analysis result with reconciliation
    let result = AnalysisResult {
        tables: Vec::new(),
        relationships: Vec::new(),
        workflows: Vec::new(),
        data_profiles: Vec::new(),
        grouping_analyses: Vec::new(),
        reconciliation_results: vec![recon_result],
        multi_value_analyses: Vec::new(),
        source_data: Vec::new(),
    };

    // Export results
    info!("Exporting results in {} format", format);
    let output = match format.to_lowercase().as_str() {
        "json" => {
            let exporter = JsonExporter::new(pretty);
            exporter.export(&result)?
        }
        "markdown" | "md" => {
            let exporter = MarkdownExporter::new();
            exporter.export(&result)?
        }
        "console" => {
            let exporter = ConsoleExporter::new(verbose);
            exporter.export(&result)?
        }
        "excel" | "xlsx" => {
            let path = output_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "analysis_results.xlsx".to_string());

            let exporter = ExcelExporter::new(path.clone());
            exporter.export_to_file(&result)?;
            info!("Exported to {}", path);
            return Ok(());
        }
        "grouped" | "grouped-excel" => {
            let path = output_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "grouped_data.xlsx".to_string());

            // Export grouped data for each source
            for (idx, (name, data, columns)) in result.source_data.iter().enumerate() {
                if idx < result.grouping_analyses.len() {
                    let grouping = &result.grouping_analyses[idx];
                    if !grouping.grouping_dimensions.is_empty() {
                        let file_path = if result.source_data.len() > 1 {
                            let base = path.trim_end_matches(".xlsx");
                            format!(
                                "{}_{}.xlsx",
                                base,
                                name.replace(".csv", "").replace(".xlsx", "")
                            )
                        } else {
                            path.clone()
                        };

                        let mv_cols: &[vinrouge::analysis::MultiValueColumnAnalysis] = result
                            .multi_value_analyses
                            .iter()
                            .find(|a| &a.table_name == name)
                            .map(|a| a.multi_value_columns.as_slice())
                            .unwrap_or(&[]);

                        let exporter = GroupedDataExporter::new(file_path.clone());
                        exporter.export_grouped_data(data, columns, grouping, mv_cols)?;
                        info!("Exported grouped data to {}", file_path);
                    }
                }
            }
            return Ok(());
        }
        _ => anyhow::bail!(
            "Unknown format: {}. Use json, markdown, excel, grouped-excel, or console",
            format
        ),
    };

    // Write output
    if let Some(output_path) = output_path {
        info!("Writing output to {:?}", output_path);
        std::fs::write(output_path, output).context("Failed to write output file")?;
    } else {
        println!("{}", output);
    }

    info!("Reconciliation complete!");

    Ok(())
}

fn handle_generate_config(output_path: PathBuf) -> Result<()> {
    info!("Generating sample configuration at {:?}", output_path);

    let sample_config = AppConfig {
        sources: vec![
            SourceConfig::Mssql {
                connection_string: "Server=localhost;Database=mydb;User=sa;Password=***;TrustServerCertificate=true".to_string(),
                name: Some("Production DB".to_string()),
            },
            SourceConfig::Csv {
                path: "data/customers.csv".to_string(),
                delimiter: Some(','),
                has_header: Some(true),
            },
            SourceConfig::Excel {
                path: "data/sales.xlsx".to_string(),
                sheet: None,
                has_header: Some(true),
            },
        ],
        export: vinrouge::config::ExportConfig {
            format: "markdown".to_string(),
            output_path: Some("report.md".to_string()),
            pretty: true,
            verbose: false,
        },
        analysis: vinrouge::config::AnalysisConfig {
            detect_relationships: true,
            detect_workflows: true,
            min_confidence: 70,
        },
    };

    sample_config.to_file(&output_path)?;

    println!("Sample configuration written to {:?}", output_path);
    println!("Edit this file to customize your analysis settings.");

    Ok(())
}
