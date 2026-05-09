use std::collections::{HashMap, HashSet};

/// Describes the shape of available data: table name → set of column names.
///
/// Column names are stored lower-cased so lookups are always case-insensitive.
#[derive(Debug, Default, Clone)]
pub struct Schema {
    pub(super) tables: HashMap<String, HashSet<String>>,
}

impl Schema {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a table with the given column names.
    /// Replaces any existing table with the same name.
    pub fn add_table(
        &mut self,
        table: impl Into<String>,
        columns: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.tables.insert(
            table.into().to_lowercase(),
            columns
                .into_iter()
                .map(|c| c.into().to_lowercase())
                .collect(),
        );
    }

    /// Check whether a table exists.
    pub fn has_table(&self, table: &str) -> bool {
        self.tables.contains_key(&table.to_lowercase())
    }

    /// Check whether a column exists in a table.
    pub fn has_column(&self, table: &str, column: &str) -> bool {
        self.tables
            .get(&table.to_lowercase())
            .map(|cols| cols.contains(&column.to_lowercase()))
            .unwrap_or(false)
    }
}
