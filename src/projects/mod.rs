pub mod db;
pub mod ocr;
pub mod prompts;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ── Domain structs ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub file_type: String,
    pub uploaded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetails {
    pub client: String,
    pub engagement_ref: String,
    pub period_start: String,
    pub period_end: String,
    pub report_due: String,
    pub audit_type: String,
    pub notes: String,
    pub standards: Vec<String>,
    pub scope: String,
    pub materiality: String,
    pub risk_framework: String,
}

// ── Audit plan structs ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditProcess {
    pub id: String,
    pub sop_file_id: String,
    pub process_name: String,
    pub description: String,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Control {
    pub id: String,
    pub process_id: String,
    pub control_ref: String,
    pub control_objective: String,
    pub control_description: String,
    pub test_procedure: String,
    pub risk_level: String,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditProcessWithControls {
    pub id: String,
    pub sop_file_id: String,
    pub process_name: String,
    pub description: String,
    pub sort_order: i64,
    pub created_at: String,
    pub controls: Vec<Control>,
}

// ── Home directory ────────────────────────────────────────────────────────────

pub fn vinrouge_home() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME env var not set".to_string())?;
    Ok(PathBuf::from(home).join("VinRouge"))
}

// ── Project management ────────────────────────────────────────────────────────

pub fn create_project(name: &str, parent_dir: &Path) -> Result<Project, String> {
    let project_dir = parent_dir.join(name);
    let files_dir = project_dir.join("files");
    std::fs::create_dir_all(&files_dir)
        .map_err(|e| format!("Failed to create project directories: {e}"))?;

    let conn = db::open_project(&project_dir).map_err(|e| e.to_string())?;
    drop(conn); // just initialise the schema

    let home = vinrouge_home()?;
    let global_conn = db::open_global(&home).map_err(|e| e.to_string())?;

    let project = Project {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        path: project_dir.to_string_lossy().to_string(),
        created_at: Utc::now().to_rfc3339(),
    };

    global_conn
        .execute(
            "INSERT INTO projects (id, name, path, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![project.id, project.name, project.path, project.created_at],
        )
        .map_err(|e| format!("DB insert failed: {e}"))?;

    Ok(project)
}

pub fn list_projects(home: &Path) -> Result<Vec<Project>, String> {
    let conn = db::open_global(home).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, path, created_at FROM projects ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;

    let projects = stmt
        .query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(projects)
}

pub fn delete_project(project_path: &Path) -> Result<(), String> {
    let home = vinrouge_home()?;
    let conn = db::open_global(&home).map_err(|e| e.to_string())?;
    let path_str = project_path.to_string_lossy().to_string();

    conn.execute(
        "DELETE FROM projects WHERE path = ?1",
        rusqlite::params![path_str],
    )
    .map_err(|e| format!("DB delete failed: {e}"))?;

    if project_path.exists() {
        std::fs::remove_dir_all(project_path)
            .map_err(|e| format!("Failed to remove project directory: {e}"))?;
    }

    Ok(())
}

pub fn load_project(project_path: &Path) -> Result<Project, String> {
    // Read project entry from the global DB by path
    let home = vinrouge_home()?;
    let conn = db::open_global(&home).map_err(|e| e.to_string())?;
    let path_str = project_path.to_string_lossy().to_string();

    conn.query_row(
        "SELECT id, name, path, created_at FROM projects WHERE path = ?1",
        rusqlite::params![path_str],
        |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                created_at: row.get(3)?,
            })
        },
    )
    .map_err(|e| format!("Project not found: {e}"))
}

// ── File management ───────────────────────────────────────────────────────────

pub fn add_file_to_project(project_dir: &Path, src_path: &Path) -> Result<ProjectFile, String> {
    let file_name = src_path
        .file_name()
        .ok_or("Source path has no filename")?
        .to_string_lossy()
        .to_string();

    let ext = src_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let dest = project_dir.join("files").join(&file_name);
    std::fs::copy(src_path, &dest)
        .map_err(|e| format!("Failed to copy file: {e}"))?;

    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;

    let file = ProjectFile {
        id: Uuid::new_v4().to_string(),
        name: file_name,
        path: dest.to_string_lossy().to_string(),
        file_type: ext,
        uploaded_at: Utc::now().to_rfc3339(),
    };

    conn.execute(
        "INSERT INTO files (id, name, path, type, uploaded_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![file.id, file.name, file.path, file.file_type, file.uploaded_at],
    )
    .map_err(|e| format!("DB insert failed: {e}"))?;

    Ok(file)
}

