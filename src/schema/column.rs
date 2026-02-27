use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DataType {
    // Numeric types
    Integer,
    BigInt,
    SmallInt,
    TinyInt,
    Decimal { precision: u8, scale: u8 },
    Float,
    Real,

    // String types
    Char { length: usize },
    VarChar { max_length: Option<usize> },
    Text,

    // Date/Time types
    Date,
    DateTime,
    DateTime2,
    Time,
    Timestamp,

    // Binary types
    Binary { length: usize },
    VarBinary { max_length: Option<usize> },

    // Other types
    Boolean,
    Uuid,
    Json,
    Xml,
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub is_foreign_key: bool,
    pub default_value: Option<String>,
    pub max_length: Option<usize>,
    pub precision: Option<u8>,
    pub scale: Option<u8>,

    // Statistics (populated during analysis)
    pub unique_count: Option<usize>,
    pub null_count: Option<usize>,
    pub sample_values: Vec<String>,
}

impl Column {
    pub fn new(name: String, data_type: DataType) -> Self {
        Self {
            name,
            data_type,
            nullable: true,
            is_primary_key: false,
            is_foreign_key: false,
            default_value: None,
            max_length: None,
            precision: None,
            scale: None,
            unique_count: None,
            null_count: None,
            sample_values: Vec::new(),
        }
    }

    pub fn with_nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    pub fn with_primary_key(mut self, is_pk: bool) -> Self {
        self.is_primary_key = is_pk;
        self
    }

    pub fn with_foreign_key(mut self, is_fk: bool) -> Self {
        self.is_foreign_key = is_fk;
        self
    }
}
