use super::ProjectDetails;

// Re-export shared static constants so callers can use either path.
pub use crate::audit_prompts::{
    ANALYZE_FILE, ANALYZE_SOP, DATA_QUALITY, RECONCILIATION, SUMMARIZE_FINDINGS,
};

// ── Dynamic prompt built from project context ─────────────────────────────────

pub fn system_prompt(details: &ProjectDetails) -> String {
    format!(
        "You are an AI audit assistant working on the following engagement:\n\
         - Client: {}\n\
         - Engagement reference: {}\n\
         - Audit type: {}\n\
         - Applicable standards: {}\n\
         - Scope: {}\n\
         - Materiality threshold: {}\n\
         - Risk rating framework: {}\n\n\
         Provide concise, professional responses grounded in audit best practice.",
        details.client,
        details.engagement_ref,
        details.audit_type,
        if details.standards.is_empty() {
            "None specified".to_string()
        } else {
            details.standards.join(", ")
        },
        details.scope,
        details.materiality,
        details.risk_framework,
    )
}
