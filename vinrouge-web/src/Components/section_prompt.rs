use leptos::prelude::*;
use crate::components::buttons::SendButton;

/// The compact AI instruction block shown at the bottom of collapsible cards.
/// Pass `on_blur_save` if the prompt text should also be persisted to the backend
/// when the textarea loses focus (step 2 uses this; step 3 does not).
#[component]
pub fn SectionPrompt(
    prompt:      RwSignal<String>,
    loading:     RwSignal<bool>,
    status:      RwSignal<Option<String>>,
    #[prop(default = "Instruct the AI to update this section…")]
    placeholder: &'static str,
    #[prop(optional)] on_blur_save: Option<Callback<String>>,
    #[prop(into)] on_send: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="audit-section-prompt">
            <label class="audit-section-prompt-label">"AI instruction"</label>
            <div class="audit-section-prompt-row">
                <textarea
                    class="audit-section-prompt-textarea"
                    placeholder=placeholder
                    prop:value=move || prompt.get()
                    on:input=move |ev| prompt.set(event_target_value(&ev))
                    on:blur=move |_| {
                        if let Some(ref cb) = on_blur_save {
                            cb.run(prompt.get_untracked());
                        }
                    }
                />
                <SendButton
                    loading=loading.into()
                    on_click=move || {
                        if !prompt.get_untracked().trim().is_empty() && !loading.get_untracked() {
                            on_send.run(());
                        }
                    }
                />
            </div>
            {move || status.get().map(|s| {
                let is_err = s.starts_with("Error") || s.starts_with("Could") || s.starts_with("Parse");
                view! {
                    <div class=if is_err { "audit-ai-error" } else { "audit-ai-ok" }>{s}</div>
                }
            })}
        </div>
    }
}
