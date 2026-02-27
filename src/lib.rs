pub mod schema;
pub mod sources;
pub mod analysis;
pub mod export;
pub mod config;
pub mod tui;

pub use schema::{Column, Table, Relationship, DataType};
pub use sources::SourceType;
pub use analysis::{RelationshipDetector, WorkflowDetector};
pub use export::ExportFormat;
