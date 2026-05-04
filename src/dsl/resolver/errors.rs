#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ResolveError {
    #[error("unknown table '{table}' referenced in '{reference}'")]
    UnknownTable { table: String, reference: String },

    #[error("unknown column '{column}' in table '{table}'")]
    UnknownColumn { table: String, column: String },

    #[error("bare column reference '{0}' — use table.column notation inside aggregates")]
    BareColumnInAggregate(String),

    #[error("column reference '{0}' has no table prefix — cannot resolve without a context table")]
    AmbiguousColumn(String),

    #[error("invalid chart type '{0}' — expected one of: bar, line, pie, scatter")]
    InvalidChartType(String),

    #[error("chart aggregate must be an aggregate function (SUM, AVG, COUNT, MIN, MAX), got {0}")]
    InvalidChartAggregate(String),

    #[error("chart dimension must be a table.column reference, got {0}")]
    InvalidChartDimension(String),
}
