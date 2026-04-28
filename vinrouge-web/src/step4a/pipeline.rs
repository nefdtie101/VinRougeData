use std::collections::HashMap;
use leptos::prelude::*;

use crate::ipc::{tauri_invoke, tauri_invoke_args};
use crate::ollama::{ask_ollama_structured, OLLAMA_DEFAULT_MODEL, OLLAMA_DEFAULT_URL};
use crate::types::{AuditProcessWithControls, DslScript, PbcGroup, RelCandidate, SessionSchema};
use vinrouge::audit_prompts::{dsl_script_schema, GENERATE_DSL};
use vinrouge::dsl::{parse as dsl_parse, resolve, Schema};
use super::types::{Phase, ScriptStatus, ScriptState};
use super::prompts::{build_schema_section, build_table_names_section, build_plan_section, build_example_section};

// Note: relationship detection is handled directly in step4a/view.rs.
// This pipeline only contains do_load_or_generate (used by step4b).

pub async fn do_load_or_generate(
    audit_plan:      RwSignal<Vec<AuditProcessWithControls>>,
    phase:           RwSignal<Phase>,
    schemas:         RwSignal<Vec<SessionSchema>>,
    scripts:         RwSignal<Vec<DslScript>>,
    script_states:   RwSignal<HashMap<String, ScriptState>>,
    progress_msg:    RwSignal<String>,
    status:          RwSignal<String>,
    selected_id:     RwSignal<Option<String>>,
    preview_cols:    RwSignal<Vec<String>>,
    preview_rows:    RwSignal<Vec<Vec<String>>>,
    preview_source:  RwSignal<String>,
    join_candidates: RwSignal<Vec<RelCandidate>>,
    accepted_joins:  RwSignal<Vec<bool>>,
    generate_new:    bool,
) {
    phase.set(Phase::Loading);
    progress_msg.set("Loading imported data…".to_string());

    // 1. Load session schemas
    let session_schemas: Vec<SessionSchema> =
        match tauri_invoke("get_session_schemas").await {
            Ok(s) => s,
            Err(e) => {
                phase.set(Phase::Error(format!("Could not load data: {e}")));
                return;
            }
        };
    if session_schemas.is_empty() {
        phase.set(Phase::Error(
            "No data imported yet. Go back to Step 4 and upload files.".to_string(),
        ));
        return;
    }
    schemas.set(session_schemas.clone());

    // 2. Pick which import to preview (prefer master if present)
    let master = session_schemas.iter().find(|s| s.source_type == "master");
    let non_master_count = session_schemas.iter().filter(|s| s.source_type != "master").count();

    // 2a. If no master exists AND 2+ raw imports: run relationship detection, pause for review
    if master.is_none() && non_master_count >= 2 {
        progress_msg.set("Detecting data relationships…".to_string());
        match tauri_invoke::<Vec<RelCandidate>>("detect_data_relationships").await {
            Ok(candidates) if !candidates.is_empty() => {
                let n = candidates.len();
                join_candidates.set(candidates);
                accepted_joins.set(vec![true; n]); // accept all by default
                phase.set(Phase::RelationshipReview);
                return; // wait for user to confirm / skip
            }
            _ => {
                // No auto-detected relationships → still show the panel so the user
                // can define joins manually before building a master record.
                phase.set(Phase::RelationshipReview);
                return;
            }
        }
    }

    // 2b. Preview data is loaded by the tab-selection Effect in the view,
    //     not here, to avoid a race where the pipeline overwrites a tab the
    //     user has already clicked.  We just make sure selected_preview_id
    //     is cleared so the Effect fires when schemas are set below.

    // 3. Load PBC context
    let pbc_groups: Vec<PbcGroup> =
        tauri_invoke("list_pbc_groups").await.unwrap_or_default();

    // 4. Try to load existing scripts from database (unless forcing regeneration)
    if !generate_new {
        progress_msg.set("Loading existing DSL scripts…".to_string());
        match tauri_invoke::<Vec<DslScript>>("list_dsl_scripts").await {
            Ok(existing_scripts) if !existing_scripts.is_empty() => {
                // Build script states from existing scripts
                let mut new_states: HashMap<String, ScriptState> = HashMap::new();
                for script in &existing_scripts {
                    new_states.insert(script.id.clone(), ScriptState {
                        status: ScriptStatus::Generated,
                        text:   script.script_text.clone(),
                    });
                }

                // Set state and select first script
                let first_id = existing_scripts.first().map(|s| s.id.clone());
                scripts.set(existing_scripts);
                script_states.set(new_states);
                selected_id.set(first_id);
                phase.set(Phase::Review);
                return; // Successfully loaded existing scripts, done!
            }
            _ => {
                // No existing scripts or error loading - proceed to generation
            }
        }
    }

    // 5. Clear previous scripts and generate new ones
    let _ = tauri_invoke::<()>("clear_dsl_scripts").await;
    phase.set(Phase::Generating);
    progress_msg.set("Generating DSL algorithms via AI…".to_string());

    let schema_section      = build_schema_section(&session_schemas);
    let table_names_section = build_table_names_section(&session_schemas);
    let example_section     = build_example_section(&session_schemas);
    let plan_section        = build_plan_section(&audit_plan.get_untracked(), &pbc_groups, &session_schemas);
    let prompt = format!(
        "{GENERATE_DSL}\
         AVAILABLE DATA:\n{schema_section}\
         {table_names_section}\
         {example_section}\
         AUDIT CONTROLS TO TEST:\n{plan_section}\n\
         Return ONLY a JSON object: \
         {{\"scripts\": [{{\"control_ref\": \"C-1\", \"label\": \"Test label\", \
         \"script\": \"DSL code here\"}}]}}"
    );

    let ollama_schema = dsl_script_schema();
    let raw = match ask_ollama_structured(
        OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &prompt, ollama_schema.clone()
    ).await {
        Ok(r) => r,
        Err(e) => {
            phase.set(Phase::Error(format!("AI generation failed: {e}")));
            return;
        }
    };

    // 6. Parse JSON
    let mut script_dtos: Vec<(String, String, String)> = parse_script_json(&raw);

    if script_dtos.is_empty() {
        phase.set(Phase::Error("AI did not generate any valid DSL scripts.".to_string()));
        return;
    }

    // 7. Validate + deterministically repair invented table names, then AI-fix remaining issues.
    progress_msg.set("Validating DSL scripts…".to_string());
    let dsl_schema = build_dsl_schema(&session_schemas);

    // 7a. Syntax normalisation — fix common AI mistakes before any validation.
    for dto in script_dtos.iter_mut() {
        dto.2 = normalize_dsl_syntax(&dto.2);
    }

    // 7b. Deterministic repairs — no AI involved, runs before the AI fix pass.
    //
    //   Pass 1: unknown table names — replace invented table with best-scoring real table.
    //   Pass 2: wrong table for column — if col exists in another real table, move the ref.
    //           e.g. master_record.driver_age_band → c5_basepremium.driver_age_band
    //
    // Both passes repeat until stable (max 3 iterations) so chained fixes work.
    for dto in script_dtos.iter_mut() {
        for _ in 0..3 {
            let errs = validate_script(&dto.2, &dsl_schema);
            if errs.is_empty() { break; }

            let mut changed = false;

            if errs.iter().any(|e| e.starts_with("unknown table '")) {
                let repaired = repair_unknown_tables(&dto.2, &errs, &session_schemas);
                if repaired != dto.2 { dto.2 = repaired; changed = true; }
            }

            if errs.iter().any(|e| e.starts_with("unknown column '")) {
                let repaired = repair_column_in_wrong_table(&dto.2, &errs, &session_schemas);
                if repaired != dto.2 { dto.2 = repaired; changed = true; }
            }

            if !changed { break; }
        }
    }

    let broken: Vec<(String, String, String, String)> = script_dtos.iter()
        .filter_map(|(ctrl, label, script)| {
            let errs = validate_script(script, &dsl_schema);
            if errs.is_empty() { None } else {
                Some((ctrl.clone(), label.clone(), script.clone(), errs.join("; ")))
            }
        })
        .collect();

    if !broken.is_empty() {
        progress_msg.set(format!("Fixing {} broken script(s)…", broken.len()));
        let fix_prompt = build_fix_prompt(&broken, &schema_section, &table_names_section, &example_section, &session_schemas);
        if let Ok(fix_raw) = ask_ollama_structured(
            OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &fix_prompt, ollama_schema
        ).await {
            let fixed = parse_script_json(&fix_raw);
            // Replace broken scripts with fixed versions (matched by control_ref)
            let fixed_map: HashMap<String, (String, String)> = fixed.into_iter()
                .map(|(ctrl, label, script)| (ctrl, (label, script)))
                .collect();
            for dto in script_dtos.iter_mut() {
                if let Some((label, script)) = fixed_map.get(&dto.0) {
                    let normalised = normalize_dsl_syntax(script);
                    let re_errors = validate_script(&normalised, &dsl_schema);
                    if re_errors.is_empty() {
                        dto.1 = label.clone();
                        dto.2 = normalised;
                    }
                }
            }
        }
    }

    // 8. Save scripts to DB, build frontend state
    let plan         = audit_plan.get_untracked();
    let ctrl_lookup: HashMap<String, String> = plan.iter()
        .flat_map(|p| p.controls.iter())
        .map(|c| (c.control_ref.clone(), c.id.clone()))
        .collect();

    let mut saved: Vec<DslScript>              = vec![];
    let mut new_states: HashMap<String, ScriptState> = HashMap::new();

    for (control_ref, label, script_text) in &script_dtos {
        let control_id = ctrl_lookup.get(control_ref).cloned().unwrap_or_default();
        // Always normalise syntax before final validation — catches any bracket/!= issues
        // that slipped through earlier passes (e.g. introduced by the AI fix pass).
        let script_text = normalize_dsl_syntax(script_text);
        let final_errors = validate_script(&script_text, &dsl_schema);
        let (effective_script, script_status) = if final_errors.is_empty() {
            (script_text.clone(), ScriptStatus::Generated)
        } else {
            // Step 1: try to salvage by dropping lines with invalid column refs.
            let salvaged = drop_invalid_column_lines(&script_text, &final_errors);
            let salvage_errors = validate_script(&salvaged, &dsl_schema);

            if salvage_errors.is_empty() && !salvaged.trim().is_empty() {
                (salvaged, ScriptStatus::Generated)
            } else if salvaged.trim().is_empty() {
                // All statements were dropped — the required columns simply don't exist
                // in any uploaded file.  A nonsensical SAMPLE from an unrelated column
                // would produce misleading results, so emit a comment-only placeholder
                // instead that tells the auditor exactly what data is missing.
                let mut missing: Vec<String> = final_errors.iter()
                    .filter_map(|e| {
                        let col_rest = e.strip_prefix("unknown column '")?;
                        let col = col_rest.split('\'').next()?.to_string();
                        let exists_anywhere = session_schemas.iter()
                            .any(|s| s.columns.iter().any(|c| c.eq_ignore_ascii_case(&col)));
                        if exists_anywhere { None } else { Some(col) }
                    })
                    .collect();
                missing.sort();
                missing.dedup();

                let available_hint: String = session_schemas.iter()
                    .map(|s| format!("--   {} : {}", s.table_name, s.columns.join(", ")))
                    .collect::<Vec<_>>()
                    .join("\n");

                let missing_lines = if missing.is_empty() {
                    format!("-- Errors: {}", final_errors.join("; "))
                } else {
                    missing.iter()
                        .map(|c| format!("--   {c}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                };

                let placeholder = format!(
                    "-- Required columns not found in any uploaded data file:\n\
                     {missing_lines}\n\
                     --\n\
                     -- AVAILABLE DATA:\n\
                     {available_hint}\n\
                     --\n\
                     -- Upload the relevant data file, or rewrite this script\n\
                     -- using the available columns listed above."
                );
                (placeholder, ScriptStatus::ValidationError(final_errors.join("; ")))
            } else {
                // Some statements remain but still have errors (e.g. a parse error that
                // survived all repair passes). Fall back to a semantically-relevant SAMPLE.
                let fallback_col = fallback_sample_col(&final_errors, &script_text, label, &session_schemas);
                let placeholder = format!(
                    "-- Validation failed: {}\n-- Edit this script using the column names in AVAILABLE DATA.\nSAMPLE RANDOM FROM {} SIZE 10",
                    final_errors.join("; "),
                    fallback_col
                );
                (placeholder, ScriptStatus::ValidationError(final_errors.join("; ")))
            }
        };
        match tauri_invoke_args::<DslScript>(
            "save_dsl_script",
            serde_json::json!({
                "controlId":  control_id,
                "controlRef": control_ref,
                "label":      label,
                "scriptText": effective_script,
            }),
        ).await {
            Ok(s) => {
                new_states.insert(s.id.clone(), ScriptState {
                    status: script_status,
                    text:   effective_script.clone(),
                });
                saved.push(s);
            }
            Err(e) => {
                status.set(format!("Warning: could not save script for {control_ref}: {e}"));
            }
        }
    }

    // Auto-select first script and reset result/chat.
    let first_id = saved.first().map(|s| s.id.clone());
    scripts.set(saved);
    script_states.set(new_states);
    selected_id.set(first_id);
    phase.set(Phase::Review);
}

// ── DSL validation helpers ────────────────────────────────────────────────────

/// Build a resolver Schema from the session schemas.
fn build_dsl_schema(session_schemas: &[SessionSchema]) -> Schema {
    let mut s = Schema::new();
    for tbl in session_schemas {
        s.add_table(&tbl.table_name, tbl.columns.iter().map(String::as_str));
    }
    s
}

/// Parse a DSL script and resolve column references. Returns a list of error strings.
fn validate_script(script: &str, schema: &Schema) -> Vec<String> {
    match dsl_parse(script) {
        Err(e) => vec![format!("parse error: {}", e.message)],
        Ok(stmts) => resolve(&stmts, schema)
            .into_iter()
            .map(|e| e.to_string())
            .collect(),
    }
}

/// Extract (control_ref, label, script) triples from a raw JSON string.
fn parse_script_json(raw: &str) -> Vec<(String, String, String)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else { return vec![] };
    v["scripts"].as_array().cloned().unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let control_ref = item["control_ref"].as_str()?.to_string();
            let label       = item["label"].as_str().unwrap_or(&control_ref).to_string();
            let script      = item["script"].as_str()?.to_string();
            if script.trim().is_empty() { return None; }
            Some((control_ref, label, script))
        })
        .collect()
}

