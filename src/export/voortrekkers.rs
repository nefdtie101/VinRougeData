use super::AnalysisResult;
use anyhow::Result;
use rust_xlsxwriter::*;

/// Aliases: maps a normalised template column name to alternative normalised
/// column names that may appear in real data exports (e.g. from Pipedrive/SAS).
const COLUMN_ALIASES: &[(&str, &[&str])] = &[
    // "Kursus" vs "Kurses" spelling variation
    ("kursus opsie 1", &["kurses opsie 1", "kamp - kursus"]),
    ("kursus opsie 2", &["kurses opsie 2"]),
    ("kursus opsie 3", &["kurses opsie 3"]),
    // Lid nommer — no space in real data
    ("lid nommer", &["lidnommer"]),
    // T-shirt size lives in the "T-Hemp" column
    ("t hemp", &["t-hemp"]),
    // Bus transport lives in this column
    ("bus", &["branders busvervoer retoer"]),
    // Parent contact normalisation
    ("ma kontak nommer", &["ma kontaknommer"]),
    ("pa kontak nommer", &["pa kontaknommer"]),
    // Date field
    ("geboorte datum", &["geboortedatum"]),
    // Spesialisasie spelling
    ("spesialsasie", &["spesialisasie"]),
];

/// Posisie values excluded from the Offisiere Lys (youth ranks, not officers).
const OFFISIERE_EXCLUDE_POSISIE: &[&str] = &["penkop/drawwertjie", "verkenner", "ontdekker"];

/// Columns used for each per-division sheet.
const DIVISIE_COLS: &[&str] = &[
    "naam",
    "noemnaam",
    "van",
    "lid nommer",
    "posisie",
    "kommando",
    "kursus opsie 1",
];

/// Report sheet definitions from the Johan Stelsel template.
/// Each entry is (sheet_name, columns).
/// An empty columns list means "all columns" (Vollidge Lys).
/// "Divisie Lys" is excluded here — it is generated dynamically per Opsie 1 value.
const REPORT_SHEETS: &[(&str, &[&str])] = &[
    ("Vollidge Lys", &[]),
    (
        "Waglys",
        &[
            "kursus opsie 1",
            "kursus opsie 2",
            "kursus opsie 3",
            "kommando",
            "spesialsasie",
            "naam",
            "noemnaam",
            "van",
            "lid nommer",
            "stage",
            "last updated on",
        ],
    ),
    (
        "Allergiee Lys",
        &[
            "naam",
            "noemnaam",
            "van",
            "geslag",
            "lid nommer",
            "kursus opsie 1",
            "kontaknommer voor kamp",
            "kontaknommer tydens kamp",
            "kontakpersoon tydens kamp",
            "lid kontaknommer",
            "skoolgraad",
            "posisie",
            "kommando",
            "ma kontak nommer",
            "pa kontak nommer",
            "allergieë",
            "allergiee",
            "addisionele notas",
        ],
    ),
    (
        "Medies",
        &[
            "naam",
            "van",
            "noemnaam",
            "geslag",
            "lid nommer",
            "kursus opsie 1",
            "kontaknommer voor kamp",
            "kontaknommer tydens kamp",
            "kontakpersoon tydens kamp",
            "lid kontaknommer",
            "skoolgraad",
            "posisie",
            "kommando",
            "ma kontak nommer",
            "pa kontak nommer",
            "mediese fonds naam",
            "mediese fonds nommer",
            "allergieë",
            "allergiee",
            "mediese kondisies",
            "kroniese medikasie",
            "mediese notas",
            "addisionele notas",
        ],
    ),
    (
        "Hempde Lys",
        &[
            "kommando",
            "kursus opsie 1",
            "naam",
            "noemnaam",
            "van",
            "lid nommer",
            "t hemp",
        ],
    ),
    (
        "Bus Lys",
        &[
            "kommando",
            "kursus opsie 1",
            "naam",
            "noemnaam",
            "van",
            "lid nommer",
            "ouer kontak nommer",
            "lid tipe",
            "bus",
        ],
    ),
    (
        "Offisiere Lys",
        &[
            "naam",
            "van",
            "lid nommer",
            "posisie",
            "kommando",
            "kursus opsie 1",
            "lid eposadres",
        ],
    ),
    (
        "Sertifikaat Lys1",
        &[
            "kursus opsie 1",
            "kommando",
            "spesialsasie",
            "naam",
            "noemnaam",
            "van",
            "lid nommer",
        ],
    ),
    (
        "Sertifikaat Lys2",
        &[
            "kursus opsie 1",
            "kommando",
            "bywoning",
            "naam",
            "noemnaam",
            "van",
            "lid nommer",
        ],
    ),
    (
        "Verblyf",
        &[
            "naam",
            "van",
            "lid nommer",
            "posisie",
            "kommando",
            "kursus opsie 1",
        ],
    ),
    (
        "Verjaarsdae",
        &[
            "naam",
            "van",
            "lid nommer",
            "posisie",
            "kommando",
            "geboorte datum",
            "geslag",
            "skoolgraad",
            "kursus opsie 1",
        ],
    ),
];

