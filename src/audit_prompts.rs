/// Static audit prompt constants — available on all targets including WASM.

pub const EXTRACT_SETUP: &str =
    "You are an audit setup assistant. Read the SOP below and identify:\n\
     1. Which audit/compliance standards are most relevant\n\
     2. The distinct business processes described\n\n\
     Return ONLY valid JSON — no markdown, no explanation:\n\
     {\n\
       \"standards\": [\"ISO 27001\", \"GDPR\"],\n\
       \"scope\": [\"Process name 1\", \"Process name 2\"]\n\
     }\n\n\
     Pick standards from: ISO 27001, SOC 2, GDPR, GAAP, IFRS, PCI-DSS, POPIA, COBIT, COSO, \
     ISA 315, SOX — only include ones clearly relevant to this SOP.\n\
     List every distinct business process you can identify from the SOP as a short name.\n\
     Return ONLY the JSON.\n\n\
     SOP TEXT:";

pub const UPDATE_SECTION: &str =
    "You are an audit assistant. Apply the user's instruction to update the audit process section below.\n\n\
     Return ONLY a valid JSON object — no markdown, no explanation — with this exact shape:\n\
     {\n\
       \"process_name\": \"Name of the process\",\n\
       \"description\": \"One-sentence description\",\n\
       \"controls\": [\n\
         {\n\
           \"control_ref\": \"C-1\",\n\
           \"control_objective\": \"What this control achieves\",\n\
           \"control_description\": \"How the control operates\",\n\
           \"test_procedure\": \"Step-by-step test procedure\",\n\
           \"risk_level\": \"High\"\n\
         }\n\
       ]\n\
     }\n\n\
     Rules:\n\
     - risk_level must be exactly one of: High, Medium, Low\n\
     - To ADD a control: include a new entry in the controls array with a new sequential control_ref\n\
     - To REMOVE a control: omit it from the controls array entirely — only controls present in your response will be kept\n\
     - To EDIT a control: include it in the controls array with the same control_ref and updated fields\n\
     - The returned controls array is the complete authoritative list — anything omitted will be deleted\n\
     - Return ONLY the JSON object, nothing else\n\n";

pub const ANALYZE_FILE: &str =
    "Analyze this data file and identify risks, anomalies, and control weaknesses. \
     Highlight any transactions or patterns that warrant further investigation.";

pub const SUMMARIZE_FINDINGS: &str =
    "Summarize the key audit findings from this analysis in a concise report format. \
     Group findings by risk level and suggest recommended actions for each.";

pub const RECONCILIATION: &str =
    "Compare these two datasets and identify unmatched items, duplicates, and \
     discrepancies. Quantify the total value of any variances found.";

pub const DATA_QUALITY: &str =
    "Assess the quality of this dataset. Check for missing values, formatting \
     inconsistencies, outliers, and any fields that appear unreliable.";

pub const GENERATE_PBC: &str =
    "You are a senior audit data analyst. Read the audit plan below and generate a comprehensive \
     Provided-By-Client (PBC) data request list.\n\n\
     For EVERY control you must produce at least one data request. Use the control's test procedure \
     and control description to determine exactly which table and fields the auditor needs. \
     If a test procedure references two distinct data sources (e.g. compare invoices against \
     payments), create a separate request for each source.\n\n\
     Return ONLY a valid JSON object — no markdown, no explanation:\n\
     {\n\
       \"items\": [\n\
         {\n\
           \"control_ref\": \"C-1\",\n\
           \"name\": \"Descriptive name that identifies the specific data set\",\n\
           \"item_type\": \"SQL\",\n\
           \"table_name\": \"exact_table_name\",\n\
           \"fields\": [\"field1\", \"field2\", \"field3\"],\n\
           \"purpose\": \"One sentence stating exactly what auditor will verify with this data\",\n\
           \"scope_format\": \"e.g. All records in the audit period, pre-rental inspections only\"\n\
         }\n\
       ]\n\
     }\n\n\
     Rules:\n\
     - Every control in the plan MUST appear at least once — do not skip any\n\
     - item_type must be exactly SQL or CSV\n\
     - Use SQL for database tables; CSV for manual registers, certificates, scanned logs\n\
     - table_name must be null for CSV items\n\
     - fields must be real column names inferred from the system described — be specific and complete\n\
     - Include all fields the auditor needs to perform the test procedure, not just a few\n\
     - scope_format must reflect any filter described in the test procedure (date range, record type)\n\
     - Return ONLY the JSON object, no other text\n\n\
     AUDIT PLAN:";

