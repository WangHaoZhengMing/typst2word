//! Curve segments -> `<a:custGeom>` path lists and bounding boxes.

use ir::CurveSegmentIr;
use ir::units::pt_to_emu;

pub(crate) fn compute_segments_bbox(segments: &[CurveSegmentIr]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    let mut visit = |x: f64, y: f64| {
        if x.is_finite() && y.is_finite() {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    };

    for seg in segments {
        match *seg {
            CurveSegmentIr::MoveTo { x_pt, y_pt } | CurveSegmentIr::LineTo { x_pt, y_pt } => {
                visit(x_pt, y_pt);
            }
            CurveSegmentIr::CubicTo {
                cx1_pt,
                cy1_pt,
                cx2_pt,
                cy2_pt,
                x_pt,
                y_pt,
            } => {
                visit(cx1_pt, cy1_pt);
                visit(cx2_pt, cy2_pt);
                visit(x_pt, y_pt);
            }
            CurveSegmentIr::Close => {}
        }
    }

    if !min_x.is_finite() {
        (0.0, 0.0, 1.0, 1.0)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}

pub(crate) fn build_cust_geom_xml(
    segments: &[CurveSegmentIr],
    path_w_emu: i64,
    path_h_emu: i64,
    shift_x_pt: f64,
    shift_y_pt: f64,
) -> String {
    let w = path_w_emu.max(1);
    let h = path_h_emu.max(1);
    let mut path_cmds = String::new();

    for seg in segments {
        match *seg {
            CurveSegmentIr::MoveTo { x_pt, y_pt } => {
                let x = pt_to_emu(x_pt + shift_x_pt).max(0);
                let y = pt_to_emu(y_pt + shift_y_pt).max(0);
                path_cmds.push_str(&format!(r#"<a:moveTo><a:pt x="{x}" y="{y}"/></a:moveTo>"#));
            }
            CurveSegmentIr::LineTo { x_pt, y_pt } => {
                let x = pt_to_emu(x_pt + shift_x_pt).max(0);
                let y = pt_to_emu(y_pt + shift_y_pt).max(0);
                path_cmds.push_str(&format!(r#"<a:lnTo><a:pt x="{x}" y="{y}"/></a:lnTo>"#));
            }
            CurveSegmentIr::CubicTo {
                cx1_pt,
                cy1_pt,
                cx2_pt,
                cy2_pt,
                x_pt,
                y_pt,
            } => {
                let x1 = pt_to_emu(cx1_pt + shift_x_pt).max(0);
                let y1 = pt_to_emu(cy1_pt + shift_y_pt).max(0);
                let x2 = pt_to_emu(cx2_pt + shift_x_pt).max(0);
                let y2 = pt_to_emu(cy2_pt + shift_y_pt).max(0);
                let x = pt_to_emu(x_pt + shift_x_pt).max(0);
                let y = pt_to_emu(y_pt + shift_y_pt).max(0);
                path_cmds.push_str(&format!(
                    r#"<a:cubicBezTo><a:pt x="{x1}" y="{y1}"/><a:pt x="{x2}" y="{y2}"/><a:pt x="{x}" y="{y}"/></a:cubicBezTo>"#
                ));
            }
            CurveSegmentIr::Close => {
                path_cmds.push_str("<a:close/>");
            }
        }
    }

    format!(
        concat!(
            r#"<a:custGeom>"#,
            r#"<a:avLst/>"#,
            r#"<a:gdLst/>"#,
            r#"<a:ahLst/>"#,
            r#"<a:cxnLst/>"#,
            r#"<a:rect l="l" t="t" r="r" b="b"/>"#,
            r#"<a:pathLst>"#,
            r#"<a:path w="{w}" h="{h}">{cmds}</a:path>"#,
            r#"</a:pathLst>"#,
            r#"</a:custGeom>"#
        ),
        w = w,
        h = h,
        cmds = path_cmds
    )
}
