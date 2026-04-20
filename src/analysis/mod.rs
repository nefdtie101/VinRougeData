mod data_profiler;
mod grouping_analyzer;
mod multi_value_detector;
mod reconciliation;
mod relationship_detector;
mod relationship_scorer;
mod workflow_detector;

pub use data_profiler::{
    ColumnCorrelation, ColumnProfile, CorrelationType, DataPattern, DataProfile, DataProfiler,
    PatternType,
};
pub use grouping_analyzer::{
    DimensionType, GroupingAnalysis, GroupingAnalyzer, GroupingDimension, GroupingHierarchy,
    HierarchyType,
};
pub use multi_value_detector::{
    DetectionMethod, MultiValueAnalysis, MultiValueColumnAnalysis, MultiValueDetector,
};
pub use reconciliation::{
    FieldMismatch, ReconciliationConfig, ReconciliationResult, Reconciliator,
};
pub use relationship_detector::RelationshipDetector;
pub use relationship_scorer::{RelCandidate, RelationshipScorer};
pub use workflow_detector::{Workflow, WorkflowDetector, WorkflowStep, WorkflowType};
