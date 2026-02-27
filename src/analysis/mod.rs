mod relationship_detector;
mod workflow_detector;
mod data_profiler;
mod grouping_analyzer;
mod reconciliation;

pub use relationship_detector::RelationshipDetector;
pub use workflow_detector::{WorkflowDetector, Workflow, WorkflowStep, WorkflowType};
pub use data_profiler::{DataProfiler, DataProfile, ColumnProfile, DataPattern, ColumnCorrelation, PatternType, CorrelationType};
pub use grouping_analyzer::{GroupingAnalyzer, GroupingAnalysis, GroupingDimension, GroupingHierarchy, DimensionType, HierarchyType};
pub use reconciliation::{Reconciliator, ReconciliationResult, ReconciliationConfig, FieldMismatch};
