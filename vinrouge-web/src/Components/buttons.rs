use leptos::prelude::*;
use crate::components::spinner::Spinner;

/// The small square send button used at the end of AI instruction rows.
/// Shows a spinner while loading, an arrow icon when idle.
#[component]
pub fn SendButton(
    loading:  Signal<bool>,
    #[prop(default = Signal::derive(|| false))] disabled: Signal<bool>,
    #[prop(into)] on_click: Callback<()>,
) -> impl IntoView {
    view! {
        <button
            class="audit-section-send-btn"
            prop:disabled=move || loading.get() || disabled.get()
            on:click=move |_| on_click.run(())
        >
            {move || if loading.get() {
                view! { <Spinner size=13 /> }.into_any()
            } else {
                view! {
                    <svg width="13" height="13" viewBox="0 0 14 14" fill="none">
                        <path d="M1.5 7h11M8 2.5l4.5 4.5L8 11.5"
                            stroke="currentColor" stroke-width="1.4"
                            stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                }.into_any()
            }}
        </button>
    }
}

/// Dashed ghost button used for "Add control" / "Add request" within cards.
#[component]
pub fn DashedAddButton(
    label:    &'static str,
    #[prop(into)] on_click: Callback<()>,
) -> impl IntoView {
    view! {
        <button
            class="add-control-btn"
            on:click=move |_| on_click.run(())
        >
            <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                <path d="M6 1v10M1 6h10"
                    stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
            {label}
        </button>
    }
}

/// Accent-filled primary action button. Shows a spinner when loading.
#[component]
pub fn PrimaryButton(
    label:         &'static str,
    #[prop(default = Signal::derive(|| false))] loading:  Signal<bool>,
    #[prop(default = Signal::derive(|| false))] disabled: Signal<bool>,
    #[prop(default = None)] loading_label: Option<&'static str>,
    #[prop(into)] on_click: Callback<()>,
) -> impl IntoView {
    let effective_label = move || {
        if loading.get() { loading_label.unwrap_or(label) } else { label }
    };
    view! {
        <button
            style="padding:6px 16px;font-size:12px;border-radius:4px;border:none;\
                   background:var(--w-accent);color:#fff;cursor:pointer;\
                   font-family:var(--font);display:flex;align-items:center;gap:6px"
            prop:disabled=move || loading.get() || disabled.get()
            on:click=move |_| on_click.run(())
        >
            {move || loading.get().then(|| view! { <Spinner size=11 /> })}
            {effective_label}
        </button>
    }
}

/// Transparent ghost button for secondary actions (Back, Dismiss, etc.).
/// Optionally shows a left-arrow icon when `back = true`.
#[component]
pub fn GhostButton(
    label:    &'static str,
    #[prop(default = false)] back: bool,
    #[prop(default = Signal::derive(|| false))] disabled: Signal<bool>,
    #[prop(into)] on_click: Callback<()>,
) -> impl IntoView {
    view! {
        <button
            style="padding:6px 14px;font-size:12px;border-radius:4px;\
                   border:0.5px solid var(--w-border-2);background:transparent;\
                   color:var(--w-text-2);cursor:pointer;font-family:var(--font);\
                   display:flex;align-items:center;gap:6px"
            prop:disabled=move || disabled.get()
            on:click=move |_| on_click.run(())
        >
            {back.then(|| view! {
                <svg width="10" height="10" viewBox="0 0 12 12" fill="none">
                    <path d="M10 6 L2 6 M5.5 2.5 L2 6 L5.5 9.5"
                        stroke="currentColor" stroke-width="1.4"
                        stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
            })}
            {label}
        </button>
    }
}
