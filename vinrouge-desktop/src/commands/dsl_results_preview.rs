use tauri::{AppHandle, Emitter, WebviewWindowBuilder};

/// Open a new window to display DSL results with ECharts-rendered charts.
#[tauri::command]
pub async fn open_dsl_results_window(
    app: AppHandle,
    results: Vec<serde_json::Value>,
) -> Result<(), String> {
    let mut html = String::new();
    html.push_str(r##"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>DSL Results</title>
<script src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js"></script>
<style>
*{box-sizing:border-box}
body{margin:0;padding:0;background:#141414;color:#d4c4c8;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;min-height:100vh;}
.wrap{max-width:900px;margin:0 auto;padding:20px 16px 40px;}
.topbar{display:flex;align-items:center;gap:8px;flex-wrap:wrap;margin-bottom:16px;padding-bottom:12px;border-bottom:0.5px solid #2a2024;}
.title{font-size:16px;font-weight:600;color:#d4c4c8;margin:0;}
"##);
    html.push_str("</style></head><body><div class=\"wrap\">");

    // Summary bar
    let (passed, failed, errors, samples, charts, screens, sections) = count_results(&results);
    let mut summary = Vec::new();
    if passed > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(74,222,128,0.12);color:#4ade80;\">{} passed</span>", passed));
    }
    if failed > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(248,113,113,0.12);color:#f87171;\">{} failed</span>", failed));
    }
    if errors > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(251,146,60,0.12);color:#fb923c;\">{} error{}</span>", errors, if errors == 1 { "" } else { "s" }));
    }
    if samples > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(96,165,250,0.12);color:#60a5fa;\">{} sample{}</span>", samples, if samples == 1 { "" } else { "s" }));
    }
    if charts > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(192,132,252,0.12);color:#c084fc;\">{} chart{}</span>", charts, if charts == 1 { "" } else { "s" }));
    }
    if screens > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(192,132,252,0.12);color:#c084fc;\">{} screen{}</span>", screens, if screens == 1 { "" } else { "s" }));
    }
    if sections > 0 {
        summary.push(format!("<span style=\"font-size:10px;padding:2px 8px;border-radius:99px;background:rgba(192,132,252,0.12);color:#c084fc;\">{} section{}</span>", sections, if sections == 1 { "" } else { "s" }));
    }

    html.push_str("<div class=\"topbar\"><h1 class=\"title\">DSL Results</h1>");
    html.push_str(&format!(
        "<div style=\"display:flex;align-items:center;gap:6px;flex-wrap:wrap;margin-left:auto;\">{}</div>",
        summary.join("")
    ));
    html.push_str("</div>");

    // Body
    let mut chart_idx = 0;
    let body = results_to_html(&results, &mut chart_idx);
    html.push_str(&body);

    // Chart init script
    html.push_str(
        r##"
</div>
<script>
document.addEventListener('DOMContentLoaded', function(){
    var charts = document.querySelectorAll('[data-option]');
    for (var i = 0; i < charts.length; i++) {
        var el = charts[i];
        var optStr = el.getAttribute('data-option');
        if (!optStr) continue;
        try {
            var opt = JSON.parse(optStr);
            var chart = echarts.init(el, 'dark', { renderer: 'canvas' });
            chart.setOption(opt);
        } catch (e) {
            console.error('Chart init failed', e);
            el.textContent = 'Chart error: ' + e.message;
        }
    }
});
</script>
</body></html>"##,
    );

    // Write HTML to a temporary file
    use std::io::Write;
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("vinrouge_dsl_results.html");

    std::fs::File::create(&file_path)
        .and_then(|mut f| f.write_all(html.as_bytes()))
        .map_err(|e| format!("Failed to write temp file: {e}"))?;

    // Create window pointing to the temp file
    #[cfg(target_os = "windows")]
    let file_url = format!("file:///{}", file_path.display().to_string().replace('\\', "/"));
    #[cfg(not(target_os = "windows"))]
    let file_url = format!("file://{}", file_path.display());
    let window = WebviewWindowBuilder::new(
        &app,
        "dsl-results",
        tauri::WebviewUrl::External(file_url.parse().unwrap()),
    )
    .title("DSL Results")
    .inner_size(1200.0, 800.0)
    .build()
    .map_err(|e| format!("Failed to create window: {e}"))?;

    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let _ = app_handle.emit("dsl-results-closed", ());
        }
    });

    Ok(())
}

