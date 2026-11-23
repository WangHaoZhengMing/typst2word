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
    let dash_xml = build_dash_xml(s);
    let join_xml = match s.join {
        LineJoinIr::Miter => r#"<a:miter lim="320000"/>"#,
        LineJoinIr::Round => r#"<a:round/>"#,
        LineJoinIr::Bevel => r#"<a:bevel/>"#,
    };

    format!(
        concat!(
            r#"<a:ln w="{w}" cap="{cap}">"#,
            r#"<a:solidFill><a:srgbClr val="{color}"/></a:solidFill>"#,
            r#"{dash}"#,
            r#"{join}"#,
            r#"</a:ln>"#
        ),
        w = w_emu,
        cap = cap,
        color = s.color,
        dash = dash_xml,
        join = join_xml,
    )
}

/// Serialize stroke dash pattern: emit OOXML `<a:custDash>` (exact dash/space ratios relative to
/// stroke width in `1/1000%` units, ECMA-376 §20.1.8.21) when Typst provided explicit dash lengths,
/// falling back to `<a:prstDash>` for preset/solid strokes.
fn build_dash_xml(s: &StrokeIr) -> String {
    if s.dash != DashPatternIr::Solid && !s.dash_array_pt.is_empty() {
        let base_w = s.thickness_pt.max(0.25);
        let effective: Vec<f64> = if s.dash_array_pt.len() % 2 == 1 {
            s.dash_array_pt
                .iter()
                .chain(s.dash_array_pt.iter())
                .copied()
                .collect()
        } else {
            s.dash_array_pt.clone()
        };
        let mut xml = String::from("<a:custDash>");
        for pair in effective.chunks_exact(2) {
            let d = ((pair[0] / base_w) * 100_000.0).round().clamp(1_000.0, 50_000_000.0) as u64;
            let sp = ((pair[1] / base_w) * 100_000.0).round().clamp(1_000.0, 50_000_000.0) as u64;
            xml.push_str(&format!(r#"<a:ds d="{d}" sp="{sp}"/>"#));
        }
        xml.push_str("</a:custDash>");
        return xml;
    }

    let prst_dash = match s.dash {
        DashPatternIr::Solid => "solid",
        DashPatternIr::Dotted => "dot",
        DashPatternIr::Dashed | DashPatternIr::Custom => "dash",
        DashPatternIr::DashDot => "dashDot",
        DashPatternIr::DashDotDot => "lgDashDotDot",
    };
    format!(r#"<a:prstDash val="{prst_dash}"/>"#)
}
