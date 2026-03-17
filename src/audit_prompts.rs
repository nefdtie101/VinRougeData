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
           \"control_ref\": \"C-01\",\n\
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
    "You are an audit data analyst. Given the audit plan below, generate a Provided-By-Client (PBC) \
     data request list. For each control, produce one or more data requests specifying the exact \
     database table and column-level fields the auditor needs to test that control.\n\n\
     Return ONLY a valid JSON object — no markdown, no explanation:\n\
     {\n\
       \"items\": [\n\
         {\n\
           \"control_ref\": \"C-01\",\n\
           \"name\": \"Short descriptive name for this request\",\n\
           \"item_type\": \"SQL\",\n\
           \"table_name\": \"exact_table_name_or_null\",\n\
           \"fields\": [\"field1\", \"field2\"],\n\
           \"purpose\": \"One sentence: what this data proves\",\n\
           \"scope_format\": \"e.g. All records in audit period\"\n\
         }\n\
       ]\n\
     }\n\n\
     Rules:\n\
     - item_type must be exactly SQL or CSV\n\
     - Use SQL when data comes from a database table; CSV when it is a manual register or file upload\n\
     - table_name must be null for CSV items\n\
     - fields must be specific column names likely to exist in the real system\n\
     - Generate 1-3 requests per control depending on complexity\n\
     - Return ONLY the JSON object\n\n\
     AUDIT PLAN (JSON):";

pub const ANALYZE_SOP: &str =
    "You are an audit planning assistant. Read the Standard Operating Procedure (SOP) below \
     carefully and produce a structured audit plan grounded entirely in the SOP's content.\n\n\
     IMPORTANT — ground every field in the SOP:\n\
     - process_name: use the exact name or heading from the SOP wherever possible\n\
     - description: reference specific activities, actors, or systems named in the SOP\n\
     - control_objective: state what specific risk or failure mode from the SOP this control addresses\n\
     - control_description: describe how the control operates using the SOP's own terminology — \
       name the roles, systems, forms, thresholds, or approval steps mentioned in the SOP\n\
     - test_procedure: write concrete steps an auditor would follow, referencing the SOP's named \
       documents, systems, or roles (e.g. \"Obtain the approval log from [system named in SOP] and \
       verify that each entry bears the authorised signatory defined in section X\")\n\
     - risk_level: assign based on the consequence of the control failing given what the SOP describes\n\n\
     Return ONLY a valid JSON object — no markdown fences, no explanation — with this exact shape:\n\
     {\n\
       \"processes\": [\n\
         {\n\
           \"process_name\": \"Exact name from SOP\",\n\
           \"description\": \"What this process does, using SOP-specific details\",\n\
           \"controls\": [\n\
             {\n\
               \"control_ref\": \"C-01\",\n\
               \"control_objective\": \"Risk or failure mode this control prevents, as described in the SOP\",\n\
               \"control_description\": \"How the control operates, naming SOP roles/systems/documents\",\n\
               \"test_procedure\": \"Step-by-step audit test referencing SOP artefacts and roles\",\n\
               \"risk_level\": \"High\"\n\
             }\n\
           ]\n\
         }\n\
       ]\n\
     }\n\n\
     Rules:\n\
     - risk_level must be exactly one of: High, Medium, Low\n\
     - control_ref must be sequential and unique across all processes (C-01, C-02, …)\n\
     - Each distinct business process should be a separate entry\n\
     - Each process should have between 2 and 6 controls\n\
     - Do NOT use generic filler language — every sentence must reflect a specific fact from the SOP\n\
     - Return ONLY the JSON object, nothing else\n\n\
     SOP TEXT:";
