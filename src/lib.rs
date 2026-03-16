pub mod analysis;
pub mod audit_prompts;
pub mod config;
pub mod export;
pub mod schema;
pub mod sources;

#[cfg(not(target_arch = "wasm32"))]
pub mod ollama;

#[cfg(not(target_arch = "wasm32"))]
pub mod projects;

#[cfg(not(target_arch = "wasm32"))]
pub mod tui;

pub use analysis::{RelationshipDetector, WorkflowDetector};
pub use export::ExportFormat;
pub use schema::{Column, DataType, Relationship, Table};
pub use sources::SourceType;
