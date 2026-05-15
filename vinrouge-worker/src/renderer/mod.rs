use serde_json::Value;

/// Recursively collect CSS styles from results, drilling into sections.
fn collect_css(results: &[Value]) -> Vec<String> {
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

/// Render a list of DSL result values to a self-contained HTML string.
/// Output style matches the studio's pop-out results window exactly.
pub fn render_results(results: &[Value]) -> String {
    let css_blocks = collect_css(results);
    let css_html = if css_blocks.is_empty() {
        String::new()
    } else {
        format!("<style>{}</style>", css_blocks.join("\n"))
    };
    let mut chart_idx = 0usize;
    let body: String = results.iter().map(|r| render_one(r, &mut chart_idx)).collect();
    format!("{}{}", css_html, body)
}

fn render_one(r: &Value, chart_idx: &mut usize) -> String {
    match r["kind"].as_str().unwrap_or("") {
        "assert"   => render_assert(r),
        "sample"   => render_sample(r),
        "error"    => render_error(r),
        "value"    => render_value(r),
        "relation" => render_relation(r),
        "chart"    => render_chart(r, chart_idx),
        "section"  => render_section(r, chart_idx),
        "css"      => String::new(),
        // schema is intentionally not shown in dashboard output
        _ => String::new(),
    }
}

fn render_assert(r: &Value) -> String {
    let ok  = r["passed"].as_bool().unwrap_or(false);
    let lbl = r["label"].as_str().unwrap_or("");
    let lhs = r["lhs_value"].as_str().unwrap_or("?");
    let rhs = r["rhs_value"].as_str().unwrap_or("?");
    let op  = r["op"].as_str().unwrap_or("=");

    // SVG uses single-quote attrs to avoid conflicting with Rust raw-string delimiters
    let icon = if ok {
        "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><path d='M2 6.5l3.5 3.5 5.5-6' stroke='#4ade80' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'/></svg>"
    } else {
        "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><path d='M2.5 2.5l8 8M10.5 2.5l-8 8' stroke='#f87171' stroke-width='1.6' stroke-linecap='round'/></svg>"
    };
    let (bg, border, icon_bg, val_col) = if ok {
        ("rgba(74,222,128,0.06)", "rgba(74,222,128,0.15)", "rgba(74,222,128,0.12)", "#86efac")
    } else {
        ("rgba(248,113,113,0.06)", "rgba(248,113,113,0.15)", "rgba(248,113,113,0.12)", "#fca5a5")
    };
    let label_html = if !lbl.is_empty() {
        format!("<div style=\"font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:2px;\">{}</div>", esc(lbl))
    } else {
        String::new()
    };
    format!(
        "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:{bg};border:0.5px solid {border};margin-bottom:6px;\">\
          <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:{icon_bg};\">{icon}</span>\
          <div style=\"min-width:0;\">{label_html}\
            <div style=\"font-family:ui-monospace,monospace;font-size:12px;color:#d4c4c8;\">\
              <span style=\"color:{val_col};\">{lhs}</span>\
              <span style=\"color:#8a6e74;font-size:11px;\"> {op} </span>\
              <span style=\"color:{val_col};\">{rhs}</span>\
            </div>\
          </div>\
        </div>",
        lhs = esc(lhs), rhs = esc(rhs), op = esc(op),
    )
}

fn render_sample(r: &Value) -> String {
    let method = r["method"].as_str().unwrap_or("SAMPLE");
    let pop    = r["population_size"].as_u64().unwrap_or(0);
    let sel    = r["selected_count"].as_u64().unwrap_or(0);
    let pct    = if pop > 0 { sel as f64 / pop as f64 * 100.0 } else { 0.0 };
    let icon   = "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><rect x='1' y='1' width='11' height='11' rx='1.5' stroke='#60a5fa' stroke-width='1.1'/><path d='M1 4.5h11M4.5 4.5v7' stroke='#60a5fa' stroke-width='1.1'/></svg>";
    format!(
        "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(96,165,250,0.06);border:0.5px solid rgba(96,165,250,0.15);margin-bottom:6px;\">\
          <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(96,165,250,0.12);\">{icon}</span>\
          <div style=\"min-width:0;\">\
            <div style=\"font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:2px;\">{method}</div>\
            <div style=\"font-family:ui-monospace,monospace;font-size:12px;color:#d4c4c8;\">{sel} of {pop} rows ({pct:.1}%)</div>\
          </div>\
        </div>",
        method = esc(method),
    )
}

fn render_error(r: &Value) -> String {
    let msg  = r["error"].as_str().unwrap_or("unknown error");
    let icon = "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><path d='M6.5 1.5L11.5 10.5H1.5L6.5 1.5Z' stroke='#fb923c' stroke-width='1.2' stroke-linejoin='round'/><path d='M6.5 5v3M6.5 9.5v.5' stroke='#fb923c' stroke-width='1.3' stroke-linecap='round'/></svg>";
    format!(
        "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(251,146,60,0.06);border:0.5px solid rgba(251,146,60,0.15);margin-bottom:6px;\">\
          <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(251,146,60,0.12);\">{icon}</span>\
          <div style=\"min-width:0;font-family:ui-monospace,monospace;font-size:12px;color:#fdba74;word-break:break-word;\">{}</div>\
        </div>",
        esc(msg)
    )
}

fn render_value(r: &Value) -> String {
    let val  = r["value"].as_str().unwrap_or("");
    let icon = "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><path d='M2 6.5h9M7.5 3l3.5 3.5L7.5 10' stroke='#c084fc' stroke-width='1.3' stroke-linecap='round' stroke-linejoin='round'/></svg>";
    format!(
        "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(192,132,252,0.06);border:0.5px solid rgba(192,132,252,0.15);margin-bottom:6px;\">\
          <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(192,132,252,0.12);\">{icon}</span>\
          <div style=\"min-width:0;font-family:ui-monospace,monospace;font-size:12px;color:#d4c4c8;\">{}</div>\
        </div>",
        esc(val)
    )
}

fn render_relation(r: &Value) -> String {
    let from = r["from"].as_str().unwrap_or("");
    let to   = r["to"].as_str().unwrap_or("");
    let icon = "<svg width='13' height='13' viewBox='0 0 13 13' fill='none'><circle cx='3' cy='6.5' r='1.5' stroke='#a3a3a3' stroke-width='1.1'/><circle cx='10' cy='6.5' r='1.5' stroke='#a3a3a3' stroke-width='1.1'/><path d='M4.5 6.5h3' stroke='#a3a3a3' stroke-width='1.1'/></svg>";
    format!(
        "<div style=\"display:flex;align-items:flex-start;gap:8px;padding:8px 10px;border-radius:6px;background:rgba(163,163,163,0.05);border:0.5px solid rgba(163,163,163,0.12);margin-bottom:6px;\">\
          <span style=\"flex-shrink:0;width:22px;height:22px;border-radius:6px;display:flex;align-items:center;justify-content:center;background:rgba(163,163,163,0.10);\">{icon}</span>\
          <div style=\"min-width:0;font-family:ui-monospace,monospace;font-size:12px;color:#d4c4c8;\">\
            <span style=\"color:#aaa;\">{}</span> <span style=\"color:#666;\">&#8594;</span> <span style=\"color:#aaa;\">{}</span>\
          </div>\
        </div>",
        esc(from), esc(to)
    )
}

fn render_chart(r: &Value, chart_idx: &mut usize) -> String {
    let lbl   = r["label"].as_str().unwrap_or("");
    let ctype = r["chart_type"].as_str().unwrap_or("bar");
    let labels: Vec<&str> = r["labels"].as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let values: Vec<f64> = r["values"].as_array()
        .map(|a| a.iter()
            .filter_map(|v| v.as_str())
            .filter_map(|s| s.replace(',', "").parse::<f64>().ok())
            .collect())
        .unwrap_or_default();

    let option = build_echarts_option(ctype, &labels, &values);
    let id = format!("vr-chart-{}", *chart_idx);
    *chart_idx += 1;

    let label_html = if !lbl.is_empty() {
        format!("<div style=\"font-size:11px;font-weight:600;color:#c9a8ae;margin-bottom:6px;\">{}</div>", esc(lbl))
    } else {
        String::new()
    };
    format!(
        "<div style=\"padding:8px 10px;border-radius:6px;background:rgba(19,14,16,0.6);border:0.5px solid #2a2024;margin-bottom:6px;\">\
          {label_html}\
          <div id=\"{id}\" data-option=\"{option_esc}\" style=\"width:100%;height:220px;\"></div>\
        </div>",
        option_esc = esc(&option),
    )
}

fn render_section(r: &Value, chart_idx: &mut usize) -> String {
    let title  = r["title"].as_str().unwrap_or("Section");
    let passed = r["passed"].as_u64().unwrap_or(0);
    let failed = r["failed"].as_u64().unwrap_or(0);
    let errors = r["errors"].as_u64().unwrap_or(0);
    let inner: Vec<&Value> = r["results"].as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let inner_html: String = inner.iter().map(|c| render_one(c, chart_idx)).collect();

    let mut badges = Vec::new();
    if passed > 0 { badges.push(format!("<span style=\"font-size:10px;padding:1px 6px;border-radius:99px;background:rgba(74,222,128,0.12);color:#4ade80;\">{passed} passed</span>")); }
    if failed > 0 { badges.push(format!("<span style=\"font-size:10px;padding:1px 6px;border-radius:99px;background:rgba(248,113,113,0.12);color:#f87171;\">{failed} failed</span>")); }
    if errors > 0 { badges.push(format!("<span style=\"font-size:10px;padding:1px 6px;border-radius:99px;background:rgba(251,146,60,0.12);color:#fb923c;\">{} error{}</span>", errors, if errors == 1 { "" } else { "s" })); }
    let badges_html = badges.join("");

    // Stable collapse ID derived from title text
    let inner_id = format!("sec-{:x}", title.as_bytes().iter().fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(*b as u64)));
    let chev_svg = "<svg width='12' height='12' viewBox='0 0 12 12' fill='none'><path d='M2 4.5l4 4 4-4' stroke='currentColor' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/></svg>";

    format!(
        "<div style=\"border:0.5px solid #2a2024;border-radius:6px;background:rgba(19,14,16,0.6);overflow:hidden;margin-bottom:8px;\">\
          <div style=\"display:flex;align-items:center;gap:10px;padding:8px 12px;background:rgba(255,255,255,0.03);cursor:pointer;user-select:none;\"\
               onclick=\"var b=document.getElementById('{inner_id}');b.style.display=b.style.display==='none'?'flex':'none';var c=this.querySelector('.chev');c.style.transform=b.style.display==='none'?'rotate(-90deg)':'rotate(0deg)';\">\
            <span style=\"font-weight:600;font-size:13px;color:#d4c4c8;\">{title_esc}</span>\
            <span style=\"display:flex;align-items:center;gap:6px;margin-left:auto;\">{badges_html}</span>\
            <span class=\"chev\" style=\"color:#8a6e74;display:flex;align-items:center;transition:transform 0.15s;\">{chev_svg}</span>\
          </div>\
          <div id=\"{inner_id}\" style=\"padding:8px 12px 12px;display:flex;flex-direction:column;gap:6px;\">{inner_html}</div>\
        </div>",
        title_esc = esc(title),
    )
}

fn build_echarts_option(chart_type: &str, labels: &[&str], values: &[f64]) -> String {
    match chart_type {
        "pie" => {
            let data: Vec<String> = labels.iter().zip(values.iter())
                .map(|(l, v)| format!("{{\"name\":{},\"value\":{}}}", serde_json::to_string(l).unwrap_or_default(), v))
                .collect();
            format!("{{\"series\":[{{\"type\":\"pie\",\"radius\":\"60%\",\"data\":[{}]}}]}}", data.join(","))
        }
        _ => {
            let labels_json = serde_json::to_string(labels).unwrap_or_default();
            let values_json = serde_json::to_string(values).unwrap_or_default();
            format!(
                "{{\"xAxis\":{{\"type\":\"category\",\"data\":{labels_json},\"axisLabel\":{{\"rotate\":30}}}},\"yAxis\":{{\"type\":\"value\"}},\"series\":[{{\"type\":\"bar\",\"data\":{values_json}}}],\"grid\":{{\"containLabel\":true}}}}"
            )
        }
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}
