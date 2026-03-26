#[derive(Clone, PartialEq, Debug)]
pub enum ScriptStatus {
    Generated,
    Edited,
    Approved,
    Rejected,
}

#[derive(Clone, Debug)]
pub struct ScriptState {
    pub status: ScriptStatus,
    pub text: String,
}

#[derive(Clone, PartialEq)]
pub enum Phase {
    Loading,
    RelationshipReview, // user confirms how datasets connect
    BuildingMaster,     // hash-joining in progress
    Generating,
    Review,
    Running { done: usize, total: usize },
    Error(String),
}

#[derive(Clone)]
pub struct RunResult {
    pub expr_type: String,
    pub expected: String,
    pub actual:   String,
    pub passed:   bool,
    pub duration_ms: f64,
}

#[derive(Clone)]
pub struct ChatMsg {
    pub is_user: bool,
    pub text:    String,
    pub code:    Option<String>, // extracted DSL code block
}
