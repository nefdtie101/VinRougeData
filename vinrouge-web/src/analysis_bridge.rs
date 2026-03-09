use anyhow::Result;
use vinrouge::{
    analysis::{
        DataProfiler, GroupingAnalyzer, MultiValueDetector, ReconciliationConfig, Reconciliator,
        RelationshipDetector, WorkflowDetector,
    },
    export::AnalysisResult,
    sources::{CsvSource, DataSource, ExcelSource},
};

pub enum FileType {
    Excel,
    Csv,
}

pub struct UploadedFile {
    pub name: String,
    pub bytes: Vec<u8>,
    pub file_type: FileType,
}

impl UploadedFile {
    pub fn detect(name: String, bytes: Vec<u8>) -> Self {
        let lower = name.to_lowercase();
        let file_type = if lower.ends_with(".xlsx") || lower.ends_with(".xls") {
            FileType::Excel
        } else {
            FileType::Csv
        };
        Self {
            name,
            bytes,
            file_type,
        }
    }
}

pub async fn run_analysis(files: Vec<UploadedFile>) -> Result<AnalysisResult> {
    let mut all_tables = Vec::new();
    let mut all_profiles = Vec::new();
    let mut all_groupings = Vec::new();
    let mut source_data: Vec<(String, Vec<Vec<String>>, Vec<vinrouge::schema::Column>)> =
        Vec::new();

    for file in files {
        match file.file_type {
            FileType::Excel => {
                let mut source = ExcelSource::from_bytes(file.bytes, file.name.clone());
                let tables = source.extract_schema().await?;
                let data = source.read_data().await?;

                if let Some(table) = tables.first() {
                    let profiler = DataProfiler::new(10_000);
                    all_profiles.push(profiler.profile_data(&data, &table.columns));
                    let analyzer = GroupingAnalyzer::new(1_000);
                    all_groupings.push(analyzer.analyze_groupings(&data, &table.columns));
                    source_data.push((file.name, data, table.columns.clone()));
                }
                all_tables.extend(tables);
            }
            FileType::Csv => {
                let mut source = CsvSource::from_bytes(file.bytes, file.name.clone());
                let tables = source.extract_schema().await?;
                let data = source.read_data().await?;

                if let Some(table) = tables.first() {
                    let profiler = DataProfiler::new(10_000);
                    all_profiles.push(profiler.profile_data(&data, &table.columns));
                    let analyzer = GroupingAnalyzer::new(1_000);
                    all_groupings.push(analyzer.analyze_groupings(&data, &table.columns));
                    source_data.push((file.name, data, table.columns.clone()));
                }
                all_tables.extend(tables);
            }
        }
    }

    // Multi-value detection over all sources at once
    let mv_detector = MultiValueDetector::new(1_000);
    let multi_value_analyses = mv_detector.analyze_all_sources(&source_data);

    // Relationship + workflow detection
    let mut rel_detector = RelationshipDetector::new(all_tables.clone());
    let relationships = rel_detector.detect_relationships();
    let mut wf_detector = WorkflowDetector::new(all_tables.clone(), relationships.clone());
    let workflows = wf_detector.detect_workflows();

    // Reconciliation between first two sources (if available)
    let reconciliation_results = if source_data.len() >= 2 {
        let config = ReconciliationConfig {
            key_columns: vec![],   // auto-detect
            compare_columns: None, // compare all
            column_mappings: vec![],
            case_sensitive: false,
            trim_whitespace: true,
            max_mismatches: 1_000,
        };
        let reconciliator = Reconciliator::new(config);
        let (name_a, data_a, cols_a) = &source_data[0];
        let (name_b, data_b, cols_b) = &source_data[1];
        vec![reconciliator.reconcile(name_a, data_a, cols_a, name_b, data_b, cols_b)]
    } else {
        vec![]
    };

    Ok(AnalysisResult {
        tables: all_tables,
        relationships,
        workflows,
        data_profiles: all_profiles,
        grouping_analyses: all_groupings,
        reconciliation_results,
        multi_value_analyses,
        source_data,
    })
}
