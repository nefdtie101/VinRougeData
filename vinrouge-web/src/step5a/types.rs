use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChartTab {
    Bar,
    Pie,
    Table,
}

impl ChartTab {
    pub fn label(self) -> &'static str {
        match self {
            ChartTab::Bar   => "Bar",
            ChartTab::Pie   => "Pie",
            ChartTab::Table => "Table",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistPoint {
    pub value: String,
    pub count: usize,
}
