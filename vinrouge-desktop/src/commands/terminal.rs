use portable_pty::{CommandBuilder, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use crate::state::{CurrentScriptState, ProjectsState};
use tauri::Manager;

// ── PTY state ─────────────────────────────────────────────────────────────────

pub struct PtyState {
    pub writer: Mutex<Option<Box<dyn Write + Send>>>,
    pub master: Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>,
    pub child: Mutex<Option<Box<dyn portable_pty::Child + Send + Sync>>>,
}

impl Default for PtyState {
    fn default() -> Self {
        Self {
            writer: Mutex::new(None),
            master: Mutex::new(None),
            child: Mutex::new(None),
        }
    }
}

// ── IPC server (one JSON line in → one JSON line out) ─────────────────────────

fn start_ipc_server(app: AppHandle, project_dir: PathBuf) -> u16 {
    // Bind on a random port before spawning so we can return the port immediately.
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind IPC port");
    std_listener.set_nonblocking(true).unwrap();
    let port = std_listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let listener = TcpListener::from_std(std_listener).unwrap();
        loop {
            let Ok((stream, _addr)) = listener.accept().await else {
                continue;
            };
            let app = app.clone();
            let dir = project_dir.clone();
            tokio::spawn(async move {
                let (read_half, mut write_half) = stream.into_split();
                let mut lines = BufReader::new(read_half).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let response = handle_ipc(&line, &dir, &app);
                    let _ = write_half.write_all(response.as_bytes()).await;
                    let _ = write_half.write_all(b"\n").await;
                }
            });
        }
    });

    port
}

fn handle_ipc(line: &str, project_dir: &PathBuf, app: &AppHandle) -> String {
    let cmd: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return json_err(&format!("parse error: {e}")),
    };

    match cmd["action"].as_str().unwrap_or("") {
        "list" => match vinrouge::projects::list_dsl_scripts(project_dir) {
            Ok(scripts) => serde_json::to_string(&scripts).unwrap_or_else(|_| "[]".into()),
            Err(e) => json_err(&e),
        },
        "update" => {
            let id = match cmd["id"].as_str() {
                Some(s) => s,
                None => return json_err("missing field: id"),
            };
            let text = match cmd["script_text"].as_str() {
                Some(s) => s,
                None => return json_err("missing field: script_text"),
            };
            match vinrouge::projects::update_dsl_script(project_dir, id, text) {
                Ok(_) => {
                    let _ = app.emit("scripts-changed", ());
                    r#"{"ok":true}"#.into()
                }
                Err(e) => json_err(&e),
            }
        }
        "create" => {
            let label = cmd["label"].as_str().unwrap_or("Untitled");
            let text = cmd["script_text"].as_str().unwrap_or("");
            let control_id = cmd["control_id"].as_str().unwrap_or("");
            let control_ref = cmd["control_ref"].as_str().unwrap_or("");
            match vinrouge::projects::save_dsl_script(
                project_dir,
                control_id,
                control_ref,
                label,
                text,
            ) {
                Ok(script) => {
                    let _ = app.emit("scripts-changed", ());
                    serde_json::to_string(&script).unwrap_or_else(|_| r#"{"ok":true}"#.into())
                }
                Err(e) => json_err(&e),
            }
        }
        "schema" => match crate::helpers::schema_json(project_dir) {
            Ok(tables) => serde_json::to_string(&tables).unwrap_or_else(|_| "[]".into()),
            Err(e) => json_err(&e),
        },
        "validate" => {
            let text = match cmd["script_text"].as_str() {
                Some(s) => s,
                None => return json_err("missing field: script_text"),
            };
            let statements = match vinrouge::dsl::parse(text) {
                Ok(s) => s,
                Err(e) => {
                    return json_err(&format!("parse error at {}: {}", e.position, e.message))
                }
            };
            match crate::helpers::schema_from_imports(project_dir) {
                Ok(schema) => {
                    let errors = vinrouge::dsl::resolve(&statements, &schema);
                    if errors.is_empty() {
                        r#"{"ok":true}"#.into()
                    } else {
                        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
                        serde_json::to_string(&serde_json::json!({"errors": msgs}))
                            .unwrap_or_else(|_| json_err("serialisation error"))
                    }
                }
                Err(e) => json_err(&e),
            }
        }
        "current" => {
            let state = app.state::<CurrentScriptState>();
            let guard = state.0.lock().unwrap();
            match guard.as_ref() {
                Some(s) => serde_json::to_string(s).unwrap_or_else(|_| "null".into()),
                None => "null".into(),
            }
        }
        other => json_err(&format!("unknown action: {other}")),
    }
}

