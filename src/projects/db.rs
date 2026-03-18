use rusqlite::{Connection, Result};
use std::path::Path;

/// Open (or create) the global index DB at `~/VinRouge/vinrouge.db`.
pub fn open_global(home: &Path) -> Result<Connection> {
    let conn = Connection::open(home.join("vinrouge.db"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL UNIQUE,
            path       TEXT NOT NULL,
            created_at TEXT NOT NULL
        );",
    )?;
    Ok(conn)
}

/// Open (or create) the per-project DB at `<project_dir>/vinrouge.db`.
pub fn open_project(project_dir: &Path) -> Result<Connection> {
    let conn = Connection::open(project_dir.join("vinrouge.db"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS files (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            path        TEXT NOT NULL,
            type        TEXT NOT NULL,
            uploaded_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS ai_context (
            id         TEXT PRIMARY KEY,
            role       TEXT NOT NULL,
            content    TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS analysis_results (
            id          TEXT PRIMARY KEY,
            file_id     TEXT NOT NULL REFERENCES files(id),
            result_json TEXT NOT NULL,
            created_at  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS project_details (
            id              TEXT PRIMARY KEY DEFAULT 'singleton',
            client          TEXT NOT NULL DEFAULT '',
            engagement_ref  TEXT NOT NULL DEFAULT '',
            period_start    TEXT NOT NULL DEFAULT '',
            period_end      TEXT NOT NULL DEFAULT '',
            report_due      TEXT NOT NULL DEFAULT '',
            audit_type      TEXT NOT NULL DEFAULT 'Compliance',
            notes           TEXT NOT NULL DEFAULT '',
            standards       TEXT NOT NULL DEFAULT '[]',
            scope           TEXT NOT NULL DEFAULT '',
            materiality     TEXT NOT NULL DEFAULT '',
            risk_framework  TEXT NOT NULL DEFAULT 'High / Medium / Low'
        );
        CREATE TABLE IF NOT EXISTS audit_processes (
            id           TEXT PRIMARY KEY,
            sop_file_id  TEXT NOT NULL,
            process_name TEXT NOT NULL,
            description  TEXT NOT NULL DEFAULT '',
            sort_order   INTEGER NOT NULL DEFAULT 0,
            created_at   TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS controls (
            id                  TEXT PRIMARY KEY,
            process_id          TEXT NOT NULL,
            control_ref         TEXT NOT NULL DEFAULT '',
            control_objective   TEXT NOT NULL DEFAULT '',
            control_description TEXT NOT NULL DEFAULT '',
            test_procedure      TEXT NOT NULL DEFAULT '',
            risk_level          TEXT NOT NULL DEFAULT 'Medium',
            sort_order          INTEGER NOT NULL DEFAULT 0,
            created_at          TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS pbc_items (
            id           TEXT PRIMARY KEY,
            control_id   TEXT NOT NULL,
            control_ref  TEXT NOT NULL DEFAULT '',
            name         TEXT NOT NULL DEFAULT '',
            item_type    TEXT NOT NULL DEFAULT 'SQL',
            table_name   TEXT,
            fields       TEXT NOT NULL DEFAULT '[]',
            purpose      TEXT NOT NULL DEFAULT '',
            scope_format TEXT NOT NULL DEFAULT '',
            approved     INTEGER NOT NULL DEFAULT 0,
            sort_order   INTEGER NOT NULL DEFAULT 0,
            created_at   TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS pbc_list_status (
            id       TEXT PRIMARY KEY DEFAULT 'singleton',
            approved INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS session_imports (
            id            TEXT PRIMARY KEY,
            file_id       TEXT,
            source_type   TEXT NOT NULL,
            source_name   TEXT NOT NULL,
            row_count     INTEGER NOT NULL DEFAULT 0,
            mappings_json TEXT NOT NULL DEFAULT '[]',
            imported_at   TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS session_rows (
            id         TEXT PRIMARY KEY,
            import_id  TEXT NOT NULL REFERENCES session_imports(id) ON DELETE CASCADE,
            row_index  INTEGER NOT NULL,
            data_json  TEXT NOT NULL
        );",
    )?;
    Ok(conn)
}
