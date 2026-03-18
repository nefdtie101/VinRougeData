//! Central component library.
//! Import everything you need with `use crate::components::*;`

pub mod badges;
pub mod banner;
pub mod buttons;
pub mod inputs;
pub mod progress_ring;
pub mod section_prompt;
pub mod spinner;
pub mod stat_card;

pub use badges::{CountBadge, RiskBadge};
pub use banner::Banner;
pub use buttons::{DashedAddButton, GhostButton, PrimaryButton, SendButton};
pub use inputs::{InlineInput, InlineTextarea};
pub use progress_ring::ProgressRing;
pub use section_prompt::SectionPrompt;
pub use spinner::Spinner;
pub use stat_card::StatCard;
