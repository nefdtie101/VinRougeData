use crate::types::AnalysisResult;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

// ── Tauri detection & IPC ─────────────────────────────────────────────────────

/// Returns true when the page is running inside a Tauri WebView.
pub fn is_tauri() -> bool {
    web_sys::window()
        .and_then(|w| js_sys::Reflect::has(&w, &JsValue::from_str("__TAURI__")).ok())
        .unwrap_or(false)
}

/// Call `pick_and_analyze` Tauri command (opens OS file dialog, runs analysis
/// in native Rust, returns JSON). Returns `None` if the user cancelled.
pub async fn tauri_pick_and_analyze() -> Result<Option<AnalysisResult>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let tauri = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))
        .map_err(|_| "no __TAURI__")?;
    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))
        .map_err(|_| "no __TAURI__.core")?;
    let invoke: js_sys::Function = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))
        .map_err(|_| "no invoke")?
        .dyn_into()
        .map_err(|_| "invoke not a function")?;

    let promise: js_sys::Promise = invoke
        .call1(&JsValue::UNDEFINED, &JsValue::from_str("pick_and_analyze"))
        .map_err(|e| format!("invoke failed: {e:?}"))?
        .dyn_into()
        .map_err(|_| "not a promise")?;

    let val = JsFuture::from(promise)
        .await
        .map_err(|e| format!("command error: {e:?}"))?;

    if val.is_null() || val.is_undefined() {
        return Ok(None); // user cancelled
    }

    let json = js_sys::JSON::stringify(&val)
        .map_err(|e| format!("stringify: {e:?}"))?
        .as_string()
        .ok_or("stringify returned non-string")?;

    serde_json::from_str::<AnalysisResult>(&json)
        .map(Some)
        .map_err(|e| format!("deserialize: {e}"))
}

// ── Generic Tauri IPC helpers ─────────────────────────────────────────────────

pub async fn tauri_invoke<T: for<'de> serde::Deserialize<'de>>(cmd: &str) -> Result<T, String> {
    let window = web_sys::window().ok_or("no window")?;
    let tauri = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))
        .map_err(|_| "no __TAURI__")?;
    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))
        .map_err(|_| "no __TAURI__.core")?;
    let invoke: js_sys::Function = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))
        .map_err(|_| "no invoke")?
        .dyn_into()
        .map_err(|_| "invoke not a function")?;

    let promise: js_sys::Promise = invoke
        .call1(&JsValue::UNDEFINED, &JsValue::from_str(cmd))
        .map_err(|e| format!("invoke failed: {e:?}"))?
        .dyn_into()
        .map_err(|_| "not a promise")?;

    let val = JsFuture::from(promise)
        .await
        .map_err(|e| format!("command error: {e:?}"))?;

    let json = js_sys::JSON::stringify(&val)
        .map_err(|e| format!("stringify: {e:?}"))?
        .as_string()
        .ok_or("stringify returned non-string")?;

    serde_json::from_str::<T>(&json).map_err(|e| format!("deserialize: {e}"))
}

/// Register a callback that receives `(percent, status, done)` for every
/// `model-pull-progress` event emitted by the backend.  The listener lives
/// for the lifetime of the page, so no unlisten handle is needed.
pub fn tauri_listen_pull_progress(
    on_progress: impl Fn(u8, String, bool) + 'static,
) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let tauri = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))
        .map_err(|_| "no __TAURI__")?;
    let event_obj = js_sys::Reflect::get(&tauri, &JsValue::from_str("event"))
        .map_err(|_| "no __TAURI__.event")?;
    let listen_fn: js_sys::Function =
        js_sys::Reflect::get(&event_obj, &JsValue::from_str("listen"))
            .map_err(|_| "no listen")?
            .dyn_into()
            .map_err(|_| "listen not a function")?;

    let cb = Closure::wrap(Box::new(move |event: JsValue| {
        let payload =
            js_sys::Reflect::get(&event, &JsValue::from_str("payload")).unwrap_or(JsValue::NULL);
        let percent = js_sys::Reflect::get(&payload, &JsValue::from_str("percent"))
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as u8;
        let status = js_sys::Reflect::get(&payload, &JsValue::from_str("status"))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let done = js_sys::Reflect::get(&payload, &JsValue::from_str("done"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        on_progress(percent, status, done);
    }) as Box<dyn Fn(JsValue)>);

    listen_fn
        .call2(
            &JsValue::UNDEFINED,
            &JsValue::from_str("model-pull-progress"),
            cb.as_ref().unchecked_ref(),
        )
        .map_err(|e| format!("listen call failed: {e:?}"))?;

    cb.forget(); // listener lives for the app's lifetime
    Ok(())
}

/// Returns `true` when the `mistral` model is already available locally.
pub async fn tauri_check_model() -> Result<bool, String> {
    tauri_invoke::<bool>("check_model").await
}

/// Pulls `mistral` from the Ollama registry.  Resolves only when the download
/// is fully complete (or errors).  Can take several minutes on a slow link.
pub async fn tauri_pull_model() -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let tauri = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))
        .map_err(|_| "no __TAURI__")?;
    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))
        .map_err(|_| "no __TAURI__.core")?;
    let invoke: js_sys::Function = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))
        .map_err(|_| "no invoke")?
        .dyn_into()
        .map_err(|_| "invoke not a function")?;

    let promise: js_sys::Promise = invoke
        .call1(&JsValue::UNDEFINED, &JsValue::from_str("pull_model"))
        .map_err(|e| format!("invoke failed: {e:?}"))?
        .dyn_into()
        .map_err(|_| "not a promise")?;

    JsFuture::from(promise)
        .await
        .map_err(|e| format!("pull_model error: {e:?}"))?;

    Ok(())
}

pub async fn tauri_invoke_args<T: for<'de> serde::Deserialize<'de>>(
    cmd: &str,
    args: serde_json::Value,
) -> Result<T, String> {
    let window = web_sys::window().ok_or("no window")?;
    let tauri = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))
        .map_err(|_| "no __TAURI__")?;
    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))
        .map_err(|_| "no __TAURI__.core")?;
    let invoke: js_sys::Function = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))
        .map_err(|_| "no invoke")?
        .dyn_into()
        .map_err(|_| "invoke not a function")?;

    let js_args = js_sys::JSON::parse(
        &serde_json::to_string(&args).map_err(|e| format!("args serialize: {e}"))?,
    )
    .map_err(|e| format!("JSON.parse: {e:?}"))?;

    let promise: js_sys::Promise = invoke
        .call2(&JsValue::UNDEFINED, &JsValue::from_str(cmd), &js_args)
        .map_err(|e| format!("invoke failed: {e:?}"))?
        .dyn_into()
        .map_err(|_| "not a promise")?;

    let val = JsFuture::from(promise)
        .await
        .map_err(|e| format!("command error: {e:?}"))?;

    let json = js_sys::JSON::stringify(&val)
        .map_err(|e| format!("stringify: {e:?}"))?
        .as_string()
        .ok_or("stringify returned non-string")?;

    serde_json::from_str::<T>(&json).map_err(|e| format!("deserialize: {e}"))
}
