use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::types::{ChartTab, DistPoint};

pub fn call_js(fn_name: &str, args: &[JsValue]) {
    let window = web_sys::window().unwrap();
    let fn_val = js_sys::Reflect::get(&window, &JsValue::from_str(fn_name)).ok();
    if let Some(f) = fn_val.and_then(|v| v.dyn_into::<js_sys::Function>().ok()) {
        let arr = js_sys::Array::new();
        for a in args { arr.push(a); }
        let _ = f.apply(&window, &arr);
    }
}

fn build_option(tab: ChartTab, data: &[DistPoint]) -> String {
    match tab {
        ChartTab::Bar => {
            let cats: Vec<String> = data.iter()
                .map(|d| format!("\"{}\"", d.value.replace('"', "\\\"")))
                .collect();
            let vals: Vec<String> = data.iter().map(|d| d.count.to_string()).collect();
            format!(
                concat!(
                    r##"{{"backgroundColor":"transparent","grid":{{"left":60,"right":20,"top":20,"bottom":60}},"##,
                    r##""tooltip":{{"trigger":"axis"}},"##,
                    r##""xAxis":{{"type":"category","data":[{}],"axisLabel":{{"rotate":30,"fontSize":10,"color":"#c9a8ae"}}}},"##,
                    r##""yAxis":{{"type":"value","axisLabel":{{"color":"#c9a8ae"}}}},"##,
                    r##""series":[{{"type":"bar","data":[{}],"itemStyle":{{"color":"#8b1a2a"}}}}]}}"##,
                ),
                cats.join(","),
                vals.join(",")
            )
        }
        ChartTab::Pie => {
            let items: Vec<String> = data.iter()
                .map(|d| format!(
                    "{{\"name\":\"{}\",\"value\":{}}}",
                    d.value.replace('"', "\\\""),
                    d.count
                ))
                .collect();
            format!(
                concat!(
                    r##"{{"backgroundColor":"transparent","tooltip":{{"trigger":"item"}},"##,
                    r##""legend":{{"orient":"vertical","left":"left","textStyle":{{"color":"#c9a8ae","fontSize":11}}}},"##,
                    r##""series":[{{"type":"pie","radius":"60%","data":[{}],"##,
                    r##""label":{{"color":"#f0e6e8","fontSize":11}}}}]}}"##,
                ),
                items.join(",")
            )
        }
        ChartTab::Table => String::new(),
    }
}

#[component]
pub fn EChart(
    chart_id: String,
    tab: Signal<ChartTab>,
    data: Signal<Vec<DistPoint>>,
) -> impl IntoView {
    let id = chart_id.clone();
    let id_cleanup = chart_id.clone();

    Effect::new(move |_| {
        let t = tab.get();
        let d = data.get();
        if t == ChartTab::Table || d.is_empty() {
            call_js("vinrougeDisposeChart", &[JsValue::from_str(&id)]);
            return;
        }
        let option = build_option(t, &d);
        call_js("vinrougeInitChart", &[
            JsValue::from_str(&id),
            JsValue::from_str(&option),
        ]);
    });

    on_cleanup(move || {
        call_js("vinrougeDisposeChart", &[JsValue::from_str(&id_cleanup)]);
    });

    view! {
        <div id=chart_id style="width:100%;height:260px;display:block"></div>
    }
}

// ── RawChart — renders a pre-built ECharts option JSON string ─────────────────

#[component]
pub fn RawChart(
    chart_id: String,
    /// A reactive signal carrying the full ECharts option JSON (or empty to dispose).
    option_json: Signal<String>,
    #[prop(default = 180)] height: u32,
) -> impl IntoView {
    let id         = chart_id.clone();
    let id_cleanup = chart_id.clone();

    Effect::new(move |_| {
        let opt = option_json.get();
        if opt.is_empty() {
            call_js("vinrougeDisposeChart", &[JsValue::from_str(&id)]);
        } else {
            call_js("vinrougeInitChart", &[
                JsValue::from_str(&id),
                JsValue::from_str(&opt),
            ]);
        }
    });

    on_cleanup(move || {
        call_js("vinrougeDisposeChart", &[JsValue::from_str(&id_cleanup)]);
    });

    let h = format!("width:100%;height:{height}px;display:block");
    view! { <div id=chart_id style=h></div> }
}

// ── Summary overview chart ────────────────────────────────────────────────────