fn json_err(msg: &str) -> String {
    format!(
        r#"{{"error":{}}}"#,
        serde_json::Value::String(msg.to_string())
    )
}

// ── Helper script written into the project directory ─────────────────────────

fn write_helper_script(project_dir: &PathBuf, port: u16) {
    // A small shell function file the agent can source or call directly.
    let script = format!(
        r#"#!/usr/bin/env bash
# ============================================================
# VinRouge DSL script bridge — auto-generated, do not commit.
# ============================================================
#
# AGENT INSTRUCTIONS (Claude Code / LLM running in this terminal)
# ---------------------------------------------------------------
# You are a DSL script writer for the VinRouge audit platform.
# Your job is to write and update VinRouge DSL scripts (.vrd)
# that run audit checks against the data files loaded in the
# current project.
#
# CONTEXT
#   - Run `git diff HEAD` and `git log --oneline -10` to understand
#     what audit work is in progress and what data sources are active.
#   - Run `./vu list` to see all existing scripts with their IDs.
#   - The data tables available to your scripts match the files
#     imported in the project (check git history for filenames).
#
# TOOLS
#   ./vu list
#       Returns a JSON array of scripts: [{{id, label, script_text}}]
#
#   ./vu update <id> '<dsl script>'
#       Replaces the body of script <id>. The UI refreshes live.
#
#   ./vu create <label> '<dsl script>'
#       Creates a new named script. Returns the new script object.
#
#   ./vu create <label> '<dsl script>' [control_id] [control_ref]
#       Creates a script linked to an audit control.
#
# DSL LANGUAGE REFERENCE
#   Statements are optionally labelled:  label: EXPR
#
#   ASSERT — compare an aggregate or expression to an expected value
#     rec_check: ASSERT SUM(invoices.amount) WHERE status = "paid"
#                = payments_control
#     blank_ids: ASSERT COUNT(employees.id) WHERE IS_BLANK(employees.id) = 0
#
#   SAMPLE — draw an audit sample from a table
#     mus_sample:  SAMPLE MUS invoices.amount 50 WHERE amount > 0
#     rand_sample: SAMPLE RANDOM invoices.id 10%
#     strat:       SAMPLE STRATIFIED invoices.amount 25 WHERE status = "open"
#     sys:         SAMPLE SYSTEMATIC invoices.id 20
#
#   CHART — visualise an aggregate by a dimension
#     by_status: CHART bar SUM(invoices.amount) BY invoices.status
#     by_month:  CHART line COUNT(invoices.id) BY invoices.month
#
#   SECTION — group related checks
#     SECTION "Completeness" {{
#       ASSERT COUNT(invoices.id) = invoice_control_count
#       ASSERT SUM(invoices.amount) = invoice_control_total
#     }}
#
#   SCHEMA — print all imported tables and their columns
#     SCHEMA
#
#   AGGREGATES:  SUM(t.col) COUNT(t.col) AVG(t.col) MIN(t.col) MAX(t.col)
#                COUNT(DISTINCT t.col)
#   All aggregates accept:  WHERE <filter-expr>
#
#   FILTERS:
#     Comparison:  =  <>  >  >=  <  <=
#     Logic:       AND  OR  NOT
#     Membership:  col IN ("a","b")   col NOT IN ("x","y")
#     Range:       amount BETWEEN 1000 AND 50000
#     Null:        IS NULL   IS NOT NULL
#     Pattern:     col LIKE "INV-%"
#     Blank:       IS_BLANK(col)   IS_NOT_BLANK(col)
#     Numeric:     IS_NUMERIC(col)
#     Date:        IS_DATE(col)   DATE(col) >= DATE("2024-01-01")
#     Duplicates:  DUPLICATED(t.id)   DUPLICATED(t.a, t.b)
#     SA ID:       SA_ID_VALID(t.id_col)
#     Cross-table: col NOT IN other_table.column
#
#   FUNCTIONS:
#     String:  UPPER(t.col)  LOWER(t.col)  TRIM(t.col)  LENGTH(t.col)
#              SUBSTR(t.col, start, len)
#              CONCAT("prefix", t.col, "suffix")
#     Math:    ABS(t.col)   ROUND(t.col, 2)
#     Null:    COALESCE(t.col, 0)   NULLIF(t.col, 0)
#     Control: CASE WHEN cond THEN val ELSE default END
#
#   RELATIONS (metadata, no filter effect):
#     RELATION invoices.employee_id -> employees.id
#
# WORKFLOW
#   1. `git diff HEAD` — understand what changed and what tables exist
#   2. `./vu list` — see current scripts
#   3. Write or update scripts using the DSL above
#   4. Use `./vu update <id> '<script>'` or `./vu create <label> '<script>'`
#   5. The VinRouge UI reruns the script and shows results immediately
#
# ============================================================
VINROUGE_PORT={port}
_vu_send() {{ exec 3<>/dev/tcp/127.0.0.1/$VINROUGE_PORT; printf '%s\n' "$1" >&3; IFS= read -r _vu_r <&3; exec 3>&-; printf '%s\n' "$_vu_r"; }}
case "$1" in
  list)     _vu_send '{{"action":"list"}}' ;;
  schema)   _vu_send '{{"action":"schema"}}' ;;
  current)  _vu_send '{{"action":"current"}}' ;;
  validate) _vu_send "{{\"action\":\"validate\",\"script_text\":\"$2\"}}" ;;
  update)   _vu_send "{{\"action\":\"update\",\"id\":\"$2\",\"script_text\":\"$3\"}}" ;;
  create)   _vu_send "{{\"action\":\"create\",\"label\":\"$2\",\"script_text\":\"$3\",\"control_id\":\"${{4:-}}\",\"control_ref\":\"${{5:-}}\"}}" ;;
  *)        echo "Usage: vu list | schema | current | validate '<script>' | update <id> '<script>' | create <label> '<script>' [control_id] [control_ref]" ;;
