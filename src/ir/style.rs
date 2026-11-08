//! Shared visual style primitives: alignment, paragraph style, strokes, borders, insets.

use serde::{Deserialize, Serialize};

use super::ColorHex;

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
        }
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
