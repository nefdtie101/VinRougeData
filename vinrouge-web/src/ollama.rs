/// Ollama helpers — re-exported from the main vinrouge library.
pub use vinrouge::ollama::{
    ask_ollama_json, ask_ollama_structured, ask_ollama_wasm, DEFAULT_MODEL as OLLAMA_DEFAULT_MODEL,
    DEFAULT_URL as OLLAMA_DEFAULT_URL,
};

// ── SOP chunking ──────────────────────────────────────────────────────────────

/// ~5 pages of typical SOP text (250 words/page × 6 chars/word ≈ 7 500 chars).
const CHUNK_CHARS: usize = 7_500;

/// Split `text` into chunks of at most `CHUNK_CHARS` chars, breaking at
/// paragraph boundaries so sentences are never split mid-thought.
fn chunk_sop(text: &str) -> Vec<String> {
    if text.len() <= CHUNK_CHARS {
        return vec![text.to_string()];
    }
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    for para in text.split("\n\n") {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }
        if !current.is_empty() && current.len() + para.len() + 2 > CHUNK_CHARS {
            chunks.push(current.trim().to_string());
            current = String::new();
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(para);
    }
    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }
    if chunks.is_empty() {
        chunks.push(text.to_string());
    }
    chunks
}

/// Merge normalised audit-plan JSON strings produced from multiple SOP chunks.
/// Processes with the same name (case-insensitive) are merged; control_refs
/// are renumbered globally C-1, C-2, C-3 … across the whole merged plan.
fn merge_plans(plans: &[String]) -> Result<String, String> {
    // Preserve insertion order with a separate key list.
    let mut order: Vec<String> = Vec::new();
    let mut map: std::collections::HashMap<
        String,
        (String, String, Vec<serde_json::Value>),
    > = std::collections::HashMap::new();
    let mut industry = String::new();
    let mut framework = String::new();

    for plan in plans {
        let v: serde_json::Value =
            serde_json::from_str(plan).map_err(|e| format!("Merge parse error: {e}"))?;

        if industry.is_empty() {
            industry = v["industry"].as_str().unwrap_or("").to_string();
        }
        if framework.is_empty() {
            framework = v["regulatory_framework"].as_str().unwrap_or("").to_string();
        }

        for proc in v["processes"].as_array().into_iter().flatten() {
            let name = proc["process_name"].as_str().unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            let key = name.to_lowercase();
            let desc = proc["description"].as_str().unwrap_or("").to_string();
            let ctrls = proc["controls"].as_array().cloned().unwrap_or_default();

            if let Some(entry) = map.get_mut(&key) {
                entry.2.extend(ctrls);
            } else {
                order.push(key.clone());
                map.insert(key, (name, desc, ctrls));
            }
        }
    }

    let mut counter = 1usize;
    let mut processes: Vec<serde_json::Value> = Vec::new();
    for key in &order {
        if let Some((name, desc, ctrls)) = map.remove(key) {
            let renumbered: Vec<serde_json::Value> = ctrls
                .into_iter()
                .map(|mut c| {
                    if let Some(obj) = c.as_object_mut() {
                        obj.insert(
                            "control_ref".into(),
                            serde_json::Value::String(format!("C-{counter}")),
                        );
                        counter += 1;
                    }
                    c
                })
                .collect();
            processes.push(serde_json::json!({
                "process_name": name,
                "description":  desc,
                "controls":     renumbered
            }));
        }
    }

    serde_json::to_string(&serde_json::json!({
        "industry":             industry,
        "regulatory_framework": framework,
        "processes":            processes
    }))
    .map_err(|e| format!("Merge serialise error: {e}"))
}

// ── PBC list generation ───────────────────────────────────────────────────────

/// Number of audit processes sent to the model per PBC generation request.
/// 3 processes keeps each prompt comfortably under the context window while
/// giving the model enough plan context to produce accurate data requests.
const PBC_CHUNK_PROCESSES: usize = 3;

/// Format a slice of processes as the plain-text audit plan the model expects.
fn format_plan_chunk(processes: &[crate::types::AuditProcessWithControls]) -> String {
    let mut s = String::new();
    for p in processes {
        s.push_str(&format!("Process: {}\n", p.process_name));
        s.push_str(&format!("Description: {}\n", p.description));
        for c in &p.controls {
            s.push_str(&format!("  Control {}: {}\n", c.control_ref, c.control_objective));
            s.push_str(&format!("    How it operates: {}\n", c.control_description));
            s.push_str(&format!("    Test procedure: {}\n", c.test_procedure));
            s.push_str(&format!("    Risk: {}\n", c.risk_level));
        }
        s.push('\n');
    }
    s
}

