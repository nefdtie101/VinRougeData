/// Tracks whether the file still lives only in the browser (not yet persisted to
/// the project's files/ directory) or has already been saved.
#[derive(Clone)]
pub enum FileSource {
    /// Newly dropped file — bytes are in the browser, not yet sent to Tauri.
    Browser(web_sys::File),
    /// File was already saved to the project; holds its project-assigned ID.
    Saved(String),
}

/// A data file tracked in the Step-4 UI.
/// `local_id` equals the file name and is used as the stable selection key so
/// we can identify files before they have a project ID.
#[derive(Clone)]
pub struct DataFile {
    pub local_id: String,
    pub name: String,
    pub columns: Vec<String>,
    pub mappings: Vec<(String, String)>, // (source_col, pbc_field) — empty = unmapped
    pub source: FileSource,
}
