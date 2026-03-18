use leptos::prelude::*;

/// Transparent single-line input that saves to the backend on blur.
/// The `on_save` callback receives the current value string.
#[component]
pub fn InlineInput(
    value: RwSignal<String>,
    #[prop(default = "")] placeholder: &'static str,
    #[prop(default = "")] class: &'static str,
    #[prop(default = "")] style: &'static str,
    #[prop(into)] on_save: Callback<String>,
) -> impl IntoView {
    view! {
        <input
            class=class
            style=style
            placeholder=placeholder
            prop:value=move || value.get()
            on:input=move |ev| value.set(event_target_value(&ev))
            on:blur=move |_| on_save.run(value.get_untracked())
        />
    }
}

/// Transparent multi-line textarea that saves to the backend on blur.
#[component]
pub fn InlineTextarea(
    value: RwSignal<String>,
    #[prop(default = "")] placeholder: &'static str,
    #[prop(default = "")] class: &'static str,
    #[prop(into)] on_save: Callback<String>,
) -> impl IntoView {
    view! {
        <textarea
            class=class
            placeholder=placeholder
            prop:value=move || value.get()
            on:input=move |ev| value.set(event_target_value(&ev))
            on:blur=move |_| on_save.run(value.get_untracked())
        />
    }
}
