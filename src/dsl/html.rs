//! HTML rendering for DSL execution results.
//!
//! This module provides a single source of truth for converting DSL result JSON
//! into HTML strings.  It is used by:
//!
//! * `vinrouge-worker` – dashboard fragments embedded in Jinja templates
//! * `vinrouge-web`    – pop-out result windows and IDE CSS collection
//! * `vinrouge-desktop` – preview windows opened from the Tauri app

use serde_json::Value;

// ── Public API ───────────────────────────────────────────────────────────────

/// Default stylesheet for DSL result HTML output.
///
/// All rules use CSS classes (no inline `style` attributes) so that custom
/// CSS emitted by `CSS "…"` statements in a DSL script can override them
/// naturally via the cascade.
pub fn default_css() -> &'static str {
    r#"
.ide-results{display:flex;flex-direction:column;gap:0;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;}
.ide-result{display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;margin-bottom:6px;border:0.5px solid transparent;}
.ide-result-icon{flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;}
.ide-result-body{min-width:0;flex:1;display:flex;flex-direction:column;gap:2px;}
.ide-result-label{font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:2px;}
.ide-result-expr{font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#d4c4c8;}
.ide-result-val{font-family:inherit;}
.ide-result-op{color:#8a6e74;font-size:11px;margin:0 4px;}

.ide-result--pass{background:rgba(74,222,128,0.06);border-color:rgba(74,222,128,0.15);}
.ide-result--pass .ide-result-icon{background:rgba(74,222,128,0.12);color:#4ade80;}
.ide-result--pass .ide-result-val{color:#86efac;}

.ide-result--fail{background:rgba(248,113,113,0.06);border-color:rgba(248,113,113,0.15);}
.ide-result--fail .ide-result-icon{background:rgba(248,113,113,0.12);color:#f87171;}
.ide-result--fail .ide-result-val{color:#fca5a5;}

.ide-result--sample{background:rgba(96,165,250,0.06);border-color:rgba(96,165,250,0.15);}
.ide-result--sample .ide-result-icon{background:rgba(96,165,250,0.12);color:#60a5fa;}

.ide-result--error{background:rgba(251,146,60,0.06);border-color:rgba(251,146,60,0.15);}
.ide-result--error .ide-result-icon{background:rgba(251,146,60,0.12);color:#fb923c;}
.ide-result--error .ide-result-msg{font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#fdba74;word-break:break-word;}

.ide-result--value{background:rgba(192,132,252,0.06);border-color:rgba(192,132,252,0.15);}
.ide-result--value .ide-result-icon{background:rgba(192,132,252,0.12);color:#c084fc;}

.ide-result--relation{background:rgba(163,163,163,0.05);border-color:rgba(163,163,163,0.12);}
.ide-result--relation .ide-result-icon{background:rgba(163,163,163,0.10);color:#a3a3a3;}
.ide-result--relation .ide-result-val{color:#aaa;}
.ide-result--relation .ide-result-op{color:#666;margin:0 4px;}

.ide-result--chart{padding:8px 10px;border-radius:6px;background:rgba(19,14,16,0.6);border:0.5px solid #2a2024;margin-bottom:6px;}
.ide-result--chart .ide-result-label{margin-bottom:6px;}
.ide-chart{width:100%;height:220px;}

.ide-result--show-rows{padding:8px 10px;border-radius:6px;background:rgba(20,184,166,0.05);border:0.5px solid rgba(20,184,166,0.15);margin-bottom:6px;}
.ide-result--show-rows .ide-result-icon{background:rgba(20,184,166,0.12);color:#14b8a6;}

.ide-section{border:0.5px solid #2a2024;border-radius:6px;background:rgba(19,14,16,0.6);overflow:hidden;margin-bottom:8px;}
.ide-section-header{display:flex;align-items:center;gap:10px;padding:8px 12px;background:rgba(255,255,255,0.03);cursor:pointer;user-select:none;}
.ide-section-title{font-weight:600;font-size:13px;color:#d4c4c8;}
.ide-section-badges{display:flex;align-items:center;gap:6px;margin-left:auto;}
.ide-section-chevron{color:#8a6e74;display:flex;align-items:center;transition:transform 0.15s;}
.ide-section-body{padding:8px 12px 12px;display:flex;flex-direction:column;gap:6px;}

.ide-sum{font-size:10px;padding:1px 6px;border-radius:99px;display:inline-flex;align-items:center;gap:4px;}
.ide-sum--pass{background:rgba(74,222,128,0.12);color:#4ade80;}
.ide-sum--fail{background:rgba(248,113,113,0.12);color:#f87171;}
.ide-sum--error{background:rgba(251,146,60,0.12);color:#fb923c;}
.ide-sum--chart{background:rgba(192,132,252,0.12);color:#c084fc;}
.ide-sum--section{background:rgba(192,132,252,0.12);color:#c084fc;}

.ide-screen-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:10px;}
.ide-screen-chart{padding:6px;border-radius:4px;background:rgba(255,255,255,0.02);border:0.5px solid #2a2024;}
.ide-screen-chart-label{font-size:10px;color:#c9a8ae;margin-bottom:4px;}
.ide-screen-chart .ide-chart{height:180px;}

.ide-sample-table-wrap{overflow-x:auto;margin-bottom:6px;}
.ide-sample-table{width:100%;border-collapse:collapse;font-size:11px;font-family:ui-monospace,monospace;color:#d4c4c8;}
.ide-sample-table th{padding:4px 8px;border-bottom:1px solid rgba(255,255,255,0.08);text-align:left;color:#8a6e74;}
.ide-sample-table td{padding:3px 8px;border-bottom:1px solid rgba(255,255,255,0.04);}

.topbar-summaries{display:flex;align-items:center;gap:6px;flex-wrap:wrap;margin-left:auto;}
.topbar-summary{font-size:10px;padding:2px 8px;border-radius:99px;display:inline-flex;align-items:center;gap:4px;}
.topbar-summary--pass{background:rgba(74,222,128,0.12);color:#4ade80;}
.topbar-summary--fail{background:rgba(248,113,113,0.12);color:#f87171;}
.topbar-summary--error{background:rgba(251,146,60,0.12);color:#fb923c;}
.topbar-summary--sample{background:rgba(96,165,250,0.12);color:#60a5fa;}
.topbar-summary--chart{background:rgba(192,132,252,0.12);color:#c084fc;}
.topbar-summary--screen{background:rgba(192,132,252,0.12);color:#c084fc;}
.topbar-summary--section{background:rgba(192,132,252,0.12);color:#c084fc;}
.topbar-summary--show_rows{background:rgba(20,184,166,0.12);color:#14b8a6;}
"#
}

/// Recursively collect CSS styles from results, drilling into sections.
pub fn collect_css(results: &[Value]) -> Vec<String> {
    let mut css = Vec::new();
    for r in results {
        match r["kind"].as_str() {
            Some("css") => {
                if let Some(s) = r["styles"].as_str() {
                    css.push(s.to_string());
                }
            }
            Some("section") => {
                let inner = r["results"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                css.extend(collect_css(inner));
            }
            _ => {}
        }
    }
    css
}

/// Recursively count result kinds, drilling into sections.
pub fn count_results(
    results: &[Value],
) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
    let mut passed = 0;
    let mut failed = 0;
    let mut errors = 0;
    let mut samples = 0;
    let mut charts = 0;
    let mut screens = 0;
    let mut sections = 0;
    let mut show_rows = 0;
    for r in results {
        match r["kind"].as_str() {
            Some("assert") => {
                if r["passed"].as_bool() == Some(true) {
                    passed += 1;
                } else {
                    failed += 1;
                }
            }
            Some("error") => errors += 1,
            Some("sample") => samples += 1,
            Some("chart") => charts += 1,
            Some("screen") => screens += 1,
            Some("show_rows") => show_rows += 1,
            Some("section") => {
                sections += 1;
                let inner = r["results"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                let (p, f, e, sa, c, sc, se, sr) = count_results(inner);
                passed += p;
                failed += f;
                errors += e;
                samples += sa;
                charts += c;
                screens += sc;
                sections += se;
                show_rows += sr;
            }
            _ => {}
        }
    }
    (passed, failed, errors, samples, charts, screens, sections, show_rows)
}

/// Build an ECharts option JSON string from chart data.
///
/// Supports `"bar"`, `"line"`, `"scatter"` and `"pie"`.
pub fn build_chart_option(chart_type: &str, labels: &[String], values: &[String]) -> String {
    let cats: Vec<String> = labels
        .iter()
        .map(|l| format!("\"{}\"", l.replace('"', "\\\"")))
        .collect();
    let vals: Vec<String> = values.iter().map(|v| v.to_string()).collect();

    match chart_type {
        "pie" => {
            let items: Vec<String> = labels
                .iter()
                .zip(values.iter())
                .map(|(l, v)| {
                    format!(
                        "{{\"name\":\"{}\",\"value\":{}}}",
                        l.replace('"', "\\\""),
                        v
                    )
                })
                .collect();
            format!(
                concat!(
                    r##"{{"backgroundColor":"transparent","tooltip":{{"trigger":"item"}},"##,
                    r##""legend":{{"orient":"vertical","left":"left","textStyle":{{"color":"#c9a8ae","fontSize":11}}}},"##,
                    r##""series":[{{"type":"pie","radius":"60%","data":[{}],"##,
                    r##""label":{{"color":"#f0e6e8","fontSize":11}}}}]}}"##
                ),
                items.join(",")
            )
        }
        "line" => {
            format!(
                concat!(
                    r##"{{"backgroundColor":"transparent","grid":{{"left":60,"right":20,"top":20,"bottom":60}},"##,
                    r##""tooltip":{{"trigger":"axis"}},"##,
                    r##""xAxis":{{"type":"category","data":[{}],"axisLabel":{{"rotate":30,"fontSize":10,"color":"#c9a8ae"}}}},"##,
                    r##""yAxis":{{"type":"value","axisLabel":{{"color":"#c9a8ae"}}}},"##,
                    r##""series":[{{"type":"line","data":[{}],"smooth":true,"itemStyle":{{"color":"#8b1a2a"}}}}]}}"##
                ),
                cats.join(","),
                vals.join(",")
            )
        }
        "scatter" => {
            format!(
                concat!(
                    r##"{{"backgroundColor":"transparent","grid":{{"left":60,"right":20,"top":20,"bottom":60}},"##,
                    r##""tooltip":{{"trigger":"item"}},"##,
                    r##""xAxis":{{"type":"category","data":[{}],"axisLabel":{{"rotate":30,"fontSize":10,"color":"#c9a8ae"}}}},"##,
                    r##""yAxis":{{"type":"value","axisLabel":{{"color":"#c9a8ae"}}}},"##,
                    r##""series":[{{"type":"scatter","data":[{}],"itemStyle":{{"color":"#8b1a2a"}}}}]}}"##
                ),
                cats.join(","),
                vals.join(",")
            )
        }
        _ => {
            // default to bar
            format!(
                concat!(
                    r##"{{"backgroundColor":"transparent","grid":{{"left":60,"right":20,"top":20,"bottom":60}},"##,
                    r##""tooltip":{{"trigger":"axis"}},"##,
                    r##""xAxis":{{"type":"category","data":[{}],"axisLabel":{{"rotate":30,"fontSize":10,"color":"#c9a8ae"}}}},"##,
                    r##""yAxis":{{"type":"value","axisLabel":{{"color":"#c9a8ae"}}}},"##,
                    r##""series":[{{"type":"bar","data":[{}],"itemStyle":{{"color":"#8b1a2a"}}}}]}}"##
                ),
                cats.join(","),
                vals.join(",")
            )
        }
    }
}

/// Render DSL results as an HTML fragment (style tag + content).
///
/// Suitable for embedding into an existing page (e.g. the worker dashboard
/// Jinja templates).  Default CSS is injected first; custom CSS from `CSS`
/// statements is appended after so it takes precedence.
pub fn render_html_fragment(results: &[Value]) -> String {
    let css_blocks = collect_css(results);
    let has_custom = !css_blocks.is_empty();

    let style_tag = if has_custom {
        format!(
            "<style>{}</style><style>{}</style>",
            default_css(),
            css_blocks.join("\n")
        )
    } else {
        format!("<style>{}</style>", default_css())
    };

    let mut chart_idx = 0usize;
    let body: String = results.iter().map(|r| render_one(r, &mut chart_idx)).collect();
    format!("{}<div class=\"ide-results\">{}</div>", style_tag, body)
}

/// Render DSL results as a complete self-contained HTML page.
///
/// Includes ECharts CDN, default + custom styles, a summary top-bar, and
/// inline JavaScript to initialise charts and make tables sortable.
pub fn render_html_page(results: &[Value]) -> String {
    let css_blocks = collect_css(results);
    let custom_style = if css_blocks.is_empty() {
        String::new()
    } else {
        format!("<style>{}</style>", css_blocks.join("\n"))
    };

    let (passed, failed, errors, samples, charts, screens, sections, show_rows) =
        count_results(results);
    let mut summary = Vec::new();
    if passed > 0 {
        summary.push(format!(
            "<span class=\"topbar-summary topbar-summary--pass\"><svg width=\"10\" height=\"10\" viewBox=\"0 0 12 12\" fill=\"none\"><path d=\"M2 6l3 3 5-5\" stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>{} passed</span>",
            passed
        ));
    }
    if failed > 0 {
        summary.push(format!(
            "<span class=\"topbar-summary topbar-summary--fail\"><svg width=\"10\" height=\"10\" viewBox=\"0 0 12 12\" fill=\"none\"><path d=\"M2 2l8 8M10 2l-8 8\" stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linecap=\"round\"/></svg>{} failed</span>",
            failed
        ));
    }
    if errors > 0 {
        summary.push(format!(
            "<span class=\"topbar-summary topbar-summary--error\">{} error{}</span>",
            errors,
            if errors == 1 { "" } else { "s" }
        ));
    }
    if samples > 0 {
        summary.push(format!(
            "<span class=\"topbar-summary topbar-summary--sample\">{} sample{}</span>",
            samples,
            if samples == 1 { "" } else { "s" }
        ));
    }
    if charts > 0 {
        summary.push(format!(
            "<span class=\"topbar-summary topbar-summary--chart\">{} chart{}</span>",
            charts,
            if charts == 1 { "" } else { "s" }
        ));
    }
    if screens > 0 {
        summary.push(format!(
            "<span class=\"topbar-summary topbar-summary--screen\">{} screen{}</span>",
            screens,
            if screens == 1 { "" } else { "s" }
        ));
    }
    if sections > 0 {
        summary.push(format!(
            "<span class=\"topbar-summary topbar-summary--section\">{} section{}</span>",
            sections,
            if sections == 1 { "" } else { "s" }
        ));
    }
    if show_rows > 0 {
        summary.push(format!(
            "<span class=\"topbar-summary topbar-summary--show_rows\">{} table{}</span>",
            show_rows,
            if show_rows == 1 { "" } else { "s" }
        ));
    }
    let summary_html = summary.join("");

    let mut chart_idx = 0usize;
    let body: String = results.iter().map(|r| render_one(r, &mut chart_idx)).collect();

    format!(
        r##"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>DSL Results</title>
<script src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js"></script>
<style>
*{{box-sizing:border-box}}
body{{margin:0;padding:0;background:#141414;color:#d4c4c8;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;min-height:100vh;}}
.wrap{{max-width:900px;margin:0 auto;padding:20px 16px 40px;}}
.topbar{{display:flex;align-items:center;gap:8px;flex-wrap:wrap;margin-bottom:16px;padding-bottom:12px;border-bottom:0.5px solid #2a2024;}}
.title{{font-size:16px;font-weight:600;color:#d4c4c8;margin:0;}}
{default_css}
</style>{custom_style}</head><body>
<div class="wrap">
    <div class="topbar">
        <h1 class="title">DSL Results</h1>
        <div class="topbar-summaries">{summary_html}</div>
    </div>
    <div class="ide-results">{body}</div>
</div>
<script>
document.addEventListener('DOMContentLoaded', function(){{
    var charts = document.querySelectorAll('[data-option]');
    for (var i = 0; i < charts.length; i++) {{
        var el = charts[i];
        var optStr = el.getAttribute('data-option');
        if (!optStr) continue;
        try {{
            var opt = JSON.parse(optStr);
            var chart = echarts.init(el, 'dark', {{ renderer: 'canvas' }});
            chart.setOption(opt);
        }} catch (e) {{
            console.error('Chart init failed', e);
            el.textContent = 'Chart error: ' + e.message;
        }}
    }}

    // ── Vanilla sortable tables ──────────────────────────────────────────
    function makeSortable(table) {{
        var thead = table.querySelector('thead');
        if (!thead) return;
        var ths = thead.querySelectorAll('th');
        ths.forEach(function(th, colIndex) {{
            th.style.cursor = 'pointer';
            th.style.userSelect = 'none';
            th.addEventListener('click', function() {{
                var tbody = table.querySelector('tbody');
                if (!tbody) return;
                var rows = Array.prototype.slice.call(tbody.querySelectorAll('tr'));
                var currentDir = th.getAttribute('data-sort-dir') || 'asc';
                var dir = currentDir === 'asc' ? 'desc' : 'asc';
                ths.forEach(function(h) {{
                    h.removeAttribute('data-sort-dir');
                    var ind = h.querySelector('.sort-ind');
                    if (ind) ind.textContent = '';
                }});
                th.setAttribute('data-sort-dir', dir);
                var indicator = th.querySelector('.sort-ind');
                if (!indicator) {{
                    indicator = document.createElement('span');
                    indicator.className = 'sort-ind';
                    indicator.style.marginLeft = '4px';
                    indicator.style.fontSize = '10px';
                    indicator.style.color = '#c9a8ae';
                    th.appendChild(indicator);
                }}
                indicator.textContent = dir === 'asc' ? '▲' : '▼';
                rows.sort(function(a, b) {{
                    var aCell = a.children[colIndex];
                    var bCell = b.children[colIndex];
                    var aText = aCell ? aCell.textContent.trim() : '';
                    var bText = bCell ? bCell.textContent.trim() : '';
                    var aNum = parseFloat(aText.replace(/,/g, ''));
                    var bNum = parseFloat(bText.replace(/,/g, ''));
                    var bothNumeric = !isNaN(aNum) && !isNaN(bNum) && aText !== '' && bText !== '';
                    if (bothNumeric) {{
                        return dir === 'asc' ? aNum - bNum : bNum - aNum;
                    }}
                    return dir === 'asc' ? aText.localeCompare(bText) : bText.localeCompare(aText);
                }});
                rows.forEach(function(row) {{ tbody.appendChild(row); }});
            }});
        }});
    }}
    document.querySelectorAll('table').forEach(makeSortable);
}});
</script>
</body></html>"##,
        default_css = default_css(),
        custom_style = custom_style,
        summary_html = summary_html,
        body = body,
    )
}

// ── Private helpers ──────────────────────────────────────────────────────────

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_one(r: &Value, chart_idx: &mut usize) -> String {
    match r["kind"].as_str().unwrap_or("") {
        "assert" => render_assert(r),
        "sample" => render_sample(r),
        "error" => render_error(r),
        "value" => render_value(r),
        "relation" => render_relation(r),
        "chart" => render_chart(r, chart_idx),
        "screen" => render_screen(r, chart_idx),
        "section" => render_section(r, chart_idx),
        "show_rows" => render_show_rows(r),
        "css" => String::new(),
        // schema is intentionally not shown in HTML output
        _ => String::new(),
    }
}

fn render_assert(r: &Value) -> String {
    let ok = r["passed"].as_bool().unwrap_or(false);
    let lbl = r["label"].as_str().unwrap_or("");
    let lhs = r["lhs_value"].as_str().unwrap_or("?");
    let rhs = r["rhs_value"].as_str().unwrap_or("?");
    let op = r["op"].as_str().unwrap_or("=");

    let icon = if ok {
        "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><path d='M2 6.5l3.5 3.5 5.5-6' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'/></svg>"
    } else {
        "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><path d='M2.5 2.5l8 8M10.5 2.5l-8 8' stroke='currentColor' stroke-width='1.6' stroke-linecap='round'/></svg>"
    };
    let status = if ok { "pass" } else { "fail" };

    let label_html = if !lbl.is_empty() {
        format!("<div class=\"ide-result-label\">{}</div>", esc(lbl))
    } else {
        String::new()
    };

    let mut h = String::new();
    h.push_str(&format!(
        "<div class=\"ide-result ide-result--assert ide-result--{}\">\
          <span class=\"ide-result-icon\">{}</span>\
          <div class=\"ide-result-body\">{}\
            <div class=\"ide-result-expr\">\
              <span class=\"ide-result-val\">{}</span>\
              <span class=\"ide-result-op\">{}</span>\
              <span class=\"ide-result-val\">{}</span>\
            </div>\
          </div>\
        </div>",
        status, icon, label_html, esc(lhs), esc(op), esc(rhs),
    ));

    // Failures table
    if let Some(rows) = r["failed_rows"].as_array() {
        if !rows.is_empty() {
            if let Some(first) = rows.first().and_then(|v| v.as_object()) {
                let mut cols: Vec<String> = first.keys().cloned().collect();
                cols.sort();
                h.push_str("<div class=\"ide-sample-table-wrap\">");
                h.push_str("<table class=\"ide-sample-table\"><thead><tr>");
                for col in &cols {
                    h.push_str(&format!("<th>{}</th>", esc(col)));
                }
                h.push_str("</tr></thead><tbody>");
                for row in rows {
                    if let Some(obj) = row.as_object() {
                        h.push_str("<tr>");
                        for col in &cols {
                            let cell = obj.get(col).and_then(|v| v.as_str()).unwrap_or("");
                            h.push_str(&format!("<td>{}</td>", esc(cell)));
                        }
                        h.push_str("</tr>");
                    }
                }
                h.push_str("</tbody></table></div>");
            }
        }
    }

    h
}

fn render_sample(r: &Value) -> String {
    let method = r["method"].as_str().unwrap_or("SAMPLE");
    let pop = r["population_size"].as_u64().unwrap_or(0);
    let sel = r["selected_count"].as_u64().unwrap_or(0);
    let pct = if pop > 0 {
        sel as f64 / pop as f64 * 100.0
    } else {
        0.0
    };
    let icon = "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><rect x='1' y='1' width='11' height='11' rx='1.5' stroke='currentColor' stroke-width='1.1'/><path d='M1 4.5h11M4.5 4.5v7' stroke='currentColor' stroke-width='1.1'/></svg>";
    format!(
        "<div class=\"ide-result ide-result--sample\">\
          <span class=\"ide-result-icon\">{}</span>\
          <div class=\"ide-result-body\">\
            <div class=\"ide-result-label\">{}</div>\
            <div class=\"ide-result-expr\">{} of {} rows ({:.1}%)</div>\
          </div>\
        </div>",
        icon, esc(method), sel, pop, pct
    )
}

fn render_error(r: &Value) -> String {
    let msg = r["error"].as_str().unwrap_or("unknown error");
    let icon = "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><path d='M6.5 1.5L11.5 10.5H1.5L6.5 1.5Z' stroke='currentColor' stroke-width='1.2' stroke-linejoin='round'/><path d='M6.5 5v3M6.5 9.5v.5' stroke='currentColor' stroke-width='1.3' stroke-linecap='round'/></svg>";
    format!(
        "<div class=\"ide-result ide-result--error\">\
          <span class=\"ide-result-icon\">{}</span>\
          <div class=\"ide-result-body\">\
            <span class=\"ide-result-msg\">{}</span>\
          </div>\
        </div>",
        icon, esc(msg)
    )
}

fn render_value(r: &Value) -> String {
    let val = r["value"].as_str().unwrap_or("");
    let icon = "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><path d='M2 6.5h9M7.5 3l3.5 3.5L7.5 10' stroke='currentColor' stroke-width='1.3' stroke-linecap='round' stroke-linejoin='round'/></svg>";
    format!(
        "<div class=\"ide-result ide-result--value\">\
          <span class=\"ide-result-icon\">{}</span>\
          <div class=\"ide-result-body\">\
            <span class=\"ide-result-expr\">{}</span>\
          </div>\
        </div>",
        icon, esc(val)
    )
}

fn render_relation(r: &Value) -> String {
    let from = r["from"].as_str().unwrap_or("");
    let to = r["to"].as_str().unwrap_or("");
    let icon = "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><circle cx='3' cy='6.5' r='1.5' stroke='currentColor' stroke-width='1.1'/><circle cx='10' cy='6.5' r='1.5' stroke='currentColor' stroke-width='1.1'/><path d='M4.5 6.5h3' stroke='currentColor' stroke-width='1.1'/></svg>";
    format!(
        "<div class=\"ide-result ide-result--relation\">\
          <span class=\"ide-result-icon\">{}</span>\
          <div class=\"ide-result-body\">\
            <span class=\"ide-result-expr\">\
              <span class=\"ide-result-val\">{}</span>\
              <span class=\"ide-result-op\">&#8594;</span>\
              <span class=\"ide-result-val\">{}</span>\
            </span>\
          </div>\
        </div>",
        icon, esc(from), esc(to)
    )
}

fn render_chart(r: &Value, chart_idx: &mut usize) -> String {
    let lbl = r["label"].as_str().unwrap_or("");
    let ctype = r["chart_type"].as_str().unwrap_or("bar");
    let labels: Vec<String> = r["labels"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let values: Vec<String> = r["values"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let option = build_chart_option(ctype, &labels, &values);
    let id = format!("vr-chart-{}", *chart_idx);
    *chart_idx += 1;

    let label_html = if !lbl.is_empty() {
        format!("<div class=\"ide-result-label\">{}</div>", esc(lbl))
    } else {
        String::new()
    };
    format!(
        "<div class=\"ide-result ide-result--chart\">\
          {label_html}\
          <div id=\"{id}\" class=\"ide-chart\" data-option=\"{option_esc}\"></div>\
        </div>",
        option_esc = esc(&option),
    )
}

fn render_screen(r: &Value, chart_idx: &mut usize) -> String {
    let title = r["title"].as_str().unwrap_or("Screen");
    let charts: Vec<&Value> = r["charts"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let mut h = String::new();
    h.push_str(&format!(
        "<div class=\"ide-result ide-result--chart\">\
         <div class=\"ide-result-label\">{}</div>\
         <div class=\"ide-screen-grid\">",
        esc(title)
    ));
    for c in charts {
        let clbl = c["label"].as_str().unwrap_or("");
        let ctype = c["chart_type"].as_str().unwrap_or("bar");
        let labels: Vec<String> = c["labels"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let values: Vec<String> = c["values"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let option = build_chart_option(
            ctype,
            &labels,
            &values,
        );
        let id = format!("vr-chart-{}", *chart_idx);
        *chart_idx += 1;
        h.push_str("<div class=\"ide-screen-chart\">");
        if !clbl.is_empty() {
            h.push_str(&format!(
                "<div class=\"ide-screen-chart-label\">{}</div>",
                esc(clbl)
            ));
        }
        h.push_str(&format!(
            "<div id=\"{}\" class=\"ide-chart\" data-option=\"{}\"></div>",
            id,
            esc(&option)
        ));
        h.push_str("</div>");
    }
    h.push_str("</div></div>");
    h
}

fn render_section(r: &Value, chart_idx: &mut usize) -> String {
    let title = r["title"].as_str().unwrap_or("Section");
    let passed = r["passed"].as_u64().unwrap_or(0);
    let failed = r["failed"].as_u64().unwrap_or(0);
    let errors = r["errors"].as_u64().unwrap_or(0);
    let inner: Vec<&Value> = r["results"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let inner_html: String = inner.iter().map(|c| render_one(c, chart_idx)).collect();

    let mut badges = Vec::new();
    if passed > 0 {
        badges.push(format!(
            "<span class=\"ide-sum ide-sum--pass\">{} passed</span>",
            passed
        ));
    }
    if failed > 0 {
        badges.push(format!(
            "<span class=\"ide-sum ide-sum--fail\">{} failed</span>",
            failed
        ));
    }
    if errors > 0 {
        badges.push(format!(
            "<span class=\"ide-sum ide-sum--error\">{} error{}</span>",
            errors,
            if errors == 1 { "" } else { "s" }
        ));
    }
    let badges_html = badges.join("");

    // Stable collapse ID derived from title text
    let inner_id = format!(
        "sec-{:x}",
        title
            .as_bytes()
            .iter()
            .fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(*b as u64))
    );
    let chev_svg = "<svg width='12' height='12' viewBox='0 0 12 12' fill='none'><path d='M2 4.5l4 4 4-4' stroke='currentColor' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/></svg>";

    format!(
        "<div class=\"ide-section\">\
          <div class=\"ide-section-header\" onclick=\"var b=document.getElementById('{inner_id}');b.style.display=b.style.display==='none'?'flex':'none';var c=this.querySelector('.ide-section-chevron');c.style.transform=b.style.display==='none'?'rotate(-90deg)':'rotate(0deg)';\">\
            <span class=\"ide-section-title\">{title_esc}</span>\
            <span class=\"ide-section-badges\">{badges_html}</span>\
            <span class=\"ide-section-chevron\">{chev_svg}</span>\
          </div>\
          <div class=\"ide-section-body\" id=\"{inner_id}\">{inner_html}</div>\
        </div>",
        title_esc = esc(title),
    )
}

fn render_show_rows(r: &Value) -> String {
    let lbl = r["label"].as_str().unwrap_or("");
    let table = r["table"].as_str().unwrap_or("");
    let total = r["total"].as_u64().unwrap_or(0);
    let rows = r["rows"].as_array();
    let mut h = String::new();
    h.push_str("<div class=\"ide-result ide-result--show-rows\">");
    h.push_str("<div style=\"display:flex;align-items:center;gap:8px;margin-bottom:6px;\">");
    h.push_str("<span class=\"ide-result-icon\">");
    h.push_str("<svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><rect x=\"1\" y=\"1\" width=\"11\" height=\"11\" rx=\"1.5\" stroke=\"currentColor\" stroke-width=\"1.1\"/><path d=\"M1 4.5h11M1 7.5h11\" stroke=\"currentColor\" stroke-width=\"1.1\"/></svg>");
    h.push_str("</span>");
    if !lbl.is_empty() {
        h.push_str(&format!(
            "<span class=\"ide-result-label\">{}</span>",
            esc(lbl)
        ));
    }
    h.push_str(&format!(
        "<span class=\"ide-result-expr\">{} — {} row{}</span>",
        esc(table),
        total,
        if total == 1 { "" } else { "s" }
    ));
    h.push_str("</div>");
    if let Some(rows) = rows {
        if !rows.is_empty() {
            if let Some(first) = rows.first().and_then(|v| v.as_object()) {
                let mut cols: Vec<String> = first.keys().cloned().collect();
                cols.sort();
                h.push_str("<div class=\"ide-sample-table-wrap\">");
                h.push_str("<table class=\"ide-sample-table\"><thead><tr>");
                for col in &cols {
                    h.push_str(&format!("<th>{}</th>", esc(col)));
                }
                h.push_str("</tr></thead><tbody>");
                for row in rows {
                    if let Some(obj) = row.as_object() {
                        h.push_str("<tr>");
                        for col in &cols {
                            let cell = obj.get(col).and_then(|v| v.as_str()).unwrap_or("");
                            h.push_str(&format!("<td>{}</td>", esc(cell)));
                        }
                        h.push_str("</tr>");
                    }
                }
                h.push_str("</tbody></table></div>");
            }
        }
    }
    h.push_str("</div>");
    h
}