/// Build the one-shot correction prompt for scripts that failed validation.
/// Injects the exact available columns for each table referenced in a broken script
/// so the AI can pick a valid replacement rather than guessing.
fn build_fix_prompt(
    broken: &[(String, String, String, String)],
    schema_section: &str,
    table_names_section: &str,
    example_section: &str,
    schemas: &[crate::types::SessionSchema],
) -> String {
    let mut cases = String::new();
    for (ctrl, label, script, errs) in broken {
        // Collect tables referenced in the errors so we can supply targeted column lists.
        let mut hint_tables: Vec<String> = vec![];
        for err in errs.split(';') {
            let err = err.trim();
            if let Some(rest) = err.strip_prefix("unknown column: ") {
                // "table.column" — extract table prefix
                if let Some(dot) = rest.find('.') {
                    hint_tables.push(rest[..dot].to_string());
                }
            } else if let Some(rest) = err.strip_prefix("unknown table: ") {
                hint_tables.push(rest.trim().to_string());
            }
        }
        hint_tables.dedup();

        // Build a "columns in <table>" hint for each referenced table that exists.
        let mut col_hints = String::new();
        for tbl in &hint_tables {
            if let Some(s) = schemas.iter().find(|s| s.table_name.eq_ignore_ascii_case(tbl)) {
                col_hints.push_str(&format!(
                    "  Columns in {}: {}\n",
                    s.table_name,
                    s.columns.join(", ")
                ));
            } else {
                // Table doesn't exist — tell the AI to use a valid one instead.
                let valid = schemas.iter().map(|s| s.table_name.as_str()).collect::<Vec<_>>().join(", ");
                col_hints.push_str(&format!(
                    "  Table '{tbl}' does NOT exist. Use one of: {valid}\n"
                ));
            }
        }

        cases.push_str(&format!(
            "control_ref: {ctrl}\nlabel: {label}\nbroken script:\n{script}\n\
             errors: {errs}\n{col_hints}\n"
        ));
    }
    format!(
        "The following DSL scripts failed validation. Fix ONLY the errors listed.\n\
         CRITICAL RULES:\n\
         - Use ONLY the table and column names listed in VALID TABLE NAMES and AVAILABLE DATA.\n\
         - If a column you need does not exist, OMIT that ASSERT line entirely.\n\
         - If a table does not exist, replace the entire script with:\n\
           SAMPLE RANDOM FROM <any_valid_table>.<any_valid_col> SIZE 10\n\
         - Never use != — use <> instead.\n\
         - Always write table.column — never bare column names inside aggregates.\n\n\
         AVAILABLE DATA:\n{schema_section}\
         {table_names_section}\
         {example_section}\
         BROKEN SCRIPTS TO FIX:\n{cases}\
         Return ONLY a JSON object: \
         {{\"scripts\": [{{\"control_ref\": \"C-1\", \"label\": \"Test label\", \
         \"script\": \"fixed DSL here\"}}]}}"
    )
}

