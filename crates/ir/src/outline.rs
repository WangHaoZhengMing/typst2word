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
    /// Formatted page-number runs/fields (e.g. `"第 "` + `PAGEREF` + `" 页"`), preserving
    /// target page numbering prefix/suffix and extracted font styling.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub page_inlines: Vec<InlineNode>,
    pub target_bookmark: Option<String>,
}
