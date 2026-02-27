use super::{AnalysisResult, Exporter};
use anyhow::Result;
use serde_json;

pub struct JsonExporter {
    pretty: bool,
}

impl JsonExporter {
    pub fn new(pretty: bool) -> Self {
        Self { pretty }
    }
}

impl Exporter for JsonExporter {
    fn export(&self, result: &AnalysisResult) -> Result<String> {
        let json = if self.pretty {
            serde_json::to_string_pretty(result)?
        } else {
            serde_json::to_string(result)?
        };

        Ok(json)
    }
}

// Implement Serialize for AnalysisResult
impl serde::Serialize for AnalysisResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("AnalysisResult", 6)?;
        state.serialize_field("tables", &self.tables)?;
        state.serialize_field("relationships", &self.relationships)?;
        state.serialize_field("workflows", &self.workflows)?;
        state.serialize_field("data_profiles", &self.data_profiles)?;
        state.serialize_field("grouping_analyses", &self.grouping_analyses)?;
        state.serialize_field("reconciliation_results", &self.reconciliation_results)?;
        state.end()
    }
}