pub fn list_project_files(project_dir: &Path) -> Result<Vec<ProjectFile>, String> {
    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, path, type, uploaded_at FROM files ORDER BY uploaded_at ASC")
        .map_err(|e| e.to_string())?;

    let files = stmt
        .query_map([], |row| {
            Ok(ProjectFile {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                file_type: row.get(3)?,
                uploaded_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(files)
}

// ── AI context ────────────────────────────────────────────────────────────────

pub fn save_ai_message(project_dir: &Path, role: &str, content: &str) -> Result<AiMessage, String> {
    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;

    let msg = AiMessage {
        id: Uuid::new_v4().to_string(),
        role: role.to_string(),
        content: content.to_string(),
        created_at: Utc::now().to_rfc3339(),
    };

    conn.execute(
        "INSERT INTO ai_context (id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![msg.id, msg.role, msg.content, msg.created_at],
    )
    .map_err(|e| format!("DB insert failed: {e}"))?;

    Ok(msg)
}

pub fn list_ai_messages(project_dir: &Path) -> Result<Vec<AiMessage>, String> {
    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, role, content, created_at FROM ai_context ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;

    let messages = stmt
        .query_map([], |row| {
            Ok(AiMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(messages)
}

// ── Analysis results ──────────────────────────────────────────────────────────

pub fn save_analysis(project_dir: &Path, file_id: &str, json: &str) -> Result<String, String> {
    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO analysis_results (id, file_id, result_json, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, file_id, json, now],
    )
    .map_err(|e| format!("DB insert failed: {e}"))?;

    Ok(id)
}

pub fn save_project_details(project_dir: &Path, details: &ProjectDetails) -> Result<(), String> {
    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;
    let standards_json = serde_json::to_string(&details.standards).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO project_details
            (id, client, engagement_ref, period_start, period_end, report_due,
             audit_type, notes, standards, scope, materiality, risk_framework)
         VALUES ('singleton', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            details.client,
            details.engagement_ref,
            details.period_start,
            details.period_end,
            details.report_due,
            details.audit_type,
            details.notes,
            standards_json,
            details.scope,
            details.materiality,
            details.risk_framework,
        ],
    )
    .map_err(|e| format!("DB insert failed: {e}"))?;
    Ok(())
}

pub fn load_project_details(project_dir: &Path) -> Result<Option<ProjectDetails>, String> {
    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;
    let result = conn.query_row(
        "SELECT client, engagement_ref, period_start, period_end, report_due,
                audit_type, notes, standards, scope, materiality, risk_framework
         FROM project_details WHERE id = 'singleton'",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
            ))
        },
    );
    match result {
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("DB query failed: {e}")),
        Ok((client, engagement_ref, period_start, period_end, report_due,
            audit_type, notes, standards_json, scope, materiality, risk_framework)) => {
            let standards: Vec<String> =
                serde_json::from_str(&standards_json).unwrap_or_default();
            Ok(Some(ProjectDetails {
                client,
                engagement_ref,
                period_start,
                period_end,
                report_due,
                audit_type,
                notes,
                standards,
                scope,
                materiality,
                risk_framework,
            }))
        }
    }
}

// ── Audit plan ────────────────────────────────────────────────────────────────

/// Delete all processes (and their controls) generated from a specific SOP file,
/// then insert the new batch. This keeps the table tidy on re-analysis.
pub fn replace_audit_plan(
    project_dir: &Path,
    sop_file_id: &str,
    processes: &[(String, String, Vec<(String, String, String, String, String)>)],
    // each process: (process_name, description, Vec<(ref, objective, desc, test, risk)>)
) -> Result<(), String> {
    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;

    // Remove old data for this file
    conn.execute(
        "DELETE FROM controls WHERE process_id IN \
         (SELECT id FROM audit_processes WHERE sop_file_id = ?1)",
        rusqlite::params![sop_file_id],
    ).map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM audit_processes WHERE sop_file_id = ?1",
        rusqlite::params![sop_file_id],
    ).map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();

    for (sort_p, (pname, pdesc, controls)) in processes.iter().enumerate() {
        let pid = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO audit_processes (id, sop_file_id, process_name, description, sort_order, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![pid, sop_file_id, pname, pdesc, sort_p as i64, now],
        ).map_err(|e| format!("DB insert process: {e}"))?;

        for (sort_c, (cref, cobjective, cdesc, ctest, crisk)) in controls.iter().enumerate() {
            let cid = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO controls \
                 (id, process_id, control_ref, control_objective, control_description, test_procedure, risk_level, sort_order, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![cid, pid, cref, cobjective, cdesc, ctest, crisk, sort_c as i64, now],
            ).map_err(|e| format!("DB insert control: {e}"))?;
        }
    }

    Ok(())
}

