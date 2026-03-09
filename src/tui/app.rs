use crate::analysis::{
    DataProfiler, GroupingAnalyzer, MultiValueDetector, ReconciliationConfig, Reconciliator,
    RelationshipDetector, WorkflowDetector,
};
use crate::config::SourceConfig;
use crate::export::{
    AnalysisResult, ExcelExporter, ExportFormat, Exporter, JsonExporter, MarkdownExporter,
};
use crate::sources::{CsvSource, DataSource, ExcelSource, MssqlSource};
use crate::tui::events::{is_exit_key, AppEvent};
use crate::tui::file_browser::{FileBrowser, FileFilter};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Home,
    SourceList,
    Analysis,
    Results,
    Reconcile,
    Export,
    Help,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceType {
    Mssql,
    Csv,
    Excel,
    Flatfile,
}

pub enum AppState {
    Normal,
    AddingSource {
        source_type: Option<SourceType>,
        input_text: String,
    },
    BrowsingFiles {
        source_type: SourceType,
        browser: FileBrowser,
    },
    Analyzing,
    ViewingResults,
    Reconciling {
        source1_idx: Option<usize>,
        source2_idx: Option<usize>,
    },
    Exporting {
        format: Option<crate::export::ExportFormat>,
        filename: String,
    },
}