esac
"#
    );

    let path = project_dir.join("vu");
    if std::fs::write(&path, &script).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
        }
    }

    // Windows: write vu.ps1 (PowerShell) + vu.bat (thin launcher) so cmd.exe can call `vu`
    let ps1 = format!(
        r#"# VinRouge DSL bridge — auto-generated, do not commit.
param(
    [string]$Action,
    [string]$Arg2,
    [string]$Arg3,
    [string]$Arg4,
    [string]$Arg5
)

$port = [int]$env:VINROUGE_PORT
if (-not $port) {{ $port = {port} }}

function Send-Ipc([string]$json) {{
    $c = New-Object System.Net.Sockets.TcpClient('127.0.0.1', $port)
    $s = $c.GetStream()
    $w = New-Object System.IO.StreamWriter($s)
    $r = New-Object System.IO.StreamReader($s)
    $w.WriteLine($json)
    $w.Flush()
    $result = $r.ReadLine()
    $c.Close()
    Write-Output $result
}}

switch ($Action) {{
    'list'     {{ Send-Ipc '{{"action":"list"}}' }}
    'schema'   {{ Send-Ipc '{{"action":"schema"}}' }}
    'current'  {{ Send-Ipc '{{"action":"current"}}' }}
    'validate' {{ Send-Ipc (ConvertTo-Json @{{action='validate'; script_text=$Arg2}} -Compress) }}
    'update'   {{ Send-Ipc (ConvertTo-Json @{{action='update'; id=$Arg2; script_text=$Arg3}} -Compress) }}
    'create'   {{ Send-Ipc (ConvertTo-Json @{{action='create'; label=$Arg2; script_text=$Arg3; control_id=$Arg4; control_ref=$Arg5}} -Compress) }}
    default    {{ Write-Host 'Usage: vu list | schema | current | validate "<script>" | update <id> "<script>" | create <label> "<script>" [control_id] [control_ref]' }}
}}
"#
    );
    let _ = std::fs::write(project_dir.join("vu.ps1"), &ps1);

    let bat = r#"@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0vu.ps1" %*
"#;
    let _ = std::fs::write(project_dir.join("vu.bat"), bat);

    let claude_md = r#"# VinRouge DSL Agent

You are a DSL script writer for the VinRouge audit platform running inside a live project.
The VinRouge app is open and watching this directory — any script you save will run and
display results in the UI immediately.

## Orientation (do this first)

The UI sets these environment variables before launching this terminal:

| Variable | Contains |
|---|---|
| `VU_SCRIPT_ID` | ID of the script currently open in the editor (may be empty if none) |
| `VU_SCRIPT_LABEL` | Label of that script |
| `VU_SCRIPT_TEXT` | Full current content of that script |

