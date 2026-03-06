use super::Column;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub schema: Option<String>,
    pub full_name: String, // schema.table or just table
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
    pub indexes: Vec<Index>,
    pub row_count: Option<usize>,

    // Source information
    pub source_type: String,     // "mssql", "csv", "excel", "flatfile"
    pub source_location: String, // connection string, file path, etc.

    // Metadata
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub is_primary: bool,
}

impl Table {
    pub fn new(name: String, source_type: String, source_location: String) -> Self {
        let full_name = name.clone();
        Self {
            name,
            schema: None,
            full_name,
            columns: Vec::new(),
            primary_key: Vec::new(),
            indexes: Vec::new(),
            row_count: None,
            source_type,
            source_location,
            description: None,
        }
    }

    pub fn with_schema(mut self, schema: String) -> Self {
        self.full_name = format!("{}.{}", schema, self.name);
        self.schema = Some(schema);
        self
    }

    pub fn add_column(&mut self, column: Column) {
        if column.is_primary_key {
            self.primary_key.push(column.name.clone());
        }
        self.columns.push(column);
    }

    pub fn get_column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    pub fn get_column_mut(&mut self, name: &str) -> Option<&mut Column> {
        self.columns.iter_mut().find(|c| c.name == name)
    }
}
