//! Export the PBC data request list to PDF or Word (.docx).
//! Both functions are native-only (not compiled for WASM).

#[cfg(not(target_arch = "wasm32"))]
use crate::projects::{PbcGroup, ProjectDetails};
#[cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

// ── Helpers ───────────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn wrap(text: &str, max_chars: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if cur.is_empty() {
            cur = word.to_string();
        } else if cur.len() + 1 + word.len() <= max_chars {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(cur);
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

// ── PDF ───────────────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub fn generate_pdf(
    groups: &[PbcGroup],
    details: Option<&ProjectDetails>,
    out_path: &Path,
) -> Result<()> {
    use printpdf::*;
    use std::fs::File;
    use std::io::BufWriter;

    const W: f32 = 210.0;
    const H: f32 = 297.0;
    const L: f32 = 15.0;
    const L2: f32 = 20.0;
    let top: f32 = H - 15.0;

    let (doc, p0, l0) = PdfDocument::new("PBC Data Request List", Mm(W), Mm(H), "Layer 1");
    let font = doc.add_builtin_font(BuiltinFont::Helvetica)?;
    let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;

    let mut cur_layer = doc.get_page(p0).get_layer(l0);
    let mut y: f32 = top;

    let mut new_page = || -> PdfLayerReference {
        let (np, nl) = doc.add_page(Mm(W), Mm(H), "Layer 1");
        doc.get_page(np).get_layer(nl)
    };

    macro_rules! check {
        ($need:expr) => {
            if y - $need < 22.0 {
                cur_layer = new_page();
                y = top;
            }
        };
    }

    macro_rules! line_bold {
        ($text:expr, $size:expr) => {{
            let sz: f32 = $size;
            let lh = sz * 0.50;
            check!(lh);
            cur_layer.use_text($text, sz, Mm(L), Mm(y), &font_bold);
            y -= lh + 2.0;
        }};
    }

    macro_rules! lines_at {
        ($text:expr, $x:expr, $size:expr, $font:expr) => {{
            let sz: f32 = $size;
            let lh = sz * 0.44;
            let x: f32 = $x;
            for ln in wrap($text, 90) {
                check!(lh);
                cur_layer.use_text(ln, sz, Mm(x), Mm(y), $font);
                y -= lh + 1.2;
            }
        }};
    }

    // ── Title block ───────────────────────────────────────────────────────────
    line_bold!("PBC DATA REQUEST LIST", 18.0_f32);
    y -= 1.0;

    if let Some(d) = details {
        if !d.client.is_empty() {
            lines_at!(&format!("Client: {}", d.client), L, 11.0_f32, &font);
        }
        if !d.engagement_ref.is_empty() {
            lines_at!(&format!("Engagement ref: {}", d.engagement_ref), L, 11.0_f32, &font);
        }
        if !d.period_start.is_empty() {
            lines_at!(
                &format!("Audit period: {} \u{2013} {}", d.period_start, d.period_end),
                L, 11.0_f32, &font
            );
        }
    }

    let total: usize = groups.iter().map(|g| g.items.len()).sum();
    let approved: usize = groups
        .iter()
        .flat_map(|g| g.items.iter())
        .filter(|i| i.approved)
        .count();

    lines_at!(
        &format!("Total requests: {}   Approved: {}", total, approved),
        L, 10.0_f32, &font
    );
    lines_at!(
        &format!("Generated: {}", chrono::Local::now().format("%d %B %Y")),
        L, 9.0_f32, &font
    );
    y -= 6.0;

    // ── Groups ────────────────────────────────────────────────────────────────
    for group in groups {
        if group.items.is_empty() {
            continue;
        }
        check!(18.0);
        line_bold!(
            &format!(
                "{}   {}   ({})",
                group.control_ref, group.title, group.process_name
            ),
            12.0_f32
        );

        for item in &group.items {
            check!(20.0);

            let approved_str = if item.approved { "[Approved]" } else { "[Pending]" };
            let header = format!("{}  [{}]  {}", approved_str, item.item_type, item.name);
            for ln in wrap(&header, 85) {
                check!(6.0);
                cur_layer.use_text(ln, 10.0_f32, Mm(L2), Mm(y), &font_bold);
                y -= 5.5;
            }

            if let Some(t) = &item.table_name {
                if !t.is_empty() {
                    lines_at!(&format!("Table: {}", t), L2, 9.0_f32, &font);
                }
            }
            if !item.fields.is_empty() {
                let fields_str = item.fields.join(", ");
                lines_at!(&format!("Fields: {}", fields_str), L2, 9.0_f32, &font);
            }
            if !item.purpose.is_empty() {
                lines_at!(&format!("Purpose: {}", item.purpose), L2, 9.0_f32, &font);
            }
            if !item.scope_format.is_empty() {
                lines_at!(&format!("Scope: {}", item.scope_format), L2, 9.0_f32, &font);
            }
            y -= 3.0;
        }
        y -= 4.0;
    }

    let file = File::create(out_path)?;
    doc.save(&mut BufWriter::new(file))?;
    Ok(())
}

