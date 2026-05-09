use leptos::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use wasm_bindgen::{JsCast, JsValue};

// ── Popout window helpers ─────────────────────────────────────────────────────

/// Build the self-contained interactive HTML for the data popout window.
pub fn build_data_window_html(tables: &[(String, Vec<String>, Vec<Vec<String>>)]) -> String {
    let tables_val: Vec<serde_json::Value> = tables
        .iter()
        .map(
            |(name, cols, rows)| serde_json::json!({ "name": name, "columns": cols, "rows": rows }),
        )
        .collect();

    let tables_json = serde_json::to_string(&serde_json::Value::Array(tables_val))
        .unwrap_or_default()
        .replace("</script>", "<\\/script>");

    let mut html = String::new();
    html.push_str(r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Data Preview</title>
<style>
*{box-sizing:border-box}
body{margin:0;padding:0;background:#141414;color:#c0c0c0;
font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;
display:flex;flex-direction:column;height:100vh;overflow:hidden}
.topbar{display:flex;align-items:center;padding:5px 10px;border-bottom:0.5px solid #2a2a2a;
flex-shrink:0;gap:6px;background:#161616;overflow-x:auto}
.tab{background:none;border:0.5px solid transparent;border-radius:4px;padding:3px 8px;
font-size:11px;color:#555;cursor:pointer;display:flex;align-items:center;gap:5px;white-space:nowrap}
.tab:hover{color:#888;background:#1e1e1e}
.tab.active{color:#c0c0c0;background:#1e1e1e;border-color:#333}
.tab-count{background:#252525;border-radius:99px;padding:1px 5px;font-size:9px;color:#555}
.tab.active .tab-count{color:#888}
.pill{background:#1e1e1e;border:0.5px solid #333;border-radius:99px;padding:2px 8px;
font-size:10px;color:#666;flex-shrink:0;margin-left:auto}
.table-wrap{flex:1;overflow:auto}
table{width:100%;border-collapse:collapse;table-layout:auto}
th.lbl{background:#1a1a1a;padding:4px 7px;font-size:10px;font-weight:400;color:#444;
border-bottom:0.5px solid #2a2a2a;border-right:0.5px solid #222;text-align:center;
position:sticky;top:0;z-index:2}
th.lbl.corner{background:#111}
th.fld{background:#1a1a1a;padding:4px 7px;font-size:10px;font-weight:400;color:#666;
border-bottom:0.5px solid #282828;border-right:0.5px solid #222;text-align:left;
white-space:nowrap;position:sticky;top:23px;z-index:2;cursor:pointer;user-select:none}
th.fld:hover{color:#999;background:#1e1e1e}
th.fld.sort-asc,th.fld.sort-desc{color:#9fe1cb}
td{padding:5px 7px;border-bottom:0.5px solid #1e1e1e;border-right:0.5px solid #1a1a1a;
font-size:11px;color:#c0c0c0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:160px}
td.rn{background:#181818;color:#333;font-size:10px;text-align:center;padding:5px 3px;
border-right:0.5px solid #222;width:28px;min-width:28px}
tbody tr:hover td{background:#1c1c1c}
</style>
<script>
var TABLES="#);
    html.push_str(&tables_json);
    html.push_str(r#";
var T=0,SC=null,SA=true;
var ROW_H=28,OVERSCAN=20,PENDING=false;

function L(i){return i<26?String.fromCharCode(65+i):String.fromCharCode(64+Math.floor(i/26))+String.fromCharCode(65+(i%26));}
function E(s){var v=s==null?'':String(s);return v.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');}

function render(){
  var t=TABLES[T];
  var rows=t.rows.slice();
  if(SC!==null){
    rows.sort(function(a,b){
      var va=a[SC]==null?'':String(a[SC]);
      var vb=b[SC]==null?'':String(b[SC]);
      var na=parseFloat(va);
      var nb=parseFloat(vb);
      var c=(!isNaN(na)&&!isNaN(nb))?(na-nb):String(va).toLowerCase().localeCompare(String(vb).toLowerCase());
      return SA?c:-c;
    });
  }

  var wrap=document.getElementById('g');
  var st=wrap.scrollTop;
  var vh=wrap.clientHeight;
  var total=rows.length;
  var start=Math.max(0,Math.floor(st/ROW_H)-OVERSCAN);
  var end=Math.min(total,Math.ceil((st+vh)/ROW_H)+OVERSCAN);

  var h='<table><thead><tr><th class="lbl corner"></th>';
  for(var i=0;i<t.columns.length;i++){h+='<th class="lbl">'+L(i)+'</th>';}
  h+='</tr><tr><th class="lbl" style="background:#111"></th>';
  for(var i=0;i<t.columns.length;i++){
    var cls='fld'+(SC===i?(SA?' sort-asc':' sort-desc'):'');
    var arr=SC===i?(SA?' ↑':' ↓'):'';
    h+='<th class="'+cls+'" data-ci="'+i+'">'+E(t.columns[i])+arr+'</th>';
  }
  h+='</tr></thead><tbody>';

  if(start>0){
    h+='<tr style="height:'+(start*ROW_H)+'px"><td colspan="'+(t.columns.length+1)+'"></td></tr>';
  }

  for(var ri=start;ri<end;ri++){
    h+='<tr><td class="rn">'+(ri+1)+'</td>';
    var row=rows[ri];
    for(var ci=0;ci<t.columns.length;ci++){
      h+='<td>'+E(row[ci])+'</td>';
    }
    h+='</tr>';
  }

  if(end<total){
    h+='<tr style="height:'+((total-end)*ROW_H)+'px"><td colspan="'+(t.columns.length+1)+'"></td></tr>';
  }

  h+='</tbody></table>';
  wrap.innerHTML=h;
  document.getElementById('rc').textContent=total+' rows';
}

function sw(i){T=i;SC=null;SA=true;updateTabs();render();}
function updateTabs(){var tabs=document.querySelectorAll('.tab');for(var j=0;j<tabs.length;j++){tabs[j].classList.toggle('active',j===T);}}
function srt(ci){if(SC===ci){if(SA){SA=false;}else{SC=null;SA=true;}}else{SC=ci;SA=true;}render();}

document.addEventListener('DOMContentLoaded',function(){
  render();
  document.getElementById('g').addEventListener('click',function(e){
    var th=e.target.closest('th');
    if(th&&th.hasAttribute('data-ci')){srt(parseInt(th.getAttribute('data-ci'),10));}
  });
  document.getElementById('g').addEventListener('scroll',function(){
    if(!PENDING){
      PENDING=true;
      requestAnimationFrame(function(){PENDING=false;render();});
    }
  });
  document.querySelector('.topbar').addEventListener('click',function(e){
    var btn=e.target.closest('button');
    if(btn&&btn.hasAttribute('data-ti')){sw(parseInt(btn.getAttribute('data-ti'),10));}
  });
});
</script></head><body>
<div class="topbar">
"#);

    for (i, (name, _, rows)) in tables.iter().enumerate() {
        let safe = name
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let active = if i == 0 { " active" } else { "" };
        html.push_str(&format!(
            r#"<button class="tab{active}" data-ti="{i}">{safe}<span class="tab-count">{}</span></button>"#,
            rows.len()
        ));
    }

    let first_n = tables.first().map(|(_, _, r)| r.len()).unwrap_or(0);
    html.push_str(&format!(
        r#"<span id="rc" class="pill">{first_n} rows</span></div>
<div id="g" class="table-wrap"></div></body></html>"#
    ));

    html
}

/// Open all session tables in a new window.
///
/// `pre_win`: for the browser path, pass a window already opened synchronously in the
/// click handler (before any `await`) so the browser does not block the popup.
/// For Tauri, pass `None` — the native command creates the window itself.
pub async fn open_data_window(
    tables: Vec<(String, Vec<String>, Vec<Vec<String>>)>,
    pre_win: Option<web_sys::Window>,
) {
    if tables.is_empty() {
        // Close the pre-opened blank tab if we have nothing to show
        if let Some(w) = pre_win {
            let _ = w.close();
        }
        return;
    }

    if crate::ipc::is_tauri() {
        let tables_val: Vec<serde_json::Value> = tables.iter().map(|(name, cols, rows)| {
            serde_json::json!({ "name": name, "columns": cols, "rows": rows })
        }).collect();
        let args = serde_json::json!({ "data": { "tables": tables_val } });
        let _ = crate::ipc::tauri_invoke_args::<()>("open_data_preview_window", args).await;
        return;
    }

    // Browser: build HTML, create a Blob URL, then navigate the pre-opened tab to it.
    let html = build_data_window_html(&tables);

    let blob = match web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(&JsValue::from_str(
        &html,
    ))) {
        Ok(b) => b,
        Err(_) => return,
    };

    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(_) => return,
    };

    if let Some(win) = pre_win {
        // Navigate the already-open tab to the blob URL (no popup blocking possible)
        let _ = win.location().set_href(&url);
    } else {
        // Fallback if caller did not pre-open a window
        if let Some(w) = web_sys::window() {
            let _ = w.open_with_url_and_target(&url, "_blank");
        }
    }
}

// ── In-app DataGrid ───────────────────────────────────────────────────────────

const ROW_H: f64 = 28.0;
const OVERSCAN: usize = 12;

fn col_letter(i: usize) -> String {
    if i < 26 {
        ((b'A' + i as u8) as char).to_string()
    } else {
        format!(
            "{}{}",
            (b'A' + (i / 26 - 1) as u8) as char,
            (b'A' + (i % 26) as u8) as char
        )
    }
}

/// Shared spreadsheet grid — virtual scrolling, column filtering, column sorting.
///
/// Letter row (A, B, …): click to open value-filter dropdown.
/// Column name row: click to sort ascending → descending → none.
#[component]
pub fn DataGrid(
    cols: Vec<String>,
    rows: Vec<Vec<String>>,
    selected_cell: RwSignal<Option<(usize, usize)>>,
) -> impl IntoView {
    let num_cols = cols.len();

    // ── Filter state ──────────────────────────────────────────────────────────
    let filters: RwSignal<HashMap<usize, HashSet<String>>> = RwSignal::new(HashMap::new());
    let open_col: RwSignal<Option<usize>> = RwSignal::new(None);

    // ── Sort state ────────────────────────────────────────────────────────────
    let sort_col: RwSignal<Option<(usize, bool)>> = RwSignal::new(None);

    // ── Virtual-scroll state ──────────────────────────────────────────────────
    let scroll_top: RwSignal<f64> = RwSignal::new(0.0);
    let viewport_h: RwSignal<f64> = RwSignal::new(500.0);
    let container_ref: NodeRef<leptos::html::Div> = NodeRef::new();

    let rows_arc = Arc::new(rows);
    let rows_for_filter = rows_arc.clone();
    let rows_for_sort = rows_arc.clone();
    let rows_for_body = rows_arc.clone();
    let rows_for_dropdown = rows_arc.clone();

    // ── Filter: keep only rows that pass all active column filters ────────────
    let filtered: Memo<Vec<usize>> = Memo::new(move |_| {
        let active = filters.get();
        rows_for_filter
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                active.iter().all(|(ci, allowed)| {
                    row.get(*ci)
                        .map(|v| allowed.contains(v.as_str()))
                        .unwrap_or(true)
                })
            })
            .map(|(i, _)| i)
            .collect()
    });

    // ── Sort the filtered indices ─────────────────────────────────────────────
    let sorted_filtered: Memo<Vec<usize>> = Memo::new(move |_| {
        let mut indices = filtered.get();
        if let Some((ci, asc)) = sort_col.get() {
            let rows = rows_for_sort.clone();
            indices.sort_by(|&a, &b| {
                let va = rows
                    .get(a)
                    .and_then(|r| r.get(ci))
                    .map(|s| s.as_str())
                    .unwrap_or("");
                let vb = rows
                    .get(b)
                    .and_then(|r| r.get(ci))
                    .map(|s| s.as_str())
                    .unwrap_or("");
                let ord = match (va.parse::<f64>(), vb.parse::<f64>()) {
                    (Ok(na), Ok(nb)) => na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal),
                    _ => va.to_lowercase().cmp(&vb.to_lowercase()),
                };
                if asc {
                    ord
                } else {
                    ord.reverse()
                }
            });
        }
        indices
    });

    // ── Memoize visible window ────────────────────────────────────────────────
    let visible_window: Memo<(usize, usize, usize)> = Memo::new(move |_| {
        let st = scroll_top.get();
        let vh = viewport_h.get().max(200.0);
        let total = sorted_filtered.with(|f| f.len());
        let start = ((st / ROW_H) as usize).saturating_sub(OVERSCAN);
        let end = (((st + vh) / ROW_H).ceil() as usize + OVERSCAN).min(total);
        (start, end, total)
    });

    // ── Scroll handler ────────────────────────────────────────────────────────
    let on_scroll = move |ev: web_sys::Event| {
        if let Some(el) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
        {
            scroll_top.set(el.scroll_top() as f64);
            viewport_h.set(el.client_height() as f64);
        }
    };

    // ── Keyboard navigation ───────────────────────────────────────────────────
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if open_col.get_untracked().is_some() {
            return;
        }
        let Some((r, c)) = selected_cell.get_untracked() else {
            return;
        };
        let visible = sorted_filtered.get_untracked();
        let cur_pos = visible.iter().position(|&i| i == r);

        let next_row = match ev.key().as_str() {
            "ArrowUp" => cur_pos
                .and_then(|p| visible.get(p.saturating_sub(1)))
                .copied(),
            "ArrowDown" => cur_pos
                .and_then(|p| visible.get((p + 1).min(visible.len().saturating_sub(1))))
                .copied(),
            _ => None,
        };
        let next_col = match ev.key().as_str() {
            "ArrowLeft" => Some(c.saturating_sub(1)),
            "ArrowRight" => Some((c + 1).min(num_cols.saturating_sub(1))),
            _ => None,
        };

        if let Some(nr) = next_row {
            ev.prevent_default();
            selected_cell.set(Some((nr, c)));
            if let Some(el) = container_ref.get_untracked() {
                let new_pos = visible.iter().position(|&i| i == nr).unwrap_or(0);
                let row_top = new_pos as f64 * ROW_H;
                let cur_scroll = el.scroll_top() as f64;
                let height = el.client_height() as f64;
                let header_h = ROW_H * 2.0;
                if row_top < cur_scroll + header_h {
                    let _ = el.set_scroll_top((row_top - header_h) as i32);
                } else if row_top + ROW_H > cur_scroll + height {
                    let _ = el.set_scroll_top((row_top + ROW_H - height) as i32);
                }
            }
        } else if let Some(nc) = next_col {
            ev.prevent_default();
            selected_cell.set(Some((r, nc)));
            if let Some(el) = container_ref.get_untracked() {
                let selector = format!("thead tr:first-child th:nth-child({})", nc + 2);
                if let Ok(Some(th)) = el.query_selector(&selector) {
                    if let Ok(th_el) = th.dyn_into::<web_sys::HtmlElement>() {
                        let th_left = th_el.offset_left() as f64;
                        let th_right = th_left + th_el.offset_width() as f64;
                        let cur_sl = el.scroll_left() as f64;
                        let visible = el.client_width() as f64;
                        if th_left < cur_sl {
                            let _ = el.set_scroll_left(th_left as i32);
                        } else if th_right > cur_sl + visible {
                            let _ = el.set_scroll_left((th_right - visible) as i32);
                        }
                    }
                }
            }
        }
    };

    let close_dropdown = move || {
        open_col.set(None);
    };

    let letters = (0..cols.len()).map(col_letter).collect::<Vec<_>>();
    let cols_clone = cols.clone();
    let nc1 = num_cols;

    view! {
        // Backdrop closes the open filter dropdown
        {move || open_col.get().is_some().then(move || view! {
            <div
                style="position:fixed;inset:0;z-index:1"
                on:click=move |_| close_dropdown()
            />
        })}

        <div
            node_ref=container_ref
            class="vr-sheet-wrap"
            tabindex="0"
            on:keydown=on_keydown
            on:scroll=on_scroll
            on:click=move |_| close_dropdown()
        >
            <table class="vr-sheet-tbl">
                <thead>
                    // ── Letter row (click to open value-picker) ────────────────
                    <tr>
                        <th class="vr-th-letter corner"></th>
                        {letters.into_iter().enumerate().map(|(ci, letter)| {
                            let rd = rows_for_dropdown.clone();
                            view! {
                                <th
                                    class=move || {
                                        let has_filter = filters.get().contains_key(&ci);
                                        let is_open    = open_col.get() == Some(ci);
                                        match (has_filter, is_open) {
                                            (_, true)  => "vr-th-letter vr-th-letter--open",
                                            (true, _)  => "vr-th-letter vr-th-letter--active",
                                            _          => "vr-th-letter",
                                        }
                                    }
                                    on:click=move |ev| {
                                        ev.stop_propagation();
                                        if open_col.get_untracked() == Some(ci) {
                                            close_dropdown();
                                        } else {
                                            open_col.set(Some(ci));
                                        }
                                    }
                                    style=move || {
                                        if open_col.get() == Some(ci) {
                                            "cursor:pointer;z-index:3"
                                        } else {
                                            "cursor:pointer"
                                        }
                                    }
                                >
                                    <span class="vr-th-letter-text">
                                        {letter}
                                        {move || filters.get().contains_key(&ci).then(|| " ▼")}
                                    </span>

                                    {move || (open_col.get() == Some(ci)).then(|| {
                                        let mut all_vals: Vec<String> = rd
                                            .iter()
                                            .map(|row| row.get(ci).cloned().unwrap_or_default())
                                            .collect::<HashSet<_>>()
                                            .into_iter()
                                            .collect();
                                        all_vals.sort_unstable_by(|a, b| {
                                            a.to_lowercase().cmp(&b.to_lowercase())
                                        });
                                        let all_arc = Arc::new(all_vals);

                                        view! {
                                            <div
                                                class="vr-filter-dropdown"
                                                on:click=|ev| ev.stop_propagation()
                                            >
                                                <div class="vr-filter-actions">
                                                    <button
                                                        class=move || {
                                                            if !filters.get().contains_key(&ci) {
                                                                "vr-filter-action-btn vr-filter-action-btn--on"
                                                            } else {
                                                                "vr-filter-action-btn"
                                                            }
                                                        }
                                                        on:click=move |_| filters.update(|f| { f.remove(&ci); })
                                                    >"Select All"</button>
                                                    <button
                                                        class="vr-filter-action-btn"
                                                        on:click=move |_| filters.update(|f| { f.insert(ci, HashSet::new()); })
                                                    >"Clear"</button>
                                                </div>

                                                <div class="vr-filter-value-list">
                                                    {move || {
                                                        all_arc.iter()
                                                            .map(|val| {
                                                                let v_check  = val.clone();
                                                                let v_change = val.clone();
                                                                let ua2      = all_arc.clone();
                                                                let label    = if val.is_empty() { "(empty)".to_string() } else { val.clone() };
                                                                view! {
                                                                    <label class="vr-filter-value-item">
                                                                        <input
                                                                            type="checkbox"
                                                                            prop:checked=move || {
                                                                                filters.with(|f| {
                                                                                    f.get(&ci)
                                                                                        .map(|s| s.contains(v_check.as_str()))
                                                                                        .unwrap_or(true)
                                                                                })
                                                                            }
                                                                            on:change=move |ev| {
                                                                                let checked = ev
                                                                                    .target()
                                                                                    .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                                                                    .map(|el| el.checked())
                                                                                    .unwrap_or(false);
                                                                                let v   = v_change.clone();
                                                                                let all = ua2.clone();
                                                                                filters.update(|f| {
                                                                                    let set = f.entry(ci).or_insert_with(|| {
                                                                                        all.iter().cloned().collect::<HashSet<_>>()
                                                                                    });
                                                                                    if checked {
                                                                                        set.insert(v);
                                                                                        if all.iter().all(|vv| set.contains(vv)) {
                                                                                            f.remove(&ci);
                                                                                        }
                                                                                    } else {
                                                                                        set.remove(&v);
                                                                                    }
                                                                                });
                                                                            }
                                                                        />
                                                                        <span class="vr-filter-value-label">{label}</span>
                                                                    </label>
                                                                }
                                                            }).collect_view()
                                                    }}
                                                </div>
                                            </div>
                                        }
                                    })}
                                </th>
                            }
                        }).collect_view()}
                    </tr>

                    // ── Column name row (click to sort) ────────────────────────
                    <tr>
                        <th class="vr-th-name"></th>
                        {cols_clone.into_iter().enumerate().map(|(ci, c)| {
                            let c_label = c.clone();
                            view! {
                                <th
                                    class=move || {
                                        if filters.get().contains_key(&ci) { "vr-th-name vr-th-name--active" }
                                        else { "vr-th-name" }
                                    }
                                    style="cursor:pointer;user-select:none"
                                    title="Click to sort"
                                    on:click=move |_| {
                                        sort_col.update(|s| {
                                            *s = match *s {
                                                None => Some((ci, true)),
                                                Some((col, true))  if col == ci => Some((ci, false)),
                                                Some((col, false)) if col == ci => None,
                                                _ => Some((ci, true)),
                                            };
                                        });
                                    }
                                >
                                    {move || {
                                        let arrow = sort_col.get()
                                            .filter(|&(col, _)| col == ci)
                                            .map(|(_, asc)| if asc { " ↑" } else { " ↓" })
                                            .unwrap_or("");
                                        format!("{c_label}{arrow}")
                                    }}
                                </th>
                            }
                        }).collect_view()}
                    </tr>
                </thead>

                <tbody>
                    // Top spacer
                    {move || {
                        let (start, _, _) = visible_window.get();
                        let top_px = (start as f64 * ROW_H) as u32;
                        (top_px > 0).then(|| view! {
                            <tr style=format!("height:{top_px}px")>
                                <td colspan={nc1 + 1}></td>
                            </tr>
                        })
                    }}

                    <For
                        each=move || {
                            let (start, end, _) = visible_window.get();
                            sorted_filtered.with(|indices| indices[start..end].to_vec())
                        }
                        key=|&orig_ri| orig_ri
                        children=move |orig_ri| {
                            let display_rn = move || {
                                sorted_filtered.with(|f| {
                                    f.iter().position(|&i| i == orig_ri).map(|p| p + 1).unwrap_or(0)
                                })
                            };
                            let row_data = rows_for_body[orig_ri].clone();
                            let cells = row_data.iter().enumerate().map(|(ci, val)| {
                                let v = val.clone();
                                view! {
                                    <td
                                        class=move || {
                                            let sel = selected_cell.get();
                                            if sel == Some((orig_ri, ci))                         { "vr-td selected" }
                                            else if sel.map(|(r,_)| r==orig_ri).unwrap_or(false) { "vr-td selected-row" }
                                            else if sel.map(|(_,c)| c==ci).unwrap_or(false)      { "vr-td selected-col" }
                                            else                                                   { "vr-td" }
                                        }
                                        on:click=move |_| selected_cell.set(Some((orig_ri, ci)))
                                    >{v}</td>
                                }
                            }).collect_view();
                            view! {
                                <tr>
                                    <td class="vr-td-rn">{display_rn}</td>
                                    {cells}
                                </tr>
                            }
                        }
                    />

                    // Bottom spacer
                    {move || {
                        let (_, end, total) = visible_window.get();
                        let bottom_px = (total.saturating_sub(end) as f64 * ROW_H) as u32;
                        (bottom_px > 0).then(|| view! {
                            <tr style=format!("height:{bottom_px}px")>
                                <td colspan={nc1 + 1}></td>
                            </tr>
                        })
                    }}
                </tbody>
            </table>
        </div>
    }
}
