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
    // List ALL tables (master and individual imports) so the LLM can generate
    // tests against any imported table, not only the master joined record.
    let mut out = String::new();
    // Master first (if present), then individual imports
    let master_first: Vec<&SessionSchema> = schemas.iter()
        .filter(|s| s.source_type == "master")
        .chain(schemas.iter().filter(|s| s.source_type != "master"))
        .collect();
    for s in master_first {
        let tag = if s.source_type == "master" { " [MASTER - joined record]" } else { "" };
        out.push_str(&format!(
            "Table: {}{} ({} rows)\n  Columns: {}\n\n",
            s.table_name, tag, s.row_count, s.columns.join(", ")
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

            if !pbc_items.is_empty() {
                // Collect distinct table names from PBC items that have uploaded data
                let mut required_tables: Vec<String> = pbc_items.iter()
                    .filter_map(|i| i.table_name.clone())
                    .filter(|t| schemas.iter().any(|s| s.table_name.eq_ignore_ascii_case(t)))
                    .collect();
                required_tables.sort();
                required_tables.dedup();

                if !required_tables.is_empty() {
                    // Hard directive: name the exact table(s) and their columns
                    s.push_str(&format!(
                        "    *** REQUIRED TABLES FOR THIS CONTROL (use these, NOT master_record): {}\n",
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
                }

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