pub struct VoortrekkersExporter;

impl VoortrekkersExporter {
    pub fn new() -> Self {
        Self
    }

    /// Export the first available source dataset as a multi-sheet Voortrekkers
    /// workbook and return the raw bytes for browser download.
    pub fn export_to_bytes(&self, result: &AnalysisResult) -> Result<Vec<u8>> {
        let Some((_, data, columns)) = result.source_data.first() else {
            anyhow::bail!("No source data available to export.");
        };

        // Build a normalised-name → column-index map from the source data.
        let mut col_map: std::collections::HashMap<String, usize> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| (normalize(&c.name), i))
            .collect();

        // Insert reverse-alias entries so template names resolve to data indices.
        for (template_name, aliases) in COLUMN_ALIASES {
            if !col_map.contains_key(*template_name) {
                for alias in *aliases {
                    if let Some(&idx) = col_map.get(*alias) {
                        col_map.insert(template_name.to_string(), idx);
                        break;
                    }
                }
            }
        }

        let mut workbook = Workbook::new();

        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White);

        // Index of the Posisie column — used for Offisiere Lys exclusion filter.
        let posisie_idx = col_map.get("posisie").copied();

        // ── Static report sheets ─────────────────────────────────────────────
        for (sheet_name, template_cols) in REPORT_SHEETS {
            let worksheet = workbook.add_worksheet().set_name(*sheet_name)?;

            if template_cols.is_empty() {
                self.write_all_columns(worksheet, columns, data, &header_format)?;
            } else {
                let resolved = self.resolve_columns(template_cols, &col_map, columns);

                // Offisiere Lys: exclude youth ranks.
                let excl = if *sheet_name == "Offisiere Lys" {
                    posisie_idx.map(|idx| (idx, OFFISIERE_EXCLUDE_POSISIE))
                } else {
                    None
                };

                self.write_selected_columns(worksheet, &resolved, data, None, excl, &header_format)?;
            }
        }

        // ── Per-division sheets (one per unique Kursus Opsie 1 value) ────────
        if let Some(&opsie1_idx) = col_map.get("kursus opsie 1") {
            // Collect unique division values (trimmed, preserve original casing).
            let mut divisions: Vec<String> = {
                let mut seen = std::collections::HashSet::new();
                data.iter()
                    .filter_map(|row| row.get(opsie1_idx))
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
                    .filter(|v| seen.insert(v.to_lowercase()))
                    .collect()
            };
            divisions.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

            let resolved_divisie = self.resolve_columns(&DIVISIE_COLS, &col_map, columns);

            for division in &divisions {
                // Excel sheet names are max 31 chars; prefix "D-" then truncate.
                let raw = format!("D-{}", division);
                let sheet_name = truncate_sheet_name(&raw);

                let worksheet = workbook.add_worksheet().set_name(&sheet_name)?;

                // Filter rows to only this division (case-insensitive).
                let div_lower = division.to_lowercase();
                let filter: Option<(usize, &str)> = Some((opsie1_idx, &div_lower));

                self.write_selected_columns(
                    worksheet,
                    &resolved_divisie,
                    data,
                    filter,
                    None,
                    &header_format,
                )?;
            }
        }

        workbook.save_to_buffer().map_err(Into::into)
    }

    /// Resolve a list of template column names to (display_header, Option<src_idx>).
    /// Deduplicates alias collisions (e.g. allergieë / allergiee both mapping to
    /// the same source column).
    fn resolve_columns(
        &self,
        template_cols: &[&str],
        col_map: &std::collections::HashMap<String, usize>,
        columns: &[crate::schema::Column],
    ) -> Vec<(String, Option<usize>)> {
        let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut resolved = Vec::new();
        for &tc in template_cols {
            if let Some(&idx) = col_map.get(tc) {
                if seen.insert(idx) {
                    resolved.push((columns[idx].name.trim().to_string(), Some(idx)));
                }
                // duplicate alias — skip
            } else {
                resolved.push((tc.to_string(), None));
            }
        }
        resolved
    }

    fn write_all_columns(
        &self,
        worksheet: &mut Worksheet,
        columns: &[crate::schema::Column],
        data: &[Vec<String>],
        header_format: &Format,
    ) -> Result<()> {
        for (col_i, col) in columns.iter().enumerate() {
            worksheet.write_with_format(0, col_i as u16, col.name.trim(), header_format)?;
        }
        for (row_i, row) in data.iter().enumerate() {
            for (col_i, value) in row.iter().enumerate() {
                worksheet.write((row_i + 1) as u32, col_i as u16, value.trim())?;
            }
        }
        for col_i in 0..columns.len() {
            worksheet.set_column_width(col_i as u16, 18)?;
        }
        Ok(())
    }

    /// Write selected columns to a worksheet.
    /// - `include_filter`: if Some((col_idx, val)), only rows where that column == val are written.
    /// - `exclude_vals`: if Some((col_idx, &[vals])), rows where that column matches any val are skipped.
    fn write_selected_columns(
        &self,
        worksheet: &mut Worksheet,
        resolved: &[(String, Option<usize>)],
        data: &[Vec<String>],
        include_filter: Option<(usize, &str)>,
        exclude_vals: Option<(usize, &[&str])>,
        header_format: &Format,
    ) -> Result<()> {
        for (col_i, (header, _)) in resolved.iter().enumerate() {
            worksheet.write_with_format(0, col_i as u16, header.as_str(), header_format)?;
        }

        let mut out_row = 1u32;
        for row in data {
            // Include filter (e.g. per-division sheets).
            if let Some((filter_col, filter_val)) = include_filter {
                let cell = row.get(filter_col).map(|s| s.trim()).unwrap_or("");
                if cell.to_lowercase() != filter_val {
                    continue;
                }
            }

            // Exclusion filter (e.g. Offisiere Lys excludes youth ranks).
            if let Some((excl_col, excl_vals)) = exclude_vals {
                let cell = row.get(excl_col).map(|s| s.trim().to_lowercase()).unwrap_or_default();
                if excl_vals.iter().any(|v| *v == cell.as_str()) {
                    continue;
                }
            }

            for (col_i, (_, src_idx)) in resolved.iter().enumerate() {
                let value = src_idx
                    .and_then(|i| row.get(i))
                    .map(|s| s.trim())
                    .unwrap_or("");
                worksheet.write(out_row, col_i as u16, value)?;
            }
            out_row += 1;
        }

        for col_i in 0..resolved.len() {
            worksheet.set_column_width(col_i as u16, 18)?;
        }
        Ok(())
    }
}

/// Truncate a string to 31 chars (Excel sheet name limit).
fn truncate_sheet_name(s: &str) -> String {
    s.chars().take(31).collect()
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}