fn count_results(
    results: &[serde_json::Value],
) -> (usize, usize, usize, usize, usize, usize, usize) {
    let mut passed = 0;
    let mut failed = 0;
    let mut errors = 0;
    let mut samples = 0;
    let mut charts = 0;
    let mut screens = 0;
    let mut sections = 0;
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
            Some("section") => {
                sections += 1;
                let inner = r["results"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                let (p, f, e, sa, c, sc, se) = count_results(inner);
                passed += p;
                failed += f;
                errors += e;
                samples += sa;
                charts += c;
                screens += sc;
                sections += se;
            }
            _ => {}
        }
    }
    (passed, failed, errors, samples, charts, screens, sections)
}

fn build_chart_option(chart_type: &str, labels: &[String], values: &[String]) -> String {
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
                r##"{{"backgroundColor":"transparent","tooltip":{{"trigger":"item"}},"legend":{{"orient":"vertical","left":"left","textStyle":{{"color":"#c9a8ae","fontSize":11}}}},"series":[{{"type":"pie","radius":"60%","data":[{}],"label":{{"color":"#f0e6e8","fontSize":11}}}}]}}"##,
                items.join(",")
            )
        }
        "line" => {
            format!(
                r##"{{"backgroundColor":"transparent","grid":{{"left":60,"right":20,"top":20,"bottom":60}},"tooltip":{{"trigger":"axis"}},"xAxis":{{"type":"category","data":[{}],"axisLabel":{{"rotate":30,"fontSize":10,"color":"#c9a8ae"}}}},"yAxis":{{"type":"value","axisLabel":{{"color":"#c9a8ae"}}}},"series":[{{"type":"line","data":[{}],"smooth":true,"itemStyle":{{"color":"#8b1a2a"}}}}]}}"##,
                cats.join(","),
                vals.join(",")
            )
        }
        "scatter" => {
            format!(
                r##"{{"backgroundColor":"transparent","grid":{{"left":60,"right":20,"top":20,"bottom":60}},"tooltip":{{"trigger":"item"}},"xAxis":{{"type":"category","data":[{}],"axisLabel":{{"rotate":30,"fontSize":10,"color":"#c9a8ae"}}}},"yAxis":{{"type":"value","axisLabel":{{"color":"#c9a8ae"}}}},"series":[{{"type":"scatter","data":[{}],"itemStyle":{{"color":"#8b1a2a"}}}}]}}"##,
                cats.join(","),
                vals.join(",")
            )
        }
        _ => {
            format!(
                r##"{{"backgroundColor":"transparent","grid":{{"left":60,"right":20,"top":20,"bottom":60}},"tooltip":{{"trigger":"axis"}},"xAxis":{{"type":"category","data":[{}],"axisLabel":{{"rotate":30,"fontSize":10,"color":"#c9a8ae"}}}},"yAxis":{{"type":"value","axisLabel":{{"color":"#c9a8ae"}}}},"series":[{{"type":"bar","data":[{}],"itemStyle":{{"color":"#8b1a2a"}}}}]}}"##,
                cats.join(","),
                vals.join(",")
            )
        }
    }
}

fn results_to_html(results: &[serde_json::Value], chart_idx: &mut usize) -> String {
    results
        .iter()
        .map(|r| result_to_html(r, chart_idx))
        .collect()
}