// ── DOCX ──────────────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub fn generate_docx(
    groups: &[PbcGroup],
    details: Option<&ProjectDetails>,
    out_path: &Path,
) -> Result<()> {
    use docx_rs::*;

    let mut doc = Docx::new();

    doc = doc.add_paragraph(
        Paragraph::new().add_run(
            Run::new()
                .add_text("PBC Data Request List")
                .bold()
                .size(44),
        ),
    );

    if let Some(d) = details {
        if !d.client.is_empty() {
            doc = doc.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(format!("Client: {}", d.client)).size(24)),
            );
        }
        if !d.engagement_ref.is_empty() {
            doc = doc.add_paragraph(
                Paragraph::new().add_run(
                    Run::new()
                        .add_text(format!("Engagement ref: {}", d.engagement_ref))
                        .size(24),
                ),
            );
        }
        if !d.period_start.is_empty() {
            doc = doc.add_paragraph(
                Paragraph::new().add_run(
                    Run::new()
                        .add_text(format!(
                            "Audit period: {} \u{2013} {}",
                            d.period_start, d.period_end
                        ))
                        .size(24),
                ),
            );
        }
    }

    let total: usize = groups.iter().map(|g| g.items.len()).sum();
    let approved: usize = groups
        .iter()
        .flat_map(|g| g.items.iter())
        .filter(|i| i.approved)
        .count();

    doc = doc.add_paragraph(
        Paragraph::new().add_run(
            Run::new()
                .add_text(format!(
                    "Total requests: {}   Approved: {}",
                    total, approved
                ))
                .size(22),
        ),
    );
    doc = doc.add_paragraph(
        Paragraph::new().add_run(
            Run::new()
                .add_text(format!(
                    "Generated: {}",
                    chrono::Local::now().format("%d %B %Y")
                ))
                .size(20),
        ),
    );
    doc = doc.add_paragraph(Paragraph::new());

    for group in groups {
        if group.items.is_empty() {
            continue;
        }

        doc = doc.add_paragraph(
            Paragraph::new().add_run(
                Run::new()
                    .add_text(format!(
                        "{}  \u{2013}  {}  ({})",
                        group.control_ref, group.title, group.process_name
                    ))
                    .bold()
                    .size(26),
            ),
        );

        let header_row = TableRow::new(vec![
            TableCell::new().add_paragraph(
                Paragraph::new().add_run(Run::new().add_text("#").bold().size(18)),
            ),
            TableCell::new().add_paragraph(
                Paragraph::new().add_run(Run::new().add_text("Type").bold().size(18)),
            ),
            TableCell::new().add_paragraph(
                Paragraph::new().add_run(Run::new().add_text("Request name").bold().size(18)),
            ),
            TableCell::new().add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text("Table / Source").bold().size(18)),
            ),
            TableCell::new().add_paragraph(
                Paragraph::new().add_run(Run::new().add_text("Fields").bold().size(18)),
            ),
            TableCell::new().add_paragraph(
                Paragraph::new().add_run(Run::new().add_text("Purpose").bold().size(18)),
            ),
            TableCell::new().add_paragraph(
                Paragraph::new().add_run(Run::new().add_text("Scope").bold().size(18)),
            ),
            TableCell::new().add_paragraph(
                Paragraph::new().add_run(Run::new().add_text("Approved").bold().size(18)),
            ),
        ]);

        let mut rows = vec![header_row];
        for (i, item) in group.items.iter().enumerate() {
            let fields_str = item.fields.join(", ");
            let table_str = item.table_name.as_deref().unwrap_or("\u{2014}").to_string();
            rows.push(TableRow::new(vec![
                TableCell::new().add_paragraph(
                    Paragraph::new()
                        .add_run(Run::new().add_text(format!("{}", i + 1)).size(18)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new()
                        .add_run(Run::new().add_text(item.item_type.clone()).size(18)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text(item.name.clone()).size(18)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text(table_str).size(18)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text(fields_str).size(18)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new()
                        .add_run(Run::new().add_text(item.purpose.clone()).size(18)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new()
                        .add_run(Run::new().add_text(item.scope_format.clone()).size(18)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new().add_run(
                        Run::new()
                            .add_text(if item.approved { "Yes" } else { "No" })
                            .size(18),
                    ),
                ),
            ]));
        }

        doc = doc.add_table(Table::new(rows));
        doc = doc.add_paragraph(Paragraph::new());
    }

    let file = std::fs::File::create(out_path)?;
    doc.build().pack(file)?;
    Ok(())
}