pub const SYNC_PBC_TO_PLAN: &str =
    "You are a senior auditor reviewing a PBC (Provided-By-Client) data request list alongside \
     the audit plan that produced it. Based on the actual data being requested you may now refine \
     the audit plan — tighten test procedures to reference the exact tables/fields being collected, \
     adjust risk levels if the data scope reveals higher or lower risk, or clarify control descriptions.\n\n\
     Return ONLY a valid JSON object — no markdown, no explanation:\n\
     {\n\
       \"summary\": \"One sentence describing what was updated and why\",\n\
       \"updates\": [\n\
         {\n\
           \"control_ref\": \"C-01\",\n\
           \"field\": \"test_procedure\",\n\
           \"value\": \"Updated value\"\n\
         }\n\
       ]\n\
     }\n\n\
     Rules:\n\
     - field must be one of: control_objective, control_description, test_procedure, risk_level\n\
     - risk_level must be exactly one of: High, Medium, Low\n\
     - Only include updates where the PBC data genuinely warrants a change\n\
     - If nothing needs to change, return an empty updates array\n\
     - Return ONLY the JSON object, nothing else\n\n";

pub const UPDATE_PBC_GROUP: &str =
    "You are an audit data analyst. Update the PBC (Provided-By-Client) data requests \
     for the specified control based on the user's instruction.\\n\\n\
     Return ONLY a valid JSON object — no markdown, no explanation:\\n\
     {\\n\
       \\\"summary\\\": \\\"one sentence describing what was changed\\\",\\n\
       \\\"add_items\\\": [\\n\
         {\\n\
           \\\"name\\\": \\\"Descriptive request name\\\",\\n\
           \\\"itemType\\\": \\\"SQL\\\",\\n\
           \\\"tableName\\\": \\\"exact_table_name_or_null\\\",\\n\
           \\\"fields\\\": [\\\"field1\\\", \\\"field2\\\"],\\n\
           \\\"purpose\\\": \\\"What the auditor will verify with this data\\\",\\n\
           \\\"scopeFormat\\\": \\\"e.g. All records in the audit period\\\"\\n\
         }\\n\
       ],\\n\
       \\\"remove_item_ids\\\": [\\\"id-of-item-to-delete\\\"]\\n\
     }\\n\\n\
     Rules:\\n\
     - Only include add_items if you need to add new requests\\n\
     - Only include remove_item_ids if you need to remove existing requests\\n\
     - itemType must be exactly SQL or CSV\\n\
     - tableName must be null for CSV items\\n\
     - Return ONLY the JSON object, nothing else\\n\\n";

/// Global PBC list refinement — AI adjusts add_items / add_fields / remove_fields.
/// Append the serialised current list and the user instruction before sending.
pub const REFINE_PBC_LIST: &str =
    "You are an audit data analyst. Update a PBC list based on the user instruction.\n\
     Return ONLY JSON, no markdown:\n\
     {\"summary\":\"one sentence\",\
     \"add_items\":[{\"controlRef\":\"C-01\",\"name\":\"\",\"itemType\":\"SQL\",\"tableName\":null,\
     \"fields\":[],\"purpose\":\"\",\"scopeFormat\":\"\"}],\
     \"add_fields\":[{\"itemId\":\"...\",\"fields\":[\"f1\"]}],\
     \"remove_fields\":[{\"itemId\":\"...\",\"fields\":[\"f1\"]}]}\n\
     Only include keys where changes are needed.\n\
     Current PBC list: ";