// ── Deterministic table-name repair ──────────────────────────────────────────

/// Replace every invented (unknown) table name in a DSL script with the closest
/// real table from the session schemas.  Uses keyword overlap + column-name
/// matching so that e.g. `premium_arrears` → `c9_premiumcollection` (which has
/// `grace_period_breached`) and `new_policyholders` → `c12_onboarding` (which
/// has `id_verified_flag`, `fica_officer`).
fn repair_unknown_tables(
    script: &str,
    errors: &[String],
    schemas: &[crate::types::SessionSchema],
) -> String {
    // Extract invented table names from error strings:
    // format: "unknown table 'foo' referenced in 'foo.bar'"
    let mut invented: Vec<String> = errors.iter()
        .filter_map(|e| {
            let rest = e.strip_prefix("unknown table '")?;
            let end  = rest.find('\'')?;
            Some(rest[..end].to_string())
        })
        .collect();
    invented.sort();
    invented.dedup();

    let mut result = script.to_string();
    for bad_table in &invented {
        if let Some(good_table) = best_table_match(bad_table, &result, schemas) {
            result = replace_table_in_script(&result, bad_table, good_table);
        }
    }
    result
}

/// Score every real table against the invented name and return the best match.
///
/// Scoring:
///   +10  each domain keyword (≥4 chars, non-generic) from invented name found in real table name
///   + 5  each domain keyword found in any real table column name
///   + 6  each domain-specific column the script uses with the invented table that exists in candidate
///   + 1  each generic/id column match (policy_id, id, etc.) — low weight to avoid false boosts
///
/// Generic columns (id, policy_id, policyholder_id, name, date, ref) are down-weighted
/// because they appear in nearly every table and would bias the score toward the wrong winner.
fn best_table_match<'a>(
    invented: &str,
    script:   &str,
    schemas:  &'a [crate::types::SessionSchema],
) -> Option<&'a str> {
    // Columns that are too common to use as strong signals
    const GENERIC_COLS: &[&str] = &[
        "id", "policy_id", "policyholder_id", "name", "date", "ref",
        "record_id", "row_id", "entry_id",
    ];

    let inv_lower = invented.to_lowercase();
    // Only use keywords ≥4 chars to filter noise ("the", "and", "for", etc.)
    let keywords: Vec<&str> = inv_lower
        .split('_')
        .filter(|w| w.len() >= 4)
        .collect();

    let used_cols = cols_used_with(script, invented);

    // Also extract keywords from the column names the AI tried to use.
    // e.g. "grace_period_days" → ["grace", "period", "days"]
    // This lets us match c9_premiumcollection.grace_period_breached even when the
    // column name itself doesn't exist.
    let col_keywords: Vec<String> = used_cols.iter()
        .flat_map(|c| c.split('_').filter(|w| w.len() >= 4).map(|w| w.to_lowercase()).collect::<Vec<_>>())
        .collect();

    let mut best: Option<(&str, i32)> = None;

    for schema in schemas {
        let tbl_lower = schema.table_name.to_lowercase();
        let mut score = 0i32;

        // Keyword (from invented table name) → table name / column name match
        for kw in keywords.iter().copied() {
            if tbl_lower.contains(kw) {
                score += 10;
            }
            if schema.columns.iter().any(|c| c.to_lowercase().contains(kw)) {
                score += 5;
            }
        }

        // Keyword (from AI's attempted column names) → real column name match
        // This catches "grace_period_days" matching "grace_period_breached"
        for kw in &col_keywords {
            if schema.columns.iter().any(|c| c.to_lowercase().contains(kw.as_str())) {
                score += 4;
            }
        }

        // Exact column overlap: domain columns score higher than generic ones
        for col in &used_cols {
            let is_generic = GENERIC_COLS.iter().any(|g| col.eq_ignore_ascii_case(g));
            if schema.columns.iter().any(|c| c.eq_ignore_ascii_case(col)) {
                score += if is_generic { 1 } else { 6 };
            }
        }

        if score > 0 && best.map(|(_, s)| score > s).unwrap_or(true) {
            best = Some((&schema.table_name, score));
        }
    }

    best.map(|(name, _)| name)
}

