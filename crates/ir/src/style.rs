//! Shared visual style primitives: alignment, paragraph style, strokes, borders, insets.

use serde::{Deserialize, Serialize};
pub use crate::policy::{
    KeepLinesPolicy, KeepWithNextPolicy, KinsokuSet, LineBreakPolicy, WidowOrphanPolicy,
};

use super::ColorHex;
use super::pdf::{InterBlockFlowIr, ParagraphPdfMetricsIr};

/// Horizontal alignment in paragraphs, cells, and blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HAlignIr {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Vertical alignment in cells and boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VAlignIr {
    #[default]
    Top,
    Center,
    Bottom,
}

/// Paragraph layout and spacing properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParagraphStyleIr {
    pub align: HAlignIr,
    pub first_line_indent_pt: f64,
    pub hanging_indent_pt: f64,
    pub left_indent_pt: f64,
    pub right_indent_pt: f64,
    pub space_before_pt: f64,
    pub space_after_pt: f64,
    pub line_spacing_pt: Option<f64>,
    #[serde(default)]
    pub keep_with_next: bool,
    #[serde(default)]
    pub keep_lines: bool,
    #[serde(default)]
    pub line_break_policy: LineBreakPolicy,
    #[serde(default)]
    pub kinsoku: KinsokuSet,
    #[serde(default)]
    pub widow_orphan: WidowOrphanPolicy,
    #[serde(default)]
    pub pdf_metrics: ParagraphPdfMetricsIr,
    #[serde(default)]
    pub inter_block_flow: InterBlockFlowIr,
    #[serde(default)]
    pub extra_trailing_space_pt: f64,
}

impl Default for ParagraphStyleIr {
    fn default() -> Self {
        Self {
            align: HAlignIr::Left,
            first_line_indent_pt: 0.0,
            hanging_indent_pt: 0.0,
            left_indent_pt: 0.0,
            right_indent_pt: 0.0,
            space_before_pt: 0.0,
            space_after_pt: 6.0,
            line_spacing_pt: None,
            keep_with_next: false,
            keep_lines: false,
            line_break_policy: LineBreakPolicy::default(),
            kinsoku: KinsokuSet::default(),
            widow_orphan: WidowOrphanPolicy::default(),
            pdf_metrics: ParagraphPdfMetricsIr::default(),
            inter_block_flow: InterBlockFlowIr::default(),
            extra_trailing_space_pt: 0.0,
        }
    }
}

impl ParagraphStyleIr {
    #[inline]
    pub const fn with_align(mut self, align: HAlignIr) -> Self {
        self.align = align;
        self
    }

    #[inline]
    pub const fn with_spacing(mut self, before_pt: f64, after_pt: f64) -> Self {
        self.space_before_pt = before_pt;
        self.space_after_pt = after_pt;
        self
    }

    #[inline]
    pub const fn with_line_spacing(mut self, leading_pt: Option<f64>) -> Self {
        self.line_spacing_pt = leading_pt;
        self
    }

    #[inline]
    pub const fn with_keep_with_next(mut self, keep: bool) -> Self {
        self.keep_with_next = keep;
        self
    }

    #[inline]
    pub const fn with_keep_lines(mut self, keep: bool) -> Self {
        self.keep_lines = keep;
        self
    }

    #[inline]
    pub const fn with_line_break_policy(mut self, policy: LineBreakPolicy) -> Self {
        self.line_break_policy = policy;
        self
    }

    #[inline]
    pub fn with_kinsoku(mut self, kinsoku: KinsokuSet) -> Self {
        self.kinsoku = kinsoku;
        self
    }

    #[inline]
    pub const fn with_widow_orphan(mut self, policy: WidowOrphanPolicy) -> Self {
        self.widow_orphan = policy;
        self
    }

    #[inline]
    pub const fn keep_lines_policy(&self) -> KeepLinesPolicy {
        KeepLinesPolicy::from_bool(self.keep_lines)
    }

    #[inline]
    pub const fn keep_with_next_policy(&self) -> KeepWithNextPolicy {
        KeepWithNextPolicy::from_bool(self.keep_with_next)
    }
}

/// Stroke style for borders, lines, and vector shapes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrokeIr {
    pub color: ColorHex,
    pub thickness_pt: f64,
    pub dash: DashPatternIr,
    pub cap: LineCapIr,
    pub join: LineJoinIr,
}

impl Default for StrokeIr {
    fn default() -> Self {
        Self {
            color: "000000".to_string(),
            thickness_pt: 1.0,
            dash: DashPatternIr::Solid,
            cap: LineCapIr::Butt,
            join: LineJoinIr::Miter,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DashPatternIr {
    #[default]
    Solid,
    Dotted,
    Dashed,
    DashDot,
    DashDotDot,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineCapIr {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineJoinIr {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// Four-sided border strokes (e.g. for table cells or rectangular blocks).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BordersIr {
    pub top: Option<StrokeIr>,
    pub bottom: Option<StrokeIr>,
    pub left: Option<StrokeIr>,
    pub right: Option<StrokeIr>,
}

impl BordersIr {
    pub fn splat(stroke: Option<StrokeIr>) -> Self {
        Self {
            top: stroke.clone(),
            bottom: stroke.clone(),
            left: stroke.clone(),
            right: stroke,
        }
    }

    pub fn has_any(&self) -> bool {
        self.top.is_some() || self.bottom.is_some() || self.left.is_some() || self.right.is_some()
    }
}

/// Four-sided padding / inset in points.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SidesPtIr {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

impl SidesPtIr {
    pub const fn zero() -> Self {
        Self {
            top: 0.0,
            bottom: 0.0,
            left: 0.0,
            right: 0.0,
        }
    }
}

impl Default for SidesPtIr {
    fn default() -> Self {
        Self {
            top: 5.0,
            bottom: 5.0,
            left: 5.0,
            right: 5.0,
        }
    }
}
