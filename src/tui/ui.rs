use crate::analysis::DetectionMethod;
use crate::tui::app::{App, AppState, Screen};
use crate::tui::file_browser::FileBrowser;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status bar
        ])
        .split(f.area());

    // Title
    draw_title(f, chunks[0]);

    // Content based on screen and state
    match &app.state {
        AppState::BrowsingFiles { browser, .. } => draw_file_browser(f, chunks[1], browser),
        _ => match app.screen {
            Screen::Home => draw_home(f, chunks[1]),
            Screen::SourceList => draw_source_list(f, chunks[1], app),
            Screen::Analysis => draw_analysis(f, chunks[1], app),
            Screen::Results => draw_results(f, chunks[1], app),
            Screen::Reconcile => draw_reconcile(f, chunks[1], app),
            Screen::Export => draw_export(f, chunks[1], app),
            Screen::Help => draw_help(f, chunks[1]),
        },
    }

    // Status bar
    draw_status_bar(f, chunks[2], app);
}

fn draw_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new(" VinRouge - Data Analysis Tool ")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

fn draw_home(f: &mut Frame, area: Rect) {
    let menu_items = vec![
        "1. Manage Data Sources",
        "2. Run Analysis",
        "3. View Results",
        "4. Reconcile Data",
        "5. Export Results",
        "",
        "?. Help",
        "q. Quit",
    ];

    let items: Vec<ListItem> = menu_items
        .iter()
        .map(|item| ListItem::new(Line::from(vec![Span::raw(format!("  {}", item))])))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Main Menu ")
                .style(Style::default().fg(Color::White)),
        )
        .style(Style::default().fg(Color::White));

    let centered_area = center_rect(50, 50, area);
    f.render_widget(list, centered_area);
}

fn draw_source_list(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Configured Sources
            Constraint::Length(8), // Add New Source
        ])
        .split(area);

    // 1. Configured Sources List
    if app.sources.is_empty() {
        let message =
            Paragraph::new("No sources configured.\n\nPress a number below to add a source.")
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Configured Sources "),
                );
        f.render_widget(message, chunks[0]);
    } else {
        let items: Vec<ListItem> = app
            .sources
            .iter()
            .enumerate()
            .map(|(i, source)| {
                let content = match source {
                    crate::config::SourceConfig::Mssql { name, .. } => {
                        format!(
                            "MSSQL: {}",
                            name.as_ref().unwrap_or(&"Database".to_string())
                        )
                    }
                    crate::config::SourceConfig::Csv { path, .. } => format!("CSV: {}", path),
                    crate::config::SourceConfig::Excel { path, .. } => format!("Excel: {}", path),
                    crate::config::SourceConfig::Flatfile { path, .. } => {
                        format!("Flatfile: {}", path)
                    }
                };

                let style = if i == app.selected_index {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(vec![Span::styled(
                    format!("  {}", content),
                    style,
                )]))
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Configured Sources ")
                .title_bottom(" ↑↓: Navigate | d: Delete | Esc: Back "),
        );
        f.render_widget(list, chunks[0]);
    }

    // 2. Add Source Section
    match &app.state {
        AppState::AddingSource {
            source_type,
            input_text,
        } => {
            let add_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(0),
                ])
                .split(chunks[1]);

            // Prompt
            let prompt_text = match source_type.as_ref().unwrap() {
                crate::tui::app::SourceType::Csv => "Enter CSV file path:",
                crate::tui::app::SourceType::Excel => "Enter Excel file path:",
                crate::tui::app::SourceType::Mssql => "Enter MSSQL connection string:",
                crate::tui::app::SourceType::Flatfile => "Enter flat file path:",
            };

            let prompt = Paragraph::new(prompt_text)
                .style(Style::default().fg(Color::Cyan))
                .block(Block::default().borders(Borders::ALL).title(" Add Source "));
            f.render_widget(prompt, add_chunks[0]);

            // Input box
            let input_display = format!("{}_", input_text);
            let input_widget = Paragraph::new(input_display)
                .style(Style::default().fg(Color::White))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(input_widget, add_chunks[1]);

            // Help text
            let help = Paragraph::new("Press Enter to add, Esc to cancel")
                .style(Style::default().fg(Color::Gray));
            f.render_widget(help, add_chunks[2]);
        }
        _ => {
            let menu_items = vec![
                "1. Add CSV File",
                "2. Add Excel File",
                "3. Add MSSQL Database",
                "",
                "Esc: Back to Main Menu",
            ];

            let items: Vec<ListItem> = menu_items
                .iter()
                .map(|item| ListItem::new(Line::from(vec![Span::raw(format!("  {}", item))])))
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" Add Source "))
                .style(Style::default().fg(Color::White));

            f.render_widget(list, chunks[1]);
        }
    }
}

