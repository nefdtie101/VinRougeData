//! OCR support for scanned PDF documents.
//!
//! Uses:
//! - **PDFium** (bundled shared library) to render each PDF page to a bitmap
//! - **ocrs** (pure Rust) to extract text from those bitmaps
//!
//! OCR model files (~60 MB total) are downloaded automatically on first use
//! and cached at `~/VinRouge/models/`.
//!
//! # Bundling PDFium with the Tauri app
//!
//! Download the pre-built PDFium binary for each target platform from:
//!   <https://github.com/bblanchon/pdfium-binaries/releases>
//!
//! Place it next to the executable in the app bundle, e.g.:
//! - macOS  → `Contents/MacOS/libpdfium.dylib`
//! - Windows → `pdfium.dll`  (next to `.exe`)
//! - Linux   → `libpdfium.so` (next to the binary)
//!
//! In `tauri.conf.json` add:
//! ```json
//! { "bundle": { "resources": ["libpdfium.dylib"] } }
//! ```

use std::path::{Path, PathBuf};

// ── Model management ──────────────────────────────────────────────────────────

const DET_URL: &str = "https://ocrs-models.s3.amazonaws.com/text-detection.rten";
const REC_URL: &str = "https://ocrs-models.s3.amazonaws.com/text-recognition.rten";

fn models_dir() -> Result<PathBuf, String> {
    let dir = super::vinrouge_home()?.join("models");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Download `url` to `dest`, streaming directly to disk.
fn download_to(url: &str, dest: &Path) -> Result<(), String> {
    eprintln!(
        "[ocr] downloading {} …",
        dest.file_name().unwrap_or_default().to_string_lossy()
    );
    let resp = ureq::get(url)
        .call()
        .map_err(|e| format!("Download failed for {url}: {e}"))?;

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(dest)
        .map_err(|e| format!("Cannot create {}: {e}", dest.display()))?;

    std::io::copy(&mut reader, &mut file)
        .map_err(|e| format!("Write failed for {}: {e}", dest.display()))?;

    eprintln!("[ocr] saved {}", dest.display());
    Ok(())
}

/// Ensure both OCR model files are present, downloading if needed.
fn ensure_models() -> Result<(PathBuf, PathBuf), String> {
    let dir = models_dir()?;
    let det = dir.join("text-detection.rten");
    let rec = dir.join("text-recognition.rten");

    if !det.exists() {
        download_to(DET_URL, &det)?;
    }
    if !rec.exists() {
        download_to(REC_URL, &rec)?;
    }

    Ok((det, rec))
}

// ── PDFium binding ────────────────────────────────────────────────────────────

fn bind_pdfium() -> Result<pdfium_render::prelude::Pdfium, String> {
    use pdfium_render::prelude::*;

    // 1. Look next to the running executable (works in a Tauri app bundle)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let lib = Pdfium::pdfium_platform_library_name_at_path(dir);
            if let Ok(b) = Pdfium::bind_to_library(&lib) {
                return Ok(Pdfium::new(b));
            }
        }
    }

    // 2. Look in the current working directory (dev convenience)
    {
        let lib = Pdfium::pdfium_platform_library_name_at_path(".");
        if let Ok(b) = Pdfium::bind_to_library(&lib) {
            return Ok(Pdfium::new(b));
        }
    }

    // 3. Fall back to any system-installed PDFium
    Pdfium::bind_to_system_library()
        .map(Pdfium::new)
        .map_err(|e| {
            format!(
                "PDFium library not found. Bundle libpdfium with the app — see \
             src/projects/ocr.rs for instructions. Error: {e}"
            )
        })
}

// ── Public entry point ────────────────────────────────────────────────────────

/// OCR a scanned PDF and return all extracted text.
///
/// On first call, model files (~60 MB) are downloaded to `~/VinRouge/models/`.
/// PDFium must be bundled with the app (see module-level docs).
pub fn ocr_pdf(pdf_path: &str) -> Result<String, String> {
    use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
    use rten::Model;

    // 1. Load (or download) OCR models
    let (det_path, rec_path) = ensure_models()?;

    let det =
        Model::load_file(&det_path).map_err(|e| format!("Cannot load detection model: {e}"))?;
    let rec =
        Model::load_file(&rec_path).map_err(|e| format!("Cannot load recognition model: {e}"))?;

    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(det),
        recognition_model: Some(rec),
        ..Default::default()
    })
    .map_err(|e| format!("OCR engine init failed: {e}"))?;

    // 2. Bind to PDFium and open the document
    let pdfium = bind_pdfium()?;
    let doc = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| format!("PDFium could not open {pdf_path}: {e}"))?;

    let render_cfg = pdfium_render::prelude::PdfRenderConfig::new()
        .set_target_width(2000)
        .set_maximum_height(3000);

    // 3. Render each page and OCR it
    let mut all_text = String::new();

    for (i, page) in doc.pages().iter().enumerate() {
        let img = page
            .render_with_config(&render_cfg)
            .map_err(|e| format!("Page {} render failed: {e}", i + 1))?
            .as_image()
            .into_rgb8();

        let (w, h) = img.dimensions();

        let src = ImageSource::from_bytes(img.as_raw(), (w, h))
            .map_err(|e| format!("ImageSource error on page {}: {e}", i + 1))?;

        let input = engine
            .prepare_input(src)
            .map_err(|e| format!("OCR prepare_input page {}: {e}", i + 1))?;

        let words = engine
            .detect_words(&input)
            .map_err(|e| format!("OCR detect_words page {}: {e}", i + 1))?;

        let lines = engine.find_text_lines(&input, &words);

        let recognised = engine
            .recognize_text(&input, &lines)
            .map_err(|e| format!("OCR recognize page {}: {e}", i + 1))?;

        for line in recognised.into_iter().flatten() {
            all_text.push_str(&line.to_string());
            all_text.push('\n');
        }
        all_text.push('\n'); // blank line between pages
    }

    if all_text.trim().is_empty() {
        Err(
            "OCR produced no text. The PDF may use an unusual font encoding \
             or the image quality may be too low."
                .to_string(),
        )
    } else {
        Ok(all_text)
    }
}