pub fn list_audit_plan(project_dir: &Path) -> Result<Vec<AuditProcessWithControls>, String> {
    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;

    let mut pstmt = conn
        .prepare("SELECT id, sop_file_id, process_name, description, sort_order, created_at \
                  FROM audit_processes ORDER BY sort_order ASC")
        .map_err(|e| e.to_string())?;

    let process_rows = pstmt
        .query_map([], |row| {
            Ok(AuditProcessWithControls {
                id:           row.get(0)?,
                sop_file_id:  row.get(1)?,
                process_name: row.get(2)?,
                description:  row.get(3)?,
                sort_order:   row.get(4)?,
                created_at:   row.get(5)?,
                controls:     vec![],
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut result = Vec::with_capacity(process_rows.len());
    for mut proc in process_rows {
        let mut cstmt = conn
            .prepare("SELECT id, process_id, control_ref, control_objective, control_description, \
                      test_procedure, risk_level, sort_order, created_at \
                      FROM controls WHERE process_id = ?1 ORDER BY sort_order ASC")
            .map_err(|e| e.to_string())?;

        let controls = cstmt
            .query_map(rusqlite::params![proc.id], |row| {
                Ok(Control {
                    id:                  row.get(0)?,
                    process_id:          row.get(1)?,
                    control_ref:         row.get(2)?,
                    control_objective:   row.get(3)?,
                    control_description: row.get(4)?,
                    test_procedure:      row.get(5)?,
                    risk_level:          row.get(6)?,
                    sort_order:          row.get(7)?,
                    created_at:          row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        proc.controls = controls;
        result.push(proc);
    }

    Ok(result)
}

pub fn update_control_field(
    project_dir: &Path,
    control_id: &str,
    field: &str,
    value: &str,
) -> Result<(), String> {
    let sql = match field {
        "control_ref"         => "UPDATE controls SET control_ref         = ?1 WHERE id = ?2",
        "control_objective"   => "UPDATE controls SET control_objective   = ?1 WHERE id = ?2",
        "control_description" => "UPDATE controls SET control_description = ?1 WHERE id = ?2",
        "test_procedure"      => "UPDATE controls SET test_procedure      = ?1 WHERE id = ?2",
        "risk_level"          => "UPDATE controls SET risk_level          = ?1 WHERE id = ?2",
        _ => return Err(format!("Unknown control field: {field}")),
    };
    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;
    conn.execute(sql, rusqlite::params![value, control_id])
        .map_err(|e| format!("DB update: {e}"))?;
    Ok(())
}

pub fn update_process_field(
    project_dir: &Path,
    process_id: &str,
    field: &str,
    value: &str,
) -> Result<(), String> {
    let sql = match field {
        "process_name" => "UPDATE audit_processes SET process_name = ?1 WHERE id = ?2",
        "description"  => "UPDATE audit_processes SET description  = ?1 WHERE id = ?2",
        _ => return Err(format!("Unknown process field: {field}")),
    };
    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;
    conn.execute(sql, rusqlite::params![value, process_id])
        .map_err(|e| format!("DB update: {e}"))?;
    Ok(())
}

/// Read the text content of a project file.
///
/// - `.txt` / other plain-text formats → `read_to_string`
/// - `.pdf` with a text layer → `pdf-extract`
/// - `.pdf` that appears to be scanned (< 50 words extracted) → OCR via
///   `pdftoppm` + `tesseract` CLI tools (both available via `brew install
///   poppler tesseract`)
pub fn read_project_file_text(project_dir: &Path, file_id: &str) -> Result<String, String> {
    let conn = db::open_project(project_dir).map_err(|e| e.to_string())?;
    let path: String = conn
        .query_row(
            "SELECT path FROM files WHERE id = ?1",
            rusqlite::params![file_id],
            |row| row.get(0),
        )
        .map_err(|_| format!("File {file_id} not found in project"))?;

    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "pdf" => {
            // 1. Try the embedded text layer first (fast, no models needed)
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("Could not read PDF {path}: {e}"))?;
            let text = pdf_extract::extract_text_from_mem(&bytes)
                .unwrap_or_default();

            // 2. If sparse the PDF is likely scanned — fall back to OCR
            if text.split_whitespace().count() >= 50 {
                Ok(text)
            } else {
                ocr::ocr_pdf(&path).or(Ok(text))
            }
        }
        _ => std::fs::read_to_string(&path)
            .map_err(|e| format!("Could not read file {path}: {e}")),
    }
}

