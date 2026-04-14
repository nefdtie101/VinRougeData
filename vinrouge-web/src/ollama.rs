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
/// How many source columns to send per LLM call.
/// Keeps each prompt well within the model's context window.
const COLUMN_CHUNK_SIZE: usize = 20;

/// Map source file headers to PBC fields using the LLM.
///
/// Columns are processed in chunks of [`COLUMN_CHUNK_SIZE`] so no single
/// prompt overwhelms the model — the same pattern used by `ask_pbc_list` and
/// `ask_audit_plan`.  The full PBC context is included in every chunk so the
/// model has enough information to reason semantically regardless of which
/// columns it is currently looking at.
pub async fn ask_column_mapping(
    base_url: &str,
    model: &str,
    headers: &[String],
    pbc_groups: &[crate::types::PbcGroup],
) -> Result<Vec<(String, String)>, String> {
    use leptos::logging::log;
    use vinrouge::audit_prompts::{map_columns_schema, MAP_COLUMNS};

    log!("[column_mapping] starting — {} headers, {} pbc groups", headers.len(), pbc_groups.len());

    // Valid field set — used to reject hallucinated targets after each call.
    let valid_fields: std::collections::HashSet<String> = pbc_groups
        .iter()
        .flat_map(|g| g.items.iter())
        .flat_map(|i| i.fields.iter().cloned())
        .collect();

    // Rich PBC context sent with every chunk so the model can reason about
    // semantic meaning (e.g. "Loading Applied %" → "premium_loading").
    // Items without fields are skipped — they carry no mappable targets.
    let pbc_context: Vec<serde_json::Value> = pbc_groups
        .iter()
        .flat_map(|g| {
            g.items.iter().filter(|i| !i.fields.is_empty()).map(|item| {
                serde_json::json!({
                    "control_ref":    item.control_ref,
                    "request_name":   item.name,
                    "audit_purpose":  item.purpose,
                    "required_fields": item.fields
                })
            })
        })
        .collect();
    let pbc_json = serde_json::to_string(&pbc_context).map_err(|e| e.to_string())?;
    log!("[column_mapping] pbc context: {} items", pbc_context.len());

    // Flat sorted list of allowed target names — sent in every chunk prompt so
    // the model can copy them character-for-character without digging into JSON.
    let mut allowed: Vec<String> = valid_fields.iter().cloned().collect();
    allowed.sort();
    let allowed_json = serde_json::to_string(&allowed).map_err(|e| e.to_string())?;

    let chunks: Vec<&[String]> = headers.chunks(COLUMN_CHUNK_SIZE).collect();
    let total = chunks.len();
    let mut all_mappings: Vec<(String, String)> = Vec::new();
    // Track used targets across chunks so each subsequent chunk knows what's already claimed.
    let mut used_targets: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (i, chunk) in chunks.iter().enumerate() {
        log!(
            "[column_mapping] chunk {}/{} — {} columns",
            i + 1, total, chunk.len()
        );

        // Number the source columns so the model must process them in order.
        let numbered: Vec<String> = chunk.iter().enumerate()
            .map(|(j, h)| format!("{}. {}", j + 1, h))
            .collect();
        let chunk_json = serde_json::to_string(&numbered).map_err(|e| e.to_string())?;

        // Tell the model which targets are already taken by previous chunks.
        let mut taken: Vec<String> = used_targets.iter().cloned().collect();
        taken.sort();
        let taken_json = serde_json::to_string(&taken).map_err(|e| e.to_string())?;

        let prompt = format!(
            "{MAP_COLUMNS}\
             ALLOWED FIELDS:\n{allowed_json}\n\n\
             AUDIT DATA REQUESTS (context):\n{pbc_json}\n\n\
             ALREADY ASSIGNED (do NOT use these targets — they are taken):\n{taken_json}\n\n\
             SOURCE COLUMNS — output EXACTLY {} entries, one per column:\n{chunk_json}",
            chunk.len()
        );
        log!("[column_mapping] prompt {} chars", prompt.len());

        let raw = match ask_ollama_structured(base_url, model, &prompt, map_columns_schema()).await {
            Ok(r) => {
                log!("[column_mapping] chunk {} response: {}", i + 1, r);
                r
            }
            Err(e) => {
                log!("[column_mapping] chunk {} ollama error: {}", i + 1, e);
                // On error, emit unmapped entries for every column in this chunk.
                for col in chunk.iter() {
                    all_mappings.push((col.clone(), String::new()));
                }
                continue;
            }
        };

        let v: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                log!("[column_mapping] chunk {} parse error: {}", i + 1, e);
                for col in chunk.iter() {
                    all_mappings.push((col.clone(), String::new()));
                }
                continue;
            }
        };

        // Build a set of the actual source column names in this chunk for validation.
        let chunk_set: std::collections::HashSet<&str> =
            chunk.iter().map(|s| s.as_str()).collect();

        if let Some(arr) = v["mappings"].as_array() {
            for m in arr {
                let src = match m["source"].as_str() {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                // Reject sources that aren't in this chunk (model hallucinated column names).
                if !chunk_set.contains(src.as_str()) {
                    log!("[column_mapping] rejected unknown source '{}' in chunk {}", src, i + 1);
                    continue;
                }
                let tgt = m["target"].as_str().unwrap_or("").to_string();
                let tgt = if tgt.is_empty() {
                    tgt
                } else if !valid_fields.contains(&tgt) {
                    log!("[column_mapping] rejected hallucinated target '{}' for '{}'", tgt, src);
                    String::new()
                } else if used_targets.contains(&tgt) {
                    log!("[column_mapping] demoted '{}' → '{}': target already taken", src, tgt);
                    String::new()
                } else {
                    tgt
                };
                if !tgt.is_empty() {
                    used_targets.insert(tgt.clone());
                }
                all_mappings.push((src, tgt));
            }
        } else {
            log!("[column_mapping] chunk {} missing mappings key", i + 1);
        }
    }

    log!("[column_mapping] done — {} raw mappings", all_mappings.len());

    // Final dedup: remove any duplicate source entries the model may have produced within a chunk.
    // Target uniqueness is already guaranteed by `used_targets` maintained during the loop.
    let mut seen_sources: std::collections::HashSet<String> = std::collections::HashSet::new();
    let all_mappings: Vec<(String, String)> = all_mappings
        .into_iter()
        .filter_map(|(src, tgt)| {
            if seen_sources.contains(&src) {
                log!("[column_mapping] dropped duplicate source '{}'", src);
                return None;
            }
            seen_sources.insert(src.clone());
            Some((src, tgt))
        })
        .collect();

    log!("[column_mapping] {} mappings after dedup", all_mappings.len());
    if all_mappings.is_empty() {
        return Err("No mappings returned by LLM".to_string());
    }
    Ok(all_mappings)
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
