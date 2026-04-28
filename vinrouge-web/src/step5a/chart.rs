use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::types::{ChartTab, DistPoint};

fn call_js(fn_name: &str, args: &[JsValue]) {
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
