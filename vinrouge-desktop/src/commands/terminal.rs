use portable_pty::{CommandBuilder, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use crate::state::{CurrentScriptState, DslCacheState, ProjectsState};
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
        "run" => {
            let id = match cmd["id"].as_str() {
                Some(s) => s.to_string(),
                None => return json_err("missing field: id"),
            };
            let cache = app.state::<DslCacheState>();
            let cache_arc = cache.0.clone();
            let datasource = {
                let mut guard = cache_arc.lock().unwrap();
                let cache_hit =
                    guard.project_dir.as_ref() == Some(project_dir) && guard.datasource.is_some();
                if cache_hit {
                    guard.datasource.clone().unwrap()
                } else {
                    match crate::helpers::build_datasource(project_dir) {
                        Ok(ds) => {
                            let ds = std::sync::Arc::new(ds);
                            guard.project_dir = Some(project_dir.clone());
                            guard.datasource = Some(ds.clone());
                            ds
                        }
                        Err(e) => return json_err(&format!("datasource error: {e}")),
                    }
                }
            };
            match crate::helpers::run_dsl_script_blocking(id, project_dir.clone(), datasource) {
                Ok(results) => {
                    let _ = app.emit("dsl-run-results", serde_json::json!({ "results": results }));
                    serde_json::to_string(&results).unwrap_or_else(|_| "[]".into())
                }
                Err(e) => json_err(&e),
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

// ── Helper binary / scripts written into the project directory ───────────────

fn find_vu_binary() -> Option<std::path::PathBuf> {
    let exe_path = std::env::current_exe().ok()?;
    let exe_dir = exe_path.parent()?;

    // Candidate paths to search for the vu binary
    let mut candidates = vec![
        exe_dir.join(if cfg!(windows) { "vu.exe" } else { "vu" }),
    ];

    // During development: look in the root workspace target directory
    // relative to vinrouge-desktop's manifest dir
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest_dir.join("../target/debug/vu"));
    candidates.push(manifest_dir.join("../target/release/vu"));

    #[cfg(windows)]
    {
        candidates.push(manifest_dir.join("../target/debug/vu.exe"));
        candidates.push(manifest_dir.join("../target/release/vu.exe"));
    }

    candidates.into_iter().find(|p| p.exists())
}

fn write_helper_script(project_dir: &std::path::PathBuf, port: u16) {
    // Try to use the native Rust vu binary first
    if let Some(vu_src) = find_vu_binary() {
        let vu_dst = project_dir.join(if cfg!(windows) { "vu.exe" } else { "vu" });
        if std::fs::copy(&vu_src, &vu_dst).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&vu_dst, std::fs::Permissions::from_mode(0o755));
            }
            // Remove any old fallback scripts so they don't shadow the binary
            let _ = std::fs::remove_file(project_dir.join("vu.ps1"));
            let _ = std::fs::remove_file(project_dir.join("vu.bat"));
            // Continue to write CLAUDE.md below
        }
    } else {
        // Fallback: write the old bash script (with Python JSON encoding)
        let script = format!(
            r#"#!/usr/bin/env bash
# VinRouge DSL bridge fallback — auto-generated, do not commit.
VINROUGE_PORT={port}
_vu_send() {{
    if ! exec 3<>/dev/tcp/127.0.0.1/$VINROUGE_PORT 2>/dev/null; then
        printf '%s\n' '{{"error":"vu: cannot connect to VinRouge IPC server (port '"$VINROUGE_PORT"'). Is the terminal open?"}}'
        return 1
    fi
    printf '%s\n' "$1" >&3
    if ! IFS= read -r -t 120 _vu_r <&3; then
        printf '%s\n' '{{"error":"vu: IPC request timed out after 120s"}}'
        exec 3>&-
        return 1
    fi
    exec 3>&-
    printf '%s\n' "$_vu_r"
}}
case "$1" in
  list)     _vu_send '{{"action":"list"}}' ;;
  schema)   _vu_send '{{"action":"schema"}}' ;;
  current)  _vu_send '{{"action":"current"}}' ;;
  validate)
    if command -v python3 >/dev/null 2>&1; then
      payload=$(python3 -c "import json,sys; print(json.dumps({{'action':'validate','script_text':sys.argv[1]}}))" "$2")
    else
      payload="{{\"action\":\"validate\",\"script_text\":\"$2\"}}"
    fi
    _vu_send "$payload"
    ;;
  update)
    if command -v python3 >/dev/null 2>&1; then
      payload=$(python3 -c "import json,sys; print(json.dumps({{'action':'update','id':sys.argv[1],'script_text':sys.argv[2]}}))" "$2" "$3")
    else
      payload="{{\"action\":\"update\",\"id\":\"$2\",\"script_text\":\"$3\"}}"
    fi
    _vu_send "$payload"
    ;;
  create)
    if command -v python3 >/dev/null 2>&1; then
      payload=$(python3 -c "import json,sys; print(json.dumps({{'action':'create','label':sys.argv[1],'script_text':sys.argv[2],'control_id':sys.argv[3],'control_ref':sys.argv[4]}}))" "$2" "$3" "${{4:-}}" "${{5:-}}")
    else
      payload="{{\"action\":\"create\",\"label\":\"$2\",\"script_text\":\"$3\",\"control_id\":\"${{4:-}}\",\"control_ref\":\"${{5:-}}\"}}"
    fi
    _vu_send "$payload"
    ;;
  run)      _vu_send "{{\"action\":\"run\",\"id\":\"$2\"}}" ;;
  *)        echo "Usage: vu list | schema | current | validate '<script>' | update <id> '<script>' | create <label> '<script>' [control_id] [control_ref] | run <id>" ;;
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

        #[cfg(windows)]
        {
            let ps1 = format!(
                r#"# VinRouge DSL bridge fallback — auto-generated, do not commit.
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
    'run'      {{ Send-Ipc (ConvertTo-Json @{{action='run'; id=$Arg2}} -Compress) }}
    default    {{ Write-Host 'Usage: vu list | schema | current | validate "<script>" | update <id> "<script>" | create <label> "<script>" [control_id] [control_ref] | run <id>' }}
}}
"#
            );
            let _ = std::fs::write(project_dir.join("vu.ps1"), &ps1);
            let bat = r#"@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0vu.ps1" %*