/// Generate PBC (Provided-By-Client) data requests for an audit plan.
///
/// The plan is split into chunks of [`PBC_CHUNK_PROCESSES`] processes each.
/// Every chunk is sent to Ollama independently; the resulting `items` arrays
/// are concatenated and returned as a flat `Vec<serde_json::Value>`.
///
/// `on_progress` receives a human-readable status string before each request.
pub async fn ask_pbc_list(
    base_url: &str,
    model: &str,
    plan: &[crate::types::AuditProcessWithControls],
    on_progress: impl Fn(String),
) -> Result<Vec<serde_json::Value>, String> {
    use vinrouge::audit_prompts::{pbc_list_schema, GENERATE_PBC};

    let chunks: Vec<&[crate::types::AuditProcessWithControls]> =
        plan.chunks(PBC_CHUNK_PROCESSES).collect();
    let total = chunks.len();
    let schema = pbc_list_schema();
    let mut all_items: Vec<serde_json::Value> = Vec::new();

    for (i, chunk) in chunks.iter().enumerate() {
        if total > 1 {
            on_progress(format!(
                "Generating data requests: processes {} – {} of {}…",
                i * PBC_CHUNK_PROCESSES + 1,
                (i * PBC_CHUNK_PROCESSES + chunk.len()).min(plan.len()),
                plan.len()
            ));
        }

        let plan_text = format_plan_chunk(chunk);
        let prompt = format!("{GENERATE_PBC}\n\n{plan_text}");

        let raw = ask_ollama_structured(base_url, model, &prompt, schema.clone()).await?;

        let extract_items = |s: &str| -> Option<Vec<serde_json::Value>> {
            serde_json::from_str::<serde_json::Value>(s)
                .ok()?
                ["items"]
                .as_array()
                .filter(|a| !a.is_empty())
                .cloned()
        };

        if let Some(items) = extract_items(&raw) {
            all_items.extend(items);
            continue;
        }

        // One retry per chunk.
        if total > 1 {
            on_progress(format!("Retrying chunk {} of {}…", i + 1, total));
        }
        let retry = format!(
            "CRITICAL: Your response was missing the required 'items' array or was invalid. \
             Return ONLY valid JSON with an 'items' array of data request objects.\
             \n\nOriginal request:\n{prompt}"
        );
        let raw2 = ask_ollama_structured(base_url, model, &retry, schema.clone()).await?;
        if let Some(items) = extract_items(&raw2) {
            all_items.extend(items);
        }
    }

    if all_items.is_empty() {
        return Err("No PBC items were generated.".to_string());
    }
    Ok(all_items)
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Fetch an audit plan from Ollama.
///
/// If the SOP text is longer than ~5 pages it is automatically split into
/// chunks, each analysed independently, and the results merged into a single
/// plan with globally unique control refs.
///
/// `on_progress` is called with a human-readable status string before each
/// network request so the UI can show chunk progress.
pub async fn ask_audit_plan(
    base_url: &str,
    model: &str,
    sop_text: &str,
    on_progress: impl Fn(String),
) -> Result<String, String> {
    use vinrouge::audit_prompts::{audit_plan_schema, normalize_audit_plan_json, ANALYZE_SOP};

    let chunks = chunk_sop(sop_text);
    let total = chunks.len();
    let schema = audit_plan_schema();
    let mut results: Vec<String> = Vec::new();

    for (i, chunk) in chunks.iter().enumerate() {
        if total > 1 {
            on_progress(format!("Analysing section {} of {}…", i + 1, total));
        }

        // For multi-chunk runs, tell the model it is seeing one section of many
        // so it does not try to invent content it hasn't read.
        let prompt = if total > 1 {
            format!(
                "{ANALYZE_SOP}\
                 \n[Document is split into {total} sections for processing. \
                 This is section {} of {total}. Analyse ONLY the content below — \
                 do not fabricate content for sections not shown. \
                 Sections are merged after all have been processed.]\
                 \n\n{}",
                i + 1,
                chunk
            )
        } else {
            format!("{ANALYZE_SOP}\n\n{chunk}")
        };

        let raw = ask_ollama_structured(base_url, model, &prompt, schema.clone()).await?;

        let is_valid = |s: &str| -> bool {
            normalize_audit_plan_json(s)
                .ok()
                .and_then(|n| serde_json::from_str::<serde_json::Value>(&n).ok())
                .and_then(|v| v["processes"].as_array().map(|a| !a.is_empty()))
                .unwrap_or(false)
        };

        if is_valid(&raw) {
            results.push(normalize_audit_plan_json(&raw).unwrap());
            continue;
        }

        // One retry per chunk.
        if total > 1 {
            on_progress(format!("Retrying section {} of {}…", i + 1, total));
        }
        let retry = format!(
            "CRITICAL: Your previous response was missing the required 'processes' array \
             or was structurally invalid. Return ONLY valid JSON with a 'processes' array.\
             \n\nOriginal request:\n{prompt}"
        );
        let raw2 = ask_ollama_structured(base_url, model, &retry, schema.clone()).await?;
        if let Ok(n) = normalize_audit_plan_json(&raw2) {
            results.push(n);
        }
    }

    if results.is_empty() {
        return Err("No valid audit plan sections were generated.".to_string());
    }
    if results.len() == 1 {
        return Ok(results.remove(0));
    }

    on_progress("Merging sections…".to_string());
    merge_plans(&results)
}

// ── Column mapping ────────────────────────────────────────────────────────────

/// Send file headers and PBC fields to Ollama and return a mapping.
///
/// Both arrays are serialised to JSON strings and appended to the MAP_COLUMNS
/// prompt. The model returns `{"mappings":[{"source":"...","target":"..."}]}`.
/// Falls back gracefully: returns `Err` so the caller can use `normalize_map`.
pub async fn ask_column_mapping(
    base_url: &str,
    model: &str,
    headers: &[String],
    pbc_groups: &[crate::types::PbcGroup],
) -> Result<Vec<(String, String)>, String> {
    use leptos::logging::log;
    use vinrouge::audit_prompts::{map_columns_schema, MAP_COLUMNS};

    log!("[column_mapping] starting — {} headers, {} pbc groups", headers.len(), pbc_groups.len());

    // Build valid field set first — used both in the prompt and for post-filtering.
    let valid_fields: std::collections::HashSet<String> = pbc_groups
        .iter()
        .flat_map(|g| g.items.iter())
        .flat_map(|i| i.fields.iter().cloned())
        .collect();

    // Serialise headers to a JSON array string.
    let headers_json = serde_json::to_string(headers).map_err(|e| e.to_string())?;
    log!("[column_mapping] headers JSON: {}", headers_json);

    // Build a compact PBC fields list: name + control_ref + fields per item.
    let pbc_items: Vec<serde_json::Value> = pbc_groups
        .iter()
        .flat_map(|g| g.items.iter())
        .map(|item| {
            serde_json::json!({
                "control_ref": item.control_ref,
                "name":        item.name,
                "purpose":     item.purpose,
                "fields":      item.fields
            })
        })
        .collect();
    let pbc_json = serde_json::to_string(&pbc_items).map_err(|e| e.to_string())?;
    log!("[column_mapping] pbc JSON: {}", pbc_json);

    // Flat sorted list of allowed target fields to inject into the prompt.
    let mut allowed: Vec<String> = valid_fields.iter().cloned().collect();
    allowed.sort();
    let allowed_json = serde_json::to_string(&allowed).map_err(|e| e.to_string())?;
    log!("[column_mapping] allowed fields: {}", allowed_json);

    let prompt = format!(
        "{MAP_COLUMNS}\
         ALLOWED FIELDS:\n{allowed_json}\n\n\
         SOURCE COLUMNS:\n{headers_json}\n\n\
         DATA REQUESTS (context only):\n{pbc_json}"
    );
    log!("[column_mapping] sending prompt ({} chars) to {}  model={}", prompt.len(), base_url, model);

    let raw = match ask_ollama_structured(base_url, model, &prompt, map_columns_schema()).await {
        Ok(r) => {
            log!("[column_mapping] raw response: {}", r);
            r
        }
        Err(e) => {
            log!("[column_mapping] ollama error: {}", e);
            return Err(e);
        }
    };

    // Parse {"mappings":[{"source":"...","target":"..."}]}
    let v: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            log!("[column_mapping] JSON parse error: {}", e);
            return Err(format!("Parse error: {e}"));
        }
    };

    let arr = match v["mappings"].as_array() {
        Some(a) => a,
        None => {
            log!("[column_mapping] no 'mappings' key in response — full value: {:?}", v);
            return Err("No mappings array".to_string());
        }
    };

    let result: Vec<(String, String)> = arr
        .iter()
        .filter_map(|m| {
            let src = m["source"].as_str()?.to_string();
            let tgt = m["target"].as_str().unwrap_or("").to_string();
            // Discard targets the LLM invented that aren't real PBC fields.
            let tgt = if tgt.is_empty() || valid_fields.contains(&tgt) {
                tgt
            } else {
                log!("[column_mapping] rejected hallucinated target '{}' for source '{}'", tgt, src);
                String::new()
            };
            Some((src, tgt))
        })
        .collect();

    log!("[column_mapping] mapped {} columns: {:?}", result.len(), result);
    Ok(result)
}

use crate::types::AnalysisResult;

// ── Analysis summary for Ollama context ───────────────────────────────────────

pub fn build_web_summary(result: &AnalysisResult) -> String {
    let mut s = String::new();

    s.push_str(&format!("Tables ({}):\n", result.tables.len()));
    for t in &result.tables {
        s.push_str(&format!(
            "  - {} ({} columns, ~{} rows)\n",
            t.name,
            t.columns.len(),
            t.row_count.unwrap_or(0)
        ));
        for c in &t.columns {
            s.push_str(&format!("      {}: {:?}\n", c.name, c.data_type));
        }
    }

    if !result.relationships.is_empty() {
        s.push_str(&format!(
            "\nRelationships ({}):\n",
            result.relationships.len()
        ));
        for r in &result.relationships {
            s.push_str(&format!(
                "  - {}.{} -> {}.{}\n",
                r.from_table, r.from_column, r.to_table, r.to_column
            ));
        }
    }

    if !result.workflows.is_empty() {
        s.push_str(&format!("\nWorkflows ({}):\n", result.workflows.len()));
        for w in &result.workflows {
            s.push_str(&format!("  - {:?}: {}\n", w.workflow_type, w.description));
        }
    }

    s
}
