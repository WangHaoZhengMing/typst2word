//! Native Typst shapes (`rect`, `circle`, `line`, `polygon`, `path`, `curve`, styled boxes).

use serde::{Deserialize, Serialize};

use super::{BlockNode, BordersIr, ColorHex, HAlignIr, SidesPtIr, StrokeIr};

/// Typst shape representation (`rect`, `square`, `circle`, `ellipse`,
/// `line`, `polygon`, `path`, `curve`, or styled `block`/`box`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeIr {
    pub kind: ShapeKindIr,
    pub width_pt: f64,
    pub height_pt: f64,
    pub fill: Option<ColorHex>,
    pub fill_rule_even_odd: bool,
    pub stroke: Option<StrokeIr>,
    pub borders: BordersIr,
    pub corner_radius_pt: Option<f64>,
    pub inset: SidesPtIr,
    pub rotation_deg: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    /// Optional content inside the shape (e.g. `#rect[Hello]` or `#circle[A]`).
    pub body: Vec<BlockNode>,
    pub align: HAlignIr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShapeKindIr {
    /// Rectangle (`rect` or `square`), optionally with rounded corners (`radius`).
    Rect,
    /// Circle or ellipse (`circle` or `ellipse`).
    Ellipse,
    /// Straight line segment (`line`) from `(x1, y1)` to `(x2, y2)` in points.
    Line {
        x1_pt: f64,
        y1_pt: f64,
        x2_pt: f64,
        y2_pt: f64,
    },
    /// Closed polygon (`polygon`) with vertices in points.
    Polygon { points_pt: Vec<(f64, f64)> },
    /// General Bézier/line path or curve (`path` or `curve`).
    CustomPath { segments: Vec<CurveSegmentIr> },
}

/// Segment of a `Curve` or `Path` in points relative to the shape's top-left origin.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CurveSegmentIr {
    MoveTo {
        x_pt: f64,
        y_pt: f64,
    },
    LineTo {
        x_pt: f64,
        y_pt: f64,
    },
    CubicTo {
        cx1_pt: f64,
        cy1_pt: f64,
        cx2_pt: f64,
        cy2_pt: f64,
        x_pt: f64,
        y_pt: f64,
    },
    Close,
}
