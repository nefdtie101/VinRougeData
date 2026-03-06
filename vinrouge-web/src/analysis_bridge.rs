use anyhow::Result;
use vinrouge::{
    analysis::{DataProfiler, GroupingAnalyzer, MultiValueAnalyzer, ReconciliationAnalyzer, RelationshipDetector, WorkflowDetector},
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
        Self { name, bytes, file_type }
    }
}

pub async fn run_analysis(files: Vec<UploadedFile>) -> Result<AnalysisResult> {
    let mut all_tables = Vec::new();
    let mut all_profiles = Vec::new();
    let mut all_groupings = Vec::new();
    let mut all_multi_value = Vec::new();
    let mut source_data = Vec::new();

    for file in files {
        match file.file_type {
            FileType::Excel => {
                let mut source = ExcelSource::from_bytes(file.bytes, file.name.clone());
                let tables = source.extract_schema().await?;
                let data = source.read_data().await?;

                if let Some(table) = tables.first() {
                    let profiler = DataProfiler::new(10_000);
                    let profile = profiler.profile_data(&data, &table.columns);
                    all_profiles.push(profile);

                    let analyzer = GroupingAnalyzer::new(1_000);
                    let grouping = analyzer.analyze_groupings(&data, &table.columns);
                    all_groupings.push(grouping);

                    let mv_analyzer = MultiValueAnalyzer::new();
                    let mv = mv_analyzer.analyze(&data, &table.columns);
                    all_multi_value.push(mv);

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
                    let profile = profiler.profile_data(&data, &table.columns);
                    all_profiles.push(profile);

                    let analyzer = GroupingAnalyzer::new(1_000);
                    let grouping = analyzer.analyze_groupings(&data, &table.columns);
                    all_groupings.push(grouping);

                    let mv_analyzer = MultiValueAnalyzer::new();
                    let mv = mv_analyzer.analyze(&data, &table.columns);
                    all_multi_value.push(mv);

                    source_data.push((file.name, data, table.columns.clone()));
                }

                all_tables.extend(tables);
            }
        }
    }

    let mut rel_detector = RelationshipDetector::new(all_tables.clone());
    let relationships = rel_detector.detect_relationships();

    let mut wf_detector = WorkflowDetector::new(all_tables.clone(), relationships.clone());
    let workflows = wf_detector.detect_workflows();

    let reconciliation_results = if source_data.len() >= 2 {
        let analyzer = ReconciliationAnalyzer::new(1_000);
        let (name_a, data_a, cols_a) = &source_data[0];
        let (name_b, data_b, cols_b) = &source_data[1];
        analyzer.analyze(name_a, data_a, cols_a, name_b, data_b, cols_b)
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
        multi_value_analyses: all_multi_value,
        source_data,
    })
}
