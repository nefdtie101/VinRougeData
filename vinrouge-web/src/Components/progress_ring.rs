use leptos::prelude::*;

/// SVG ring showing approved/total progress with a percentage label.
#[component]
pub fn ProgressRing(
    approved: Signal<usize>,
    total:    Signal<usize>,
) -> impl IntoView {
    let pct = move || {
        let t = total.get();
        if t == 0 { 0.0f64 } else { approved.get() as f64 / t as f64 * 100.0 }
    };
    view! {
        <div style="display:flex;align-items:center;gap:8px;flex-shrink:0">
            <svg width="34" height="34" viewBox="0 0 36 36">
                <circle cx="18" cy="18" r="14"
                    fill="none" stroke="var(--w-border)" stroke-width="3"/>
                <circle cx="18" cy="18" r="14"
                    fill="none" stroke="#178856" stroke-width="3"
                    stroke-dasharray="87.96 87.96"
                    stroke-dashoffset=move || format!("{:.2}", 87.96 * (1.0 - pct() / 100.0))
                    stroke-linecap="round"
                    transform="rotate(-90 18 18)"/>
                <text x="18" y="22" text-anchor="middle" font-size="9" fill="#178856"
                    font-family="var(--font)" font-weight="500">
                    {move || format!("{:.0}%", pct())}
                </text>
            </svg>
            <span style="font-size:12px;color:var(--w-text-3);white-space:nowrap">
                {move || format!("{} of {}", approved.get(), total.get())}
            </span>
        </div>
    }
}
