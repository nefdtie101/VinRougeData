use crate::types::{AuditProcessWithControls, PbcGroup, SessionSchema};

/// Build a concrete few-shot DSL example using the first real table and its first columns.
/// This grounds the LLM in the actual schema before it writes any scripts.
pub fn build_example_section(schemas: &[SessionSchema]) -> String {
    let Some(tbl) = schemas.first() else { return String::new() };
    let name = &tbl.table_name;
    // Pick a plausible numeric column (heuristic: name contains amount/value/total/sum/price/cost)
    let numeric_hints = ["amount", "value", "total", "sum", "price", "cost", "bal", "qty"];
    let num_col = tbl.columns.iter()
        .find(|c| numeric_hints.iter().any(|h| c.to_lowercase().contains(h)))
        .or_else(|| tbl.columns.get(1))
        .or_else(|| tbl.columns.first())
        .map(String::as_str)
        .unwrap_or("id");
    // Pick any text-like column for a WHERE example
    let str_col = tbl.columns.iter()
        .find(|c| {
            let lc = c.to_lowercase();
            lc.contains("status") || lc.contains("type") || lc.contains("code")
        })
        .or_else(|| tbl.columns.first())
        .map(String::as_str)
        .unwrap_or("id");

    format!(
        "WORKED EXAMPLE using your actual table '{name}':\n\
         -- Count check\n\
         ASSERT COUNT({name}.{str_col}) > 0\n\
         -- Completeness\n\
         ASSERT {name}.{str_col} IS NOT NULL\n\
         -- Aggregate threshold\n\
         ASSERT SUM({name}.{num_col}) > 0\n\
         -- MUS sample\n\
         SAMPLE MUS FROM {name}.{num_col} SIZE 30\n\n\
         Use EXACTLY these table/column names — never invent others.\n\n"
    )
}

pub fn build_schema_section(schemas: &[SessionSchema]) -> String {
    let mut out = String::new();
    for s in schemas {
        out.push_str(&format!(
            "Table: {} ({} rows)\n  Columns: {}\n\n",
            s.table_name, s.row_count, s.columns.join(", ")
        ));
    }
    out
}

/// Returns a bullet-list of every table name the LLM is allowed to reference.
/// Called separately so it can be injected as a distinct prompt section.
pub fn build_table_names_section(schemas: &[SessionSchema]) -> String {
    let mut out = String::from("VALID TABLE NAMES (copy these EXACTLY — no other names are permitted):\n");
    for s in schemas {
        out.push_str(&format!("  - {}\n", s.table_name));
    }
    out.push('\n');
    out
}

pub fn build_plan_section(
    plan:       &[AuditProcessWithControls],
    pbc_groups: &[PbcGroup],
    schemas:    &[SessionSchema],
) -> String {
    let mut s = String::new();
    for process in plan {
        s.push_str(&format!("Process: {}\n", process.process_name));
        for ctrl in &process.controls {
            s.push_str(&format!(
                "  Control {}: {}\n    How it operates: {}\n    Test: {}\n    Risk: {}\n",
                ctrl.control_ref, ctrl.control_objective,
                ctrl.control_description,
                ctrl.test_procedure, ctrl.risk_level
            ));

            let pbc_items: Vec<_> = pbc_groups.iter()
                .filter(|g| g.control_id == ctrl.id)
                .flat_map(|g| g.items.iter())
                .collect();

            // Collect required tables from PBC items that have uploaded data
            let mut required_tables: Vec<String> = pbc_items.iter()
                .filter_map(|i| i.table_name.clone())
                .filter(|t| schemas.iter().any(|s| s.table_name.eq_ignore_ascii_case(t)))
                .collect();
            required_tables.sort();
            required_tables.dedup();

            if !required_tables.is_empty() {
                s.push_str(&format!(
                    "    *** REQUIRED TABLES FOR THIS CONTROL: {}\n",
                    required_tables.join(", ")
                ));
                for tbl in &required_tables {
                    if let Some(schema) = schemas.iter().find(|sc| sc.table_name.eq_ignore_ascii_case(tbl)) {
                        s.push_str(&format!(
                            "    Exact columns in {}: {}\n",
                            schema.table_name,
                            schema.columns.join(", ")
                        ));
                    }
                }
            } else if !schemas.is_empty() {
                // No explicit PBC table link — pick best-matching table by keyword overlap
                // so the AI doesn't default to whatever table it happens to remember.
                let keywords: Vec<String> = format!(
                    "{} {} {}",
                    ctrl.control_objective, ctrl.control_description, ctrl.test_procedure
                )
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w: &&str| w.len() >= 4)
                .map(|w| w.to_lowercase())
                .collect();

                let best = schemas.iter()
                    .filter_map(|schema| {
                        let tbl_lower = schema.table_name.to_lowercase();
                        let mut score = 0i32;
                        for kw in &keywords {
                            if tbl_lower.contains(kw.as_str()) { score += 10; }
                            for col in &schema.columns {
                                if col.to_lowercase().contains(kw.as_str()) { score += 3; }
                            }
                        }
                        if score > 0 { Some((schema, score)) } else { None }
                    })
                    .max_by_key(|(_, sc)| *sc)
                    .map(|(schema, _)| schema);

                if let Some(best_schema) = best {
                    s.push_str(&format!(
                        "    BEST MATCH TABLE: {} — use this as the primary table.\n    Columns: {}\n",
                        best_schema.table_name,
                        best_schema.columns.join(", ")
                    ));
                } else {
                    // No keyword match — tell the AI to pick the most relevant available table
                    let table_list: Vec<String> = schemas.iter()
                        .map(|sc| format!("{} ({})", sc.table_name, sc.columns.join(", ")))
                        .collect();
                    s.push_str(&format!(
                        "    NO DIRECT TABLE MATCH — choose the most relevant table from: {}\n",
                        table_list.join(" | ")
                    ));
                }
            }

            if !pbc_items.is_empty() {
                let items_txt: Vec<String> = pbc_items.iter()
                    .map(|i| {
                        if let Some(tn) = &i.table_name {
                            format!("{} (table: {}, fields: {})", i.name, tn, i.fields.join(", "))
                        } else {
                            format!("{} (fields: {})", i.name, i.fields.join(", "))
                        }
                    })
                    .collect();
                s.push_str(&format!("    Data: {}\n", items_txt.join("; ")));
            }
        }
        s.push('\n');
    }
    s
}