fn draw_analysis(f: &mut Frame, area: Rect, app: &App) {
    let text = if matches!(app.state, AppState::Analyzing) {
        "Running analysis...\n\nPlease wait..."
    } else {
        "Analysis complete!\n\nPress 3 to view results."
    };

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" Analysis "));

    let centered_area = center_rect(50, 30, area);
    f.render_widget(paragraph, centered_area);
}

fn draw_results(f: &mut Frame, area: Rect, app: &App) {
    if let Some(result) = &app.analysis_result {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Summary table
                Constraint::Min(0),    // Detailed tables
            ])
            .split(area);

        // 1. Summary Table
        let summary_header = Row::new(vec!["Metric", "Count"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let tables_count = result.tables.len().to_string();
        let relationships_count = result.relationships.len().to_string();
        let workflows_count = result.workflows.len().to_string();
        let data_profiles_count = result.data_profiles.len().to_string();
        let grouping_analysis_count = result.grouping_analyses.len().to_string();
        let reconciliation_count = result.reconciliation_results.len().to_string();

        let summary_rows = vec![
            Row::new(vec!["Tables", &tables_count]),
            Row::new(vec!["Relationships", &relationships_count]),
            Row::new(vec!["Workflows", &workflows_count]),
            Row::new(vec!["Data Profiles", &data_profiles_count]),
            Row::new(vec!["Grouping Analyses", &grouping_analysis_count]),
            Row::new(vec!["Reconciliations", &reconciliation_count]),
        ];

        let summary_table = Table::new(
            summary_rows,
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .header(summary_header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Analysis Summary "),
        );

        f.render_widget(summary_table, chunks[0]);

        // 2. Detailed results rendered in a table layout
        let mut detail_entries: Vec<(String, String)> = Vec::new();
        {
            let mut push_entry = |section: &str, detail: String| {
                detail_entries.push((section.to_string(), detail));
            };

            if result.tables.is_empty() {
                push_entry("Tables", "No tables discovered".to_string());
            } else {
                for table in &result.tables {
                    let rows = table
                        .row_count
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    push_entry(
                        "Tables",
                        format!(
                            "{} • {} cols • {} rows • {} @ {}",
                            table.full_name,
                            table.columns.len(),
                            rows,
                            table.source_type,
                            table.source_location
                        ),
                    );
                }
            }

            if result.relationships.is_empty() {
                push_entry("Relationships", "No relationships detected".to_string());
            } else {
                for rel in &result.relationships {
                    push_entry(
                        "Relationships",
                        format!(
                            "{}.{} → {}.{} ({:?})",
                            rel.from_table,
                            rel.from_column,
                            rel.to_table,
                            rel.to_column,
                            rel.relationship_type
                        ),
                    );
                }
            }

            if result.workflows.is_empty() {
                push_entry("Workflows", "No workflows identified".to_string());
            } else {
                for workflow in &result.workflows {
                    push_entry(
                        "Workflows",
                        format!(
                            "{:?} – {} ({:.1}% confidence)",
                            workflow.workflow_type, workflow.description, workflow.confidence
                        ),
                    );
                }
            }

            if result.data_profiles.is_empty() {
                push_entry("Profiling", "No data profiles available".to_string());
            } else {
                for profile in &result.data_profiles {
                    let table_label = if profile.table_name.is_empty() {
                        "table".to_string()
                    } else {
                        profile.table_name.clone()
                    };
                    for col_profile in &profile.column_profiles {
                        let pattern_list = if col_profile.data_patterns.is_empty() {
                            "patterns: none".to_string()
                        } else {
                            format!(
                                "patterns: {}",
                                col_profile
                                    .data_patterns
                                    .iter()
                                    .map(|p| format!("{:?}", p))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        };
                        push_entry(
                            "Profiling",
                            format!(
                                "{}.{} • {} • unique {}/{} • nulls {}",
                                table_label,
                                col_profile.column_name,
                                pattern_list,
                                col_profile.unique_values,
                                col_profile.total_values,
                                col_profile.null_count
                            ),
                        );
                    }
                    for corr in &profile.correlations {
                        push_entry(
                            "Correlations",
                            format!(
                                "{} ↔ {} ({:?}, strength {:.2}) – {}",
                                corr.column_a,
                                corr.column_b,
                                corr.correlation_type,
                                corr.strength,
                                corr.description
                            ),
                        );
                    }
                }
            }

            if result.reconciliation_results.is_empty() {
                push_entry("Reconciliation", "No reconciliation results".to_string());
            } else {
                for recon in &result.reconciliation_results {
                    push_entry(
                        "Reconciliation",
                        format!(
                            "{} vs {} • {:.1}% match • {} matches",
                            recon.source1_name,
                            recon.source2_name,
                            recon.match_percentage,
                            recon.matches
                        ),
                    );
                    push_entry(
                        "Reconciliation",
                        format!(
                            "Only {}: {} • Only {}: {} • Dups {} / {}",
                            recon.source1_name,
                            recon.only_in_source1,
                            recon.source2_name,
                            recon.only_in_source2,
                            recon.duplicates_source1,
                            recon.duplicates_source2
                        ),
                    );
                    if !recon.field_mismatches.is_empty() {
                        let mismatch_summary = recon
                            .field_mismatches
                            .iter()
                            .take(2)
                            .map(|mismatch| {
                                format!(
                                    "{} [{}]: '{}' vs '{}'",
                                    mismatch.key_value,
                                    mismatch.column_name,
                                    mismatch.source1_value,
                                    mismatch.source2_value
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(" | ");
                        push_entry(
                            "Reconciliation",
                            format!("Field mismatches (sample): {}", mismatch_summary),
                        );
                    }
                }
            }

            if result.multi_value_analyses.is_empty() {
                push_entry("Multi-Value", "No multi-value columns detected".to_string());
            } else {
                for mv_analysis in &result.multi_value_analyses {
                    for col in &mv_analysis.multi_value_columns {
                        let method_str = match &col.detection_method {
                            DetectionMethod::Delimited(d) => format!("Delimited({})", d),
                            DetectionMethod::VocabularySegmented => "VocabSeg".to_string(),
                            DetectionMethod::PatternRepetition => "PatternRep".to_string(),
                            DetectionMethod::LengthOutlier => "LenOutlier".to_string(),
                        };
                        push_entry(
                            "Multi-Value",
                            format!(
                                "{}.{} [{}] • {:.0}% conf • {}/{} cells",
                                col.table_name,
                                col.column_name,
                                method_str,
                                col.confidence * 100.0,
                                col.multi_value_cell_count,
                                col.total_cell_count
                            ),
                        );
                        if let (Some(raw), Some(parts)) =
                            (col.example_raw.first(), col.example_split.first())
                        {
                            push_entry(
                                "  -> Split",
                                format!("\"{}\" -> [{}]", raw, parts.join(" | ")),
                            );
                        }
                    }
                }
            }

            if result.grouping_analyses.is_empty() {
                push_entry("Grouping", "No grouping analyses".to_string());
            } else {
                for analysis in &result.grouping_analyses {
                    let prefix = if analysis.table_name.is_empty() {
                        String::new()
                    } else {
                        format!("{}: ", analysis.table_name)
                    };

                    if analysis.grouping_dimensions.is_empty() {
                        push_entry(
                            "Grouping",
                            format!("{}No suitable grouping dimensions", prefix),
                        );
                    } else {
                        for dim in &analysis.grouping_dimensions {
                            let mut detail = format!(
                                "{}{} ({:?}) • {} groups avg {:.1} records/group",
                                prefix,
                                dim.column_name,
                                dim.dimension_type,
                                dim.group_count,
                                dim.records_per_group.avg
                            );
                            if !dim.example_groups.is_empty() {
                                let examples = dim
                                    .example_groups
                                    .iter()
                                    .take(2)
                                    .map(|example| {
                                        format!(
                                            "{} ({} records)",
                                            example.group_value, example.record_count
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                detail.push_str(&format!(" • Examples: {}", examples));
                            }
                            if !dim.insights.is_empty() {
                                detail
                                    .push_str(&format!(" • Insights: {}", dim.insights.join("; ")));
                            }
                            push_entry("Grouping", detail);
                        }
                    }

                    if !analysis.hierarchies.is_empty() {
                        for hierarchy in &analysis.hierarchies {
                            let levels_str = hierarchy.levels.join(" → ");
                            push_entry(
                                "Hierarchy",
                                format!(
                                    "{}{} ({:?}) • {}",
                                    prefix,
                                    levels_str,
                                    hierarchy.hierarchy_type,
                                    hierarchy.description
                                ),
                            );
                        }
                    }

                    if !analysis.suggested_analyses.is_empty() {
                        for suggestion in analysis.suggested_analyses.iter().take(3) {
                            push_entry("Suggestions", format!("{}{}", prefix, suggestion));
                        }
                    }
                }
            }
        }

        if detail_entries.is_empty() {
            detail_entries.push((
                "Details".to_string(),
                "No detailed analysis entries".to_string(),
            ));
        }

        let detail_height = area.height.saturating_sub(3);
        let visible_rows = detail_height as usize;
        let total_rows = detail_entries.len();
        let max_scroll = if visible_rows == 0 {
            0
        } else {
            total_rows.saturating_sub(visible_rows)
        };
        let scroll_offset = if max_scroll == 0 {
            0
        } else {
            app.scroll_offset.min(max_scroll)
        };

        let header_row = Row::new(vec![
            Cell::from(Span::styled(
                "Section",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Cell::from(Span::styled(
                "Details",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
        ]);

        let table_rows = detail_entries
            .iter()
            .skip(scroll_offset)
            .take(visible_rows)
            .map(|(section, detail)| {
                Row::new(vec![
                    Cell::from(section.as_str()),
                    Cell::from(detail.as_str()),
                ])
            })
            .collect::<Vec<_>>();

        let detail_table = Table::new(table_rows, [Constraint::Length(20), Constraint::Min(10)])
            .header(header_row)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Detailed Results "),
            )
            .column_spacing(1);

        f.render_widget(detail_table, chunks[1]);
    } else {
        let message = Paragraph::new("No analysis results available.\n\nRun analysis first.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(" Results "));
        let centered_area = center_rect(50, 30, area);
        f.render_widget(message, centered_area);
    }
}

fn draw_reconcile(f: &mut Frame, area: Rect, app: &App) {
    if app.sources.is_empty() {
        let message = Paragraph::new("No sources configured.\n\nPress Esc to return to main menu.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(" Reconcile "));

        let centered_area = center_rect(50, 30, area);
        f.render_widget(message, centered_area);
        return;
    }

    if app.sources.len() < 2 {
        let message = Paragraph::new(
            "Need at least 2 sources to reconcile.\n\nPress Esc to return to main menu.",
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" Reconcile "));

        let centered_area = center_rect(50, 30, area);
        f.render_widget(message, centered_area);
        return;
    }

    let (source1_idx, source2_idx) = if let AppState::Reconciling {
        source1_idx,
        source2_idx,
    } = &app.state
    {
        (*source1_idx, *source2_idx)
    } else {
        (None, None)
    };

    let items: Vec<ListItem> = app
        .sources
        .iter()
        .enumerate()
        .map(|(i, source)| {
            let content = match source {
                crate::config::SourceConfig::Mssql { name, .. } => {
                    format!(
                        "MSSQL: {}",
                        name.as_ref().unwrap_or(&"Database".to_string())
                    )
                }
                crate::config::SourceConfig::Csv { path, .. } => format!("CSV: {}", path),
                crate::config::SourceConfig::Excel { path, .. } => format!("Excel: {}", path),
                crate::config::SourceConfig::Flatfile { path, .. } => {
                    format!("Flatfile: {}", path)
                }
            };

            let mut style = Style::default();
            let mut prefix = "  ";

            if Some(i) == source1_idx {
                prefix = "1️⃣ ";
                style = style.fg(Color::Green).add_modifier(Modifier::BOLD);
            } else if Some(i) == source2_idx {
                prefix = "2️⃣ ";
                style = style.fg(Color::Blue).add_modifier(Modifier::BOLD);
            } else if i == app.selected_index {
                style = style.fg(Color::Yellow).add_modifier(Modifier::BOLD);
            }

            ListItem::new(Line::from(vec![Span::styled(
                format!("{}{}", prefix, content),
                style,
            )]))
        })
        .collect();

    let title = if source1_idx.is_none() {
        " Reconcile - Select Source 1 "
    } else if source2_idx.is_none() {
        " Reconcile - Select Source 2 "
    } else {
        " Reconcile - Running... "
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_bottom(" ↑↓: Navigate | Enter: Select | r: Reset | Esc: Cancel "),
    );

    f.render_widget(list, area);
}

fn draw_export(f: &mut Frame, area: Rect, app: &App) {
    if app.analysis_result.is_none() {
        let message =
            Paragraph::new("No analysis results to export.\n\nRun analysis first (option 2).")
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).title(" Export "));

        let centered_area = center_rect(50, 30, area);
        f.render_widget(message, centered_area);
        return;
    }

    match &app.state {
        AppState::Exporting { format, filename } => {
            let list_height = if format.is_none() { 6 } else { 3 };
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(list_height),
                    Constraint::Min(0),
                ])
                .split(area);

            let prompt_text = if format.is_none() {
                "Select export format:"
            } else {
                "Enter filename:"
            };

            let prompt = Paragraph::new(prompt_text)
                .style(Style::default().fg(Color::Cyan))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(prompt, chunks[0]);

            if format.is_none() {
                // Show format selection
                let format_items = vec![
                    "1. JSON (.json)",
                    "2. Markdown (.md)",
                    "3. Excel (.xlsx)",
                    "4. Grouped Data Excel (.xlsx)",
                ];

                let items: Vec<ListItem> = format_items
                    .iter()
                    .map(|item| ListItem::new(Line::from(vec![Span::raw(format!("  {}", item))])))
                    .collect();

                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(" Formats "))
                    .style(Style::default().fg(Color::White));

                f.render_widget(list, chunks[1]);

                let help = Paragraph::new("Press 1-4 to select format, Esc to cancel")
                    .style(Style::default().fg(Color::Gray));
                f.render_widget(help, chunks[2]);
            } else {
                // Show filename input
                let input_display = format!("{}_", filename);
                let input_widget = Paragraph::new(input_display)
                    .style(Style::default().fg(Color::White))
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(input_widget, chunks[1]);

                let help = Paragraph::new(
                    "Enter filename (without extension), then press Enter. Esc to go back.",
                )
                .style(Style::default().fg(Color::Gray));
                f.render_widget(help, chunks[2]);
            }
        }
        _ => {
            // Should not reach here
        }
    }
}

fn draw_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(vec![Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Global:"),
        Line::from("  q           - Quit application"),
        Line::from("  Ctrl+C      - Quit application"),
        Line::from("  Esc         - Go back / Cancel"),
        Line::from("  ?           - Show help"),
        Line::from(""),
        Line::from("Main Menu:"),
        Line::from("  1           - Manage data sources"),
        Line::from("  2           - Run analysis"),
        Line::from("  3           - View results"),
        Line::from("  4           - Reconcile data"),
        Line::from("  5           - Export results"),
        Line::from(""),
        Line::from("Source List:"),
        Line::from("  ↑/k         - Move up"),
        Line::from("  ↓/j         - Move down"),
        Line::from("  d           - Delete selected source"),
        Line::from(""),
        Line::from("Results View:"),
        Line::from("  ↑/k         - Scroll up"),
        Line::from("  ↓/j         - Scroll down"),
        Line::from("  PgUp/PgDn   - Page up/down"),
        Line::from("  Home        - Go to top"),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title(" Help "))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let status = Paragraph::new(app.status_message.as_str())
        .style(Style::default().fg(Color::White).bg(Color::DarkGray))
        .alignment(Alignment::Left)
        .block(Block::default());

    f.render_widget(status, area);
}

fn draw_file_browser(f: &mut Frame, area: Rect, browser: &FileBrowser) {
    let items: Vec<ListItem> = browser
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let icon = if entry.is_dir { "📁" } else { "📄" };

            let size_str = if let Some(size) = entry.size {
                format!(" ({})", FileBrowser::format_size(size))
            } else {
                String::new()
            };

            let content = format!("{} {}{}", icon, entry.name, size_str);

            let style = if i == browser.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if entry.is_dir {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![Span::styled(
                format!("  {}", content),
                style,
            )]))
        })
        .collect();

    let current_path = browser.current_dir.to_string_lossy().to_string();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Select File - {} ", current_path))
            .title_bottom(
                " ↑↓/jk: Navigate | Enter/→: Select | ←/Backspace: Parent | Esc: Cancel ",
            ),
    );

    f.render_widget(list, area);
}

fn center_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
