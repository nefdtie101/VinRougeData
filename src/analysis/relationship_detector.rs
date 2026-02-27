use crate::schema::{Relationship, RelationshipType, Table};
use std::collections::HashMap;

pub struct RelationshipDetector {
    tables: Vec<Table>,
    relationships: Vec<Relationship>,
}

impl RelationshipDetector {
    pub fn new(tables: Vec<Table>) -> Self {
        Self {
            tables,
            relationships: Vec::new(),
        }
    }

    pub fn detect_relationships(&mut self) -> Vec<Relationship> {
        // First, detect explicit foreign keys (from MSSQL)
        self.detect_explicit_foreign_keys();

        // Then, detect heuristic relationships
        self.detect_by_column_names();
        self.detect_by_naming_patterns();

        self.relationships.clone()
    }

    fn detect_explicit_foreign_keys(&mut self) {
        // For MSSQL tables, we already have FK information in the column metadata
        for table in &self.tables {
            if table.source_type != "mssql" {
                continue;
            }

            for column in &table.columns {
                if column.is_foreign_key {
                    // Try to find the referenced table by naming convention
                    // Common patterns: CustomerId -> Customer(Id), CustomerKey -> Customer(Key)
                    if let Some((ref_table, ref_column)) =
                        self.infer_foreign_key_target(&column.name)
                    {
                        let relationship = Relationship::new(
                            table.full_name.clone(),
                            column.name.clone(),
                            ref_table,
                            ref_column,
                            RelationshipType::ForeignKey,
                        );
                        self.relationships.push(relationship);
                    }
                }
            }
        }
    }

    fn detect_by_column_names(&mut self) {
        // Build an index of all columns by name
        let mut column_index: HashMap<String, Vec<(String, String)>> = HashMap::new();

        for table in &self.tables {
            for column in &table.columns {
                let normalized = column.name.to_lowercase();
                column_index
                    .entry(normalized)
                    .or_insert_with(Vec::new)
                    .push((table.full_name.clone(), column.name.clone()));
            }
        }

        // Find exact matches (same column name in different tables)
        for (col_name, occurrences) in column_index.iter() {
            if occurrences.len() < 2 {
                continue;
            }

            // Check if any of these columns are likely keys
            let mut potential_keys = Vec::new();
            let mut potential_foreign_keys = Vec::new();

            for (table_name, original_col_name) in occurrences {
                if let Some(table) = self.tables.iter().find(|t| &t.full_name == table_name) {
                    if let Some(column) = table.get_column(original_col_name) {
                        // Heuristic: column is likely a key if it's named "id", has "id" suffix,
                        // or has all unique values
                        let is_likely_key = col_name == "id"
                            || col_name.ends_with("_id")
                            || col_name.ends_with("id")
                            || column.is_primary_key
                            || (column.unique_count.is_some()
                                && table.row_count.is_some()
                                && column.unique_count.unwrap() == table.row_count.unwrap());

                        if is_likely_key {
                            potential_keys.push((table_name.clone(), original_col_name.clone()));
                        } else {
                            potential_foreign_keys
                                .push((table_name.clone(), original_col_name.clone()));
                        }
                    }
                }
            }

            // Create relationships: foreign keys -> keys
            for (fk_table, fk_column) in &potential_foreign_keys {
                for (pk_table, pk_column) in &potential_keys {
                    if fk_table != pk_table {
                        // Avoid self-references for now
                        // Calculate confidence based on naming
                        let confidence = 85; // High confidence for exact name match

                        let relationship = Relationship::new(
                            fk_table.clone(),
                            fk_column.clone(),
                            pk_table.clone(),
                            pk_column.clone(),
                            RelationshipType::NameMatch { confidence },
                        );
                        self.relationships.push(relationship);
                    }
                }
            }
        }
    }

    fn detect_by_naming_patterns(&mut self) {
        // Detect relationships based on naming patterns like:
        // - CustomerID -> Customer.ID
        // - OrderCustomerId -> Customer.Id
        // - customer_id -> customer.id

        for table in &self.tables {
            for column in &table.columns {
                if let Some((ref_table_name, ref_column_name)) =
                    self.find_reference_by_pattern(&column.name)
                {
                    // Check if this relationship already exists
                    let exists = self.relationships.iter().any(|r| {
                        r.from_table == table.full_name
                            && r.from_column == column.name
                            && r.to_table == ref_table_name
                            && r.to_column == ref_column_name
                    });

                    if !exists {
                        let confidence = 70; // Medium confidence for pattern match
                        let relationship = Relationship::new(
                            table.full_name.clone(),
                            column.name.clone(),
                            ref_table_name,
                            ref_column_name,
                            RelationshipType::NameMatch { confidence },
                        );
                        self.relationships.push(relationship);
                    }
                }
            }
        }
    }

    fn infer_foreign_key_target(&self, column_name: &str) -> Option<(String, String)> {
        let lower = column_name.to_lowercase();

        // Pattern 1: CustomerId -> Customer.Id
        if lower.ends_with("id") {
            let table_name = &column_name[..column_name.len() - 2];
            if let Some(table) = self.find_table_by_name(table_name) {
                return Some((table.full_name.clone(), "Id".to_string()));
            }
        }

        // Pattern 2: CustomerKey -> Customer.Key
        if lower.ends_with("key") {
            let table_name = &column_name[..column_name.len() - 3];
            if let Some(table) = self.find_table_by_name(table_name) {
                return Some((table.full_name.clone(), "Key".to_string()));
            }
        }

        None
    }

    fn find_reference_by_pattern(&self, column_name: &str) -> Option<(String, String)> {
        let lower = column_name.to_lowercase();

        // Try various patterns
        let patterns = vec![
            ("id", "id"),
            ("_id", "id"),
            ("key", "key"),
            ("_key", "key"),
            ("_pk", "id"),
        ];

        for (suffix, target_col) in patterns {
            if lower.ends_with(suffix) && lower.len() > suffix.len() {
                let potential_table = &column_name[..column_name.len() - suffix.len()];

                if let Some(table) = self.find_table_by_name(potential_table) {
                    // Check if target column exists
                    if table.columns.iter().any(|c| {
                        c.name.to_lowercase() == target_col
                            || c.name.to_lowercase() == format!("{}_{}", potential_table, target_col).to_lowercase()
                    }) {
                        return Some((table.full_name.clone(), target_col.to_string()));
                    }
                }
            }
        }

        None
    }

    fn find_table_by_name(&self, name: &str) -> Option<&Table> {
        let normalized = name.to_lowercase().replace("_", "");

        self.tables.iter().find(|t| {
            let table_name = t.name.to_lowercase().replace("_", "");
            table_name == normalized || table_name.starts_with(&normalized)
        })
    }

    pub fn get_relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    pub fn get_relationships_for_table(&self, table_name: &str) -> Vec<&Relationship> {
        self.relationships
            .iter()
            .filter(|r| r.from_table == table_name || r.to_table == table_name)
            .collect()
    }
}