pub const ANALYZE_SOP: &str =
    "You are an expert audit planning assistant. Before doing anything else, read the SOP \
     below and identify the industry, domain, and regulatory environment it operates in. \
     You will act as a domain expert for that specific industry throughout this task — \
     using the precise terminology, standards, and risk language that an expert practitioner \
     in that field would use. Do not use generic audit language when industry-specific \
     language exists.\n\n\
     STEP 1 — INDUSTRY DETECTION (internal reasoning only, do not output this):\n\
     Read the SOP and determine:\n\
     - What industry or sector this SOP belongs to (e.g. short-term insurance, banking, \
       healthcare, manufacturing, logistics, IT services)\n\
     - What regulatory framework governs it (e.g. Insurance Act, GDPR, ISO 13485, \
       Basel III, OSHA)\n\
     - What the core operational risks are for that industry\n\
     - What a domain expert in this industry would call the key controls, documents, \
       and failure modes\n\
     Use this understanding to inform every field you populate below.\n\n\
     STEP 2 — AUDIT PLAN GENERATION:\n\
     Produce a structured audit plan grounded entirely in the SOP content, written from \
     the perspective of a domain expert in the detected industry.\n\n\
     IMPORTANT — ground every field in the SOP:\n\
     - process_name: use the exact name or heading from the SOP wherever possible\n\
     - description: reference specific activities, actors, or systems named in the SOP\n\
     - control_objective: state what specific risk or failure mode from the SOP this \
       control addresses — use the risk language an expert in this industry would use\n\
     - control_description: describe how the control operates using the SOP's own \
       terminology — name the roles, systems, forms, thresholds, or approval steps \
       mentioned in the SOP\n\
     - test_procedure: write concrete steps an expert auditor in this industry would \
       follow — reference the SOP's named documents, systems, and roles — each step \
       must end with a FAIL condition and include a SAMPLING line\n\
     - risk_level: assign based on the consequence of the control failing, informed by \
       industry norms for that type of failure\n\n\
     SAMPLING REQUIREMENT — every control MUST include a sampling section in test_procedure:\n\
     Format: \"SAMPLING: [method] — [sample size] — [justification]\"\n\
     Method must be one of: MUS per ISA 530 / judgmental / full population.\n\
     A test_procedure without a SAMPLING line is invalid.\n\n\
     NEGATIVE EXAMPLE — do NOT produce tests like this:\n\
     BAD: \"Ensure that the relevant process complies with applicable standards.\"\n\
     GOOD: \"Obtain [specific document named in SOP] from [role or system named in SOP]. \
     Verify that [specific condition from SOP] is met. FAIL: if [measurable threshold] \
     is not present or is exceeded. SAMPLING: [method] — [sample size] — [justification].\"\n\n\
     SELF-CHECK — before finalising output verify all of the following are true:\n\
     1. Every test_procedure contains a SAMPLING line\n\
     2. Every test_procedure contains a FAIL condition with a measurable threshold\n\
     3. Every control_description names a specific SOP role, system, document, or threshold\n\
     4. The language used throughout reflects the terminology of an expert in the \
        detected industry — not generic audit boilerplate\n\
     5. control_ref is sequential and unique across all processes (C-1, C-2, C-3 ...)\n\
     If any check fails, fix it before outputting.\n\n\
     Return ONLY a valid JSON object — no markdown fences, no explanation — with this exact shape:\n\
     {\n\
       \"industry\": \"Detected industry or sector from SOP\",\n\
       \"regulatory_framework\": \"Primary regulation or standard governing this SOP\",\n\
       \"processes\": [\n\
         {\n\
           \"process_name\": \"Exact name from SOP\",\n\
           \"description\": \"What this process does, using SOP-specific details\",\n\
           \"controls\": [\n\
             {\n\
               \"control_ref\": \"C-1\",\n\
               \"control_objective\": \"Risk or failure mode this control prevents, using industry-expert language\",\n\
               \"control_description\": \"How the control operates, naming SOP roles/systems/documents\",\n\
               \"test_procedure\": \"Step-by-step audit test referencing SOP artefacts and roles. FAIL: [measurable threshold]. SAMPLING: [method] — [sample size] — [justification].\",\n\
               \"risk_level\": \"High\",\n\
               \"sop_gap\": false\n\
             }\n\
           ]\n\
         }\n\
       ]\n\
     }\n\n\
     Rules:\n\
     - risk_level must be exactly one of: High, Medium, Low\n\
     - sop_gap must be exactly one of: true, false — set true if this control cannot be \
       fully traced to a specific SOP section\n\
     - control_ref must be sequential and unique across all processes: C-1, C-2, C-3 \
       (no zero padding)\n\
     - Each distinct business process should be a separate entry\n\
     - Each process should have between 2 and 6 controls\n\
     - Do NOT use generic filler language — every sentence must reflect a specific fact \
       from the SOP or established expert practice in the detected industry\n\
     - Return ONLY the JSON object, nothing else\n\n\
     SOP TEXT:";

/// JSON Schema for the PBC list generation output.
pub fn pbc_list_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "control_ref":   { "type": "string" },
                        "name":          { "type": "string" },
                        "item_type":     { "type": "string", "enum": ["SQL", "CSV"] },
                        "table_name":    { "type": ["string", "null"] },
                        "fields":        { "type": "array", "items": { "type": "string" } },
                        "purpose":       { "type": "string" },
                        "scope_format":  { "type": "string" }
                    },
                    "required": [
                        "control_ref", "name", "item_type",
                        "fields", "purpose", "scope_format"
                    ]
                }
            }
        },
        "required": ["items"]
    })
}