pub struct App {
    pub screen: Screen,
    pub state: AppState,
    pub sources: Vec<SourceConfig>,
    pub selected_index: usize,
    pub analysis_result: Option<AnalysisResult>,
    pub status_message: String,
    pub scroll_offset: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Home,
            state: AppState::Normal,
            sources: Vec::new(),
            selected_index: 0,
            analysis_result: None,
            status_message: String::from("Ready"),
            scroll_offset: 0,
        }
    }

    pub async fn handle_event(&mut self, event: AppEvent) -> Result<bool> {
        match event {
            AppEvent::Key(key) => {
                // Global exit keys
                if is_exit_key(&key) {
                    return Ok(false); // Exit app
                }

                match &mut self.state {
                    AppState::Normal => self.handle_normal_mode(key).await?,
                    AppState::AddingSource { .. } => self.handle_adding_source(key).await?,
                    AppState::BrowsingFiles { .. } => self.handle_file_browser(key).await?,
                    AppState::Analyzing => {
                        // Can't interact during analysis
                    }
                    AppState::ViewingResults => self.handle_results_view_key(key)?,
                    AppState::Reconciling { .. } => self.handle_reconciling(key).await?,
                    AppState::Exporting { .. } => self.handle_exporting(key).await?,
                }
            }
            AppEvent::Mouse(mouse) => {
                // Handle mouse events only in results view
                if matches!(self.state, AppState::ViewingResults) {
                    self.handle_results_view_mouse(mouse)?;
                }
            }
        }

        Ok(true)
    }

    async fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        match self.screen {
            Screen::Home => match key.code {
                KeyCode::Char('1') => self.screen = Screen::SourceList,
                KeyCode::Char('2') => {
                    if !self.sources.is_empty() {
                        self.run_analysis().await?;
                    } else {
                        self.status_message =
                            "No sources configured. Add a source first.".to_string();
                    }
                }
                KeyCode::Char('3') => {
                    if self.analysis_result.is_some() {
                        self.screen = Screen::Results;
                        self.state = AppState::ViewingResults;
                    } else {
                        self.status_message = "No results yet. Run analysis first.".to_string();
                    }
                }
                KeyCode::Char('4') => {
                    if self.sources.len() >= 2 {
                        self.screen = Screen::Reconcile;
                        self.state = AppState::Reconciling {
                            source1_idx: None,
                            source2_idx: None,
                        };
                        self.selected_index = 0;
                    } else {
                        self.status_message = "Need at least 2 sources to reconcile.".to_string();
                    }
                }
                KeyCode::Char('5') => {
                    if self.analysis_result.is_some() {
                        self.screen = Screen::Export;
                        self.state = AppState::Exporting {
                            format: None,
                            filename: String::from("analysis_results"),
                        };
                    } else {
                        self.status_message =
                            "No results to export. Run analysis first.".to_string();
                    }
                }
                KeyCode::Char('?') | KeyCode::F(1) => self.screen = Screen::Help,
                _ => {}
            },
            Screen::SourceList => match key.code {
                KeyCode::Char('1') => self.start_file_browser(SourceType::Csv),
                KeyCode::Char('2') => self.start_file_browser(SourceType::Excel),
                KeyCode::Char('3') => self.start_adding_source(SourceType::Mssql),
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.selected_index < self.sources.len().saturating_sub(1) {
                        self.selected_index += 1;
                    }
                }
                KeyCode::Delete | KeyCode::Char('d') => {
                    if !self.sources.is_empty() && self.selected_index < self.sources.len() {
                        self.sources.remove(self.selected_index);
                        if self.selected_index >= self.sources.len() && self.selected_index > 0 {
                            self.selected_index -= 1;
                        }
                        self.status_message = "Source deleted".to_string();
                    }
                }
                KeyCode::Esc => self.screen = Screen::Home,
                _ => {}
            },
            Screen::Help => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    self.screen = Screen::Home;
                }
            }
            _ => {
                if key.code == KeyCode::Esc {
                    self.screen = Screen::Home;
                }
            }
        }

        Ok(())
    }

    async fn handle_adding_source(&mut self, key: KeyEvent) -> Result<()> {
        if let AppState::AddingSource {
            source_type,
            input_text,
        } = &mut self.state
        {
            match key.code {
                KeyCode::Esc => {
                    self.state = AppState::Normal;
                    self.screen = Screen::SourceList;
                }
                KeyCode::Enter => {
                    if !input_text.trim().is_empty() {
                        // Create source config based on type
                        let source = match source_type {
                            Some(SourceType::Csv) => SourceConfig::Csv {
                                path: input_text.clone(),
                                delimiter: Some(','),
                                has_header: Some(true),
                            },
                            Some(SourceType::Excel) => SourceConfig::Excel {
                                path: input_text.clone(),
                                sheet: None,
                                has_header: Some(true),
                            },
                            Some(SourceType::Mssql) => SourceConfig::Mssql {
                                connection_string: input_text.clone(),
                                name: None,
                            },
                            _ => return Ok(()),
                        };

                        self.sources.push(source);
                        self.status_message = "Source added successfully".to_string();
                        self.state = AppState::Normal;
                        self.screen = Screen::SourceList;
                    }
                }
                KeyCode::Backspace => {
                    input_text.pop();
                }
                KeyCode::Char(c) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        // Handle ctrl+c exit handled globally
                    } else {
                        input_text.push(c);
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_results_view_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_offset += 1;
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.scroll_offset += 10;
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
            }
            KeyCode::Esc => {
                self.state = AppState::Normal;
                self.screen = Screen::Home;
                self.scroll_offset = 0;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_results_view_mouse(&mut self, mouse: crossterm::event::MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                if self.scroll_offset > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
            }
            MouseEventKind::ScrollDown => {
                self.scroll_offset += 3;
            }
            _ => {}
        }

        Ok(())
    }

    fn start_adding_source(&mut self, source_type: SourceType) {
        self.state = AppState::AddingSource {
            source_type: Some(source_type),
            input_text: String::new(),
        };
    }

    fn start_file_browser(&mut self, source_type: SourceType) {
        let filter = match source_type {
            SourceType::Csv => FileFilter::Csv,
            SourceType::Excel => FileFilter::Excel,
            _ => FileFilter::All,
        };

        self.state = AppState::BrowsingFiles {
            source_type,
            browser: FileBrowser::new(filter),
        };
    }

    async fn handle_file_browser(&mut self, key: KeyEvent) -> Result<()> {
        if let AppState::BrowsingFiles {
            source_type,
            browser,
        } = &mut self.state
        {
            match key.code {
                KeyCode::Esc => {
                    self.state = AppState::Normal;
                    self.screen = Screen::SourceList;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    browser.move_up();
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    browser.move_down();
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
                    browser.go_parent();
                }
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                    if let Some(path) = browser.enter_selected() {
                        // File selected
                        let path_str = path.to_string_lossy().to_string();

                        let source = match source_type {
                            SourceType::Csv => SourceConfig::Csv {
                                path: path_str,
                                delimiter: Some(','),
                                has_header: Some(true),
                            },
                            SourceType::Excel => SourceConfig::Excel {
                                path: path_str,
                                sheet: None,
                                has_header: Some(true),
                            },
                            _ => return Ok(()),
                        };

                        self.sources.push(source);
                        self.status_message = "Source added successfully".to_string();
                        self.state = AppState::Normal;
                        self.screen = Screen::SourceList;
                    }
                    // If directory, browser.enter_selected() already navigated into it
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn run_analysis(&mut self) -> Result<()> {
        self.state = AppState::Analyzing;
        self.status_message = "Running analysis...".to_string();

        // Extract schemas and data
        let mut all_tables = Vec::new();
        let mut all_data_profiles = Vec::new();
        let mut all_grouping_analyses = Vec::new();
        let mut loaded_sources: Vec<(String, Vec<Vec<String>>, Vec<crate::schema::Column>)> =
            Vec::new();

        for source_config in &self.sources {
            let tables = match source_config {
                SourceConfig::Mssql {
                    connection_string, ..
                } => {
                    let mut source = MssqlSource::new(connection_string.clone());
                    match source.extract_schema().await {
                        Ok(tables) => tables,
                        Err(e) => {
                            self.status_message = format!("Error connecting to MSSQL: {}", e);
                            self.state = AppState::Normal;
                            return Ok(());
                        }
                    }
                }
                SourceConfig::Csv {
                    path,
                    delimiter,
                    has_header,
                } => {
                    let mut source = CsvSource::new(path.clone());
                    if let Some(delim) = delimiter {
                        source = source.with_delimiter(*delim);
                    }
                    if let Some(header) = has_header {
                        source = source.with_header(*header);
                    }
                    match source.extract_schema().await {
                        Ok(tables) => {
                            // Also get the data for profiling
                            match source.read_data().await {
                                Ok(data) => {
                                    if !tables.is_empty() && !data.is_empty() {
                                        let table = &tables[0];

                                        // Store for reconciliation
                                        let name =
                                            path.split('/').last().unwrap_or(path).to_string();
                                        loaded_sources.push((
                                            name,
                                            data.clone(),
                                            table.columns.clone(),
                                        ));

                                        // Run data profiling
                                        let profiler = DataProfiler::new(10000);
                                        let profile = profiler.profile_data(&data, &table.columns);
                                        all_data_profiles.push(profile);

                                        // Run grouping analysis
                                        let analyzer = GroupingAnalyzer::new(1000);
                                        let grouping =
                                            analyzer.analyze_groupings(&data, &table.columns);
                                        let dim_count = grouping.grouping_dimensions.len();
                                        all_grouping_analyses.push(grouping);

                                        self.status_message = format!(
                                            "Analyzed {} rows, found {} grouping dimensions",
                                            data.len(),
                                            dim_count
                                        );
                                    } else if data.is_empty() {
                                        self.status_message =
                                            "Warning: File has no data rows".to_string();
                                    }
                                }
                                Err(e) => {
                                    self.status_message = format!("Error reading data: {}", e);
                                }
                            }
                            tables
                        }
                        Err(e) => {
                            self.status_message = format!("Error reading CSV: {}", e);
                            self.state = AppState::Normal;
                            return Ok(());
                        }
                    }
                }
                SourceConfig::Excel {
                    path,
                    sheet,
                    has_header,
                } => {
                    let mut source = ExcelSource::new(path.clone());
                    if let Some(sheet_name) = sheet {
                        source = source.with_sheet(sheet_name.clone());
                    }
                    if let Some(header) = has_header {
                        source = source.with_header(*header);
                    }
                    match source.extract_schema().await {
                        Ok(tables) => {
                            // Also get the data for profiling
                            match source.read_data().await {
                                Ok(data) => {
                                    if !tables.is_empty() && !data.is_empty() {
                                        let table = &tables[0];

                                        // Store for reconciliation
                                        let name =
                                            path.split('/').last().unwrap_or(path).to_string();
                                        loaded_sources.push((
                                            name,
                                            data.clone(),
                                            table.columns.clone(),
                                        ));

                                        // Run data profiling
                                        let profiler = DataProfiler::new(10000);
                                        let profile = profiler.profile_data(&data, &table.columns);
                                        all_data_profiles.push(profile);

                                        // Run grouping analysis
                                        let analyzer = GroupingAnalyzer::new(1000);
                                        let grouping =
                                            analyzer.analyze_groupings(&data, &table.columns);
                                        let dim_count = grouping.grouping_dimensions.len();
                                        all_grouping_analyses.push(grouping);

                                        self.status_message = format!(
                                            "Analyzed {} rows, found {} grouping dimensions",
                                            data.len(),
                                            dim_count
                                        );
                                    } else if data.is_empty() {
                                        self.status_message =
                                            "Warning: File has no data rows".to_string();
                                    }
                                }
                                Err(e) => {
                                    self.status_message = format!("Error reading data: {}", e);
                                }
                            }
                            tables
                        }
                        Err(e) => {
                            self.status_message = format!("Error reading Excel: {}", e);
                            self.state = AppState::Normal;
                            return Ok(());
                        }
                    }
                }
                _ => continue,
            };

            all_tables.extend(tables);
        }

        // Detect relationships
        let mut relationship_detector = RelationshipDetector::new(all_tables.clone());
        let relationships = relationship_detector.detect_relationships();

        // Detect workflows
        let mut workflow_detector =
            WorkflowDetector::new(all_tables.clone(), relationships.clone());
        let workflows = workflow_detector.detect_workflows();

        // Auto-reconcile all pairs of loaded sources
        let mut reconciliation_results = Vec::new();
        if loaded_sources.len() >= 2 {
            let config = ReconciliationConfig::default();
            let reconciliator = Reconciliator::new(config);

            for i in 0..loaded_sources.len() {
                for j in (i + 1)..loaded_sources.len() {
                    let (name1, data1, cols1) = &loaded_sources[i];
                    let (name2, data2, cols2) = &loaded_sources[j];

                    let result = reconciliator.reconcile(name1, data1, cols1, name2, data2, cols2);
                    reconciliation_results.push(result);
                }
            }

            if !reconciliation_results.is_empty() {
                self.status_message = format!(
                    "Analysis complete! {} reconciliations performed.",
                    reconciliation_results.len()
                );
            }
        }

        // Multi-value detection
        let mv_detector = MultiValueDetector::new(5000);
        let all_multi_value_analyses = mv_detector.analyze_all_sources(&loaded_sources);

        // Store results
        self.analysis_result = Some(AnalysisResult {
            tables: all_tables,
            relationships,
            workflows,
            data_profiles: all_data_profiles,
            grouping_analyses: all_grouping_analyses,
            reconciliation_results,
            multi_value_analyses: all_multi_value_analyses,
            source_data: loaded_sources,
        });

        self.status_message = "Analysis complete!".to_string();
        self.state = AppState::Normal;
        self.screen = Screen::Results;
        self.state = AppState::ViewingResults;

        Ok(())
    }

    async fn handle_reconciling(&mut self, key: KeyEvent) -> Result<()> {
        let (source1_idx, source2_idx) = if let AppState::Reconciling {
            source1_idx,
            source2_idx,
        } = &self.state
        {
            (*source1_idx, *source2_idx)
        } else {
            return Ok(());
        };

        match key.code {
            KeyCode::Esc => {
                self.state = AppState::Normal;
                self.screen = Screen::Home;
                self.selected_index = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index < self.sources.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if source1_idx.is_none() {
                    self.state = AppState::Reconciling {
                        source1_idx: Some(self.selected_index),
                        source2_idx: None,
                    };
                    self.status_message = format!("Source 1 selected. Select source 2...");
                } else if source2_idx.is_none() && Some(self.selected_index) != source1_idx {
                    self.state = AppState::Reconciling {
                        source1_idx,
                        source2_idx: Some(self.selected_index),
                    };
                    // Run reconciliation
                    self.run_reconciliation(source1_idx, Some(self.selected_index))
                        .await?;
                } else if Some(self.selected_index) == source1_idx {
                    self.status_message = "Cannot reconcile a source with itself.".to_string();
                }
            }
            KeyCode::Char('r') => {
                // Reset selection
                self.state = AppState::Reconciling {
                    source1_idx: None,
                    source2_idx: None,
                };
                self.selected_index = 0;
                self.status_message = "Selection reset. Select source 1...".to_string();
            }
            _ => {}
        }
        Ok(())
    }

    async fn run_reconciliation(
        &mut self,
        source1_idx: Option<usize>,
        source2_idx: Option<usize>,
    ) -> Result<()> {
        let (idx1, idx2) = match (source1_idx, source2_idx) {
            (Some(i1), Some(i2)) => (i1, i2),
            _ => {
                self.status_message = "Both sources must be selected".to_string();
                return Ok(());
            }
        };

        if idx1 >= self.sources.len() || idx2 >= self.sources.len() {
            self.status_message = "Invalid source indices".to_string();
            return Ok(());
        }

        self.status_message = "Running reconciliation...".to_string();

        // Load data from both sources
        let (data1, columns1, name1) = match self.load_source_data(idx1).await {
            Ok(result) => result,
            Err(e) => {
                self.status_message = format!("Error loading source 1: {}", e);
                return Ok(());
            }
        };

        let (data2, columns2, name2) = match self.load_source_data(idx2).await {
            Ok(result) => result,
            Err(e) => {
                self.status_message = format!("Error loading source 2: {}", e);
                return Ok(());
            }
        };

        // Run reconciliation
        let config = ReconciliationConfig::default();
        let reconciliator = Reconciliator::new(config);
        let result = reconciliator.reconcile(&name1, &data1, &columns1, &name2, &data2, &columns2);

        // Store result
        if let Some(analysis_result) = &mut self.analysis_result {
            analysis_result.reconciliation_results.push(result.clone());
        } else {
            self.analysis_result = Some(AnalysisResult {
                tables: Vec::new(),
                relationships: Vec::new(),
                workflows: Vec::new(),
                data_profiles: Vec::new(),
                grouping_analyses: Vec::new(),
                reconciliation_results: vec![result.clone()],
                multi_value_analyses: Vec::new(),
                source_data: Vec::new(),
            });
        }

        self.status_message = format!("Reconciliation complete: {}", result.summary);
        self.state = AppState::Normal;
        self.screen = Screen::Results;
        self.state = AppState::ViewingResults;

        Ok(())
    }

    async fn load_source_data(
        &self,
        idx: usize,
    ) -> Result<(Vec<Vec<String>>, Vec<crate::schema::Column>, String)> {
        let source_config = &self.sources[idx];

        match source_config {
            SourceConfig::Csv {
                path,
                delimiter,
                has_header,
            } => {
                let mut source = CsvSource::new(path.clone());
                if let Some(delim) = delimiter {
                    source = source.with_delimiter(*delim);
                }
                if let Some(header) = has_header {
                    source = source.with_header(*header);
                }
                let tables = source.extract_schema().await?;
                let data = source.read_data().await?;
                let name = path.split('/').last().unwrap_or(path).to_string();
                Ok((
                    data,
                    tables
                        .first()
                        .map(|t| t.columns.clone())
                        .unwrap_or_default(),
                    name,
                ))
            }
            SourceConfig::Excel {
                path,
                sheet,
                has_header,
            } => {
                let mut source = ExcelSource::new(path.clone());
                if let Some(sheet_name) = sheet {
                    source = source.with_sheet(sheet_name.clone());
                }
                if let Some(header) = has_header {
                    source = source.with_header(*header);
                }
                let tables = source.extract_schema().await?;
                let data = source.read_data().await?;
                let name = path.split('/').last().unwrap_or(path).to_string();
                Ok((
                    data,
                    tables
                        .first()
                        .map(|t| t.columns.clone())
                        .unwrap_or_default(),
                    name,
                ))
            }
            _ => {
                anyhow::bail!("Reconciliation only supported for CSV and Excel sources")
            }
        }
    }

    async fn handle_exporting(&mut self, key: KeyEvent) -> Result<()> {
        let (format, filename) = if let AppState::Exporting { format, filename } = &self.state {
            (format.clone(), filename.clone())
        } else {
            return Ok(());
        };

        match key.code {
            KeyCode::Esc => {
                if format.is_none() {
                    // Cancel export
                    self.state = AppState::Normal;
                    self.screen = Screen::Home;
                } else {
                    // Go back to format selection
                    self.state = AppState::Exporting {
                        format: None,
                        filename,
                    };
                }
            }
            KeyCode::Char('1') if format.is_none() => {
                self.state = AppState::Exporting {
                    format: Some(ExportFormat::Json),
                    filename: String::from("analysis_results"),
                };
            }
            KeyCode::Char('2') if format.is_none() => {
                self.state = AppState::Exporting {
                    format: Some(ExportFormat::Markdown),
                    filename: String::from("analysis_results"),
                };
            }
            KeyCode::Char('3') if format.is_none() => {
                self.state = AppState::Exporting {
                    format: Some(ExportFormat::Excel),
                    filename: String::from("analysis_results"),
                };
            }
            KeyCode::Char('4') if format.is_none() => {
                self.state = AppState::Exporting {
                    format: Some(ExportFormat::GroupedExcel),
                    filename: String::from("grouped_data"),
                };
            }
            KeyCode::Backspace if format.is_some() => {
                let mut new_filename = filename.clone();
                new_filename.pop();
                self.state = AppState::Exporting {
                    format: format.clone(),
                    filename: new_filename,
                };
            }
            KeyCode::Char(c) if format.is_some() => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    let mut new_filename = filename.clone();
                    new_filename.push(c);
                    self.state = AppState::Exporting {
                        format: format.clone(),
                        filename: new_filename,
                    };
                }
            }
            KeyCode::Enter if format.is_some() => {
                // Perform export
                if let Some(result) = &self.analysis_result {
                    let export_result = match format.as_ref().unwrap() {
                        ExportFormat::Json => {
                            let path = format!("{}.json", filename);
                            let exporter = JsonExporter::new(true);
                            let output = exporter.export(result)?;
                            std::fs::write(&path, output)?;
                            format!("Exported to {}", path)
                        }
                        ExportFormat::Markdown => {
                            let path = format!("{}.md", filename);
                            let exporter = MarkdownExporter::new();
                            let output = exporter.export(result)?;
                            std::fs::write(&path, output)?;
                            format!("Exported to {}", path)
                        }
                        ExportFormat::Excel => {
                            let path = format!("{}.xlsx", filename);
                            let exporter = ExcelExporter::new(path.clone());
                            exporter.export_to_file(result)?;
                            format!("Exported to {}", path)
                        }
                        ExportFormat::GroupedExcel => {
                            use crate::export::GroupedDataExporter;
                            let mut exported_count = 0;

                            for (idx, (name, data, columns)) in
                                result.source_data.iter().enumerate()
                            {
                                if idx < result.grouping_analyses.len() {
                                    let grouping = &result.grouping_analyses[idx];
                                    if !grouping.grouping_dimensions.is_empty() {
                                        let file_path = if result.source_data.len() > 1 {
                                            format!(
                                                "{}_{}.xlsx",
                                                filename,
                                                name.replace(".csv", "").replace(".xlsx", "")
                                            )
                                        } else {
                                            format!("{}.xlsx", filename)
                                        };

                                        let mv_cols: &[crate::analysis::MultiValueColumnAnalysis] = result
                                            .multi_value_analyses
                                            .iter()
                                            .find(|a| &a.table_name == name)
                                            .map(|a| a.multi_value_columns.as_slice())
                                            .unwrap_or(&[]);

                                        let exporter = GroupedDataExporter::new(file_path);
                                        exporter.export_grouped_data(
                                            data, columns, grouping, mv_cols,
                                        )?;
                                        exported_count += 1;
                                    }
                                }
                            }

                            if exported_count > 0 {
                                format!("Exported {} grouped data file(s)", exported_count)
                            } else {
                                "No grouping dimensions found to export".to_string()
                            }
                        }
                        _ => "Unknown format".to_string(),
                    };

                    self.status_message = export_result;
                    self.state = AppState::Normal;
                    self.screen = Screen::Home;
                } else {
                    self.status_message = "No analysis results to export".to_string();
                    self.state = AppState::Normal;
                    self.screen = Screen::Home;
                }
            }
            _ => {}
        }

        Ok(())
    }
}
