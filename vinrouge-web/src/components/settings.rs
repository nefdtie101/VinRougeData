use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::storage::{AiProvider, AppSettings};

#[component]
pub fn SettingsModal(
    on_close: Callback<()>,
) -> impl IntoView {
    let settings = AppSettings::load();

    let provider: RwSignal<AiProvider> = RwSignal::new(settings.provider);
    let ollama_url: RwSignal<String> = RwSignal::new(settings.ollama_url);
    let ollama_model: RwSignal<String> = RwSignal::new(settings.ollama_model);
    let claude_api_key: RwSignal<String> = RwSignal::new(settings.claude_api_key);
    let claude_model: RwSignal<String> = RwSignal::new(settings.claude_model);
    let saved: RwSignal<bool> = RwSignal::new(false);

    let do_save = move || {
        let s = AppSettings {
            provider: provider.get(),
            ollama_url: ollama_url.get().trim().to_string(),
            ollama_model: ollama_model.get().trim().to_string(),
            claude_api_key: claude_api_key.get().trim().to_string(),
            claude_model: claude_model.get().trim().to_string(),
        };
        s.save();
        saved.set(true);
        let _ = web_sys::window()
            .and_then(|w| w.set_timeout_with_callback_and_timeout_and_arguments_0(
                &wasm_bindgen::closure::Closure::once_into_js(move || {
                    saved.set(false);
                }).unchecked_into(),
                1200,
            ).ok());
    };

    view! {
        // Backdrop
        <div
            style="position:fixed;inset:0;z-index:900;background:rgba(13,10,11,0.85);
                   display:flex;align-items:center;justify-content:center"
            on:click=move |_| on_close.run(())>

            // Panel
            <div
                style="position:relative;background:var(--w-bg-2);border:1px solid var(--w-border-2);
                       border-radius:8px;width:min(460px,92vw);max-height:88vh;
                       display:flex;flex-direction:column;overflow:hidden"
                on:click=|e| e.stop_propagation()>

                // ── Header ────────────────────────────────────────────────────
                <div style="padding:14px 16px;border-bottom:0.5px solid var(--w-border);
                            display:flex;align-items:center;gap:10px">
                    <div style="flex:1;min-width:0">
                        <div style="font-size:14px;font-weight:700;color:var(--w-text-1)">
                            "Settings"
                        </div>
                        <div style="font-size:11px;color:var(--w-text-3);margin-top:2px">
                            "AI provider & API configuration"
                        </div>
                    </div>
                    <button
                        style="background:none;border:none;color:var(--w-text-3);
                               font-size:16px;cursor:pointer;padding:2px 6px;flex-shrink:0"
                        on:click=move |_| on_close.run(())>
                        "✕"
                    </button>
                </div>

                // ── Body ──────────────────────────────────────────────────────
                <div style="flex:1;overflow-y:auto;padding:14px 16px;display:flex;
                            flex-direction:column;gap:16px">

                    // Provider selector
                    <div>
                        <div style="font-size:11px;font-weight:600;color:var(--w-text-3);margin-bottom:6px">
                            "AI Provider"
                        </div>
                        <div style="display:flex;gap:8px">
                            <button
                                style=move || format!(
                                    "font-size:12px;padding:5px 12px;border-radius:4px;cursor:pointer;border:1px solid var(--w-border);{}",
                                    if provider.get() == AiProvider::Ollama {
                                        "background:var(--w-accent);color:#fff;"
                                    } else {
                                        "background:var(--w-bg);color:var(--w-text-2);"
                                    }
                                )
                                on:click=move |_| provider.set(AiProvider::Ollama)>
                                "Ollama (Local)"
                            </button>
                            <button
                                style=move || format!(
                                    "font-size:12px;padding:5px 12px;border-radius:4px;cursor:pointer;border:1px solid var(--w-border);{}",
                                    if provider.get() == AiProvider::Claude {
                                        "background:var(--w-accent);color:#fff;"
                                    } else {
                                        "background:var(--w-bg);color:var(--w-text-2);"
                                    }
                                )
                                on:click=move |_| provider.set(AiProvider::Claude)>
                                "Claude (Cloud)"
                            </button>
                        </div>
                    </div>

                    // Ollama settings
                    {move || (provider.get() == AiProvider::Ollama).then(|| view! {
                        <div style="display:flex;flex-direction:column;gap:12px">
                            <div>
                                <label style="font-size:11px;font-weight:600;color:var(--w-text-3);display:block;margin-bottom:4px">
                                    "Ollama URL"
                                </label>
                                <input
                                    type="text"
                                    style="width:100%;font-size:12px;padding:6px 8px;background:var(--w-bg);color:var(--w-text-2);border:1px solid var(--w-border);border-radius:4px"
                                    prop:value=move || ollama_url.get()
                                    on:input=move |ev| ollama_url.set(event_target_value(&ev))
                                />
                            </div>
                            <div>
                                <label style="font-size:11px;font-weight:600;color:var(--w-text-3);display:block;margin-bottom:4px">
                                    "Model"
                                </label>
                                <input
                                    type="text"
                                    style="width:100%;font-size:12px;padding:6px 8px;background:var(--w-bg);color:var(--w-text-2);border:1px solid var(--w-border);border-radius:4px"
                                    prop:value=move || ollama_model.get()
                                    on:input=move |ev| ollama_model.set(event_target_value(&ev))
                                />
                            </div>
                        </div>
                    })}

                    // Claude settings
                    {move || (provider.get() == AiProvider::Claude).then(|| view! {
                        <div style="display:flex;flex-direction:column;gap:12px">
                            <div>
                                <label style="font-size:11px;font-weight:600;color:var(--w-text-3);display:block;margin-bottom:4px">
                                    "API Key"
                                </label>
                                <input
                                    type="password"
                                    style="width:100%;font-size:12px;padding:6px 8px;background:var(--w-bg);color:var(--w-text-2);border:1px solid var(--w-border);border-radius:4px"
                                    prop:value=move || claude_api_key.get()
                                    on:input=move |ev| claude_api_key.set(event_target_value(&ev))
                                    placeholder="sk-ant-api03-..."
                                />
                                <div style="font-size:10px;color:var(--w-text-4);margin-top:3px">
                                    "Your key is stored locally in the browser."
                                </div>
                            </div>
                            <div>
                                <label style="font-size:11px;font-weight:600;color:var(--w-text-3);display:block;margin-bottom:4px">
                                    "Model"
                                </label>
                                <input
                                    type="text"
                                    style="width:100%;font-size:12px;padding:6px 8px;background:var(--w-bg);color:var(--w-text-2);border:1px solid var(--w-border);border-radius:4px"
                                    prop:value=move || claude_model.get()
                                    on:input=move |ev| claude_model.set(event_target_value(&ev))
                                />
                            </div>
                        </div>
                    })}

                </div>

                // ── Footer ────────────────────────────────────────────────────
                <div style="padding:12px 16px;border-top:0.5px solid var(--w-border);
                            display:flex;align-items:center;justify-content:flex-end;gap:8px">
                    {move || saved.get().then(|| view! {
                        <span style="font-size:12px;color:#4ade80;margin-right:4px">"Saved"</span>
                    })}
                    <button
                        style="font-size:12px;padding:5px 14px;border-radius:4px;cursor:pointer;
                               border:1px solid var(--w-border);background:var(--w-bg);color:var(--w-text-2)"
                        on:click=move |_| on_close.run(())>
                        "Close"
                    </button>
                    <button
                        style="font-size:12px;padding:5px 14px;border-radius:4px;cursor:pointer;
                               border:1px solid var(--w-accent);background:var(--w-accent);color:#fff"
                        on:click=move |_| do_save()>
                        "Save"
                    </button>
                </div>
            </div>
        </div>
    }
}
