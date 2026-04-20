use crate::schema::{Relationship, RelationshipType};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A detected + scored join candidate ready to show in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelCandidate {
    /// Opaque import ID from the session DB (left side of the join).
    pub left_import_id: String,
    pub left_table: String,
    pub left_col: String,
    /// Opaque import ID from the session DB (right side of the join).
    pub right_import_id: String,
    pub right_table: String,
    pub right_col: String,
    /// Combined confidence score 0–100 (60 % name signal + 40 % value overlap).
    pub confidence: u8,
    /// Number of sampled left-side values that matched a right-side value.
    pub overlap_count: usize,
}

/// Scores detected [`Relationship`]s using pre-fetched value samples.
///
/// Keeping this in the shared library means the scoring formula is testable
/// without a SQLite session DB — the caller is responsible for supplying the
/// samples.
pub struct RelationshipScorer;

impl RelationshipScorer {
    /// Score every relationship and return sorted `RelCandidate`s.
    ///
    /// # Arguments
    /// * `relationships` — raw output from [`RelationshipDetector::detect_relationships`]
    /// * `table_to_import` — maps `table.full_name` → opaque import ID string
    /// * `samples` — maps `(import_id, column_name)` → sampled non-empty values
    pub fn score(
        relationships: &[Relationship],
        table_to_import: &HashMap<String, String>,
        samples: &HashMap<(String, String), Vec<String>>,
    ) -> Vec<RelCandidate> {
        let mut seen: HashSet<(String, String, String, String)> = HashSet::new();
        let mut candidates: Vec<RelCandidate> = Vec::new();

        for rel in relationships {
            let Some(lid) = table_to_import.get(&rel.from_table) else {
                continue;
            };
            let Some(rid) = table_to_import.get(&rel.to_table) else {
                continue;
            };

            let key = (
                lid.clone(),
                rel.from_column.clone(),
                rid.clone(),
                rel.to_column.clone(),
            );
            if !seen.insert(key) {
                continue;
            }

            let name_conf: u8 = match &rel.relationship_type {
                RelationshipType::ForeignKey => 95,
                RelationshipType::NameMatch { confidence } => *confidence,
                RelationshipType::UniquePattern => 70,
                RelationshipType::Composite => 85,
                RelationshipType::ValueOverlap { overlap_percent } => *overlap_percent,
            };

            let empty = Vec::new();
            let lv = samples
                .get(&(lid.clone(), rel.from_column.clone()))
                .unwrap_or(&empty);
            let rv = samples
                .get(&(rid.clone(), rel.to_column.clone()))
                .unwrap_or(&empty);

            let rv_set: HashSet<&str> = rv.iter().map(|s| s.as_str()).collect();
            let overlap = lv.iter().filter(|v| rv_set.contains(v.as_str())).count();
            let overlap_ratio = if lv.is_empty() {
                0.0_f64
            } else {
                overlap as f64 / lv.len().min(rv.len()).max(1) as f64
            };

            // 60 % name/pattern confidence + 40 % value overlap
            let confidence = ((name_conf as f64 * 0.6) + (overlap_ratio * 100.0 * 0.4)) as u8;

            candidates.push(RelCandidate {
                left_import_id: lid.clone(),
                left_table: rel.from_table.clone(),
                left_col: rel.from_column.clone(),
                right_import_id: rid.clone(),
                right_table: rel.to_table.clone(),
                right_col: rel.to_column.clone(),
                confidence: confidence.min(100),
                overlap_count: overlap,
            });
        }

        candidates.sort_by(|a, b| b.confidence.cmp(&a.confidence));
        candidates
    }
}