/// Build a stacked bar chart showing passed / failed / error counts per control.
pub fn build_summary_option(items: &[(&str, i64, i64, i64)]) -> String {
    if items.is_empty() { return String::new(); }
    let cats  = items.iter().map(|(r,..)| format!("\"{}\"", r)).collect::<Vec<_>>().join(",");
    let pass  = items.iter().map(|(_,p,..)| p.to_string()).collect::<Vec<_>>().join(",");
    let fail  = items.iter().map(|(_,_,f,_)| f.to_string()).collect::<Vec<_>>().join(",");
    let errs  = items.iter().map(|(_,_,_,e)| e.to_string()).collect::<Vec<_>>().join(",");
    format!(
        concat!(
            r##"{{"backgroundColor":"transparent","grid":{{"left":55,"right":12,"top":10,"bottom":48}},"##,
            r##""tooltip":{{"trigger":"axis","axisPointer":{{"type":"shadow"}}}},"##,
            r##""legend":{{"data":["Passed","Failed","Errors"],"bottom":4,"textStyle":{{"color":"#c9a8ae","fontSize":10}}}},"##,
            r##""xAxis":{{"type":"category","data":[{cats}],"axisLabel":{{"color":"#c9a8ae","fontSize":10,"rotate":30}}}},"##,
            r##""yAxis":{{"type":"value","minInterval":1,"axisLabel":{{"color":"#c9a8ae","fontSize":10}}}},"##,
            r##""series":["##,
            r##"{{"name":"Passed","type":"bar","stack":"s","data":[{pass}],"itemStyle":{{"color":"#38a749"}}}},"##,
            r##"{{"name":"Failed","type":"bar","stack":"s","data":[{fail}],"itemStyle":{{"color":"#e05c5c"}}}},"##,
            r##"{{"name":"Errors","type":"bar","stack":"s","data":[{errs}],"itemStyle":{{"color":"#f0a500"}}}}"##,
            r##"]}}"##,
        ),
        cats = cats, pass = pass, fail = fail, errs = errs,
    )
}

// ── Distribution from sample rows ────────────────────────────────────────────

/// Pick the most chart-worthy column from sample rows: prefer categorical columns
/// (2–15 distinct values) and skip ID/date/amount columns.
pub fn pick_sample_column(rows: &[serde_json::Value]) -> Option<String> {
    let first = rows.first()?.as_object()?;
    const SKIP: &[&str] = &["_id", "id", "date", "desc", "ref", "amt", "pct", "amount", "disc", "score"];

    let mut best: Option<(String, usize)> = None;
    for col in first.keys() {
        let lower = col.to_lowercase();
        if SKIP.iter().any(|s| lower.ends_with(s) || lower == *s) { continue; }
        let distinct: std::collections::HashSet<&str> = rows.iter()
            .filter_map(|r| r[col].as_str())
            .collect();
        let card = distinct.len();
        if card >= 2 && card <= 15 {
            if best.as_ref().map(|(_, c)| card < *c).unwrap_or(true) {
                best = Some((col.clone(), card));
            }
        }
    }
    best.map(|(c, _)| c)
        .or_else(|| first.keys().next().cloned())
}

/// Build an ECharts option from DSL ChartResult data.
pub fn build_dsl_chart_option(chart_type: &str, labels: &[String], values: &[String]) -> String {
    let cats: Vec<String> = labels.iter()
        .map(|l| format!("\"{}\"", l.replace('"', "\\\"")))
        .collect();
    let vals: Vec<String> = values.iter().map(|v| v.to_string()).collect();

    match chart_type {
        "pie" => {
            let items: Vec<String> = labels.iter().zip(values.iter())
                .map(|(l, v)| format!(
                    "{{\"name\":\"{}\",\"value\":{}}}",
                    l.replace('"', "\\\""),
                    v
                ))
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

/// Compute a value-count distribution for one column across sample rows.
pub fn sample_distribution(rows: &[serde_json::Value], col: &str) -> Vec<super::types::DistPoint> {
    let mut counts: std::collections::HashMap<String, usize> = Default::default();
    for row in rows {
        let val = row[col].as_str().unwrap_or("").to_string();
        if !val.is_empty() { *counts.entry(val).or_default() += 1; }
    }
    let mut pts: Vec<super::types::DistPoint> = counts.into_iter()
        .map(|(value, count)| super::types::DistPoint { value, count })
        .collect();
    pts.sort_by(|a, b| b.count.cmp(&a.count).then(a.value.cmp(&b.value)));
    pts
}
