//! Shared visual style primitives: alignment, paragraph style, strokes, borders, insets.
//!
//! Contains ONLY source document intent and scoped style properties (zero layout coordinates).

use serde::{Deserialize, Serialize};

use super::{ColorHex, LocaleIr, TextDirIr};

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

/// Typst semantic paragraph line-breaking mode (`#set par(linebreaks: ...)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LinebreaksIr {
    /// `auto`: Typst chooses based on `justify`.
    #[default]
    Auto,
    /// `"simple"`: greedy line-by-line breaking.
    Simple,
    /// `"optimized"`: paragraph-wide Knuth-Plass optimization.
    Optimized,
}

const fn default_true() -> bool {
    true
}

/// Pure semantic paragraph style and source intent resolved from Typst's `StyleChain`.
///
/// Layout coordinates and sibling bounding-box gaps live exclusively in
/// `DocumentIr::reference_layout` (indexed by `NodeId`), keeping `ParagraphStyleIr`
/// purely semantic and comparable (`PartialEq`) across paragraphs at different positions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParagraphStyleIr {
    pub align: HAlignIr,
    pub first_line_indent_pt: f64,
    pub hanging_indent_pt: f64,
    pub left_indent_pt: f64,
    pub right_indent_pt: f64,
    pub space_before_pt: f64,
    pub space_after_pt: f64,
    pub line_spacing_pt: Option<f64>,
    /// Typst `par.linebreaks` (`auto`, `"simple"`, or `"optimized"`).
    #[serde(default)]
    pub linebreaks: LinebreaksIr,
    /// Typst `par.justify` (`#set par(justify: ...)`).
    #[serde(default)]
    pub justify: bool,
    /// Typst `block.breakable` (`#set block(breakable: ...)`). Defaults to `true`.
    #[serde(default = "default_true")]
    pub breakable: bool,
    /// Typst `block.sticky` (`#set block(sticky: ...)`). Defaults to `false`.
    #[serde(default)]
    pub sticky: bool,
    /// Source intent: whether the paragraph ends with an explicit `\` / `#linebreak()`.
    #[serde(default)]
    pub ends_with_explicit_linebreak: bool,
    /// Scoped `StyleChain` resolved paragraph font size (`text.size` in `pt`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_font_size_pt: Option<f64>,
    /// Document root default font size (`pt`) for proportional spacing calibration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_default_font_size_pt: Option<f64>,
    /// Scoped `StyleChain` resolved `par.spacing` (`pt`, with local `em` evaluated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_par_spacing_pt: Option<f64>,
    /// Scoped `StyleChain` resolved `par.leading` (`pt`, with local `em` evaluated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_par_leading_pt: Option<f64>,
    /// Scoped `StyleChain` resolved language/region (`#set text(lang: ..., region: ...)`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<LocaleIr>,
    /// Scoped `StyleChain` resolved text direction (`ltr` / `rtl`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<TextDirIr>,
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
            linebreaks: LinebreaksIr::Auto,
            justify: false,
            breakable: true,
            sticky: false,
            ends_with_explicit_linebreak: false,
            resolved_font_size_pt: None,
            resolved_default_font_size_pt: None,
            resolved_par_spacing_pt: None,
            resolved_par_leading_pt: None,
            locale: None,
            dir: None,
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
    pub const fn with_space_after(mut self, space_after_pt: f64) -> Self {
        self.space_after_pt = space_after_pt;
        self
    }

    #[inline]
    pub const fn with_space_before(mut self, space_before_pt: f64) -> Self {
        self.space_before_pt = space_before_pt;
        self
    }

    #[inline]
    pub const fn with_spacing(mut self, space_before_pt: f64, space_after_pt: f64) -> Self {
        self.space_before_pt = space_before_pt;
        self.space_after_pt = space_after_pt;
        self
    }

    #[inline]
    pub const fn with_left_indent(mut self, left_indent_pt: f64) -> Self {
        self.left_indent_pt = left_indent_pt;
        self
    }
}

/// Stroke styling (color, thickness, dash pattern, cap, join).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrokeIr {
    pub color: ColorHex,
    pub thickness_pt: f64,
    pub dash: DashPatternIr,
    #[serde(default)]
    pub cap: LineCapIr,
    #[serde(default)]
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
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
