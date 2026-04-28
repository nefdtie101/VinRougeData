use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::ollama::{ask_audit_report, OLLAMA_DEFAULT_MODEL, OLLAMA_DEFAULT_URL};
use crate::types::{AuditProcessWithControls, DslScript, TestResult};

/// Kick off report generation: serialise results, call the AI, update signals.
pub fn do_generate_report(
    scripts: Vec<DslScript>,
    results: Vec<TestResult>,
    plan: Vec<AuditProcessWithControls>,
    report: RwSignal<Option<serde_json::Value>>,
    generating: RwSignal<bool>,
    status: RwSignal<String>,
) {
    let summary = build_test_summary(&scripts, &results, &plan);
    generating.set(true);
    report.set(None);
    status.set(String::new());

    spawn_local(async move {
        match ask_audit_report(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &summary).await {
            Ok(raw) => match serde_json::from_str::<serde_json::Value>(&raw) {
                Ok(v)  => report.set(Some(v)),
                Err(_) => status.set("AI returned an invalid report format.".to_string()),
            },
            Err(e) => status.set(format!("Report generation failed: {e}")),
        }
        generating.set(false);
    });
}

/// Serialise scripts + results into compact text for the AI report prompt.
pub fn build_test_summary(
    scripts: &[DslScript],
    results: &[TestResult],
    plan: &[AuditProcessWithControls],
) -> String {
    let plan_map: std::collections::HashMap<&str, (&str, &str)> = plan.iter()
        .flat_map(|p| p.controls.iter())
        .map(|c| (c.control_ref.as_str(), (c.control_objective.as_str(), c.risk_level.as_str())))
        .collect();

    let mut failed_lines = String::new();
    let mut passed_refs: Vec<String> = Vec::new();

    for script in scripts {
        let Some(res) = results.iter().find(|r| r.script_id == script.id) else {
            continue;
        };

        if res.failed_count == 0 && res.error_count == 0 {
            passed_refs.push(script.control_ref.clone());
            continue;
        }

        let (obj, risk) = plan_map.get(script.control_ref.as_str())
            .copied()
            .unwrap_or(("", "Medium"));

        failed_lines.push_str(&format!(
            "\nControl {} ({} risk): {}\n",
            script.control_ref, risk, script.label
        ));
        if !obj.is_empty() {
            failed_lines.push_str(&format!("  Objective: {obj}\n"));
        }
        failed_lines.push_str(&format!(
            "  Failed assertions: {} · Errors: {} · Passed: {}\n",
            res.failed_count, res.error_count, res.passed_count
        ));

        for stmt in &res.results {
            match stmt["kind"].as_str().unwrap_or("") {
                "assert" if !stmt["passed"].as_bool().unwrap_or(true) => {
                    let label = stmt["label"].as_str().unwrap_or("unnamed");
                    let lhs   = stmt["lhs_value"].as_str().unwrap_or("?");
                    let op    = stmt["op"].as_str().unwrap_or("=");
                    let rhs   = stmt["rhs_value"].as_str().unwrap_or("?");
                    failed_lines.push_str(&format!(
                        "    FAIL: \"{label}\" → actual {lhs} {op} {rhs}\n"
                    ));
                }
                "error" => {
                    let err = stmt["error"].as_str().unwrap_or("unknown error");
                    failed_lines.push_str(&format!("    ERROR: {err}\n"));
                }
                _ => {}
            }
        }
    }

    let mut out = String::new();
    if !failed_lines.is_empty() {
        out.push_str("FAILED CONTROLS:\n");
        out.push_str(&failed_lines);
    }
    if !passed_refs.is_empty() {
        out.push_str(&format!("\nPASSED CONTROLS: {}\n", passed_refs.join(", ")));
    }
    if out.is_empty() {
        out.push_str("No test results available.\n");
    }
    out
}