/// Return the column names used with a given table prefix in the script text.
fn cols_used_with(script: &str, table: &str) -> Vec<String> {
    let prefix = format!("{}.", table.to_lowercase());
    let lower   = script.to_lowercase();
    let mut cols = Vec::new();
    let mut pos  = 0;

    while pos < lower.len() {
        match lower[pos..].find(&prefix) {
            None => break,
            Some(rel) => {
                let col_start = pos + rel + prefix.len();
                let col_len = script[col_start..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .count();
                if col_len > 0 {
                    cols.push(script[col_start..col_start + col_len].to_string());
                }
                pos = col_start;
            }
        }
    }

    cols.sort();
    cols.dedup();
    cols
}

/// Replace every occurrence of `old_table.` with `new_table.` in the script,
/// case-insensitively on the old name.
fn replace_table_in_script(script: &str, old_table: &str, new_table: &str) -> String {
    let old_prefix = format!("{}.", old_table.to_lowercase());
    let new_prefix = format!("{}.", new_table);
    let lower      = script.to_lowercase();

    let mut out = String::with_capacity(script.len());
    let mut pos = 0usize;

    while pos < script.len() {
        match lower[pos..].find(&old_prefix) {
            None => {
                out.push_str(&script[pos..]);
                break;
            }
            Some(rel) => {
                out.push_str(&script[pos..pos + rel]);
                out.push_str(&new_prefix);
                pos += rel + old_prefix.len();
            }
        }
    }

    out
}

// ── Syntax normalisation ─────────────────────────────────────────────────────

/// Fix common AI syntax mistakes so the DSL parser doesn't reject the script
/// before we even get to semantic validation.
///
/// Patterns corrected:
///   IN ["a", "b"]      →  IN ("a", "b")      (square-bracket lists)
///   NOT IN ["a"]       →  NOT IN ("a")
///   !=                 →  <>                  (SQL inequality operator)
///   `backtick.col`     →  col                 (some models wrap identifiers)
fn normalize_dsl_syntax(script: &str) -> String {
    script.lines().map(|line| {
        let trimmed = line.trim_start();
        if trimmed.starts_with("--") {
            return line.to_string();
        }

        let mut s = line.to_string();

        // Replace square brackets AND curly braces with round brackets, leaving string
        // literals alone.  Catches every bracket-list variant the AI produces:
        //   IN [...], IN {...}    →  IN (...)
        //   col = ['v']          →  col = ('v')
        //   SIZE {10}            →  SIZE (10)  — fixed further below
        {
            let mut out = String::with_capacity(s.len());
            let mut in_str = false;
            let mut str_ch = ' ';
            for ch in s.chars() {
                if in_str {
                    if ch == str_ch { in_str = false; }
                    out.push(ch);
                } else if ch == '\'' || ch == '"' {
                    in_str = true;
                    str_ch = ch;
                    out.push(ch);
                } else if ch == '[' || ch == '{' {
                    out.push('(');
                } else if ch == ']' || ch == '}' {
                    out.push(')');
                } else {
                    out.push(ch);
                }
            }
            s = out;
        }

        // SIZE (n) / SIZE (n%) → SIZE n / SIZE n%
        // Produced when the AI writes SIZE {10} or SIZE [10], which the bracket pass
        // above converts to SIZE (10).  The parser expects a bare number after SIZE.
        {
            let upper = s.to_uppercase();
            if let Some(size_pos) = upper.find("SIZE (") {
                let after = &s[size_pos + 6..];
                let num_len = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').count();
                if num_len > 0 {
                    let has_pct  = after.chars().nth(num_len) == Some('%');
                    let paren_ok = after.chars().nth(num_len + if has_pct { 1 } else { 0 }) == Some(')');
                    if paren_ok {
                        let num = &after[..num_len];
                        let pct = if has_pct { "%" } else { "" };
                        let skip = 6 + num_len + if has_pct { 1 } else { 0 } + 1; // "(" + num + opt% + ")"
                        s = format!("{}SIZE {num}{pct}{}", &s[..size_pos], &s[size_pos + skip..]);
                    }
                }
            }
        }

        // != → <>
        s = s.replace("!=", "<>");

        // Remove backtick quoting around identifiers.
        s = s.replace('`', "");

        // SAMPLE FULL_POPULATION → SAMPLE RANDOM
        {
            let upper = s.to_uppercase();
            if let Some(pos) = upper.find("SAMPLE FULL_POPULATION") {
                s = format!("{}SAMPLE RANDOM{}", &s[..pos], &s[pos + "SAMPLE FULL_POPULATION".len()..]);
            }
        }

        // COUNT(*) is not valid DSL — comment out the whole line so parse succeeds
        // and the AI-fix or drop-salvage pass can handle whatever is left.
        if s.to_uppercase().contains("COUNT(*)") {
            return format!("-- (removed: COUNT(*) not supported) {s}");
        }

        // ASSERT COUNT(tbl1.col) WHERE tbl2.col ... — strip the WHERE when it references
        // a different table than the COUNT argument (cross-table filter is not valid DSL).
        if let Some(where_pos) = s.to_uppercase().find(" WHERE ") {
            let upper = s.to_uppercase();
            if upper.contains("ASSERT") && upper.contains("COUNT(") {
                if let Some(count_start) = upper.find("COUNT(") {
                    let after      = &s[count_start + 6..];
                    let count_tbl  = after.split('.').next().unwrap_or("").trim().to_uppercase();
                    let where_part = &s[where_pos + 7..];
                    let where_tbl  = where_part.split('.').next().unwrap_or("").trim().to_uppercase();
                    if !count_tbl.is_empty() && !where_tbl.is_empty() && count_tbl != where_tbl {
                        s = s[..where_pos].to_string();
                    }
                }
            }
        }

        s
    }).collect::<Vec<_>>().join("\n")
}

// ── Cross-table column search ─────────────────────────────────────────────────

/// For every `unknown column 'col' in table 'tbl'` error, search ALL other real
/// tables for a column with the same name.  If exactly one other table has it,
/// rewrite `tbl.col` → `that_table.col` in the script.  If multiple tables have
/// it, prefer non-master tables; if still a tie, skip (let the AI fix pass handle it).
///
/// This fixes the common case where the AI writes `master_record.driver_age_band`
/// when `driver_age_band` actually lives in `c5_basepremium`.
fn repair_column_in_wrong_table(
    script:  &str,
    errors:  &[String],
    schemas: &[crate::types::SessionSchema],
) -> String {
    // Collect (wrong_table, col) pairs from errors
    let col_errors: Vec<(String, String)> = errors.iter()
        .filter_map(|e| {
            let col_rest = e.strip_prefix("unknown column '")?;
            let col_end  = col_rest.find('\'')?;
            let col      = col_rest[..col_end].to_string();
            let tbl_rest = col_rest.find("in table '")?;
            let after    = &col_rest[tbl_rest + "in table '".len()..];
            let tbl_end  = after.find('\'')?;
            let tbl      = after[..tbl_end].to_string();
            Some((tbl, col))
        })
        .collect();

    let mut result = script.to_string();

    for (bad_tbl, col) in &col_errors {
        // Tables (other than bad_tbl) that have this column
        let candidates: Vec<&crate::types::SessionSchema> = schemas.iter()
            .filter(|s| !s.table_name.eq_ignore_ascii_case(bad_tbl))
            .filter(|s| s.columns.iter().any(|c| c.eq_ignore_ascii_case(col)))
            .collect();

        let target = match candidates.len() {
            0 => continue,   // column doesn't exist anywhere — leave for drop logic
            1 => candidates[0],
            _ => {
                // Prefer specific source tables over master (master joins everything,
                // so the source table is more precise and less ambiguous).
                candidates.iter()
                    .find(|s| s.source_type != "master")
                    .copied()
                    .unwrap_or(candidates[0])
            }
        };

        result = rewrite_col_ref(&result, bad_tbl, col, &target.table_name);
    }

    result
}

/// Replace every occurrence of `old_table.col` with `new_table.col` in the script,
/// case-insensitively on both old_table and col.
fn rewrite_col_ref(script: &str, old_table: &str, col: &str, new_table: &str) -> String {
    let needle = format!("{}.{}", old_table.to_lowercase(), col.to_lowercase());
    let replace = format!("{}.{}", new_table, col);
    let lower   = script.to_lowercase();

    let mut out = String::with_capacity(script.len());
    let mut pos = 0usize;

    while pos < script.len() {
        match lower[pos..].find(&needle) {
            None => { out.push_str(&script[pos..]); break; }
            Some(rel) => {
                out.push_str(&script[pos..pos + rel]);
                out.push_str(&replace);
                pos += rel + needle.len();
            }
        }
    }
    out
}

// ── Column-level salvage helpers ──────────────────────────────────────────────

/// Remove every line from the script that references an invalid `table.column`
/// pair reported in the error list. Valid lines (correct ASSERT, SAMPLE,
/// comments) are preserved exactly.
///
/// Error format: `"unknown column 'col' in table 'tbl'"`
fn drop_invalid_column_lines(script: &str, errors: &[String]) -> String {
    let bad_refs: Vec<String> = errors.iter()
        .filter_map(|e| {
            let col_start = e.find("unknown column '")?;
            let rest = &e[col_start + "unknown column '".len()..];
            let col_end = rest.find('\'')?;
            let col = &rest[..col_end];
            let tbl_start = rest.find("in table '")?;
            let rest2 = &rest[tbl_start + "in table '".len()..];
            let tbl_end = rest2.find('\'')?;
            let tbl = &rest2[..tbl_end];
            Some(format!("{}.{}", tbl.to_lowercase(), col.to_lowercase()))
        })
        .collect();

    if bad_refs.is_empty() {
        return script.to_string();
    }

    let kept: Vec<&str> = script
        .lines()
        .filter(|line| {
            let lower = line.to_lowercase();
            !bad_refs.iter().any(|bad| lower.contains(bad.as_str()))
        })
        .collect();

    kept.join("\n")
}

/// Return a `table.col` string for a fallback SAMPLE statement.
///
/// Scores every real table against the control label and the invented column
/// names from the error list so the sample comes from the most on-topic table,
/// not just the first column of whichever table happened to appear in an error.
///
/// Resolution order:
///   1. Semantic best-match using label + error column keywords
///   2. First real table referenced in the script text
///   3. Any available table's first column
fn fallback_sample_col(
    errors:  &[String],
    script:  &str,
    label:   &str,
    schemas: &[crate::types::SessionSchema],
) -> String {
    // Collect keywords from the control label and from the invented column names
    // in the errors (e.g. "risk_acceptance_philosophy" → ["risk", "acceptance", "philosophy"]).
    let mut keywords: Vec<String> = label
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4)
        .map(|w| w.to_lowercase())
        .collect();

    for e in errors {
        if let Some(col_rest) = e.strip_prefix("unknown column '") {
            if let Some(col) = col_rest.split('\'').next() {
                for part in col.split('_').filter(|w| w.len() >= 4) {
                    keywords.push(part.to_lowercase());
                }
            }
        }
    }
    keywords.sort();
    keywords.dedup();

    // Score each real table against the keyword set.
    let best_by_label = schemas.iter()
        .filter_map(|s| {
            let tbl_lower = s.table_name.to_lowercase();
            let mut score = 0i32;
            for kw in &keywords {
                if tbl_lower.contains(kw.as_str()) { score += 10; }
                if s.columns.iter().any(|c| c.to_lowercase().contains(kw.as_str())) { score += 4; }
            }
            if score > 0 { Some((s, score)) } else { None }
        })
        .max_by_key(|(_, sc)| *sc)
        .map(|(s, _)| s.table_name.clone());

    // Fall back to the first real table explicitly referenced in the script.
    let table_from_script = || -> Option<String> {
        let lower = script.to_lowercase();
        for schema in schemas {
            let prefix = format!("{}.", schema.table_name.to_lowercase());
            if lower.contains(&prefix) {
                return Some(schema.table_name.clone());
            }
        }
        None
    };

    let tbl = best_by_label.or_else(table_from_script);

    if let Some(tbl) = tbl {
        if let Some(schema) = schemas.iter().find(|s| s.table_name.eq_ignore_ascii_case(&tbl)) {
            if let Some(col) = schema.columns.first() {
                return format!("{}.{}", schema.table_name, col);
            }
        }
    }

    // Last resort
    schemas.iter()
        .flat_map(|s| s.columns.first().map(|c| format!("{}.{}", s.table_name, c)))
        .next()
        .unwrap_or_else(|| "data.id".to_string())
}
