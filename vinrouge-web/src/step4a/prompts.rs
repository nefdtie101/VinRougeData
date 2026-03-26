use crate::types::{AuditProcessWithControls, PbcGroup, SessionSchema};

pub fn build_schema_section(schemas: &[SessionSchema]) -> String {
    // If a master record exists, feature it first with individual tables as notes
    if let Some(master) = schemas.iter().find(|s| s.source_type == "master") {
        let sources: Vec<String> = schemas
            .iter()
            .filter(|s| s.source_type != "master")
            .map(|s| format!("  - {} ({} rows, {} cols)", s.table_name, s.row_count, s.columns.len()))
            .collect();
        let source_note = if sources.is_empty() {
            String::new()
        } else {
            format!("  (Joined from:\n{}\n  )", sources.join("\n"))
        };
        format!(
            "Master Table: {} ({} rows)\n  Columns: {}\n{}\n\n",
            master.table_name, master.row_count, master.columns.join(", "), source_note
        )
    } else {
        schemas.iter().map(|s| {
            format!("Table: {} ({} rows)\n  Columns: {}\n\n",
                s.table_name, s.row_count, s.columns.join(", "))
        }).collect()
    }
}

pub fn build_plan_section(plan: &[AuditProcessWithControls], pbc_groups: &[PbcGroup]) -> String {
    let mut s = String::new();
    for process in plan {
        s.push_str(&format!("Process: {}\n", process.process_name));
        for ctrl in &process.controls {
            s.push_str(&format!(
                "  Control {}: {}\n    Test: {}\n    Risk: {}\n",
                ctrl.control_ref, ctrl.control_objective,
                ctrl.test_procedure, ctrl.risk_level
            ));
            let pbc_items: Vec<String> = pbc_groups.iter()
                .filter(|g| g.control_id == ctrl.id)
                .flat_map(|g| g.items.iter())
                .map(|i| {
                    if let Some(tn) = &i.table_name {
                        format!("{} (table: {}, fields: {})", i.name, tn, i.fields.join(", "))
                    } else {
                        format!("{} (fields: {})", i.name, i.fields.join(", "))
                    }
                })
                .collect();
            if !pbc_items.is_empty() {
                s.push_str(&format!("    Data: {}\n", pbc_items.join("; ")));
            }
        }
        s.push('\n');
    }
    s
}
