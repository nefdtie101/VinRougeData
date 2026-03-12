use crate::analysis::{
    DataProfiler, GroupingAnalyzer, MultiValueDetector, ReconciliationConfig, Reconciliator,
    RelationshipDetector, WorkflowDetector,
};
use crate::config::SourceConfig;
use crate::export::{
    AnalysisResult, ExcelExporter, ExportFormat, Exporter, JsonExporter, MarkdownExporter,
};
use crate::ollama::{self, OllamaClient};
use crate::sources::{CsvSource, DataSource, ExcelSource, MssqlSource};
use crate::tui::events::{is_exit_key, AppEvent};
use crate::tui::file_browser::{FileBrowser, FileFilter};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use serde::{Deserialize, Serialize};
use std::process::Child;

// ── Persisted TUI settings ────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct TuiSettings {
    #[serde(default)]
    ollama_url: String,
    #[serde(default)]
    ollama_model: String,
    #[serde(default)]
    ollama_models_dir: Option<String>,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self {
            ollama_url: ollama::DEFAULT_URL.to_string(),
            ollama_model: ollama::DEFAULT_MODEL.to_string(),
            ollama_models_dir: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Home,
    SourceList,
    Analysis,
    Results,
    Reconcile,
    Export,
    Help,
    Ollama,
    OllamaModelPicker,
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
    AskingOllama {
        input: String,
        response: Option<String>,
        loading: bool,
        editing_model: bool,
        editing_models_dir: bool,
        available_models: Vec<String>,
    },
    PickingOllamaModel {
        models: Vec<String>,
        selected: usize,
        loading: bool,
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
    pub results_tab: usize,
    pub results_row: usize,
    pub ollama_url: String,
    pub ollama_model: String,
    pub ollama_models_dir: Option<String>,
    pub ollama_process: Option<Child>,
    pub ollama_is_running: bool,
}

impl App {
    pub fn new() -> Self {
        let settings = Self::load_settings();
        Self {
            screen: Screen::Home,
            state: AppState::Normal,
            sources: Vec::new(),
            selected_index: 0,
            analysis_result: None,
            status_message: String::from("Ready"),
            scroll_offset: 0,
            results_tab: 0,
            results_row: 0,
            ollama_url: settings.ollama_url,
            ollama_model: settings.ollama_model,
            ollama_models_dir: settings.ollama_models_dir,
            ollama_process: None,
            ollama_is_running: false,
        }
    }

    // ── Settings persistence ──────────────────────────────────────────────────

    fn settings_path() -> std::path::PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            std::path::PathBuf::from(home)
                .join(".config")
                .join("vinrouge")
                .join("tui.toml")
        } else {
            std::path::PathBuf::from("vinrouge-tui.toml")
        }
    }

    fn load_settings() -> TuiSettings {
        let path = Self::settings_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self) {
        let path = Self::settings_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let settings = TuiSettings {
            ollama_url: self.ollama_url.clone(),
            ollama_model: self.ollama_model.clone(),
            ollama_models_dir: self.ollama_models_dir.clone(),
        };
        if let Ok(content) = toml::to_string_pretty(&settings) {
            let _ = std::fs::write(&path, content);
        }
    }

    // ── Ollama process management ─────────────────────────────────────────────

    fn spawn_ollama(&mut self) -> std::io::Result<()> {
        let binary = ollama::find_binary()?;
        let mut cmd = std::process::Command::new(binary);
        cmd.arg("serve");

        // Resolve port — pick a free one if the preferred port is already in use
        let preferred = ollama::port_from_url(&self.ollama_url);
        let (port, changed) = ollama::resolve_port(preferred);
        if changed {
            self.ollama_url = format!("http://127.0.0.1:{port}");
            self.status_message =
                format!("Port {preferred} in use — Ollama starting on port {port}");
        }
        cmd.env("OLLAMA_HOST", format!("127.0.0.1:{port}"));

        // Models dir: TUI override (F3) first, then DEFAULT_MODELS_DIR in code
        if let Some(dir) = ollama::resolve_models_dir(self.ollama_models_dir.as_deref()) {
            cmd.env("OLLAMA_MODELS", dir);
        }

        self.ollama_process = Some(cmd.spawn()?);
        self.ollama_is_running = true;
        Ok(())
    }

    /// Refresh the cached `ollama_is_running` flag by polling the child process.
    pub fn refresh_ollama_status(&mut self) {
        self.ollama_is_running = match &mut self.ollama_process {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        };
    }

    pub async fn handle_event(&mut self, event: AppEvent) -> Result<bool> {
        self.refresh_ollama_status();

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
                    AppState::AskingOllama { .. } => self.handle_ollama_key(key).await?,
                    AppState::PickingOllamaModel { .. } => {
                        self.handle_model_picker_key(key).await?
                    }
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
                KeyCode::Char('6') => {
                    if !self.ollama_is_running {
                        match self.spawn_ollama() {
                            Ok(()) => {
                                self.status_message = "Ollama started automatically.".to_string()
                            }
                            Err(e) => {
                                self.status_message =
                                    format!("Warning: could not start Ollama: {e}")
                            }
                        }
                    }
                    self.screen = Screen::Ollama;
                    self.state = AppState::AskingOllama {
                        input: String::new(),
                        response: None,
                        loading: false,
                        editing_model: false,
                        editing_models_dir: false,
                        available_models: Vec::new(),
                    };
                }
                KeyCode::Char('7') => {
                    if self.ollama_is_running {
                        if let Some(child) = &mut self.ollama_process {
                            let _ = child.kill();
                        }
                        self.ollama_process = None;
                        self.ollama_is_running = false;
                        self.status_message = "Ollama stopped.".to_string();
                    } else {
                        match self.spawn_ollama() {
                            Ok(()) => {
                                self.status_message =
                                    "Ollama started. Use option 6 to chat.".to_string()
                            }
                            Err(e) => self.status_message = format!("Could not launch ollama: {e}"),
                        }
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
            KeyCode::Tab => {
                self.results_tab = (self.results_tab + 1) % 7;
                self.results_row = 0;
            }
            KeyCode::BackTab => {
                self.results_tab = (self.results_tab + 6) % 7;
                self.results_row = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.results_row = self.results_row.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.results_row += 1;
            }
            KeyCode::PageUp => {
                self.results_row = self.results_row.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.results_row += 10;
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.results_row = 0;
            }
            KeyCode::Esc => {
                self.state = AppState::Normal;
                self.screen = Screen::Home;
                self.results_tab = 0;
                self.results_row = 0;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_results_view_mouse(&mut self, mouse: crossterm::event::MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.results_row = self.results_row.saturating_sub(3);
            }
            MouseEventKind::ScrollDown => {
                self.results_row += 3;
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

    pub async fn handle_ollama_key(&mut self, key: KeyEvent) -> Result<()> {
        let editing_model = matches!(
            &self.state,
            AppState::AskingOllama {
                editing_model: true,
                ..
            }
        );
        let editing_dir = matches!(
            &self.state,
            AppState::AskingOllama {
                editing_models_dir: true,
                ..
            }
        );
        let editing = editing_model || editing_dir;

        match key.code {
            KeyCode::Esc => {
                if editing_model {
                    if let AppState::AskingOllama { editing_model, .. } = &mut self.state {
                        *editing_model = false;
                    }
                } else if editing_dir {
                    if let AppState::AskingOllama {
                        editing_models_dir, ..
                    } = &mut self.state
                    {
                        *editing_models_dir = false;
                    }
                } else {
                    self.state = AppState::Normal;
                    self.screen = Screen::Home;
                }
            }

            // F2: edit model name
            KeyCode::F(2) => {
                if let AppState::AskingOllama {
                    editing_model,
                    editing_models_dir,
                    ..
                } = &mut self.state
                {
                    *editing_models_dir = false;
                    *editing_model = !*editing_model;
                }
            }

            // F3: edit models storage directory
            KeyCode::F(3) => {
                if let AppState::AskingOllama {
                    editing_model,
                    editing_models_dir,
                    ..
                } = &mut self.state
                {
                    *editing_model = false;
                    *editing_models_dir = !*editing_models_dir;
                }
            }

            // F5: open model picker
            KeyCode::F(5) => {
                if !self.ollama_is_running {
                    match self.spawn_ollama() {
                        Ok(()) => tokio::time::sleep(std::time::Duration::from_millis(1500)).await,
                        Err(e) => {
                            self.status_message = format!("Could not start Ollama: {e}");
                            return Ok(());
                        }
                    }
                }
                self.status_message = "Fetching installed models…".to_string();
                let client = OllamaClient::new(self.ollama_url.clone(), self.ollama_model.clone());
                match client.list_models().await {
                    Ok(models) if models.is_empty() => {
                        self.status_message = "No models found — is Ollama running?".to_string();
                    }
                    Ok(models) => {
                        let selected = models
                            .iter()
                            .position(|m| m == &self.ollama_model)
                            .unwrap_or(0);
                        self.screen = Screen::OllamaModelPicker;
                        self.state = AppState::PickingOllamaModel {
                            models,
                            selected,
                            loading: false,
                        };
                        self.status_message = "Pick a model with ↑↓, Enter to confirm".to_string();
                    }
                    Err(e) => self.status_message = format!("Could not list models: {e}"),
                }
            }

            KeyCode::Backspace => {
                if editing_model {
                    self.ollama_model.pop();
                } else if editing_dir {
                    let dir = self.ollama_models_dir.get_or_insert_with(String::new);
                    dir.pop();
                    if dir.is_empty() {
                        self.ollama_models_dir = None;
                    }
                } else if let AppState::AskingOllama { input, .. } = &mut self.state {
                    input.pop();
                }
            }

            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if editing_model {
                    self.ollama_model.push(c);
                } else if editing_dir {
                    self.ollama_models_dir
                        .get_or_insert_with(String::new)
                        .push(c);
                } else if let AppState::AskingOllama { input, .. } = &mut self.state {
                    input.push(c);
                }
            }

            KeyCode::Enter => {
                if editing_model {
                    if let AppState::AskingOllama { editing_model, .. } = &mut self.state {
                        *editing_model = false;
                    }
                    self.save_settings();
                    self.status_message = format!("Model set to '{}'", self.ollama_model);
                    return Ok(());
                }

                if editing_dir {
                    if let AppState::AskingOllama {
                        editing_models_dir, ..
                    } = &mut self.state
                    {
                        *editing_models_dir = false;
                    }
                    self.save_settings();
                    let msg = match &self.ollama_models_dir {
                        Some(d) => format!("Models dir set to '{d}'"),
                        None => "Models dir reset to Ollama default".to_string(),
                    };
                    self.status_message = msg;
                    return Ok(());
                }

                let prompt = if let AppState::AskingOllama { input, .. } = &self.state {
                    input.trim().to_string()
                } else {
                    return Ok(());
                };

                if prompt.is_empty() {
                    return Ok(());
                }

                // Ensure Ollama is running before we send
                if !self.ollama_is_running {
                    match self.spawn_ollama() {
                        Ok(()) => tokio::time::sleep(std::time::Duration::from_millis(1500)).await,
                        Err(e) => {
                            self.status_message = format!("Could not start Ollama: {e}");
                            return Ok(());
                        }
                    }
                }

                let context = self
                    .analysis_result
                    .as_ref()
                    .map(|r| build_analysis_summary(r))
                    .unwrap_or_else(|| "No analysis has been run yet.".to_string());

                let full_prompt = ollama::build_prompt(&context, &prompt);

                if let AppState::AskingOllama {
                    loading, response, ..
                } = &mut self.state
                {
                    *loading = true;
                    *response = None;
                }

                self.status_message = format!("Asking {} …", self.ollama_model);

                let client = OllamaClient::new(self.ollama_url.clone(), self.ollama_model.clone());
                match client.chat(&full_prompt).await {
                    Ok(answer) => {
                        if let AppState::AskingOllama {
                            loading, response, ..
                        } = &mut self.state
                        {
                            *loading = false;
                            *response = Some(answer);
                        }
                        self.status_message = "Response received".to_string();
                    }
                    Err(e) => {
                        if let AppState::AskingOllama { loading, .. } = &mut self.state {
                            *loading = false;
                        }
                        self.status_message = format!("Ollama error: {e}");
                    }
                }
            }

            _ => {}
        }

        Ok(())
    }

    pub async fn handle_model_picker_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                // Go back to Ollama chat screen
                self.screen = Screen::Ollama;
                self.state = AppState::AskingOllama {
                    input: String::new(),
                    response: None,
                    loading: false,
                    editing_model: false,
                    editing_models_dir: false,
                    available_models: Vec::new(),
                };
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let AppState::PickingOllamaModel {
                    selected, models, ..
                } = &mut self.state
                {
                    if *selected > 0 {
                        *selected -= 1;
                    } else {
                        *selected = models.len().saturating_sub(1);
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let AppState::PickingOllamaModel {
                    selected, models, ..
                } = &mut self.state
                {
                    *selected = (*selected + 1) % models.len().max(1);
                }
            }
            KeyCode::Enter => {
                let chosen = if let AppState::PickingOllamaModel {
                    selected, models, ..
                } = &self.state
                {
                    models.get(*selected).cloned()
                } else {
                    None
                };
                if let Some(model) = chosen {
                    self.ollama_model = model;
                }
                self.screen = Screen::Ollama;
                self.state = AppState::AskingOllama {
                    input: String::new(),
                    response: None,
                    loading: false,
                    editing_model: false,
                    editing_models_dir: false,
                    available_models: Vec::new(),
                };
                self.status_message = format!("Model set to '{}'", self.ollama_model);
            }
            _ => {}
        }
        Ok(())
    }
}

/// Produce a short plain-text summary of an analysis result for use as LLM context.
fn build_analysis_summary(result: &AnalysisResult) -> String {
    let mut s = String::new();

    s.push_str(&format!("Tables ({}):\n", result.tables.len()));
    for t in &result.tables {
        s.push_str(&format!(
            "  - {} ({} columns, ~{} rows)\n",
            t.full_name,
            t.columns.len(),
            t.row_count.unwrap_or(0)
        ));
        for c in &t.columns {
            s.push_str(&format!(
                "      {}: {:?}{}\n",
                c.name,
                c.data_type,
                if c.nullable { "" } else { " NOT NULL" }
            ));
        }
    }

    if !result.relationships.is_empty() {
        s.push_str(&format!(
            "\nRelationships ({}):\n",
            result.relationships.len()
        ));
        for r in &result.relationships {
            s.push_str(&format!(
                "  - {}.{} -> {}.{}\n",
                r.from_table, r.from_column, r.to_table, r.to_column
            ));
        }
    }

    if !result.workflows.is_empty() {
        s.push_str(&format!("\nWorkflows ({}):\n", result.workflows.len()));
        for w in &result.workflows {
            s.push_str(&format!("  - {:?}: {}\n", w.workflow_type, w.description));
        }
    }

    s
}
