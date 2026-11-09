//! Bibliography entries (`bibliography`).

use serde::{Deserialize, Serialize};

use super::InlineNode;

/// Entry in a `#bibliography(...)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BibEntryIr {
    pub prefix: Option<String>,
    pub content: Vec<InlineNode>,
    pub bookmark: Option<String>,
}
