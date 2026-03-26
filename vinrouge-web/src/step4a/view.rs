use std::collections::HashMap;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::components::{Banner, GhostButton, Spinner};
use crate::ipc::{tauri_invoke_args};
use crate::ollama::{ask_ollama_wasm, OLLAMA_DEFAULT_MODEL, OLLAMA_DEFAULT_URL};
use crate::types::{AuditProcessWithControls, DslScript, JoinSpec, RelCandidate, SessionSchema};
use super::types::{Phase, ScriptStatus, ScriptState, RunResult, ChatMsg};
use super::helpers::{parse_run_result, extract_dsl_code};
use super::pipeline::do_load_or_generate;

#[component]
pub fn Step4aView(
    audit_plan: RwSignal<Vec<AuditProcessWithControls>>,
    audit_ui_step: RwSignal<u8>,
    status: RwSignal<String>,
) -> impl IntoView {
    // ── Core signals ──────────────────────────────────────────────────────────
    let phase: RwSignal<Phase>                           = RwSignal::new(Phase::Loading);
    let schemas: RwSignal<Vec<SessionSchema>>             = RwSignal::new(vec![]);
    let scripts: RwSignal<Vec<DslScript>>                 = RwSignal::new(vec![]);
    let script_states: RwSignal<HashMap<String, ScriptState>> = RwSignal::new(HashMap::new());
    let progress_msg: RwSignal<String>                   = RwSignal::new("Loading data…".to_string());

    // ── Relationship review signals ───────────────────────────────────────────
    let join_candidates: RwSignal<Vec<RelCandidate>>     = RwSignal::new(vec![]);
    let accepted_joins: RwSignal<Vec<bool>>              = RwSignal::new(vec![]);

    // ── Manual relationship form signals ──────────────────────────────────────
    let manual_left_table: RwSignal<String>              = RwSignal::new(String::new());
    let manual_left_col: RwSignal<String>                = RwSignal::new(String::new());
    let manual_right_table: RwSignal<String>             = RwSignal::new(String::new());
    let manual_right_col: RwSignal<String>               = RwSignal::new(String::new());

    // ── Three-zone signals ────────────────────────────────────────────────────
    let selected_id: RwSignal<Option<String>>            = RwSignal::new(None);
    let preview_cols: RwSignal<Vec<String>>              = RwSignal::new(vec![]);
    let preview_rows: RwSignal<Vec<Vec<String>>>         = RwSignal::new(vec![]);
    let preview_source: RwSignal<String>                 = RwSignal::new(String::new());
    let last_result: RwSignal<Option<RunResult>>         = RwSignal::new(None);
    let run_loading: RwSignal<bool>                      = RwSignal::new(false);
    let chat_msgs: RwSignal<Vec<ChatMsg>>                = RwSignal::new(vec![]);
    let chat_input: RwSignal<String>                     = RwSignal::new(String::new());
    let chat_loading: RwSignal<bool>                     = RwSignal::new(false);

    // ── Resize state for horizontal divider ──────────────────────────────────
    let left_width: RwSignal<f64>                        = RwSignal::new(60.0); // percentage
    let is_resizing: RwSignal<bool>                      = RwSignal::new(false);

    // ── Pop-out window state ──────────────────────────────────────────────────
    let is_popped_out: RwSignal<bool>                    = RwSignal::new(false);

    // ── Cell selection state ──────────────────────────────────────────────────
    let selected_cell: RwSignal<Option<(usize, usize)>>  = RwSignal::new(None); // (row, col)

    // ── Hide-empty-columns toggle ─────────────────────────────────────────────
    let hide_empty_cols: RwSignal<bool>                  = RwSignal::new(false);

    // ── Keyboard navigation handler ───────────────────────────────────────────
    use leptos::ev::KeyboardEvent;
    let on_keydown = move |ev: KeyboardEvent| {
        if let Some((row, col)) = selected_cell.get() {
            let rows = preview_rows.get();
            let cols = preview_cols.get();
            let max_row = rows.len().saturating_sub(1);
            let max_col = cols.len().saturating_sub(1);

            let new_pos = match ev.key().as_str() {
                "ArrowUp" if row > 0 => {
                    ev.prevent_default();
                    Some((row - 1, col))
                }
                "ArrowDown" if row < max_row => {
                    ev.prevent_default();
                    Some((row + 1, col))
                }
                "ArrowLeft" if col > 0 => {
                    ev.prevent_default();
                    Some((row, col - 1))
                }
                "ArrowRight" if col < max_col => {
                    ev.prevent_default();
                    Some((row, col + 1))
                }
                _ => None,
            };

            if let Some((new_row, new_col)) = new_pos {
                selected_cell.set(Some((new_row, new_col)));

                // Scroll to make the selected cell visible
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        let selector = format!("tbody tr:nth-child({}) td:nth-child({})", new_row + 1, new_col + 2);
                        if let Ok(Some(cell)) = document.query_selector(&selector) {
                            if let Some(element) = cell.dyn_ref::<web_sys::HtmlElement>() {
                                element.scroll_into_view_with_bool(false);
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

    // ── Listen for pop-out window close event ─────────────────────────────────
    let is_popped_out_listener = is_popped_out.clone();
    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |_event: JsValue| {
            is_popped_out_listener.set(false);
        }) as Box<dyn Fn(JsValue)>);

        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))
                .and_then(|tauri| js_sys::Reflect::get(&tauri, &JsValue::from_str("event")))
                .and_then(|event_module| {
                    js_sys::Reflect::get(&event_module, &JsValue::from_str("listen"))
                })
                .and_then(|listen_fn| {
                    let listen = listen_fn.dyn_ref::<js_sys::Function>().unwrap();
                    listen.call2(
                        &JsValue::NULL,
                        &JsValue::from_str("data-preview-closed"),
                        closure.as_ref()
                    )
                });
        }
        closure.forget();
    });

    // ── Derived counts ────────────────────────────────────────────────────────
    let total_count    = move || scripts.get().len();
    let approved_count = move || {
        script_states.get().values()
            .filter(|s| s.status == ScriptStatus::Approved).count()
    };
    let pending_count  = move || {
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

    // ── "Generate all" handler ────────────────────────────────────────────────
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

    // ── Add manual join handler ───────────────────────────────────────────────
    let on_add_manual_join = move |_| {
        let lt = manual_left_table.get_untracked();
        let lc = manual_left_col.get_untracked();
        let rt = manual_right_table.get_untracked();
        let rc = manual_right_col.get_untracked();
        if lt.is_empty() || lc.is_empty() || rt.is_empty() || rc.is_empty() || lt == rt {
            return;
        }
        let schemas_snap = schemas.get_untracked();
        let left_s  = schemas_snap.iter().find(|s| s.table_name == lt).cloned();
        let right_s = schemas_snap.iter().find(|s| s.table_name == rt).cloned();
        if let (Some(l), Some(r)) = (left_s, right_s) {
            let candidate = RelCandidate {
                left_import_id:  l.import_id,
                left_table:      lt,
                left_col:        lc,
                right_import_id: r.import_id,
                right_table:     rt,
                right_col:       rc,
                confidence:      100,
                overlap_count:   0,
            };
            join_candidates.update(|v| v.push(candidate));
            accepted_joins.update(|v| v.push(true));
            manual_left_col.set(String::new());
            manual_right_col.set(String::new());
        }
    };

    // ── Build master record handler ───────────────────────────────────────────
    let on_build_master = move |_| {
        let candidates = join_candidates.get_untracked();
        let accepted   = accepted_joins.get_untracked();
        let specs: Vec<JoinSpec> = candidates
            .into_iter()
            .zip(accepted.into_iter())
            .filter(|(_, ok)| *ok)
            .map(|(c, _)| JoinSpec {
                left_import_id:  c.left_import_id,
                left_col:        c.left_col,
                right_import_id: c.right_import_id,
                right_col:       c.right_col,
            })
            .collect();

        if specs.is_empty() {
            phase.set(Phase::Error("No joins selected — tick at least one.".to_string()));
            return;
        }

        phase.set(Phase::BuildingMaster);
        spawn_local(async move {
            match tauri_invoke_args::<String>(
                "build_master_record",
                serde_json::json!({ "joins": specs }),
            )
            .await
            {
                Ok(_) => {
                    do_load_or_generate(
                        audit_plan, phase, schemas, scripts, script_states,
                        progress_msg, status, selected_id,
                        preview_cols, preview_rows, preview_source,
                        join_candidates, accepted_joins,
                        false,
                    )
                    .await;
                }
                Err(e) => phase.set(Phase::Error(format!("Master build failed: {e}"))),
            }
        });
    };

    // ── Skip relationship review handler ──────────────────────────────────────
    let on_skip_join = move |_| {
        spawn_local(async move {
            do_load_or_generate(
                audit_plan, phase, schemas, scripts, script_states,
                progress_msg, status, selected_id,
                preview_cols, preview_rows, preview_source,
                join_candidates, accepted_joins,
                false,
            )
            .await;
        });
    };

    // ── Zone 2: run the selected script ──────────────────────────────────────
    let on_z2_run = move |_| {
        let sid = match selected_id.get_untracked() { Some(s) => s, None => return };
        let text = script_states.get_untracked()
            .get(&sid).map(|s| s.text.clone()).unwrap_or_default();
        if text.trim().is_empty() { return; }
        run_loading.set(true);
        let t0 = js_sys::Date::now();
        spawn_local(async move {
            // Persist any edits first.
            let _ = tauri_invoke_args::<()>(
                "update_dsl_script",
                serde_json::json!({ "scriptId": sid, "scriptText": text }),
            ).await;
            match tauri_invoke_args::<Vec<serde_json::Value>>(
                "run_dsl_script",
                serde_json::json!({ "scriptId": sid }),
            ).await {
                Ok(results) => {
                    let dt = js_sys::Date::now() - t0;
                    last_result.set(Some(parse_run_result(&results, dt)));
                }
                Err(e) => {
                    last_result.set(Some(RunResult {
                        expr_type: "ERROR".to_string(),
                        expected:  "—".to_string(),
                        actual:    e,
                        passed:    false,
                        duration_ms: js_sys::Date::now() - t0,
                    }));
                }
            }
            run_loading.set(false);
        });
    };

    // ── Zone 2: clear (reset to original text) ────────────────────────────────
    let on_z2_clear = move |_| {
        let sid = match selected_id.get_untracked() { Some(s) => s, None => return };
        let original = scripts.get_untracked().into_iter()
            .find(|s| s.id == sid).map(|s| s.script_text.clone()).unwrap_or_default();
        script_states.update(|m| {
            if let Some(st) = m.get_mut(&sid) {
                st.text   = original;
                st.status = ScriptStatus::Generated;
            }
        });
        last_result.set(None);
    };

    // ── Zone 2: save ──────────────────────────────────────────────────────────
    let on_z2_save = move |_| {
        let sid = match selected_id.get_untracked() { Some(s) => s, None => return };
        let text = script_states.get_untracked()
            .get(&sid).map(|s| s.text.clone()).unwrap_or_default();
        spawn_local(async move {
            let _ = tauri_invoke_args::<()>(
                "update_dsl_script",
                serde_json::json!({ "scriptId": sid, "scriptText": text }),
            ).await;
        });
    };

    // ── Zone 3: send chat message ─────────────────────────────────────────────
    let on_chat_send = move |_| {
        let msg = chat_input.get_untracked();
        let msg = msg.trim().to_string();
        if msg.is_empty() || chat_loading.get_untracked() { return; }

        chat_msgs.update(|v| v.push(ChatMsg { is_user: true, text: msg.clone(), code: None }));
        chat_input.set(String::new());
        chat_loading.set(true);

        // Build context for the AI.
        let schema_ctx = schemas.get_untracked().iter()
            .map(|s| format!("Table {} ({} rows): {}", s.table_name, s.row_count, s.columns.join(", ")))
            .collect::<Vec<_>>().join("\n");
        let current_script = selected_id.get_untracked()
            .and_then(|sid| script_states.get_untracked().get(&sid).map(|s| s.text.clone()))
            .unwrap_or_default();
        let context = format!(
            "You are a VinRouge audit DSL assistant. \
             Help the auditor write and improve DSL test scripts.\n\n\
             Available data:\n{schema_ctx}\n\n\
             Current script:\n{current_script}\n\n\
             DSL language reference:\n\
             - EXCEPTIONS <table> WHERE <condition> [AND <condition>]\n\
             - RECONCILE <field>=<value> <field>=<value> [threshold=<n>]\n\
             - SAMPLE <table> RANDOM|INTERVAL <n>\n\
             - TOTAL <table>.<field>\n\
             - COUNT <table> [WHERE <condition>]\n\n\
             When suggesting DSL code, wrap it in ``` fences."
        );

        spawn_local(async move {
            match ask_ollama_wasm(OLLAMA_DEFAULT_URL, OLLAMA_DEFAULT_MODEL, &context, &msg).await {
                Ok(resp) => {
                    let code = extract_dsl_code(&resp);
                    chat_msgs.update(|v| v.push(ChatMsg { is_user: false, text: resp, code }));
                }
                Err(e) => {
                    chat_msgs.update(|v| v.push(ChatMsg {
                        is_user: false,
                        text: format!("Could not reach AI: {e}"),
                        code: None,
                    }));
                }
            }
            chat_loading.set(false);
        });
    };

    // ── Zone 3: inject code snippet into Zone 2 editor ───────────────────────
    let inject_code = move |code: String| {
        let sid = match selected_id.get_untracked() { Some(s) => s, None => return };
        script_states.update(|m| {
            if let Some(st) = m.get_mut(&sid) {
                st.text   = code;
                st.status = ScriptStatus::Edited;
            }
        });
    };

    // ── Resize handlers for horizontal divider ────────────────────────────────
    let on_resize_start = move |ev: web_sys::MouseEvent| {
        ev.prevent_default();
        is_resizing.set(true);
    };

    let on_resize_move = move |ev: web_sys::MouseEvent| {
        if !is_resizing.get() { return; }
        if let Some(window) = web_sys::window() {
            let window_width = window.inner_width().ok()
                .and_then(|w| w.as_f64()).unwrap_or(1200.0);
            let mouse_x = ev.client_x() as f64;
            let percentage = (mouse_x / window_width) * 100.0;
            let clamped = percentage.max(30.0).min(80.0);
            left_width.set(clamped);
        }
    };

    let on_resize_end = move |_ev: web_sys::MouseEvent| {
        is_resizing.set(false);
    };

    // ── Pop-out handler ────────────────────────────────────────────────────────
    let on_popout = move |_| {
        let cols = preview_cols.get_untracked();
        let rows = preview_rows.get_untracked();
        let source = preview_source.get_untracked();

        let data = serde_json::json!({
            "columns": cols,
            "rows": rows,
            "source": source
        });

        spawn_local(async move {
            let result = tauri_invoke_args::<()>(
                "open_data_preview_window",
                serde_json::json!({ "data": data }),
            ).await;

            match result {
                Ok(_) => {
                    is_popped_out.set(true);
                }
                Err(e) => {
                    status.set(format!("Could not open pop-out window: {e}"));
                }
            }
        });
    };

    // ── Run engine (all approved scripts → step 5) ────────────────────────────
    let on_run_engine = move |_| {
        if !can_run() { return; }
        let ss      = script_states.get_untracked();
        let all     = scripts.get_untracked();
        let has_any = ss.values().any(|s| s.status == ScriptStatus::Approved);
        let to_run: Vec<DslScript> = all.into_iter().filter(|s| {
            ss.get(&s.id).map(|st| {
                if has_any { st.status == ScriptStatus::Approved }
                else       { st.status != ScriptStatus::Rejected }
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
            audit_ui_step.set(6);
        });
    };

    // ── View ──────────────────────────────────────────────────────────────────
    view! {
        <div style="flex:1;display:flex;flex-direction:column;overflow:hidden">

            // ── Page header ────────────────────────────────────────────────────
            <div class="s4a-page-hdr">
                <div class="s4a-hdr-left">
                    <span class="s4a-page-step">"Step 4a"</span>
                    <span class="s4a-page-title">"Algorithm review"</span>
                    <span class="s4a-page-sub">
                        "Review, edit and run DSL algorithms against your data"
                    </span>
                </div>
                <div class="s4a-page-stats">
                    {move || match phase.get() {
                        Phase::Loading | Phase::Generating | Phase::Running { .. }
                        | Phase::BuildingMaster => {
                            view! {
                                <div style="display:flex;align-items:center;gap:8px;color:var(--w-text-3);font-size:12px">
                                    <Spinner size=12 />
                                    {move || progress_msg.get()}
                                </div>
                            }.into_any()
                        }
                        Phase::RelationshipReview => view! {
                            <div style="color:var(--w-text-3);font-size:12px">
                                "Review how your datasets connect"
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
                                    disabled=Signal::derive(move || {
                                        !matches!(phase.get(), Phase::Review | Phase::Error(_))
                                    })
                                    on:click=on_generate_all
                                >
                                    "↻  Regenerate"
                                </button>
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

            // ── Relationship review panel ──────────────────────────────────────
            {move || (phase.get() == Phase::RelationshipReview).then(|| {
                let candidates = join_candidates.get();
                let schemas_snap = schemas.get();
                let primary = schemas_snap.iter()
                    .filter(|s| s.source_type != "master")
                    .max_by_key(|s| s.row_count)
                    .cloned();
                let primary_name = primary.map(|s| s.table_name).unwrap_or_default();

                view! {
                    <div style="flex:1;overflow-y:auto;padding:20px 24px;display:flex;flex-direction:column;gap:16px">
                        <div style="font-size:13px;color:var(--w-text-2);max-width:600px">
                            "Multiple datasets detected. We found these possible connections — "
                            "tick the ones that are correct, then build a unified master record."
                        </div>

                        // Primary table badge
                        <div style="display:flex;align-items:center;gap:8px;font-size:11px;color:var(--w-text-3)">
                            <span style="background:#1e3a1e;color:#4ade80;padding:2px 8px;border-radius:3px;font-weight:600">
                                "PRIMARY"
                            </span>
                            {primary_name.clone()}
                            " — all rows kept; secondary tables joined in"
                        </div>

                        // No-candidates notice
                        {candidates.is_empty().then(|| view! {
                            <div style="font-size:12px;color:#f87171;padding:8px 10px;\
                                        background:#1a0a0a;border:0.5px solid #4a1a1a;\
                                        border-radius:4px;max-width:700px">
                                "No relationships were detected automatically between your datasets. \
                                 Add one manually below, or choose \"Use Separately\" to keep the tables independent."
                            </div>
                        })}

                        // Candidate list
                        <div style="display:flex;flex-direction:column;gap:6px;max-width:700px">
                            {candidates.into_iter().enumerate().map(|(i, c)| {
                                let is_accepted = accepted_joins.get().get(i).copied().unwrap_or(false);
                                let conf = c.confidence;
                                let bar_color = if conf >= 80 { "#4ade80" }
                                    else if conf >= 50 { "#facc15" }
                                    else { "#f87171" };
                                let desc = format!(
                                    "{}.{} ↔ {}.{}",
                                    c.left_table, c.left_col, c.right_table, c.right_col
                                );
                                let overlap = c.overlap_count;
                                view! {
                                    <div
                                        style=move || format!(
                                            "display:flex;align-items:center;gap:10px;padding:8px 10px;\
                                             background:{};border-radius:4px;border:0.5px solid {};cursor:pointer",
                                            if is_accepted { "#0f1f0f" } else { "#111" },
                                            if is_accepted { "#2d5a2d" } else { "var(--w-border)" }
                                        )
                                        on:click=move |_| {
                                            accepted_joins.update(|v| {
                                                if let Some(b) = v.get_mut(i) { *b = !*b; }
                                            });
                                        }
                                    >
                                        // Checkbox
                                        <div style=move || format!(
                                            "width:14px;height:14px;border-radius:2px;flex-shrink:0;\
                                             background:{};border:1px solid {}",
                                            if is_accepted { "#4ade80" } else { "var(--w-border)" },
                                            if is_accepted { "#4ade80" } else { "var(--w-border)" }
                                        )>
                                            {move || is_accepted.then(|| view! {
                                                <span style="font-size:10px;line-height:14px;display:flex;justify-content:center">
                                                    "✓"
                                                </span>
                                            })}
                                        </div>

                                        // Join description
                                        <span style="font-family:monospace;font-size:12px;flex:1;color:var(--w-text-1)">
                                            {desc}
                                        </span>

                                        // Overlap count
                                        <span style="font-size:11px;color:var(--w-text-3)">
                                            {format!("{overlap} matching values")}
                                        </span>

                                        // Confidence bar
                                        <div style="display:flex;align-items:center;gap:4px;width:80px;flex-shrink:0">
                                            <div style=format!(
                                                "height:4px;background:{};border-radius:2px;width:{}px",
                                                bar_color, (conf as f32 / 100.0 * 64.0) as u8
                                            )></div>
                                            <span style=format!("font-size:10px;color:{bar_color}")>
                                                {format!("{conf}%")}
                                            </span>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>

                        // Manual add form
                        <div style="border:0.5px solid #333;border-radius:4px;padding:12px;max-width:700px">
                            <div style="font-size:10px;color:var(--w-text-3);margin-bottom:8px;\
                                        text-transform:uppercase;letter-spacing:0.06em">
                                "Add relationship manually"
                            </div>
                            <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap">
                                // Left table
                                <select
                                    style="background:#111;color:var(--w-text-1);border:0.5px solid #333;\
                                           border-radius:3px;padding:4px 6px;font-size:11px"
                                    on:change=move |ev| {
                                        manual_left_table.set(event_target_value(&ev));
                                        manual_left_col.set(String::new());
                                    }
                                >
                                    <option value="">"— left table —"</option>
                                    {schemas_snap.iter().filter(|s| s.source_type != "master").map(|s| {
                                        let tn = s.table_name.clone();
                                        let tn2 = tn.clone();
                                        view! { <option value={tn}>{tn2}</option> }
                                    }).collect_view()}
                                </select>
                                // Left column (reactive on selected left table)
                                {move || {
                                    let lt = manual_left_table.get();
                                    let cols = schemas.get().iter()
                                        .find(|s| s.table_name == lt)
                                        .map(|s| s.columns.clone())
                                        .unwrap_or_default();
                                    view! {
                                        <select
                                            style="background:#111;color:var(--w-text-1);border:0.5px solid #333;\
                                                   border-radius:3px;padding:4px 6px;font-size:11px"
                                            on:change=move |ev| manual_left_col.set(event_target_value(&ev))
                                        >
                                            <option value="">"— column —"</option>
                                            {cols.into_iter().map(|c| {
                                                let c2 = c.clone();
                                                view! { <option value={c}>{c2}</option> }
                                            }).collect_view()}
                                        </select>
                                    }
                                }}
                                <span style="color:#555;font-size:13px">"↔"</span>
                                // Right table
                                <select
                                    style="background:#111;color:var(--w-text-1);border:0.5px solid #333;\
                                           border-radius:3px;padding:4px 6px;font-size:11px"
                                    on:change=move |ev| {
                                        manual_right_table.set(event_target_value(&ev));
                                        manual_right_col.set(String::new());
                                    }
                                >
                                    <option value="">"— right table —"</option>
                                    {schemas_snap.iter().filter(|s| s.source_type != "master").map(|s| {
                                        let tn = s.table_name.clone();
                                        let tn2 = tn.clone();
                                        view! { <option value={tn}>{tn2}</option> }
                                    }).collect_view()}
                                </select>
                                // Right column (reactive on selected right table)
                                {move || {
                                    let rt = manual_right_table.get();
                                    let cols = schemas.get().iter()
                                        .find(|s| s.table_name == rt)
                                        .map(|s| s.columns.clone())
                                        .unwrap_or_default();
                                    view! {
                                        <select
                                            style="background:#111;color:var(--w-text-1);border:0.5px solid #333;\
                                                   border-radius:3px;padding:4px 6px;font-size:11px"
                                            on:change=move |ev| manual_right_col.set(event_target_value(&ev))
                                        >
                                            <option value="">"— column —"</option>
                                            {cols.into_iter().map(|c| {
                                                let c2 = c.clone();
                                                view! { <option value={c}>{c2}</option> }
                                            }).collect_view()}
                                        </select>
                                    }
                                }}
                                <button
                                    style="padding:4px 12px;background:#0d2a0d;border:0.5px solid #2d5a2d;\
                                           color:#4ade80;border-radius:3px;font-size:11px;cursor:pointer"
                                    on:click=on_add_manual_join
                                >
                                    "+ Add"
                                </button>
                            </div>
                        </div>

                        // Action buttons
                        <div style="display:flex;gap:10px;margin-top:8px">
                            <button
                                style="padding:7px 18px;background:#0d2a0d;border:0.5px solid #2d5a2d;\
                                       color:#4ade80;border-radius:4px;font-size:12px;cursor:pointer"
                                on:click=on_build_master
                            >
                                "Build Master Record →"
                            </button>
                            <button
                                style="padding:7px 14px;background:transparent;border:0.5px solid var(--w-border);\
                                       color:var(--w-text-3);border-radius:4px;font-size:12px;cursor:pointer"
                                on:click=on_skip_join
                            >
                                "Use Separately"
                            </button>
                        </div>
                    </div>
                }
            })}

            // ── Building master spinner ────────────────────────────────────────
            {move || (phase.get() == Phase::BuildingMaster).then(|| view! {
                <div style="flex:1;display:flex;align-items:center;justify-content:center;gap:10px;\
                            color:var(--w-text-3);font-size:13px">
                    <Spinner size=16 />
                    "Building master record…"
                </div>
            })}

            // ── Two column layout: data preview | stacked editor+chat ──────────
            <div class="s4a-zones"
                style=move || {
                    let hide = matches!(phase.get(), Phase::RelationshipReview | Phase::BuildingMaster);
                    if hide {
                        return "display:none".to_string();
                    }
                    if is_popped_out.get() {
                        "display:grid;grid-template-columns:1fr;gap:0;flex:1;overflow:hidden".to_string()
                    } else {
                        format!("display:grid;grid-template-columns:{}% 4px 1fr;gap:0;flex:1;overflow:hidden", left_width.get())
                    }
                }
                on:mousemove=on_resize_move
                on:mouseup=on_resize_end
            >

                // ── Zone 1: dark spreadsheet ───────────────────────────────────
                <div class="s4a-z1" style=move || if is_popped_out.get() { "display:none" } else { "" }>
                    <div class="s4a-zone-hdr-dark">"data preview"</div>
                    <div class="s4a-tbl-topbar">
                        <div style="display:flex;gap:6px;align-items:center">
                            <span class="s4a-tbl-pill">
                                {move || {
                                    let r = preview_rows.get().len();
                                    format!("{r} rows")
                                }}
                            </span>
                            <span class="s4a-tbl-pill s4a-tbl-pill-src">
                                {move || preview_source.get()}
                            </span>
                        </div>
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
                            >
                                "⊘ Hide empty cols"
                            </button>
                            <button
                                class="s4a-popout-btn"
                                title="Pop out to separate window"
                                disabled=Signal::derive(move || preview_cols.get().is_empty())
                                on:click=move |_| on_popout(())
                                style="padding:4px 10px;font-size:11px;background:#1a1a1a;\
                                       border:0.5px solid #333;border-radius:3px;color:#aaa;cursor:pointer;\
                                       display:flex;align-items:center;gap:4px"
                            >
                                "⧉ Pop out"
                            </button>
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

                            // Determine which column indices to show
                            let visible_indices: Vec<usize> = if hide_empty_cols.get() {
                                (0..all_cols.len())
                                    .filter(|&ci| {
                                        all_rows.iter().any(|row| {
                                            row.get(ci).map(|v| !v.trim().is_empty()).unwrap_or(false)
                                        })
                                    })
                                    .collect()
                            } else {
                                (0..all_cols.len()).collect()
                            };

                            let cols: Vec<String> = visible_indices.iter().map(|&i| all_cols[i].clone()).collect();
                            let rows: Vec<Vec<String>> = all_rows.iter().map(|row| {
                                visible_indices.iter().map(|&i| row.get(i).cloned().unwrap_or_default()).collect()
                            }).collect();

                            // Column letter labels (A, B, C …)
                            let letters: Vec<String> = (0..cols.len())
                                .map(|i| {
                                    if i < 26 {
                                        ((b'A' + i as u8) as char).to_string()
                                    } else {
                                        format!("{}{}", (b'A' + (i / 26 - 1) as u8) as char,
                                            (b'A' + (i % 26) as u8) as char)
                                    }
                                })
                                .collect();
                            let cols2 = cols.clone();
                            let rows_view = rows.iter().enumerate().map(|(ri, row)| {
                                let cells = row.iter().enumerate().map(|(ci, val)| {
                                    let row_idx = ri;
                                    let col_idx = ci;
                                    let val_clone = val.clone();
                                    let selected = selected_cell.get();
                                    let is_selected = selected == Some((row_idx, col_idx));
                                    let is_selected_row = selected.map(|(r, _)| r == row_idx).unwrap_or(false);
                                    let is_selected_col = selected.map(|(_, c)| c == col_idx).unwrap_or(false);

                                    let class = if is_selected {
                                        "selected"
                                    } else if is_selected_row {
                                        "selected-row"
                                    } else if is_selected_col {
                                        "selected-col"
                                    } else {
                                        ""
                                    };

                                    view! {
                                        <td
                                            class=class
                                            on:click=move |_| {
                                                selected_cell.set(Some((row_idx, col_idx)));
                                            }
                                        >
                                            {val_clone}
                                        </td>
                                    }
                                }).collect_view();
                                view! {
                                    <tr>
                                        <td class="rn">{ri + 1}</td>
                                        {cells}
                                    </tr>
                                }
                            }).collect_view();
                            view! {
                                <table class="s4a-sheet-tbl">
                                    <thead>
                                        <tr>
                                            <th class="s4a-th-lbl corner"></th>
                                            {letters.iter().map(|l| {
                                                let l = l.clone();
                                                view! { <th class="s4a-th-lbl">{l}</th> }
                                            }).collect_view()}
                                        </tr>
                                        <tr>
                                            <th class="s4a-th-fld" style="background:#111"></th>
                                            {cols2.iter().map(|c| {
                                                let c = c.clone();
                                                view! { <th class="s4a-th-fld">{c}</th> }
                                            }).collect_view()}
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

                // ── Right column: stacked DSL editor + AI chat ─────────────────
                <div style="display:flex;flex-direction:column;overflow:hidden">

                // ── Zone 2: DSL editor ─────────────────────────────────────────
                <div class="s4a-z2" style="flex:1;overflow:auto;border-bottom:0.5px solid var(--w-border)">
                    <div class="s4a-zone-hdr">"DSL expression"</div>
                    <div class="s4a-z2-body">

                        // Script selector
                        {move || {
                            let all = scripts.get();
                            if all.is_empty() { return None; }
                            let opts = all.iter().map(|s| {
                                let sid   = s.id.clone();
                                let label = format!("{} — {}", s.control_ref, s.label);
                                let sel   = selected_id.get().as_deref() == Some(s.id.as_str());
                                view! {
                                    <option value=sid selected=sel>{label}</option>
                                }
                            }).collect_view();
                            Some(view! {
                                <div>
                                    <div class="s4a-z2-lbl" style="margin-bottom:4px">"script"</div>
                                    <select
                                        class="s4a-z2-select"
                                        on:change=move |ev| {
                                            selected_id.set(Some(event_target_value(&ev)));
                                            last_result.set(None);
                                        }
                                    >
                                        {opts}
                                    </select>
                                </div>
                            })
                        }}

                        // Expression editor
                        {move || {
                            let sid = selected_id.get()?;
                            let text = script_states.get()
                                .get(&sid).map(|s| s.text.clone()).unwrap_or_default();
                            let sid_input = sid.clone();
                            Some(view! {
                                <div>
                                    <div class="s4a-z2-lbl" style="margin-bottom:4px">
                                        "expression"
                                    </div>
                                    <textarea
                                        class="s4a-z2-ed"
                                        prop:value=text
                                        on:input=move |ev| {
                                            if let Some(ta) = ev.target().and_then(|t|
                                                t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
                                            {
                                                let val = ta.value();
                                                let sid = sid_input.clone();
                                                script_states.update(|m| {
                                                    if let Some(st) = m.get_mut(&sid) {
                                                        st.text = val;
                                                        if st.status == ScriptStatus::Generated {
                                                            st.status = ScriptStatus::Edited;
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    ></textarea>
                                </div>
                            })
                        }}

                        // Action buttons
                        <div class="s4a-z2-actions">
                            <button
                                class="s4a-z2-btn s4a-z2-btn-run"
                                disabled=Signal::derive(move || {
                                    run_loading.get() || selected_id.get().is_none()
                                })
                                on:click=on_z2_run
                            >
                                {move || if run_loading.get() { "…" } else { "▶ run" }}
                            </button>
                            <button
                                class="s4a-z2-btn"
                                disabled=Signal::derive(move || selected_id.get().is_none())
                                on:click=on_z2_clear
                            >
                                "clear"
                            </button>
                            <button
                                class="s4a-z2-btn"
                                disabled=Signal::derive(move || selected_id.get().is_none())
                                on:click=on_z2_save
                            >
                                "+ save"
                            </button>
                        </div>

                        // Keyword hints
                        <div class="s4a-z2-hint">
                            "EXCEPTIONS"<br/>
                            "RECONCILE"<br/>
                            "SAMPLE"<br/>
                            "TOTAL · COUNT"
                        </div>

                        // Last result block
                        {move || {
                            let res = last_result.get()?;
                            let fail = !res.passed;
                            let duration = format!("{:.1}ms", res.duration_ms);
                            let status_txt = if res.passed { "pass" } else { "fail" };
                            let actual  = res.actual.clone();
                            let expected = res.expected.clone();
                            let expr_t  = res.expr_type.clone();
                            Some(view! {
                                <div class="s4a-result-block">
                                    <div class="s4a-result-hdr">"last result"</div>
                                    <div class="s4a-result-row">
                                        <span class="s4a-result-key">"expression"</span>
                                        <span class="s4a-result-val">{expr_t}</span>
                                    </div>
                                    <div class="s4a-result-row">
                                        <span class="s4a-result-key">"expected"</span>
                                        <span class="s4a-result-val">{expected}</span>
                                    </div>
                                    <div class="s4a-result-row">
                                        <span class=move || {
                                            if fail { "s4a-result-key" } else { "s4a-result-key" }
                                        }>"actual"</span>
                                        <span class=move || {
                                            if fail { "s4a-result-val s4a-result-fail" }
                                            else    { "s4a-result-val" }
                                        }>{actual.clone()}</span>
                                    </div>
                                    <div class="s4a-result-row">
                                        <span class="s4a-result-key">"status"</span>
                                        <span class=move || {
                                            if fail { "s4a-result-val s4a-result-fail" }
                                            else    { "s4a-result-val" }
                                        }>{status_txt}</span>
                                    </div>
                                    <div class="s4a-result-row">
                                        <span class="s4a-result-key">"duration"</span>
                                        <span class="s4a-result-val">{duration}</span>
                                    </div>
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
                                let code    = m.code.clone();
                                view! {
                                    <div class="s4a-msg">
                                        <div class=lbl_cls>{lbl}</div>
                                        <div class=bbl_cls>
                                            {m.text.clone()}
                                            {code.map(|c| {
                                                let c2 = c.clone();
                                                view! {
                                                    <code
                                                        class="s4a-code-chip"
                                                        title="Click to use in editor"
                                                        on:click=move |_| inject_code(c2.clone())
                                                    >
                                                        {c}
                                                    </code>
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
                        <textarea
                            class="s4a-chat-ta"
                            rows=2
                            placeholder="prompt vin rouge…"
                            prop:value=move || chat_input.get()
                            on:input=move |ev| {
                                if let Some(ta) = ev.target().and_then(|t|
                                    t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
                                {
                                    chat_input.set(ta.value());
                                }
                            }
                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                if ev.key() == "Enter" && (ev.ctrl_key() || ev.meta_key()) {
                                    on_chat_send(());
                                }
                            }
                        ></textarea>
                        <button
                            class="s4a-send-btn"
                            disabled=Signal::derive(move || {
                                chat_loading.get() || chat_input.get().trim().is_empty()
                            })
                            on:click=move |_| on_chat_send(())
                        >
                            "→"
                        </button>
                    </div>
                </div>

                </div> // end right column wrapper
            </div>

            // ── Status bar ─────────────────────────────────────────────────────
            <div class="s4-status-bar">
                <span class=move || {
                    if can_run() { "s4-dot s4-dot--ready" } else { "s4-dot s4-dot--idle" }
                }></span>
                <span class="s4-status-text">
                    {move || match phase.get() {
                        Phase::Loading => "Loading data…".to_string(),
                        Phase::RelationshipReview => "Confirm how your datasets connect".to_string(),
                        Phase::BuildingMaster => "Building master record…".to_string(),
                        Phase::Generating => progress_msg.get(),
                        Phase::Running { done, total } => {
                            format!("Running scripts: {done}/{total}…")
                        }
                        Phase::Review => {
                            let a = approved_count();
                            let n = total_count();
                            if a == 0 {
                                format!("0 of {n} algorithms approved — approve or run directly")
                            } else {
                                format!("{a} of {n} approved")
                            }
                        }
                        Phase::Error(e) => format!("Error: {e}"),
                    }}
                </span>
                <div style="margin-left:auto;display:flex;align-items:center;gap:8px">
                    <GhostButton label="Back" back=true
                        on_click=Callback::new(move |()| audit_ui_step.set(4)) />
                    <button
                        class=move || {
                            if can_run() { "s4a-btn-run s4a-btn-run-ready" }
                            else         { "s4a-btn-run" }
                        }
                        on:click=on_run_engine
                    >
                        "▶  Run engine"
                    </button>
                </div>
            </div>
        </div>
    }
}