**Always start by reading these.** They tell you what script was open at terminal launch.
Use `./vu current` to get the *live* currently-open script (updated whenever the user switches):

```bash
./vu current                 # live JSON: {id, label, script_text, …} — most reliable
echo "$VU_SCRIPT_TEXT"       # snapshot from terminal launch — use as fallback only
```

Then orient yourself further:

```bash
git log --oneline -10        # understand what audit work is in progress
git diff HEAD                # see what changed — reveals active data tables
./vu list                    # list all existing scripts with their IDs
```

The data tables available to your scripts are the files in the `files/` subdirectory
(Excel/CSV). Their table names in DSL match the filename without extension, lowercased,
with spaces replaced by underscores.

## Your tools

| Command | What it does |
|---|---|
| `./vu current` | Returns the script currently open in the studio (`{id, label, script_text, …}` or `null`) |
| `./vu list` | Returns JSON array: `[{id, label, script_text}]` |
| `./vu schema` | Returns live tables + columns from loaded data files |
| `./vu validate '<dsl>'` | Parses and resolves the script — returns `{"ok":true}` or `{"errors":[...]}` |
| `./vu update <id> '<dsl>'` | Replaces script body — UI reruns instantly |
| `./vu create <label> '<dsl>'` | Creates a new named script |
| `./vu create <label> '<dsl>' <control_id> <control_ref>` | Creates script linked to an audit control |

Always single-quote the DSL argument to avoid shell interpolation.

**Always validate before writing.** Run `./vu validate '<script>'` first. Only call
`update` or `create` once you get `{"ok":true}`. If you get errors, fix the script
and validate again — column names must exactly match `./vu schema` output.

## DSL language

```
# Labelled statement (label is optional)
label: EXPR

# ASSERT — compare aggregate to expected value
rec_check:   ASSERT SUM(invoices.amount) WHERE status = "paid" = payments_control
blank_ids:   ASSERT COUNT(employees.id) WHERE IS_BLANK(employees.id) = 0
dup_check:   ASSERT COUNT(invoices.id) WHERE DUPLICATED(invoices.id) = 0

# ASSERT with failing rows table — append SHOW FAILURES IN TABLE to list every row that failed
neg_check:   ASSERT invoices.amount > 0 SHOW FAILURES IN TABLE
id_check:    ASSERT NOT IS_BLANK(employees.id) SHOW FAILURES IN TABLE

# SAMPLE — draw an audit sample
mus_sample:  SAMPLE MUS invoices.amount 50 WHERE amount > 0
rand_sample: SAMPLE RANDOM invoices.id 10%
strat:       SAMPLE STRATIFIED invoices.amount 25 WHERE status = "open"
sys:         SAMPLE SYSTEMATIC invoices.id 20

# CHART — visualise an aggregate
by_status:   CHART bar SUM(invoices.amount) BY invoices.status
by_month:    CHART line COUNT(invoices.id) BY invoices.month

# SECTION — group related checks
SECTION "Completeness" {
  ASSERT COUNT(invoices.id) = invoice_control_count
  ASSERT SUM(invoices.amount) = invoice_control_total
}

# SCHEMA — inspect all imported tables and columns
SCHEMA
```

**Aggregates:** `SUM` `COUNT` `AVG` `MIN` `MAX` — all accept `WHERE <filter>`
`COUNT(DISTINCT t.col)`

**Filters:**
- Comparison: `=  <>  >  >=  <  <=`
- Logic: `AND  OR  NOT`
- Membership: `col IN ("a","b")`  `col NOT IN ("x","y")`
- Range: `amount BETWEEN 1000 AND 50000`
- Null: `IS NULL`  `IS NOT NULL`
- Pattern: `col LIKE "INV-%"`
- Blank: `IS_BLANK(col)`  `IS_NOT_BLANK(col)`
- Type: `IS_NUMERIC(col)`  `IS_DATE(col)`
- Duplicates: `DUPLICATED(t.id)`  `DUPLICATED(t.a, t.b)`
- SA ID: `SA_ID_VALID(t.id_col)`
- Cross-table: `t.col NOT IN other_table.other_col`
- Date: `DATE(t.col) >= DATE("2024-01-01")`

**Scalar functions:**
`UPPER` `LOWER` `TRIM` `LENGTH` `SUBSTR(t.col, start, len)` `CONCAT(...)` `ABS` `ROUND(t.col, 2)` `COALESCE(t.col, 0)` `NULLIF(t.col, 0)`
`CASE WHEN cond THEN val ELSE default END`

