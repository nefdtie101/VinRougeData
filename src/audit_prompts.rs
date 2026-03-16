/// Static audit prompt constants — available on all targets including WASM.

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

pub const ANALYZE_SOP: &str =
    "You are an audit planning assistant. Analyze the Standard Operating Procedure below \
     and produce a structured audit plan.\n\n\
     Return ONLY a valid JSON object — no markdown fences, no explanation — with this exact shape:\n\
     {\n\
       \"processes\": [\n\
         {\n\
           \"process_name\": \"Name of the business process\",\n\
           \"description\": \"One-sentence description of what this process does\",\n\
           \"controls\": [\n\
             {\n\
               \"control_ref\": \"C-01\",\n\
               \"control_objective\": \"What this control is designed to achieve\",\n\
               \"control_description\": \"How the control operates in practice\",\n\
               \"test_procedure\": \"Step-by-step procedure an auditor would follow to test this control\",\n\
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
     - Each process should have between 1 and 5 controls\n\
     - Return ONLY the JSON object, nothing else\n\n\
     SOP TEXT:";