"#;
            let _ = std::fs::write(project_dir.join("vu.bat"), bat);
        }
    }

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

```
./vu current                 # live JSON: {id, label, script_text, …} — most reliable
$env:VU_SCRIPT_TEXT          # snapshot from terminal launch — use as fallback only
```

Then orient yourself further:

```
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
| `./vu run <id>` | Executes a script and returns full JSON results (asserts, show rows, errors, etc.) |

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

# CSS — inject custom styles into the result output
# The CSS string is injected as a <style> tag at the top of the results panel.
my_styles: CSS 'svg rect { fill: #22c55e; }'
```

## Styling the output with CSS

The UI renders every DSL result into DOM elements with stable class names. You can target these classes with `CSS '...'` to change colours, layout, spacing, fonts, or visibility.

**Result-type classes** (apply to the outer card of each result):
- `.ide-result--pass` — assert that passed
- `.ide-result--fail` — assert that failed
- `.ide-result--sample` — sample result
- `.ide-result--chart` — chart result
- `.ide-result--screen` — screen (multi-chart grid)
- `.ide-result--section` — section card
- `.ide-result--schema` — schema result
- `.ide-result--show-rows` — SHOW ROWS table
- `.ide-result--error` — parser / runtime error
- `.ide-result--value` — raw value result

**Inner structure classes:**
- `.ide-section-body` — container inside a section that holds its child results (default: flex column)
- `.ide-section-header` — clickable header bar of a section
- `.ide-result-label` — label text of any result
- `.ide-result-val` — numeric / value text inside an assert
- `.ide-result-op` — operator text inside an assert (`=`, `>`, etc.)
- `.ide-sample-table` — table rendered for samples or SHOW FAILURES IN TABLE
- `.ide-failures-table` — wrapper around a failing-rows table

**Common styling recipes**

