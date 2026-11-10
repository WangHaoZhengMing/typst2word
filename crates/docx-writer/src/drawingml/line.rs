//! `StrokeIr` -> `<a:ln>` outline (width, color, dash, cap, join).

use word_construct::model::{DashPatternIr, LineCapIr, LineJoinIr, StrokeIr};
use word_construct::units::pt_to_emu;

pub(crate) fn build_ln_xml(stroke: Option<&StrokeIr>) -> String {
    let Some(s) = stroke else {
        return r#"<a:ln><a:noFill/></a:ln>"#.to_string();
    };
    if s.thickness_pt <= 0.001 {
        return r#"<a:ln><a:noFill/></a:ln>"#.to_string();
    }

    let w_emu = pt_to_emu(s.thickness_pt).max(3175);
    let cap = match s.cap {
        LineCapIr::Butt => "flat",
        LineCapIr::Round => "rnd",
        LineCapIr::Square => "sq",
    };
    let prst_dash = match s.dash {
        DashPatternIr::Solid => "solid",
        DashPatternIr::Dotted => "dot",
        DashPatternIr::Dashed | DashPatternIr::Custom => "dash",
        DashPatternIr::DashDot => "dashDot",
        DashPatternIr::DashDotDot => "lgDashDotDot",
    };
    let join_xml = match s.join {
        LineJoinIr::Miter => r#"<a:miter lim="320000"/>"#,
        LineJoinIr::Round => r#"<a:round/>"#,
        LineJoinIr::Bevel => r#"<a:bevel/>"#,
    };

    format!(
        concat!(
            r#"<a:ln w="{w}" cap="{cap}">"#,
            r#"<a:solidFill><a:srgbClr val="{color}"/></a:solidFill>"#,
            r#"<a:prstDash val="{dash}"/>"#,
            r#"{join}"#,
            r#"</a:ln>"#
        ),
        w = w_emu,
        cap = cap,
        color = s.color,
        dash = prst_dash,
        join = join_xml,
    )
}
