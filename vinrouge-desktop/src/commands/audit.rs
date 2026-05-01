use crate::state::ProjectsState;
use tauri::State;

/// Persist an AI-generated audit plan for a SOP file.
#[tauri::command]
pub fn save_audit_plan(
    sop_file_id: String,
    processes_json: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;

    #[derive(serde::Deserialize)]
    struct ControlDto {
        #[serde(alias = "controlRef", alias = "ref", alias = "id")]
        control_ref: String,
        #[serde(alias = "controlObjective", alias = "objective")]
        control_objective: String,
        #[serde(alias = "controlDescription", alias = "description", alias = "how_it_operates", alias = "howItOperates")]
        control_description: String,
        #[serde(alias = "testProcedure", alias = "test", alias = "procedure")]
        test_procedure: String,
        #[serde(alias = "riskLevel", alias = "risk", alias = "severity")]
        risk_level: String,
        #[serde(alias = "sopGap", alias = "gap", default)]
        sop_gap: bool,
    }
    #[derive(serde::Deserialize)]
    struct ProcessDto {
        #[serde(alias = "processName", alias = "name", alias = "title")]
        process_name: String,
        #[serde(alias = "processDescription", alias = "summary", alias = "details")]
        description: String,
        #[serde(alias = "controlsList", alias = "control_list", alias = "items")]
        controls: Vec<ControlDto>,
    }
    #[derive(serde::Deserialize)]
    struct PlanDto {
        #[serde(
            alias = "plan",
            alias = "audit_plan",
            alias = "auditPlan",
            alias = "processList",
            alias = "process_list",
            alias = "items"
        )]
        processes: Vec<ProcessDto>,
    }

    // Normalise the JSON: Ollama sometimes returns a bare array or wraps the
    // list under a key other than "processes" (e.g. "plan", "audit_plan").
    let plan: PlanDto = {
        if let Ok(p) = serde_json::from_str::<PlanDto>(&processes_json) {
            p
        } else if let Ok(arr) = serde_json::from_str::<Vec<ProcessDto>>(&processes_json) {
            PlanDto { processes: arr }
        } else if let Ok(obj) =
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&processes_json)
        {
            fn extract(val: serde_json::Value) -> Option<Vec<ProcessDto>> {
                if let serde_json::Value::Array(_) = &val {
                    return serde_json::from_value::<Vec<ProcessDto>>(val).ok();
                }
                if let serde_json::Value::Object(map) = val {
                    for (_, v) in &map {
                        if let Ok(arr) = serde_json::from_value::<Vec<ProcessDto>>(v.clone()) {
                            return Some(arr);
                        }
                    }
                    if let Some(args) = map.get("arguments") {
                        return extract(args.clone());
                    }
                }
                None
            }
            match extract(serde_json::Value::Object(obj)) {
                Some(arr) => PlanDto { processes: arr },
                None => {
                    let preview: String = processes_json.chars().take(500).collect();
                    return Err(format!(
                        "Invalid plan JSON: could not find a valid processes array. Raw output (first 500 chars): {preview}"
                    ));
                }
            }
        } else {
            return Err(format!(
                "Invalid plan JSON: {}",
                serde_json::from_str::<serde_json::Value>(&processes_json)
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "unrecognised structure".into())
            ));
        }
    };

    // Normalise control_ref values to C-1, C-2, ... across all processes.
    let mut ctrl_counter = 1usize;
    let batch: Vec<(String, String, Vec<(String, String, String, String, String, bool)>)> = plan
        .processes
        .into_iter()
        .map(|p| {
            let controls = p.controls.into_iter().map(|c| {
                let normalised_ref = format!("C-{}", ctrl_counter);
                ctrl_counter += 1;
                (
                    normalised_ref,
                    c.control_objective,
                    c.control_description,
                    c.test_procedure,
                    c.risk_level,
                    c.sop_gap,
                )
            }).collect();
            (p.process_name, p.description, controls)
        })
        .collect();

    vinrouge::projects::replace_audit_plan(&project_dir, &sop_file_id, &batch)
}

#[tauri::command]
pub fn list_audit_plan(
    state: State<ProjectsState>,
) -> Result<Vec<vinrouge::projects::AuditProcessWithControls>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::list_audit_plan(&project_dir)
}

#[tauri::command]
pub fn add_control(
    process_id: String,
    control_ref: String,
    control_objective: String,
    control_description: String,
    test_procedure: String,
    risk_level: String,
    state: State<ProjectsState>,
) -> Result<vinrouge::projects::Control, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::add_control(
        &project_dir,
        &process_id,
        &control_ref,
        &control_objective,
        &control_description,
        &test_procedure,
        &risk_level,
    )
}

#[tauri::command]
pub fn delete_control(
    control_id: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::delete_control(&project_dir, &control_id)
}

/// Patch a single field on a control row.
#[tauri::command]
pub fn update_control_field(
    control_id: String,
    field: String,
    value: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::update_control_field(&project_dir, &control_id, &field, &value)
}

/// Patch a single field on a process row.
#[tauri::command]
pub fn update_process_field(
    process_id: String,
    field: String,
    value: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::update_process_field(&project_dir, &process_id, &field, &value)
}

// ── PBC items ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_pbc_groups(
    state: State<ProjectsState>,
) -> Result<Vec<vinrouge::projects::PbcGroup>, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::list_pbc_groups(&project_dir)
}

#[tauri::command]
pub fn save_pbc_item(
    control_id: String,
    control_ref: String,
    name: String,
    item_type: String,
    table_name: Option<String>,
    fields: Vec<String>,
    purpose: String,
    scope_format: String,
    state: State<ProjectsState>,
) -> Result<vinrouge::projects::PbcItem, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::save_pbc_item(
        &project_dir,
        &control_id,
        &control_ref,
        &name,
        &item_type,
        table_name.as_deref(),
        &fields,
        &purpose,
        &scope_format,
    )
}

#[tauri::command]
pub fn delete_pbc_item(
    item_id: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::delete_pbc_item(&project_dir, &item_id)
}

#[tauri::command]
pub fn clear_pbc_items(state: State<ProjectsState>) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::clear_pbc_items(&project_dir)
}

#[tauri::command]
pub fn update_pbc_item(
    item_id: String,
    name: String,
    item_type: String,
    table_name: Option<String>,
    fields: Vec<String>,
    purpose: String,
    scope_format: String,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::update_pbc_item(
        &project_dir,
        &item_id,
        &name,
        &item_type,
        table_name.as_deref(),
        &fields,
        &purpose,
        &scope_format,
    )
}

#[tauri::command]
pub fn update_pbc_item_fields(
    item_id: String,
    fields: Vec<String>,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::update_pbc_item_fields(&project_dir, &item_id, &fields)
}

#[tauri::command]
pub fn toggle_pbc_item_approved(
    item_id: String,
    state: State<ProjectsState>,
) -> Result<bool, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::toggle_pbc_item_approved(&project_dir, &item_id)
}

#[tauri::command]
pub fn get_pbc_list_approved(state: State<ProjectsState>) -> Result<bool, String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::get_pbc_list_approved(&project_dir)
}

#[tauri::command]
pub fn set_pbc_list_approved(
    approved: bool,
    state: State<ProjectsState>,
) -> Result<(), String> {
    let project_dir = state.0.lock().unwrap().clone().ok_or("No active project")?;
    vinrouge::projects::set_pbc_list_approved(&project_dir, approved)
}
