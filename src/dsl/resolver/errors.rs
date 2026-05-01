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
}
