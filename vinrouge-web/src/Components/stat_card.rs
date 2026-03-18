use leptos::prelude::*;

/// Small metric tile with a number value and a label below.
/// Set `green = true` to colour the value in the success green.
#[component]
pub fn StatCard(
    label: &'static str,
    value: Signal<String>,
    #[prop(default = false)] green: bool,
) -> impl IntoView {
    view! {
        <div style="flex:1;padding:10px 12px;background:var(--w-surface-2);border-radius:6px;border:0.5px solid var(--w-border);min-width:0">
            <div style=move || format!(
                "font-size:18px;font-weight:500;line-height:1;margin-bottom:3px;color:{}",
                if green { "#178856" } else { "var(--w-text-1)" }
            )>
                {move || value.get()}
            </div>
            <div style="font-size:11px;color:var(--w-text-3)">{label}</div>
        </div>
    }
}
