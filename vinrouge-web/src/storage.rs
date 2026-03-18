// ── Audit setup state (persisted to localStorage) ─────────────────────────────

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditSetupState {
    pub step: u8,
    pub standards: Vec<(String, bool)>,
    pub scope: Vec<String>,
    pub approved: bool,
}

pub fn ls_set(key: &str, val: &str) {
    if let Some(s) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = s.set_item(key, val);
    }
}

pub fn ls_get(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item(key).ok())
        .flatten()
}