1. **Two asserts per row inside every section**
   ```dsl
   grid_layout: CSS '.ide-section-body { display: grid !important; grid-template-columns: 1fr 1fr !important; gap: 6px !important; } .ide-section-body > .ide-result--chart, .ide-section-body > .ide-result--sample, .ide-section-body > .ide-result--schema, .ide-section-body > .ide-result--show-rows, .ide-section-body > .ide-result--screen { grid-column: 1 / -1 !important; }'
   ```

2. **Green charts (Recharts bar / line / pie)**
   ```dsl
   green_charts: CSS 'svg rect { fill: #22c55e !important; } .recharts-bar-rectangle path { fill: #22c55e !important; } .recharts-line-curve { stroke: #22c55e !important; }'
   ```

3. **Hide pass/fail icons and make asserts compact**
   ```dsl
   compact: CSS '.ide-result-icon { display: none !important; } .ide-result { padding: 4px 8px !important; }'
   ```

4. **Highlight failed asserts with a red border**
   ```dsl
   fail_style: CSS '.ide-result--fail { border: 1px solid #ef4444 !important; background: rgba(239,68,68,0.08) !important; }'
   ```

5. **Shrink sample / failure tables**
   ```dsl
   small_tables: CSS '.ide-sample-table { font-size: 10px !important; } .ide-sample-table th, .ide-sample-table td { padding: 2px 6px !important; }'
   ```

**Rules:**
- Use `!important` when overriding existing UI styles
- You can have multiple `CSS` statements; later ones override earlier ones
```

**DSL syntax traps**

- **ASSERT with WHERE**: put the expected value on its own line so the parser does not absorb it into the WHERE clause:
  ```
  WRONG:  ASSERT COUNT(t.id) WHERE IS_BLANK(t.id) = 0
  RIGHT:  ASSERT COUNT(t.id) WHERE IS_BLANK(t.id)
          = 0
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
7. `./vu update (./vu current | ConvertFrom-Json).id '<script>'` to update the open script, or `./vu create <label> '<script>'` for a new one
8. `./vu run <id>` to execute a script and inspect the full JSON results (assertions, SHOW ROWS output, errors)
9. The UI reruns immediately — iterate as needed
"#;

    let _ = std::fs::write(project_dir.join("CLAUDE.md"), claude_md);
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn pty_kill(state: State<PtyState>) -> Result<(), String> {
    if let Some(mut child) = state.child.lock().unwrap().take() {
        let _ = child.kill();
    }
    *state.master.lock().unwrap() = None;
    *state.writer.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub async fn pty_create(
    app: AppHandle,
    pty_state: State<'_, PtyState>,
    projects: State<'_, ProjectsState>,
    env: Option<HashMap<String, String>>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<(), String> {
    // Kill any stale session before starting a new one.
    if let Some(mut child) = pty_state.child.lock().unwrap().take() {
        let _ = child.kill();
    }
    *pty_state.master.lock().unwrap() = None;
    *pty_state.writer.lock().unwrap() = None;

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
    let path_sep = if cfg!(target_os = "windows") { ";" } else { ":" };
    cmd.env(
        "PATH",
        format!(
            "{}{}{}",
            project_dir.to_string_lossy(),
            path_sep,
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
        let mut leftover = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    leftover.extend_from_slice(&buf[..n]);
                    // Only emit complete UTF-8 sequences; buffer incomplete trailing bytes.
                    let valid_up_to = match std::str::from_utf8(&leftover) {
                        Ok(_) => leftover.len(),
                        Err(e) => e.valid_up_to(),
                    };
                    if valid_up_to > 0 {
                        let text = String::from_utf8_lossy(&leftover[..valid_up_to]).into_owned();
                        let _ = tx.send(text);
                    }
                    leftover = leftover.split_off(valid_up_to);
                }
            }
        }
    });

    let app2 = app.clone();
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
        // Clear state so pty_create can be called again after the user restarts.
        let state = app2.state::<PtyState>();
        *state.child.lock().unwrap() = None;
        *state.master.lock().unwrap() = None;
        *state.writer.lock().unwrap() = None;
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