/// JSON Schema for the audit plan output.
/// Pass this as the `format` field to Ollama's structured-output API so the
/// model is constrained by token sampling to produce exactly this shape,
/// regardless of its instruction-following ability.
pub fn audit_plan_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "industry":             { "type": "string" },
            "regulatory_framework": { "type": "string" },
            "processes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "process_name": { "type": "string" },
                        "description":  { "type": "string" },
                        "controls": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "control_ref":         { "type": "string" },
                                    "control_objective":   { "type": "string" },
                                    "control_description": { "type": "string" },
                                    "test_procedure":      { "type": "string" },
                                    "risk_level": {
                                        "type": "string",
                                        "enum": ["High", "Medium", "Low"]
                                    },
                                    "sop_gap": { "type": "boolean" }
                                },
                                "required": [
                                    "control_ref", "control_objective",
                                    "control_description", "test_procedure",
                                    "risk_level", "sop_gap"
                                ]
                            }
                        }
                    },
                    "required": ["process_name", "description", "controls"]
                }
            }
        },
        "required": ["industry", "regulatory_framework", "processes"]
    })
}

// ── Post-parse normalization ──────────────────────────────────────────────────

/// Normalise a raw JSON string returned by the LLM for an audit plan.
///
/// Handles the most common drift patterns:
/// - camelCase field names  (controlRef → control_ref, etc.)
/// - zero-padded control refs  (C-01 → C-1)
/// - case-insensitive risk levels  ("high" → "High")
/// - model returning a bare array instead of `{"processes":[…]}`
pub fn normalize_audit_plan_json(raw: &str) -> Result<String, String> {
    let mut v: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("JSON parse error: {e}"))?;

    // If the model returned a bare array, wrap it.
    if v.is_array() {
        v = serde_json::json!({ "processes": v });
    }

    // If `processes` is missing, look for common alternative top-level keys.
    if v.get("processes").is_none() {
        let alts = ["audit_plan", "plan", "result", "data", "output", "items"];
        let found_key = alts
            .iter()
            .find(|&&k| v.get(k).and_then(|a| a.as_array()).is_some())
            .copied();
        if let Some(key) = found_key {
            let arr = v[key].take();
            v = serde_json::json!({ "processes": arr });
        }
    }

    // Normalise process objects.
    if let Some(processes) = v["processes"].as_array_mut() {
        for proc in processes.iter_mut() {
            if let Some(obj) = proc.as_object_mut() {
                rename_key(obj, "processName", "process_name");

                if let Some(controls) = obj
                    .get_mut("controls")
                    .and_then(|c| c.as_array_mut())
                {
                    for ctrl in controls.iter_mut() {
                        if let Some(co) = ctrl.as_object_mut() {
                            rename_key(co, "controlRef", "control_ref");
                            rename_key(co, "control_reference", "control_ref");
                            rename_key(co, "ref", "control_ref");
                            rename_key(co, "controlObjective", "control_objective");
                            rename_key(co, "objective", "control_objective");
                            rename_key(co, "controlDescription", "control_description");
                            rename_key(co, "control", "control_description");
                            rename_key(co, "testProcedure", "test_procedure");
                            rename_key(co, "procedure", "test_procedure");
                            rename_key(co, "riskLevel", "risk_level");
                            rename_key(co, "risk", "risk_level");
                            rename_key(co, "sopGap", "sop_gap");
                            rename_key(co, "gap", "sop_gap");
                            rename_key(co, "best_practice", "sop_gap");

                            // Normalise risk_level capitalisation.
                            if let Some(rl) = co.get_mut("risk_level") {
                                if let Some(s) = rl.as_str() {
                                    let fixed = match s.to_lowercase().as_str() {
                                        "high" | "h" => "High",
                                        "medium" | "med" | "m" | "moderate" => "Medium",
                                        "low" | "l" => "Low",
                                        _ => s,
                                    };
                                    *rl = serde_json::Value::String(fixed.to_string());
                                }
                            }

                            // Normalise sop_gap: coerce "true"/"false" strings → bool.
                            if let Some(sg) = co.get_mut("sop_gap") {
                                if let Some(s) = sg.as_str() {
                                    *sg = serde_json::Value::Bool(
                                        s.eq_ignore_ascii_case("true"),
                                    );
                                }
                            }
                            // Default sop_gap to false if missing.
                            co.entry("sop_gap")
                                .or_insert(serde_json::Value::Bool(false));

                            // Strip zero-padding from control_ref (C-01 → C-1).
                            if let Some(cr) = co.get_mut("control_ref") {
                                if let Some(s) = cr.as_str() {
                                    *cr = serde_json::Value::String(normalize_control_ref(s));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    serde_json::to_string(&v).map_err(|e| format!("Serialisation error: {e}"))
}

fn rename_key(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    from: &str,
    to: &str,
) {
    if obj.contains_key(from) && !obj.contains_key(to) {
        if let Some(val) = obj.remove(from) {
            obj.insert(to.to_string(), val);
        }
    }
}

fn normalize_control_ref(s: &str) -> String {
    // Extract the trailing digits and reformat as C-N (no zero padding).
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.parse::<u32>() {
        Ok(n) => format!("C-{n}"),
        Err(_) => s.to_string(),
    }
}
