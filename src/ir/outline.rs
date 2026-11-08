//! Table-of-contents entries (`outline`).

use serde::{Deserialize, Serialize};

use super::InlineNode;

/// Entry in an `#outline(...)` (Table of Contents).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineEntryIr {
    pub level: usize,
    pub prefix: Option<String>,
    pub title: Vec<InlineNode>,
    pub page: String,
    pub target_bookmark: Option<String>,
}
