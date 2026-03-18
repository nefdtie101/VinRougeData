use leptos::prelude::*;

/// Small spinning SVG circle. `size` defaults to 13px.
#[component]
pub fn Spinner(#[prop(default = 13_u32)] size: u32) -> impl IntoView {
    view! {
        <svg
            width=size height=size
            viewBox="0 0 14 14" fill="none"
            style="animation:spin 1s linear infinite;flex-shrink:0"
        >
            <circle cx="7" cy="7" r="5.5"
                stroke="currentColor" stroke-width="1.4"
                stroke-dasharray="22" stroke-dashoffset="8"
                stroke-linecap="round"
            />
        </svg>
    }
}
