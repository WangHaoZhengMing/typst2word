//! List items for `list` / `enum` / `terms`.

use serde::{Deserialize, Serialize};

use super::{BlockNode, InlineNode};

/// Bullet list item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItemIr {
    pub marker: String,
    pub blocks: Vec<BlockNode>,
}

/// Numbered list item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumItemIr {
    pub number: Option<u64>,
    pub marker_text: String,
    pub blocks: Vec<BlockNode>,
}

/// Term / definition list item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermItemIr {
    pub term: Vec<InlineNode>,
    pub description: Vec<BlockNode>,
}
