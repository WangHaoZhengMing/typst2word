//! Semantic block container (`#block(...)` / `#box(...)` with fill, borders, inset, or radius).
//!
//! Decouples Typst container blocks from 1x1 `TableIr` in `DocumentIr`, allowing `word-construct`
//! to lower containers cleanly into rounded DrawingML shapes, paragraph borders/shading, or
//! breakable 1x1 Word container tables.

use serde::{Deserialize, Serialize};

use crate::{BlockNode, BordersIr, ColorHex, HAlignIr, NodeId, SidesPtIr};

/// Visual box decoration (`fill`, `borders`, `corner_radius_pt`, `inset`) shared by
/// [`ContainerIr`] and `#show heading: it => block(...)` decorated headings.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ContainerStyleIr {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<ColorHex>,
    #[serde(default)]
    pub borders: BordersIr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radius_pt: Option<f64>,
    #[serde(default)]
    pub inset: SidesPtIr,
}

impl ContainerStyleIr {
    #[inline]
    pub fn has_visual_decoration(&self) -> bool {
        self.fill.is_some()
            || self.borders.has_any()
            || self.corner_radius_pt.is_some_and(|r| r > 0.1)
    }
}

const fn default_true() -> bool {
    true
}

/// Block-level styled container (`BlockNode::Container`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerIr {
    #[serde(default, skip_serializing_if = "NodeId::is_unset")]
    pub id: NodeId,
    pub style: ContainerStyleIr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_pt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_pt: Option<f64>,
    #[serde(default)]
    pub align: HAlignIr,
    #[serde(default)]
    pub left_indent_pt: f64,
    #[serde(default = "default_true")]
    pub breakable: bool,
    #[serde(default)]
    pub sticky: bool,
    pub body: Vec<BlockNode>,
}
