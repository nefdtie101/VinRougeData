use std::collections::HashMap;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::components::{Banner, GhostButton, Spinner};
use crate::ipc::tauri_invoke_args;
use crate::ollama::{ask_ollama_wasm, OLLAMA_DEFAULT_MODEL, OLLAMA_DEFAULT_URL};
use crate::types::{AuditProcessWithControls, DslScript, JoinSpec, RelCandidate, SessionSchema};
use crate::step4a::types::{Phase, ScriptStatus, ScriptState, RunResult, ChatMsg};
use crate::step4a::helpers::{parse_run_result, extract_dsl_code};
use crate::step4a::pipeline::do_load_or_generate;

#[component]
pub fn Step4bView(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
) -> impl IntoView {
    // ── Core signals ──────────────────────────────────────────────────────────
    let phase: RwSignal<Phase>                                = RwSignal::new(Phase::Loading);
    let schemas: RwSignal<Vec<SessionSchema>>                  = RwSignal::new(vec![]);
    let scripts: RwSignal<Vec<DslScript>>                      = RwSignal::new(vec![]);
    let script_states: RwSignal<HashMap<String, ScriptState>>  = RwSignal::new(HashMap::new());
    let progress_msg: RwSignal<String>                        = RwSignal::new("Loading data…".to_string());

    // ── Unused signals kept so do_load_or_generate signature matches ──────────
    let join_candidates: RwSignal<Vec<RelCandidate>>          = RwSignal::new(vec![]);
    let accepted_joins: RwSignal<Vec<bool>>                   = RwSignal::new(vec![]);

    // ── Three-zone signals ────────────────────────────────────────────────────
    let selected_id: RwSignal<Option<String>>                 = RwSignal::new(None);
    let preview_cols: RwSignal<Vec<String>>                   = RwSignal::new(vec![]);
    let preview_rows: RwSignal<Vec<Vec<String>>>              = RwSignal::new(vec![]);
    let preview_source: RwSignal<String>                      = RwSignal::new(String::new());
    let last_result: RwSignal<Option<RunResult>>              = RwSignal::new(None);
    let run_loading: RwSignal<bool>                           = RwSignal::new(false);
    let chat_msgs: RwSignal<Vec<ChatMsg>>                     = RwSignal::new(vec![]);
    let chat_input: RwSignal<String>                          = RwSignal::new(String::new());
    let chat_loading: RwSignal<bool>                          = RwSignal::new(false);

    // ── Resize state ──────────────────────────────────────────────────────────
    let left_width: RwSignal<f64>                             = RwSignal::new(60.0);
    let is_resizing: RwSignal<bool>                           = RwSignal::new(false);
    let is_popped_out: RwSignal<bool>                         = RwSignal::new(false);
    let selected_cell: RwSignal<Option<(usize, usize)>>       = RwSignal::new(None);
    let hide_empty_cols: RwSignal<bool>                       = RwSignal::new(false);

    // ── Active table tab + pagination ─────────────────────────────────────────
    let selected_preview_id: RwSignal<Option<String>>         = RwSignal::new(None);
    let page_offset: RwSignal<usize>                          = RwSignal::new(0);
    let page_size: usize                                      = 200;
    // Reverse map (pbc_name → original_name) for the currently displayed tab.
    // Empty for master (no remapping needed). Non-master tabs use this to show
    // original file column names instead of PBC aliases.
    let active_rev_map: RwSignal<HashMap<String, String>>     = RwSignal::new(HashMap::new());
    let total_rows: RwSignal<usize>                           = RwSignal::new(0);

    // ── Auto-select first/master tab when schemas load, and load its data ────
    Effect::new(move |_| {
        let schs = schemas.get();
        if selected_preview_id.get_untracked().is_none() && !schs.is_empty() {
            let sel = schs.iter().find(|s| s.source_type == "master")
                .or_else(|| schs.first())
                .cloned();
            if let Some(s) = sel {
                let id = s.import_id.clone();
                selected_preview_id.set(Some(id.clone()));
                total_rows.set(s.row_count);
                preview_source.set(if s.source_type == "master" { "master".to_string() } else { s.table_name.clone() });
                preview_cols.set(vec![]);
                preview_rows.set(vec![]);
                // Build reverse map (pbc → original) for non-master tabs
                let rev_map: HashMap<String, String> = s.col_map.iter()
                    .map(|(orig, pbc)| (pbc.clone(), orig.clone()))
                    .collect();
                active_rev_map.set(rev_map.clone());
                let fallback_cols = s.columns.clone();
                spawn_local(async move {
                    if let Ok(raw) = tauri_invoke_args::<Vec<HashMap<String, String>>>(
                        "get_session_rows_paged",
                        serde_json::json!({ "importId": id, "offset": 0, "limit": page_size }),
                    ).await {
                        if raw.is_empty() {
                            // Show original names as fallback if available
                            let disp: Vec<String> = fallback_cols.iter()
                                .map(|c| rev_map.get(c).cloned().unwrap_or_else(|| c.clone()))
                                .collect();
                            preview_cols.set(disp);
                            return;
                        }
                        // Sort PBC keys by their display name, then remap for display
                        let mut pbc_cols: Vec<String> = raw[0].keys().cloned().collect();
                        pbc_cols.sort_by(|a, b| {
                            let da = rev_map.get(a).map(String::as_str).unwrap_or(a.as_str());
                            let db = rev_map.get(b).map(String::as_str).unwrap_or(b.as_str());
                            da.cmp(db)
                        });
                        let disp_cols: Vec<String> = pbc_cols.iter()
                            .map(|k| rev_map.get(k).cloned().unwrap_or_else(|| k.clone()))
                            .collect();
                        let rows = raw.into_iter().map(|row| {
                            pbc_cols.iter().map(|k| row.get(k).cloned().unwrap_or_default()).collect()
                        }).collect();
                        preview_cols.set(disp_cols);
                        preview_rows.set(rows);
                    }
                });
            }
        }
    });

    // ── Keyboard navigation ───────────────────────────────────────────────────
    use leptos::ev::KeyboardEvent;
    let on_keydown = move |ev: KeyboardEvent| {
        if let Some((row, col)) = selected_cell.get() {
            let rows = preview_rows.get();
            let cols = preview_cols.get();
            let max_row = rows.len().saturating_sub(1);
            let max_col = cols.len().saturating_sub(1);
            let new_pos = match ev.key().as_str() {
                "ArrowUp"    if row > 0       => { ev.prevent_default(); Some((row - 1, col)) }
                "ArrowDown"  if row < max_row => { ev.prevent_default(); Some((row + 1, col)) }
                "ArrowLeft"  if col > 0       => { ev.prevent_default(); Some((row, col - 1)) }
                "ArrowRight" if col < max_col => { ev.prevent_default(); Some((row, col + 1)) }
                _ => None,
            };
            if let Some((new_row, new_col)) = new_pos {
                selected_cell.set(Some((new_row, new_col)));
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        let sel = format!("tbody tr:nth-child({}) td:nth-child({})", new_row + 1, new_col + 2);
                        if let Ok(Some(cell)) = document.query_selector(&sel) {
                            if let Some(el) = cell.dyn_ref::<web_sys::HtmlElement>() {
                                el.scroll_into_view_with_bool(false);
                            }
                        }
                    }
                }
            }
        }
    };

    // ── Mount ─────────────────────────────────────────────────────────────────
    spawn_local(async move {
        do_load_or_generate(
            audit_plan, phase, schemas, scripts, script_states,
            progress_msg, status, selected_id,
            preview_cols, preview_rows, preview_source,
            join_candidates, accepted_joins,
            false,
        ).await;
    });

    // ── Listen for pop-out close ──────────────────────────────────────────────
    let is_popped_out_listener = is_popped_out.clone();
    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |_event: JsValue| {
            is_popped_out_listener.set(false);
        }) as Box<dyn Fn(JsValue)>);
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))
                .and_then(|t| js_sys::Reflect::get(&t, &JsValue::from_str("event")))
                .and_then(|e| js_sys::Reflect::get(&e, &JsValue::from_str("listen")))
                .and_then(|f| {
                    let listen = f.dyn_ref::<js_sys::Function>().unwrap();
                    listen.call2(&JsValue::NULL, &JsValue::from_str("data-preview-closed"), closure.as_ref())
                });
        }
        closure.forget();
    });

    // ── Derived counts ────────────────────────────────────────────────────────
    let total_count    = move || scripts.get().len();
    let approved_count = move || {
        script_states.get().values().filter(|s| s.status == ScriptStatus::Approved).count()
    };
    let pending_count = move || {
        let ss = script_states.get();
        let decided = ss.values()
            .filter(|s| matches!(s.status, ScriptStatus::Approved | ScriptStatus::Rejected))
            .count();
        scripts.get().len().saturating_sub(decided)
    };
    let can_run = move || {
        if !matches!(phase.get(), Phase::Review) { return false; }
        let ss = script_states.get();
        let rejected = ss.values().filter(|s| s.status == ScriptStatus::Rejected).count();
        !scripts.get().is_empty() && scripts.get().len() > rejected
    };

    let on_generate_all = move |_| {
        spawn_local(async move {
            do_load_or_generate(
                audit_plan, phase, schemas, scripts, script_states,
                progress_msg, status, selected_id,
                preview_cols, preview_rows, preview_source,
                join_candidates, accepted_joins,
                true,
            ).await;
        });
    };

    // ── Zone 2 handlers ───────────────────────────────────────────────────────
    let on_z2_run = move |_| {
        let sid = match selected_id.get_untracked() { Some(s) => s, None => return };
        let text = script_states.get_untracked().get(&sid).map(|s| s.text.clone()).unwrap_or_default();
        if text.trim().is_empty() { return; }
        run_loading.set(true);
        let t0 = js_sys::Date::now();
        spawn_local(async move {
            let _ = tauri_invoke_args::<()>(
                "update_dsl_script",
                serde_json::json!({ "scriptId": sid, "scriptText": text }),
            ).await;
            match tauri_invoke_args::<Vec<serde_json::Value>>(
                "run_dsl_script",
                serde_json::json!({ "scriptId": sid }),
            ).await {
                Ok(results) => last_result.set(Some(parse_run_result(&results, js_sys::Date::now() - t0))),
                Err(e) => last_result.set(Some(RunResult {
                    expr_type: "ERROR".to_string(), expected: "—".to_string(),
                    actual: e, passed: false, duration_ms: js_sys::Date::now() - t0,
                })),
            }
            run_loading.set(false);
        });
    };

    let on_z2_clear = move |_| {
        let sid = match selected_id.get_untracked() { Some(s) => s, None => return };
        let original = scripts.get_untracked().into_iter()
            .find(|s| s.id == sid).map(|s| s.script_text.clone()).unwrap_or_default();
        script_states.update(|m| {
            if let Some(st) = m.get_mut(&sid) { st.text = original; st.status = ScriptStatus::Generated; }
        });
        last_result.set(None);
    };

    let on_z2_save = move |_| {
        let sid = match selected_id.get_untracked() { Some(s) => s, None => return };
        let text = script_states.get_untracked().get(&sid).map(|s| s.text.clone()).unwrap_or_default();
        spawn_local(async move {
            let _ = tauri_invoke_args::<()>(
                "update_dsl_script",
                serde_json::json!({ "scriptId": sid, "scriptText": text }),
            ).await;
        });
    };

    // ── Chat handler ──────────────────────────────────────────────────────────
    let on_chat_send = move |_| {
        let msg = chat_input.get_untracked().trim().to_string();
        if msg.is_empty() || chat_loading.get_untracked() { return; }
        chat_msgs.update(|v| v.push(ChatMsg { is_user: true, text: msg.clone(), code: None }));
        chat_input.set(String::new());
        chat_loading.set(true);
        let schema_ctx = schemas.get_untracked().iter()
            .map(|s| format!("Table {} ({} rows): {}", s.table_name, s.row_count, s.columns.join(", ")))
            .collect::<Vec<_>>().join("\n");
        let current_script = selected_id.get_untracked()
            .and_then(|sid| script_states.get_untracked().get(&sid).map(|s| s.text.clone()))
            .unwrap_or_default();
        let context = format!(
            "You are a VinRouge audit DSL assistant. Help the auditor write and improve DSL test scripts.\n\n\
             Available data:\n{schema_ctx}\n\nCurrent script:\n{current_script}\n\n\
             DSL reference:\n\
             - EXCEPTIONS <table> WHERE <condition>\n- RECONCILE <field>=<value>\n\
             - SAMPLE <table> RANDOM|INTERVAL <n>\n- TOTAL <table>.<field>\n- COUNT <table>\n\n\
             Wrap DSL code in ``` fences."
        );
        spawn_local(async move {
            match ask_ollama_wasm(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &context, &msg).await {
                Ok(resp) => {
                    let code = extract_dsl_code(&resp);
                    chat_msgs.update(|v| v.push(ChatMsg { is_user: false, text: resp, code }));
                }
                Err(e) => chat_msgs.update(|v| v.push(ChatMsg {
                    is_user: false, text: format!("Could not reach AI: {e}"), code: None,
                })),
            }
            chat_loading.set(false);
        });
    };

    let inject_code = move |code: String| {
        let sid = match selected_id.get_untracked() { Some(s) => s, None => return };
        script_states.update(|m| {
            if let Some(st) = m.get_mut(&sid) { st.text = code; st.status = ScriptStatus::Edited; }
        });
    };

    // ── Resize handlers ───────────────────────────────────────────────────────
    let on_resize_start = move |ev: web_sys::MouseEvent| { ev.prevent_default(); is_resizing.set(true); };
    let on_resize_move = move |ev: web_sys::MouseEvent| {
        if !is_resizing.get() { return; }
        if let Some(window) = web_sys::window() {
            let w = window.inner_width().ok().and_then(|w| w.as_f64()).unwrap_or(1200.0);
            left_width.set((ev.client_x() as f64 / w * 100.0).max(30.0).min(80.0));
        }
    };
    let on_resize_end = move |_ev: web_sys::MouseEvent| { is_resizing.set(false); };

    // ── Pop-out ────────────────────────────────────────────────────────────────
    let on_popout = move |_| {
        let data = serde_json::json!({
            "columns": preview_cols.get_untracked(),
            "rows":    preview_rows.get_untracked(),
            "source":  preview_source.get_untracked(),
        });
        spawn_local(async move {
            match tauri_invoke_args::<()>("open_data_preview_window", serde_json::json!({ "data": data })).await {
                Ok(_)  => is_popped_out.set(true),
                Err(e) => status.set(format!("Could not open pop-out window: {e}")),
            }
        });
    };

    // ── Run engine → step 7 (Results) ─────────────────────────────────────────
    let on_run_engine = move |_| {
        if !can_run() { return; }
        let ss  = script_states.get_untracked();
        let all = scripts.get_untracked();
        let has_any = ss.values().any(|s| s.status == ScriptStatus::Approved);
        let to_run: Vec<DslScript> = all.into_iter().filter(|s| {
            ss.get(&s.id).map(|st| {
                if has_any { st.status == ScriptStatus::Approved } else { st.status != ScriptStatus::Rejected }
            }).unwrap_or(true)
        }).collect();
        if to_run.is_empty() { return; }
        let total = to_run.len();
        spawn_local(async move {
            for (i, script) in to_run.iter().enumerate() {
                phase.set(Phase::Running { done: i, total });
                if let Some(st) = ss.get(&script.id) {
                    if !matches!(st.status, ScriptStatus::Generated) {
                        let _ = tauri_invoke_args::<()>(
                            "update_dsl_script",
                            serde_json::json!({ "scriptId": script.id, "scriptText": st.text }),
                        ).await;
                    }
                }
                let _ = tauri_invoke_args::<Vec<serde_json::Value>>(
                    "run_dsl_script",
                    serde_json::json!({ "scriptId": script.id }),
                ).await;
            }
            audit_ui_step.set(7);
        });
    };

    // ── View ──────────────────────────────────────────────────────────────────
    view! {
        <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">

            // ── Page header ────────────────────────────────────────────────────
            <div class="s4a-page-hdr">
                <div class="s4a-hdr-left">
                    <span class="s4a-page-step">"Step 4b"</span>
                    <span class="s4a-page-title">"Data view & algorithms"</span>
                    <span class="s4a-page-sub">"Review, edit and run DSL algorithms against your data"</span>
                </div>
                <div class="s4a-page-stats">
                    {move || match phase.get() {
                        Phase::Loading | Phase::Generating | Phase::Running { .. } => view! {
                            <div style="display:flex;align-items:center;gap:8px;color:var(--w-text-3);font-size:12px">
                                <Spinner size=12 />
                                {move || progress_msg.get()}
                            </div>
                        }.into_any(),
                        _ => view! {
                            <>
                                <div class="s4a-stat-item">
                                    <span class="s4a-stat-num">{move || total_count()}</span>
                                    <span class="s4a-stat-label">"Generated"</span>
                                </div>
                                <div class="s4a-stat-divider"></div>
                                <div class="s4a-stat-item">
                                    <span class="s4a-stat-num s4a-stat-green">{move || approved_count()}</span>
                                    <span class="s4a-stat-label">"Approved"</span>
                                </div>
                                <div class="s4a-stat-divider"></div>
                                <div class="s4a-stat-item">
                                    <span class="s4a-stat-num">{move || pending_count()}</span>
                                    <span class="s4a-stat-label">"Pending"</span>
                                </div>
                                <div class="s4a-stat-divider"></div>
                                <button
                                    class="s4a-btn-gen-all"
                                    disabled=Signal::derive(move || !matches!(phase.get(), Phase::Review | Phase::Error(_)))
                                    on:click=on_generate_all
                                >"↻  Regenerate"</button>
                            </>
                        }.into_any()
                    }}
                </div>
            </div>

            // ── Error banner ───────────────────────────────────────────────────
            {move || {
                if let Phase::Error(msg) = phase.get() {
                    Some(view! {
                        <div style="padding:8px 14px;flex-shrink:0">
                            <Banner message=Signal::derive(move || msg.clone()) variant="error" />
                        </div>
                    })
                } else { None }
            }}

            // ── Three-zone layout ──────────────────────────────────────────────
            <div class="s4a-zones"
                style=move || {
                    if is_popped_out.get() {
                        "display:grid;grid-template-columns:1fr;gap:0;flex:1;overflow:hidden".to_string()
                    } else {
                        format!("display:grid;grid-template-columns:{}% 4px 1fr;gap:0;flex:1;overflow:hidden", left_width.get())
                    }
                }
                on:mousemove=on_resize_move
                on:mouseup=on_resize_end
            >
                // ── Zone 1: data preview ───────────────────────────────────────
                <div class="s4a-z1" style=move || if is_popped_out.get() { "display:none" } else { "" }>
                    <div class="s4a-zone-hdr-dark">"data preview"</div>

                    // ── Table tabs ─────────────────────────────────────────────
                    <div style="display:flex;gap:0;border-bottom:1px solid #222;overflow-x:auto;flex-shrink:0">
                        {move || {
                            let mut tab_schemas = schemas.get();
                            // Non-master tabs first (alphabetical), master tab last
                            tab_schemas.sort_by(|a, b| {
                                let a_master = a.source_type == "master";
                                let b_master = b.source_type == "master";
                                match (a_master, b_master) {
                                    (true, false) => std::cmp::Ordering::Greater,
                                    (false, true) => std::cmp::Ordering::Less,
                                    _ => a.table_name.cmp(&b.table_name),
                                }
                            });
                            tab_schemas.into_iter().map(|s| {
                                let import_id = s.import_id.clone();
                                let name      = s.table_name.clone();
                                let cols      = s.columns.clone();
                                let is_master = s.source_type == "master";
                                let tab_id    = import_id.clone();
                                let tab_name  = if is_master { "master".to_string() } else { name.clone() };
                                let tab_cols  = cols.clone();
                                let label     = if is_master { "★ master".to_string() } else { name.clone() };
                                // Build reverse map for this tab (pbc → original)
                                let tab_rev_map: HashMap<String, String> = s.col_map.iter()
                                    .map(|(orig, pbc)| (pbc.clone(), orig.clone()))
                                    .collect();
                                view! {
                                    <button
                                        on:click=move |_| {
                                            let id  = tab_id.clone();
                                            let nm  = tab_name.clone();
                                            selected_preview_id.set(Some(id.clone()));
                                            preview_source.set(nm);
                                            preview_cols.set(vec![]);
                                            preview_rows.set(vec![]);
                                            hide_empty_cols.set(false);
                                            selected_cell.set(None);
                                            page_offset.set(0);
                                            let row_count = schemas.get_untracked()
                                                .iter()
                                                .find(|s| s.import_id == id)
                                                .map(|s| s.row_count)
                                                .unwrap_or(0);
                                            total_rows.set(row_count);
                                            let fallback_cols = tab_cols.clone();
                                            let rev_map = tab_rev_map.clone();
                                            active_rev_map.set(rev_map.clone());
                                            spawn_local(async move {
                                                if let Ok(raw) = tauri_invoke_args::<Vec<HashMap<String, String>>>(
                                                    "get_session_rows_paged",
                                                    serde_json::json!({ "importId": id, "offset": 0, "limit": page_size }),
                                                ).await {
                                                    if raw.is_empty() {
                                                        let disp: Vec<String> = fallback_cols.iter()
                                                            .map(|c| rev_map.get(c).cloned().unwrap_or_else(|| c.clone()))
                                                            .collect();
                                                        preview_cols.set(disp);
                                                        preview_rows.set(vec![]);
                                                        return;
                                                    }
                                                    // Sort PBC keys by display name then remap
                                                    let mut pbc_cols: Vec<String> =
                                                        raw[0].keys().cloned().collect();
                                                    pbc_cols.sort_by(|a, b| {
                                                        let da = rev_map.get(a).map(String::as_str).unwrap_or(a.as_str());
                                                        let db = rev_map.get(b).map(String::as_str).unwrap_or(b.as_str());
                                                        da.cmp(db)
                                                    });
                                                    let disp_cols: Vec<String> = pbc_cols.iter()
                                                        .map(|k| rev_map.get(k).cloned().unwrap_or_else(|| k.clone()))
                                                        .collect();
                                                    let rows = raw.into_iter().map(|row| {
                                                        pbc_cols.iter().map(|k| row.get(k).cloned().unwrap_or_default()).collect()
                                                    }).collect();
                                                    preview_cols.set(disp_cols);
                                                    preview_rows.set(rows);
                                                }
                                            });
                                        }
                                        style=move || {
                                            let active = selected_preview_id.get().as_deref() == Some(import_id.as_str());
                                            format!(
                                                "padding:5px 14px;font-size:11px;border:none;border-bottom:2px solid {};\
                                                 background:{};color:{};cursor:pointer;white-space:nowrap;font-weight:{}",
                                                if active { "#6fa06f" } else { "transparent" },
                                                if active { "#1e2a1e" } else { "transparent" },
                                                if active { "#b0d4b0" } else { "#666" },
                                                if is_master { "600" } else { "400" },
                                            )
                                        }
                                    >{label}</button>
                                }
                            }).collect_view()
                        }}
                    </div>

                    // ── Topbar ─────────────────────────────────────────────────
                    <div class="s4a-tbl-topbar">
                        <span class="s4a-tbl-pill">
                            {move || {
                                let total = total_rows.get();
                                let off   = page_offset.get();
                                let shown = preview_rows.get().len();
                                if total > 0 {
                                    format!("{}-{} of {} rows", off + 1, off + shown, total)
                                } else {
                                    format!("{} rows", shown)
                                }
                            }}
                        </span>
                        // ── Pagination ──────────────────────────────────────
                        {move || (total_rows.get() > page_size).then(|| {
                            let off   = page_offset.get();
                            let total = total_rows.get();
                            let can_prev = off > 0;
                            let can_next = off + page_size < total;
                            view! {
                                <div style="display:flex;align-items:center;gap:4px;margin-left:8px">
                                    <button
                                        disabled=Signal::derive(move || !can_prev)
                                        on:click=move |_| {
                                            let id  = match selected_preview_id.get_untracked() { Some(id) => id, None => return };
                                            let disp_cols = preview_cols.get_untracked();
                                            let rev = active_rev_map.get_untracked();
                                            // Translate display names → PBC keys for DB lookup
                                            let pbc_cols: Vec<String> = disp_cols.iter()
                                                .map(|c| rev.iter().find(|(_, orig)| *orig == c)
                                                    .map(|(pbc, _)| pbc.clone())
                                                    .unwrap_or_else(|| c.clone()))
                                                .collect();
                                            let new_off = page_offset.get_untracked().saturating_sub(page_size);
                                            page_offset.set(new_off);
                                            spawn_local(async move {
                                                if let Ok(raw) = tauri_invoke_args::<Vec<HashMap<String, String>>>(
                                                    "get_session_rows_paged",
                                                    serde_json::json!({ "importId": id, "offset": new_off, "limit": page_size }),
                                                ).await {
                                                    let rows = raw.into_iter().map(|row| {
                                                        pbc_cols.iter().map(|k| row.get(k).cloned().unwrap_or_default()).collect()
                                                    }).collect();
                                                    preview_rows.set(rows);
                                                    selected_cell.set(None);
                                                }
                                            });
                                        }
                                        style="padding:2px 8px;font-size:11px;background:#1a1a1a;\
                                               border:0.5px solid #333;border-radius:3px;color:#aaa;cursor:pointer"
                                    >"← Prev"</button>
                                    <button
                                        disabled=Signal::derive(move || !can_next)
                                        on:click=move |_| {
                                            let id  = match selected_preview_id.get_untracked() { Some(id) => id, None => return };
                                            let disp_cols = preview_cols.get_untracked();
                                            let rev = active_rev_map.get_untracked();
                                            let pbc_cols: Vec<String> = disp_cols.iter()
                                                .map(|c| rev.iter().find(|(_, orig)| *orig == c)
                                                    .map(|(pbc, _)| pbc.clone())
                                                    .unwrap_or_else(|| c.clone()))
                                                .collect();
                                            let new_off = page_offset.get_untracked() + page_size;
                                            page_offset.set(new_off);
                                            spawn_local(async move {
                                                if let Ok(raw) = tauri_invoke_args::<Vec<HashMap<String, String>>>(
                                                    "get_session_rows_paged",
                                                    serde_json::json!({ "importId": id, "offset": new_off, "limit": page_size }),
                                                ).await {
                                                    let rows = raw.into_iter().map(|row| {
                                                        pbc_cols.iter().map(|k| row.get(k).cloned().unwrap_or_default()).collect()
                                                    }).collect();
                                                    preview_rows.set(rows);
                                                    selected_cell.set(None);
                                                }
                                            });
                                        }
                                        style="padding:2px 8px;font-size:11px;background:#1a1a1a;\
                                               border:0.5px solid #333;border-radius:3px;color:#aaa;cursor:pointer"
                                    >"Next →"</button>
                                </div>
                            }
                        })}
                        <div style="display:flex;gap:6px;align-items:center;margin-left:auto">
                            <button
                                title="Hide columns where all rows are empty"
                                disabled=Signal::derive(move || preview_cols.get().is_empty())
                                on:click=move |_| hide_empty_cols.update(|v| *v = !*v)
                                style=move || format!(
                                    "padding:4px 10px;font-size:11px;border-radius:3px;cursor:pointer;\
                                     display:flex;align-items:center;gap:4px;border:0.5px solid #333;\
                                     background:{};color:{}",
                                    if hide_empty_cols.get() { "#2a3a2a" } else { "#1a1a1a" },
                                    if hide_empty_cols.get() { "#6fa06f" } else { "#aaa" },
                                )
                            >"⊘ Hide empty cols"</button>
                            <button
                                class="s4a-popout-btn"
                                title="Pop out to separate window"
                                disabled=Signal::derive(move || preview_cols.get().is_empty())
                                on:click=move |_| on_popout(())
                                style="padding:4px 10px;font-size:11px;background:#1a1a1a;\
                                       border:0.5px solid #333;border-radius:3px;color:#aaa;cursor:pointer;\
                                       display:flex;align-items:center;gap:4px"
                            >"⧉ Pop out"</button>
                        </div>
                    </div>

                    <div class="s4a-tbl-wrap" tabindex="0" on:keydown=on_keydown>
                        {move || {
                            let all_cols = preview_cols.get();
                            let all_rows = preview_rows.get();
                            if all_cols.is_empty() {
                                return view! {
                                    <div class="s4a-no-data">
                                        {move || if matches!(phase.get(), Phase::Loading | Phase::Generating) {
                                            "Loading…"
                                        } else {
                                            "No data imported"
                                        }}
                                    </div>
                                }.into_any();
                            }
                            let visible_indices: Vec<usize> = if hide_empty_cols.get() {
                                (0..all_cols.len()).filter(|&ci| {
                                    all_rows.iter().any(|row| row.get(ci).map(|v| !v.trim().is_empty()).unwrap_or(false))
                                }).collect()
                            } else {
                                (0..all_cols.len()).collect()
                            };
                            let cols: Vec<String> = visible_indices.iter().map(|&i| all_cols[i].clone()).collect();
                            let rows: Vec<Vec<String>> = all_rows.iter().map(|row| {
                                visible_indices.iter().map(|&i| row.get(i).cloned().unwrap_or_default()).collect()
                            }).collect();
                            let letters: Vec<String> = (0..cols.len()).map(|i| {
                                if i < 26 { ((b'A' + i as u8) as char).to_string() }
                                else { format!("{}{}", (b'A' + (i / 26 - 1) as u8) as char, (b'A' + (i % 26) as u8) as char) }
                            }).collect();
                            let cols2 = cols.clone();
                            let rows_view = rows.iter().enumerate().map(|(ri, row)| {
                                let cells = row.iter().enumerate().map(|(ci, val)| {
                                    let val_clone = val.clone();
                                    let selected = selected_cell.get();
                                    let is_selected     = selected == Some((ri, ci));
                                    let is_selected_row = selected.map(|(r, _)| r == ri).unwrap_or(false);
                                    let is_selected_col = selected.map(|(_, c)| c == ci).unwrap_or(false);
                                    let class = if is_selected { "selected" } else if is_selected_row { "selected-row" } else if is_selected_col { "selected-col" } else { "" };
                                    view! {
                                        <td class=class on:click=move |_| { selected_cell.set(Some((ri, ci))); }>
                                            {val_clone}
                                        </td>
                                    }
                                }).collect_view();
                                view! { <tr><td class="rn">{ri + 1}</td>{cells}</tr> }
                            }).collect_view();
                            view! {
                                <table class="s4a-sheet-tbl">
                                    <thead>
                                        <tr>
                                            <th class="s4a-th-lbl corner"></th>
                                            {letters.iter().map(|l| { let l = l.clone(); view! { <th class="s4a-th-lbl">{l}</th> } }).collect_view()}
                                        </tr>
                                        <tr>
                                            <th class="s4a-th-fld" style="background:#111"></th>
                                            {cols2.iter().map(|c| { let c = c.clone(); view! { <th class="s4a-th-fld">{c}</th> } }).collect_view()}
                                        </tr>
                                    </thead>
                                    <tbody>{rows_view}</tbody>
                                </table>
                            }.into_any()
                        }}
                    </div>
                </div>

                // ── Resize handle ──────────────────────────────────────────────
                <div class="s4a-resize-handle"
                    style=move || if is_popped_out.get() { "display:none" } else { "" }
                    on:mousedown=on_resize_start
                ></div>

                // ── Right: DSL editor + chat ───────────────────────────────────
                <div style="display:flex;flex-direction:column;overflow:hidden">

                // ── Zone 2: DSL editor ─────────────────────────────────────────
                <div class="s4a-z2" style="flex:1;overflow:auto;border-bottom:0.5px solid var(--w-border)">
                    <div class="s4a-zone-hdr">"DSL expression"</div>
                    <div class="s4a-z2-body">
                        {move || {
                            let all = scripts.get();
                            if all.is_empty() { return None; }
                            let opts = all.iter().map(|s| {
                                let sid   = s.id.clone();
                                let label = format!("{} — {}", s.control_ref, s.label);
                                let sel   = selected_id.get().as_deref() == Some(s.id.as_str());
                                view! { <option value=sid selected=sel>{label}</option> }
                            }).collect_view();
                            Some(view! {
                                <div>
                                    <div class="s4a-z2-lbl" style="margin-bottom:4px">"script"</div>
                                    <select class="s4a-z2-select"
                                        on:change=move |ev| { selected_id.set(Some(event_target_value(&ev))); last_result.set(None); }
                                    >{opts}</select>
                                </div>
                            })
                        }}
                        {move || {
                            let sid = selected_id.get()?;
                            let text = script_states.get().get(&sid).map(|s| s.text.clone()).unwrap_or_default();
                            let sid_input = sid.clone();
                            Some(view! {
                                <div>
                                    <div class="s4a-z2-lbl" style="margin-bottom:4px">"expression"</div>
                                    <textarea class="s4a-z2-ed" prop:value=text
                                        on:input=move |ev| {
                                            if let Some(ta) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) {
                                                let val = ta.value();
                                                let sid = sid_input.clone();
                                                script_states.update(|m| {
                                                    if let Some(st) = m.get_mut(&sid) {
                                                        st.text = val;
                                                        if st.status == ScriptStatus::Generated { st.status = ScriptStatus::Edited; }
                                                    }
                                                });
                                            }
                                        }
                                    ></textarea>
                                </div>
                            })
                        }}
                        <div class="s4a-z2-actions">
                            <button class="s4a-z2-btn s4a-z2-btn-run"
                                disabled=Signal::derive(move || run_loading.get() || selected_id.get().is_none())
                                on:click=on_z2_run
                            >{move || if run_loading.get() { "…" } else { "▶ run" }}</button>
                            <button class="s4a-z2-btn"
                                disabled=Signal::derive(move || selected_id.get().is_none())
                                on:click=on_z2_clear
                            >"clear"</button>
                            <button class="s4a-z2-btn"
                                disabled=Signal::derive(move || selected_id.get().is_none())
                                on:click=on_z2_save
                            >"+ save"</button>
                        </div>
                        <div class="s4a-z2-hint">
                            "EXCEPTIONS"<br/>"RECONCILE"<br/>"SAMPLE"<br/>"TOTAL · COUNT"
                        </div>
                        {move || {
                            let res = last_result.get()?;
                            let fail = !res.passed;
                            let duration = format!("{:.1}ms", res.duration_ms);
                            let status_txt = if res.passed { "pass" } else { "fail" };
                            let actual = res.actual.clone(); let expected = res.expected.clone(); let expr_t = res.expr_type.clone();
                            Some(view! {
                                <div class="s4a-result-block">
                                    <div class="s4a-result-hdr">"last result"</div>
                                    <div class="s4a-result-row"><span class="s4a-result-key">"expression"</span><span class="s4a-result-val">{expr_t}</span></div>
                                    <div class="s4a-result-row"><span class="s4a-result-key">"expected"</span><span class="s4a-result-val">{expected}</span></div>
                                    <div class="s4a-result-row">
                                        <span class="s4a-result-key">"actual"</span>
                                        <span class=move || if fail { "s4a-result-val s4a-result-fail" } else { "s4a-result-val" }>{actual.clone()}</span>
                                    </div>
                                    <div class="s4a-result-row">
                                        <span class="s4a-result-key">"status"</span>
                                        <span class=move || if fail { "s4a-result-val s4a-result-fail" } else { "s4a-result-val" }>{status_txt}</span>
                                    </div>
                                    <div class="s4a-result-row"><span class="s4a-result-key">"duration"</span><span class="s4a-result-val">{duration}</span></div>
                                </div>
                            })
                        }}
                    </div>
                </div>

                // ── Zone 3: AI chat ────────────────────────────────────────────
                <div class="s4a-z3" style="flex:1;overflow:hidden;display:flex;flex-direction:column">
                    <div class="s4a-zone-hdr">"AI prompt"</div>
                    <div class="s4a-chat-msgs">
                        {move || {
                            let msgs = chat_msgs.get();
                            if msgs.is_empty() {
                                return view! {
                                    <div style="font-size:11px;color:var(--w-text-4);padding:4px 0">
                                        "Ask vin rouge to write or explain a DSL script."
                                    </div>
                                }.into_any();
                            }
                            msgs.into_iter().map(|m| {
                                let lbl_cls = if m.is_user { "s4a-msg-lbl" } else { "s4a-msg-lbl s4a-msg-lbl-ai" };
                                let lbl     = if m.is_user { "you" } else { "vin rouge" };
                                let bbl_cls = if m.is_user { "s4a-bubble" } else { "s4a-bubble s4a-bubble-ai" };
                                let code = m.code.clone();
                                view! {
                                    <div class="s4a-msg">
                                        <div class=lbl_cls>{lbl}</div>
                                        <div class=bbl_cls>
                                            {m.text.clone()}
                                            {code.map(|c| {
                                                let c2 = c.clone();
                                                view! {
                                                    <code class="s4a-code-chip" title="Click to use in editor"
                                                        on:click=move |_| inject_code(c2.clone())>{c}</code>
                                                }
                                            })}
                                        </div>
                                    </div>
                                }
                            }).collect_view().into_any()
                        }}
                        {move || chat_loading.get().then(|| view! {
                            <div class="s4a-msg">
                                <div class="s4a-msg-lbl s4a-msg-lbl-ai">"vin rouge"</div>
                                <div class="s4a-bubble s4a-bubble-ai" style="display:flex;align-items:center;gap:7px">
                                    <Spinner size=10 />
                                    <span style="font-size:11px;color:var(--w-text-3)">"Thinking…"</span>
                                </div>
                            </div>
                        })}
                    </div>
                    <div class="s4a-chat-input">
                        <textarea class="s4a-chat-ta" rows=2 placeholder="prompt vin rouge…"
                            prop:value=move || chat_input.get()
                            on:input=move |ev| {
                                if let Some(ta) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) {
                                    chat_input.set(ta.value());
                                }
                            }
                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                if ev.key() == "Enter" && (ev.ctrl_key() || ev.meta_key()) { on_chat_send(()); }
                            }
                        ></textarea>
                        <button class="s4a-send-btn"
                            disabled=Signal::derive(move || chat_loading.get() || chat_input.get().trim().is_empty())
                            on:click=move |_| on_chat_send(())
                        >"→"</button>
                    </div>
                </div>

                </div> // end right column
            </div>

            // ── Status bar ─────────────────────────────────────────────────────
            <div class="s4-status-bar">
                <span class=move || if can_run() { "s4-dot s4-dot--ready" } else { "s4-dot s4-dot--idle" }></span>
                <span class="s4-status-text">
                    {move || match phase.get() {
                        Phase::Loading   => "Loading data…".to_string(),
                        Phase::Generating => progress_msg.get(),
                        Phase::Running { done, total } => format!("Running scripts: {done}/{total}…"),
                        Phase::Review => {
                            let a = approved_count(); let n = total_count();
                            if a == 0 { format!("0 of {n} algorithms approved — approve or run directly") }
                            else      { format!("{a} of {n} approved") }
                        }
                        Phase::Error(e) => format!("Error: {e}"),
                        _ => String::new(),
                    }}
                </span>
                <div style="margin-left:auto;display:flex;align-items:center;gap:8px">
                    <GhostButton label="Back" back=true
                        on_click=Callback::new(move |()| audit_ui_step.set(5)) />
                    <button
                        class=move || if can_run() { "s4a-btn-run s4a-btn-run-ready" } else { "s4a-btn-run" }
                        on:click=on_run_engine
                    >"▶  Run engine"</button>
                </div>
            </div>
        </div>
    }
}