fn result_to_html(r: &serde_json::Value, chart_idx: &mut usize) -> String {
    let kind = r["kind"].as_str().unwrap_or("");
    match kind {
        "assert" => {
            let ok = r["passed"].as_bool().unwrap_or(false);
            let lbl = r["label"].as_str().unwrap_or("");
            let lhs = r["lhs_value"].as_str().unwrap_or("?");
            let rhs = r["rhs_value"].as_str().unwrap_or("?");
            let op = r["op"].as_str().unwrap_or("=");
            let icon = if ok {
                "<svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><path d=\"M2 6.5l3.5 3.5 5.5-6\" stroke=\"#4ade80\" stroke-width=\"1.6\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>"
            } else {
                "<svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><path d=\"M2.5 2.5l8 8M10.5 2.5l-8 8\" stroke=\"#f87171\" stroke-width=\"1.6\" stroke-linecap=\"round\"/></svg>"
            };
            let bg = if ok {
                "rgba(74,222,128,0.06)"
            } else {
                "rgba(248,113,113,0.06)"
            };
            let border = if ok {
                "rgba(74,222,128,0.15)"
            } else {
                "rgba(248,113,113,0.15)"
            };
            let icon_bg = if ok {
                "rgba(74,222,128,0.12)"
            } else {
                "rgba(248,113,113,0.12)"
            };
            let val_color = if ok { "#86efac" } else { "#fca5a5" };
            format!(
                "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:{};border:0.5px solid {};margin-bottom:6px;\">\
                 <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:{};\">{}</span>\
                 <div style=\"min-width:0;\">\
                 {}\
                 <div style=\"font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#d4c4c8;\"><span style=\"color:{};\">{}</span> <span style=\"color:#8a6e74;font-size:11px;\">{}</span> <span style=\"color:{};\">{}</span></div>\
                 </div></div>",
                bg, border, icon_bg, icon,
                if lbl.is_empty() { "".to_string() } else { format!("<div style=\"font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:2px;\">{}</div>", lbl) },
                val_color, lhs, op, val_color, rhs
            )
        }
        "sample" => {
            let method = r["method"].as_str().unwrap_or("SAMPLE");
            let pop = r["population_size"].as_u64().unwrap_or(0);
            let sel = r["selected_count"].as_u64().unwrap_or(0);
            let pct = if pop > 0 {
                sel as f64 / pop as f64 * 100.0
            } else {
                0.0
            };
            format!(
                "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(96,165,250,0.06);border:0.5px solid rgba(96,165,250,0.15);margin-bottom:6px;\">\
                 <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(96,165,250,0.12);\">\
                 <svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><rect x=\"1\" y=\"1\" width=\"11\" height=\"11\" rx=\"1.5\" stroke=\"#60a5fa\" stroke-width=\"1.1\"/><path d=\"M1 4.5h11M4.5 4.5v7\" stroke=\"#60a5fa\" stroke-width=\"1.1\"/></svg>\
                 </span>\
                 <div style=\"min-width:0;\">\
                 <div style=\"font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:2px;\">{}</div>\
                 <div style=\"font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#d4c4c8;\">{} of {} rows ({:.1}%)</div>\
                 </div></div>",
                method, sel, pop, pct
            )
        }
        "error" => {
            let msg = r["error"].as_str().unwrap_or("unknown error");
            format!(
                "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(251,146,60,0.06);border:0.5px solid rgba(251,146,60,0.15);margin-bottom:6px;\">\
                 <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(251,146,60,0.12);\">\
                 <svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><path d=\"M6.5 1.5L11.5 10.5H1.5L6.5 1.5Z\" stroke=\"#fb923c\" stroke-width=\"1.2\" stroke-linejoin=\"round\"/><path d=\"M6.5 5v3M6.5 9.5v.5\" stroke=\"#fb923c\" stroke-width=\"1.3\" stroke-linecap=\"round\"/></svg>\
                 </span>\
                 <div style=\"min-width:0;font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#fdba74;word-break:break-word;\">{}</div>\
                 </div>",
                msg
            )
        }
        "value" => {
            let val = r["value"].as_str().unwrap_or("");
            format!(
                "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(192,132,252,0.06);border:0.5px solid rgba(192,132,252,0.15);margin-bottom:6px;\">\
                 <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(192,132,252,0.12);\">\
                 <svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><path d=\"M2 6.5h9M7.5 3l3.5 3.5L7.5 10\" stroke=\"#c084fc\" stroke-width=\"1.3\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>\
                 </span>\
                 <div style=\"min-width:0;font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#d4c4c8;\">{}</div>\
                 </div>",
                val
            )
        }
        "relation" => {
            let from = r["from"].as_str().unwrap_or("");
            let to = r["to"].as_str().unwrap_or("");
            format!(
                "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(163,163,163,0.05);border:0.5px solid rgba(163,163,163,0.12);margin-bottom:6px;\">\
                 <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(163,163,163,0.10);\">\
                 <svg width=\"13\" height=\"13\" viewBox=\"0 0 13 13\" fill=\"none\"><circle cx=\"3\" cy=\"6.5\" r=\"1.5\" stroke=\"#a3a3a3\" stroke-width=\"1.1\"/><circle cx=\"10\" cy=\"6.5\" r=\"1.5\" stroke=\"#a3a3a3\" stroke-width=\"1.1\"/><path d=\"M4.5 6.5h3\" stroke=\"#a3a3a3\" stroke-width=\"1.1\"/></svg>\
                 </span>\
                 <div style=\"min-width:0;font-family:ui-monospace,Cascadia Code,monospace;font-size:12px;color:#d4c4c8;\"><span style=\"color:#aaa;\">{}</span> <span style=\"color:#666;\">→</span> <span style=\"color:#aaa;\">{}</span></div>\
                 </div>",
                from, to
            )
        }
        "chart" => {
            let lbl = r["label"].as_str().unwrap_or("");
            let ctype = r["chart_type"].as_str().unwrap_or("bar");
            let labels: Vec<String> = r["labels"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let values: Vec<String> = r["values"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let option = build_chart_option(ctype, &labels, &values);
            let id = format!("pop-chart-{}", *chart_idx);
            *chart_idx += 1;
            let mut h = String::new();
            h.push_str("<div style=\"padding:8px 10px;border-radius:6px;background:rgba(19,14,16,0.6);border:0.5px solid #2a2024;margin-bottom:6px;\">");
            if !lbl.is_empty() {
                h.push_str(&format!("<div style=\"font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:6px;\">{}</div>", lbl));
            }
            h.push_str(&format!(
                "<div id=\"{}\" data-option=\"{}\" style=\"width:100%;height:220px;\"></div>",
                id,
                html_escape(&option)
            ));
            h.push_str("</div>");
            h
        }
        "screen" => {
            let title = r["title"].as_str().unwrap_or("Screen");
            let charts: Vec<&serde_json::Value> = r["charts"]
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_default();
            let mut h = String::new();
            h.push_str(&format!(
                "<div style=\"padding:8px 10px;border-radius:6px;background:rgba(19,14,16,0.6);border:0.5px solid #2a2024;margin-bottom:6px;\">\
                 <div style=\"font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:6px;\">{}</div>\
                 <div style=\"display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:10px;\">",
                title
            ));
            for c in charts {
                let clbl = c["label"].as_str().unwrap_or("");
                let ctype = c["chart_type"].as_str().unwrap_or("bar");
                let labels: Vec<String> = c["labels"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let values: Vec<String> = c["values"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let option = build_chart_option(ctype, &labels, &values);
                let id = format!("pop-chart-{}", *chart_idx);
                *chart_idx += 1;
                h.push_str("<div style=\"padding:6px;border-radius:4px;background:rgba(255,255,255,0.02);border:0.5px solid #2a2024;\">");
                if !clbl.is_empty() {
                    h.push_str(&format!(
                        "<div style=\"font-size:10px;color:#c9a8ae;margin-bottom:4px;\">{}</div>",
                        clbl
                    ));
                }
                h.push_str(&format!(
                    "<div id=\"{}\" data-option=\"{}\" style=\"width:100%;height:180px;\"></div>",
                    id,
                    html_escape(&option)
                ));
                h.push_str("</div>");
            }
            h.push_str("</div></div>");
            h
        }
        "section" => {
            let title = r["title"].as_str().unwrap_or("Section");
            let passed = r["passed"].as_u64().unwrap_or(0);
            let failed = r["failed"].as_u64().unwrap_or(0);
            let errors = r["errors"].as_u64().unwrap_or(0);
            let inner: Vec<&serde_json::Value> = r["results"]
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_default();
            let mut badges = Vec::new();
            if passed > 0 {
                badges.push(format!("<span style=\"font-size:10px;padding:1px 6px;border-radius:99px;background:rgba(74,222,128,0.12);color:#4ade80;\">{} passed</span>", passed));
            }
            if failed > 0 {
                badges.push(format!("<span style=\"font-size:10px;padding:1px 6px;border-radius:99px;background:rgba(248,113,113,0.12);color:#f87171;\">{} failed</span>", failed));
            }
            if errors > 0 {
                badges.push(format!("<span style=\"font-size:10px;padding:1px 6px;border-radius:99px;background:rgba(251,146,60,0.12);color:#fb923c;\">{} error{}</span>", errors, if errors == 1 { "" } else { "s" }));
            }
            let badges_html = badges.join("");
            let inner_html: String = inner.iter().map(|c| result_to_html(c, chart_idx)).collect();
            let inner_id = format!(
                "sec-{:x}",
                title
                    .as_bytes()
                    .iter()
                    .fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(*b as u64))
            );
            format!(
                "<div style=\"border:0.5px solid #2a2024;border-radius:6px;background:rgba(19,14,16,0.6);overflow:hidden;margin-bottom:8px;\">\
                    <div style=\"display:flex;align-items:center;gap:10px;padding:8px 12px;background:rgba(255,255,255,0.03);cursor:pointer;user-select:none;\"\
                     onclick=\"var b=document.getElementById('{}');b.style.display=b.style.display==='none'?'block':'none';var c=this.querySelector('.chev');c.style.transform=b.style.display==='none'?'rotate(-90deg)':'rotate(0deg)';\">\
                        <span style=\"font-weight:600;font-size:13px;color:#d4c4c8;\">{}</span>\
                        <span style=\"display:flex;align-items:center;gap:6px;margin-left:auto;\">{}</span>\
                        <span class=\"chev\" style=\"color:#8a6e74;display:flex;align-items:center;transition:transform 0.15s;\">\
                            <svg width=\"12\" height=\"12\" viewBox=\"0 0 12 12\" fill=\"none\"><path d=\"M2 4.5l4 4 4-4\" stroke=\"currentColor\" stroke-width=\"1.2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>\
                        </span>\
                    </div>\
                    <div id=\"{}\" style=\"padding:8px 12px 12px;display:flex;flex-direction:column;gap:6px;\">{}</div>\
                </div>",
                inner_id, title, badges_html, inner_id, inner_html
            )
        }
        _ => String::new(),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
