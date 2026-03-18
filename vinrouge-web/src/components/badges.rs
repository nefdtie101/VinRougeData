use leptos::prelude::*;

/// Colour-coded High / Medium / Low risk pill.
#[component]
pub fn RiskBadge(level: Signal<String>) -> impl IntoView {
    let cls = move || match level.get().as_str() {
        "High" => "risk-badge high",
        "Low" => "risk-badge low",
        _ => "risk-badge medium",
    };
    view! { <span class=cls>{move || level.get()}</span> }
}

/// `"{approved}/{total}"` pill that turns green when all items are approved.
#[component]
pub fn CountBadge(approved: Signal<usize>, total: Signal<usize>) -> impl IntoView {
    view! {
        <span style=move || {
            let done = total.get() > 0 && approved.get() == total.get();
            format!(
                "font-size:11px;padding:2px 8px;border-radius:999px;flex-shrink:0;\
                 border:0.5px solid {};background:{};color:{}",
                if done { "#178856" } else { "var(--w-border)" },
                if done { "rgba(23,136,86,0.12)" } else { "var(--w-surface-1)" },
                if done { "#178856" } else { "var(--w-text-3)" },
            )
        }>
            {move || format!("{}/{}", approved.get(), total.get())}
        </span>
    }
}
