//! CeTZ vector canvases.

use serde::{Deserialize, Serialize};

use super::{ColorHex, CurveSegmentIr, HAlignIr, InlineNode, StrokeIr};

/// CeTZ vector drawing canvas (`#import "@preview/cetz:..."`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CetzCanvasIr {
    pub width_pt: f64,
    pub height_pt: f64,
    pub background: Option<ColorHex>,
    pub paths: Vec<CetzPathDrawableIr>,
    pub labels: Vec<CetzLabelDrawableIr>,
    /// Optional rendered SVG fallback bytes if needed by viewers.
    pub svg_fallback: Option<Vec<u8>>,
    pub align: HAlignIr,
}

/// A single vector path/shape inside a CeTZ canvas at exact canvas coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CetzPathDrawableIr {
    pub x_pt: f64,
    pub y_pt: f64,
    pub width_pt: f64,
    pub height_pt: f64,
    pub segments: Vec<CurveSegmentIr>,
    pub fill: Option<ColorHex>,
    pub fill_rule_even_odd: bool,
    pub stroke: Option<StrokeIr>,
}

/// A positioned text or formula label inside a CeTZ canvas (`content((x, y), [...])`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CetzLabelDrawableIr {
    pub x_pt: f64,
    pub y_pt: f64,
    pub width_pt: f64,
    pub height_pt: f64,
    pub rotation_deg: f64,
    pub content: Vec<InlineNode>,
}
