use leptos::prelude::*;

/// Inline error/warning banner strip.
/// `variant`: "error" | "warning" | "info"  (defaults to "error")
#[component]
pub fn Banner(
    message: Signal<String>,
    #[prop(default = "error")] variant: &'static str,
) -> impl IntoView {
    let (bg, border, color) = match variant {
        "warning" => (
            "rgba(139,26,42,0.18)",
            "var(--w-border-2)",
            "var(--w-text-2)",
        ),
        "info" => (
            "rgba(96,165,250,0.08)",
            "rgba(96,165,250,0.3)",
            "var(--w-text-2)",
        ),
        _ => ("rgba(239,68,68,0.08)", "rgba(239,68,68,0.3)", "#ef4444"),
    };
    view! {
        <div style=format!(
            "padding:10px 14px;background:{bg};\
             border:0.5px solid {border};border-radius:6px;\
             font-size:12px;color:{color}"
        )>
            {move || message.get()}
        </div>
    }
}
