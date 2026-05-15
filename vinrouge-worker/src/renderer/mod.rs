use serde_json::Value;

/// Render a list of DSL result values to a self-contained HTML fragment.
///
/// Delegates to the shared renderer in the core `vinrouge` crate so that
/// markup, CSS defaults, and chart option generation are maintained in one
/// place.
pub fn render_results(results: &[Value]) -> String {
    vinrouge::dsl::html::render_html_fragment(results)
}