**ASSERT modifier — show failing rows as a table:**
`ASSERT <row-level-expr> SHOW FAILURES IN TABLE`
Only works on row-level expressions (column references, not aggregates). Outputs every failing row as an inline table in the UI. Omit the modifier when you only need the pass/fail count.

**Relations (metadata only):**
`RELATION invoices.employee_id -> employees.id`

## Workflow

1. `./vu current` — read the script currently open in the UI (live, always up to date)
2. `git log --oneline -10` + `git diff HEAD` — understand the audit context
3. `./vu schema` — get exact table and column names (use these verbatim)
4. `./vu list` — see all scripts if you need to work across multiple
5. Draft or edit your DSL script
6. `./vu validate '<script>'` — must return `{"ok":true}` before writing
7. `./vu update $VU_SCRIPT_ID '<script>'` to update the open script, or `./vu create <label> '<script>'` for a new one
8. The UI reruns immediately — iterate as needed
"#;

    let _ = std::fs::write(project_dir.join("CLAUDE.md"), claude_md);
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn pty_create(
    app: AppHandle,
    pty_state: State<'_, PtyState>,
    projects: State<'_, ProjectsState>,
    env: Option<HashMap<String, String>>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<(), String> {
    let project_dir = projects
        .dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let port = start_ipc_server(app.clone(), project_dir.clone());
    write_helper_script(&project_dir, port);

    let pty_system = portable_pty::native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: rows.unwrap_or(24),
            cols: cols.unwrap_or(80),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let shell = if cfg!(target_os = "windows") {
        std::env::var("COMSPEC").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".to_string())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
    };

    let mut cmd = CommandBuilder::new(&shell);
    let scripts_file = project_dir.join(".vinrouge").join("scripts.json");

    cmd.cwd(&project_dir);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("VINROUGE_PORT", port.to_string());
    cmd.env("VINROUGE_PROJECT", project_dir.to_string_lossy().as_ref());
    cmd.env("DSL_SCRIPTS_FILE", scripts_file.to_string_lossy().as_ref());
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            project_dir.to_string_lossy(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );

    if let Some(vars) = env {
        for (k, v) in vars {
            cmd.env(k, v);
        }
    }

    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);

    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

    *pty_state.writer.lock().unwrap() = Some(writer);
    *pty_state.master.lock().unwrap() = Some(pair.master);
    *pty_state.child.lock().unwrap() = Some(child);

    let (tx, rx) = std::sync::mpsc::channel::<String>();

    std::thread::spawn(move || {
        let mut buf = [0u8; 65536];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let _ = tx.send(String::from_utf8_lossy(&buf[..n]).into_owned());
                }
            }
        }
    });

    std::thread::spawn(move || {
        let mut pending = String::new();
        loop {
            match rx.recv() {
                Ok(chunk) => pending.push_str(&chunk),
                Err(_) => break,
            }
            while let Ok(more) = rx.try_recv() {
                pending.push_str(&more);
                if pending.len() > 65536 { break; }
            }
            let _ = app.emit("pty-data", std::mem::take(&mut pending));
        }
        let _ = app.emit("pty-exit", ());
    });

    Ok(())
}

#[tauri::command]
pub fn pty_write(state: State<PtyState>, data: String) -> Result<(), String> {
    if let Some(w) = state.writer.lock().unwrap().as_mut() {
        w.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn pty_resize(state: State<PtyState>, cols: u16, rows: u16) -> Result<(), String> {
    if let Some(m) = state.master.lock().unwrap().as_ref() {
        m.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Called by the frontend whenever the user opens a different script in the studio editor.
/// Makes that script available to `./vu current` inside the running terminal.
#[tauri::command]
pub fn pty_set_current_script(
    state: State<'_, CurrentScriptState>,
    script: Option<vinrouge::projects::DslScript>,
) -> Result<(), String> {
    *state.0.lock().unwrap() = script;
    Ok(())
}

/// Keep .vinrouge/scripts.json in sync whenever the script list changes in the UI.
#[tauri::command]
pub fn pty_update_scripts(
    projects: State<ProjectsState>,
    scripts_json: String,
) -> Result<(), String> {
    let project_dir = projects.dir()?;
    let dir = project_dir.join(".vinrouge");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("scripts.json"), scripts_json).map_err(|e| e.to_string())
}
