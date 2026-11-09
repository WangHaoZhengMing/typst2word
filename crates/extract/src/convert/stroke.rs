//! Typst strokes -> `StrokeIr` (thickness, color, dash pattern, caps, joins).

use typst::layout::Abs;
use typst::visualize::{FixedStroke, LineCap, LineJoin, Stroke};

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use ir::{DashPatternIr, LineCapIr, LineJoinIr, StrokeIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn fixed_stroke_to_ir(stroke: &FixedStroke) -> StrokeIr {
        let color = paint_to_hex(&stroke.paint).unwrap_or_else(|| "000000".to_string());
        let thickness_pt = stroke.thickness.to_pt().max(0.25);
        let cap = match stroke.cap {
            LineCap::Butt => LineCapIr::Butt,
            LineCap::Round => LineCapIr::Round,
            LineCap::Square => LineCapIr::Square,
        };
        let join = match stroke.join {
            LineJoin::Miter => LineJoinIr::Miter,
            LineJoin::Round => LineJoinIr::Round,
            LineJoin::Bevel => LineJoinIr::Bevel,
        };
        let dash = match &stroke.dash {
            None => DashPatternIr::Solid,
            Some(dp) => Self::classify_dash_array(&dp.array, thickness_pt),
        };
        StrokeIr {
            color,
            thickness_pt,
            dash,
            cap,
            join,
        }
    }

    pub(crate) fn abs_stroke_to_ir(stroke: &Stroke<Abs>) -> StrokeIr {
        Self::fixed_stroke_to_ir(&stroke.clone().unwrap_or_default())
    }

    pub(crate) fn classify_dash_array(array: &[Abs], thickness_pt: f64) -> DashPatternIr {
        if array.is_empty() {
            return DashPatternIr::Solid;
        }
        if array.len() == 2 {
            let d0 = array[0].to_pt();
            if d0 <= thickness_pt * 1.5 {
                return DashPatternIr::Dotted;
            }
            return DashPatternIr::Dashed;
        }
        if array.len() == 4 {
            return DashPatternIr::DashDot;
        }
        DashPatternIr::Dashed
    }
}
