use super::types::RunResult;

/// Parse the raw JSON results from run_dsl_script into a display-friendly struct.
pub fn parse_run_result(results: &[serde_json::Value], dt_ms: f64) -> RunResult {
    // Prefer the first assert; fall back to value / sample / error.
    let first_assert = results.iter().find(|r| r["kind"] == "assert");
    let first_error  = results.iter().find(|r| r["kind"] == "error");

    if let Some(r) = first_assert {
        let passed = r["passed"].as_bool().unwrap_or(false);
        let failed_count = results.iter()
            .filter(|r| r["kind"] == "assert" && r["passed"] == false)
            .count();
        return RunResult {
            expr_type: "EXCEPTIONS".to_string(),
            expected:  "0".to_string(),
            actual:    failed_count.to_string(),
            passed,
            duration_ms: dt_ms,
        };
    }
    if let Some(r) = first_error {
        return RunResult {
            expr_type: "ERROR".to_string(),
            expected:  "—".to_string(),
            actual:    r["error"].as_str().unwrap_or("unknown error").to_string(),
            passed:    false,
            duration_ms: dt_ms,
        };
    }
    // sample or value
    if let Some(r) = results.first() {
        if r["kind"] == "sample" {
            let pop = r["population_size"].as_u64().unwrap_or(0);
            let sel = r["selected_count"].as_u64().unwrap_or(0);
            return RunResult {
                expr_type: "SAMPLE".to_string(),
                expected:  pop.to_string(),
                actual:    sel.to_string(),
                passed:    true,
                duration_ms: dt_ms,
            };
        }
        if r["kind"] == "value" {
            return RunResult {
                expr_type: "MATH".to_string(),
                expected:  "—".to_string(),
                actual:    r["value"].to_string(),
                passed:    true,
                duration_ms: dt_ms,
            };
        }
    }
    RunResult {
        expr_type: "RUN".to_string(),
        expected:  "—".to_string(),
        actual:    format!("{} result(s)", results.len()),
        passed:    true,
        duration_ms: dt_ms,
    }
}

/// Extract a DSL code block from an LLM response.
pub fn extract_dsl_code(text: &str) -> Option<String> {
    const KWS: &[&str] = &[
        "EXCEPTIONS", "RECONCILE", "SAMPLE", "TOTAL", "COUNT",
        "AVERAGE", "FLAG", "ASSERT", "MATH",
    ];

    // Try ``` code fences first.
    if let Some(start) = text.find("```") {
        let rest = &text[start + 3..];
        let rest = rest.find('\n').map(|n| &rest[n + 1..]).unwrap_or(rest);
        if let Some(end) = rest.find("```") {
            let code = rest[..end].trim().to_string();
            if !code.is_empty() && KWS.iter().any(|kw| code.contains(kw)) {
                return Some(code);
            }
        }
    }

    // Otherwise collect lines starting with a DSL keyword (plus continuation lines).
    let mut out: Vec<String> = vec![];
    let mut in_block = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if KWS.iter().any(|kw| trimmed.starts_with(kw)) {
            in_block = true;
        }
        if in_block {
            if trimmed.is_empty() && !out.is_empty() {
                break;
            }
            out.push(line.to_string());
        }
    }
    if out.is_empty() { None } else { Some(out.join("\n").trim().to_string()) }
}
