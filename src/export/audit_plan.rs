//! Export the Audit Plan to PDF or Word (.docx).
//! Both functions are native-only (not compiled for WASM).

#[cfg(not(target_arch = "wasm32"))]
use crate::projects::{AuditProcessWithControls, ProjectDetails};
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
    processes: &[AuditProcessWithControls],
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

    let (doc, p0, l0) = PdfDocument::new("Audit Plan", Mm(W), Mm(H), "Layer 1");
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
    line_bold!("AUDIT PLAN", 18.0_f32);
    y -= 1.0;

    if let Some(d) = details {
        if !d.client.is_empty() {
            lines_at!(&format!("Client: {}", d.client), L, 11.0_f32, &font);
        }
        if !d.engagement_ref.is_empty() {
            lines_at!(
                &format!("Engagement ref: {}", d.engagement_ref),
                L,
                11.0_f32,
                &font
            );
        }
        if !d.period_start.is_empty() {
            lines_at!(
                &format!("Audit period: {} \u{2013} {}", d.period_start, d.period_end),
                L,
                11.0_f32,
                &font
            );
        }
        if !d.audit_type.is_empty() {
            lines_at!(&format!("Type: {}", d.audit_type), L, 11.0_f32, &font);
        }
    }
    lines_at!(
        &format!("Generated: {}", chrono::Local::now().format("%d %B %Y")),
        L,
        9.0_f32,
        &font
    );
    y -= 6.0;

    // ── Processes ─────────────────────────────────────────────────────────────
    for proc in processes {
        check!(18.0);
        line_bold!(&proc.process_name, 13.0_f32);

        if !proc.description.is_empty() {
            lines_at!(&proc.description, L, 10.0_f32, &font);
        }
        y -= 2.0;

        for ctrl in &proc.controls {
            check!(25.0);

            let header = format!(
                "{}   {}   Risk: {}",
                ctrl.control_ref, ctrl.control_objective, ctrl.risk_level
            );
            for ln in wrap(&header, 85) {
                check!(6.0);
                cur_layer.use_text(ln, 10.0_f32, Mm(L2), Mm(y), &font_bold);
                y -= 5.5;
            }

            if !ctrl.control_description.is_empty() {
                let t = format!("Control: {}", ctrl.control_description);
                lines_at!(&t, L2, 9.0_f32, &font);
            }
            if !ctrl.test_procedure.is_empty() {
                let t = format!("Test: {}", ctrl.test_procedure);
                lines_at!(&t, L2, 9.0_f32, &font);
            }
            y -= 3.0;
        }
        y -= 5.0;
    }

    let file = File::create(out_path)?;
    doc.save(&mut BufWriter::new(file))?;
    Ok(())
}

// ── DOCX ──────────────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub fn generate_docx(
    processes: &[AuditProcessWithControls],
    details: Option<&ProjectDetails>,
    out_path: &Path,
) -> Result<()> {
    use docx_rs::*;

    let mut doc = Docx::new();

    doc = doc
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("Audit Plan").bold().size(44)));

    if let Some(d) = details {
        if !d.client.is_empty() {
            doc = doc.add_paragraph(
                Paragraph::new().add_run(
                    Run::new()
                        .add_text(format!("Client: {}", d.client))
                        .size(24),
                ),
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
        if !d.audit_type.is_empty() {
            doc = doc.add_paragraph(
                Paragraph::new().add_run(
                    Run::new()
                        .add_text(format!("Type: {}", d.audit_type))
                        .size(24),
                ),
            );
        }
    }

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

    for proc in processes {
        doc = doc.add_paragraph(
            Paragraph::new().add_run(
                Run::new()
                    .add_text(proc.process_name.clone())
                    .bold()
                    .size(28),
            ),
        );
        if !proc.description.is_empty() {
            doc = doc.add_paragraph(
                Paragraph::new().add_run(Run::new().add_text(proc.description.clone()).size(22)),
            );
        }

        if !proc.controls.is_empty() {
            let header_row = TableRow::new(vec![
                TableCell::new().add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text("Ref").bold().size(18)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text("Risk").bold().size(18)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text("Objective").bold().size(18)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text("Description").bold().size(18)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text("Test Procedure").bold().size(18)),
                ),
            ]);

            let mut rows = vec![header_row];
            for ctrl in &proc.controls {
                rows.push(TableRow::new(vec![
                    TableCell::new().add_paragraph(
                        Paragraph::new()
                            .add_run(Run::new().add_text(ctrl.control_ref.clone()).size(18)),
                    ),
                    TableCell::new().add_paragraph(
                        Paragraph::new()
                            .add_run(Run::new().add_text(ctrl.risk_level.clone()).size(18)),
                    ),
                    TableCell::new().add_paragraph(
                        Paragraph::new()
                            .add_run(Run::new().add_text(ctrl.control_objective.clone()).size(18)),
                    ),
                    TableCell::new().add_paragraph(
                        Paragraph::new().add_run(
                            Run::new()
                                .add_text(ctrl.control_description.clone())
                                .size(18),
                        ),
                    ),
                    TableCell::new().add_paragraph(
                        Paragraph::new()
                            .add_run(Run::new().add_text(ctrl.test_procedure.clone()).size(18)),
                    ),
                ]));
            }

            doc = doc.add_table(Table::new(rows));
        }

        doc = doc.add_paragraph(Paragraph::new());
    }

    let file = std::fs::File::create(out_path)?;
    doc.build().pack(file)?;
    Ok(())
}
