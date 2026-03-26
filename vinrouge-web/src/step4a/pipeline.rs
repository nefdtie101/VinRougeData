use std::collections::HashMap;
use leptos::prelude::*;

use crate::ipc::{tauri_invoke, tauri_invoke_args};
use crate::ollama::{ask_ollama_structured, OLLAMA_DEFAULT_MODEL, OLLAMA_DEFAULT_URL};
use crate::types::{AuditProcessWithControls, DslScript, PbcGroup, RelCandidate, SessionSchema};
use vinrouge::audit_prompts::{dsl_script_schema, GENERATE_DSL};
use super::types::{Phase, ScriptStatus, ScriptState};
use super::prompts::{build_schema_section, build_plan_section};

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

    // 2b. Load preview from master (or first import if no master)
    let preview_source_schema = master
        .or_else(|| session_schemas.first())
        .cloned();

    if let Some(s) = preview_source_schema {
        let import_id = s.import_id.clone();
        let cols      = s.columns.clone();
        preview_source.set(s.table_name.clone());
        preview_cols.set(cols.clone());

        if let Ok(raw_rows) = tauri_invoke_args::<Vec<HashMap<String, String>>>(
            "get_session_rows",
            serde_json::json!({ "importId": import_id }),
        ).await {
            let rows: Vec<Vec<String>> = raw_rows.into_iter().take(200).map(|row| {
                cols.iter().map(|c| row.get(c).cloned().unwrap_or_default()).collect()
            }).collect();
            preview_rows.set(rows);
        }
    }

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

    let schema_section = build_schema_section(&session_schemas);
    let plan_section   = build_plan_section(&audit_plan.get_untracked(), &pbc_groups);
    let prompt = format!(
        "{GENERATE_DSL}\
         AVAILABLE DATA:\n{schema_section}\n\
         AUDIT CONTROLS TO TEST:\n{plan_section}\n\
         Return ONLY a JSON object: \
         {{\"scripts\": [{{\"control_ref\": \"C-1\", \"label\": \"Test label\", \
         \"script\": \"DSL code here\"}}]}}"
    );

    let schema = dsl_script_schema();
    let raw = match ask_ollama_structured(
        OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &prompt, schema
    ).await {
        Ok(r) => r,
        Err(e) => {
            phase.set(Phase::Error(format!("AI generation failed: {e}")));
            return;
        }
    };

    // 6. Parse JSON
    let script_dtos: Vec<(String, String, String)> = {
        let v: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                phase.set(Phase::Error(format!("Invalid JSON from AI: {e}")));
                return;
            }
        };
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
    };

    if script_dtos.is_empty() {
        phase.set(Phase::Error("AI did not generate any valid DSL scripts.".to_string()));
        return;
    }

    // 7. Save scripts to DB, build frontend state
    let plan         = audit_plan.get_untracked();
    let ctrl_lookup: HashMap<String, String> = plan.iter()
        .flat_map(|p| p.controls.iter())
        .map(|c| (c.control_ref.clone(), c.id.clone()))
        .collect();

    let mut saved: Vec<DslScript>              = vec![];
    let mut new_states: HashMap<String, ScriptState> = HashMap::new();

    for (control_ref, label, script_text) in &script_dtos {
        let control_id = ctrl_lookup.get(control_ref).cloned().unwrap_or_default();
        match tauri_invoke_args::<DslScript>(
            "save_dsl_script",
            serde_json::json!({
                "controlId":  control_id,
                "controlRef": control_ref,
                "label":      label,
                "scriptText": script_text,
            }),
        ).await {
            Ok(s) => {
                new_states.insert(s.id.clone(), ScriptState {
                    status: ScriptStatus::Generated,
                    text:   script_text.clone(),
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
