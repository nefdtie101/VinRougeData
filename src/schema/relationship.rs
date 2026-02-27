use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RelationshipType {
    /// Explicit foreign key constraint from database
    ForeignKey,

    /// Inferred from column name similarity
    NameMatch { confidence: u8 }, // 0-100

    /// Inferred from data value overlap
    ValueOverlap { overlap_percent: u8 }, // 0-100

    /// Inferred from uniqueness patterns
    UniquePattern,

    /// Multiple signals suggest relationship
    Composite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
    pub relationship_type: RelationshipType,
    pub constraint_name: Option<String>,

    // Metadata for analysis
    pub notes: Vec<String>,
}

impl Relationship {
    pub fn new(
        from_table: String,
        from_column: String,
        to_table: String,
        to_column: String,
        relationship_type: RelationshipType,
    ) -> Self {
        Self {
            from_table,
            from_column,
            to_table,
            to_column,
            relationship_type,
            constraint_name: None,
            notes: Vec::new(),
        }
    }

    pub fn with_constraint_name(mut self, name: String) -> Self {
        self.constraint_name = Some(name);
        self
    }

    pub fn add_note(&mut self, note: String) {
        self.notes.push(note);
    }

    pub fn is_explicit(&self) -> bool {
        matches!(self.relationship_type, RelationshipType::ForeignKey)
    }
}
