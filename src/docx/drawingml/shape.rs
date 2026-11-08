//! Typst shapes -> `<wps:wsp>` (preset or custom geometry, fill, outline, text box body).

use crate::docx::drawingml::geometry::build_cust_geom_xml;
use crate::docx::drawingml::line::build_ln_xml;
use crate::docx::drawingml::next_drawing_id;
use crate::docx::drawingml::textbox::render_textbox_blocks;
use crate::ir::{CurveSegmentIr, ShapeIr, ShapeKindIr};
use crate::units::pt_to_emu;

/// Build a `<w:r><w:drawing>...</w:drawing></w:r>` XML fragment for a `ShapeIr`.
pub fn shape_to_drawingml_run(shape: &ShapeIr) -> String {
    let draw_id = next_drawing_id();
    let (w_pt, h_pt) = effective_shape_dimensions(shape);
    let cx = pt_to_emu(w_pt * shape.scale_x.abs().max(0.05)).max(12700);
    let cy = pt_to_emu(h_pt * shape.scale_y.abs().max(0.05)).max(12700);

    // Pad extent margins slightly when stroke is present so borders aren't clipped
    let stroke_pad_emu = shape
        .stroke
        .as_ref()
        .or(shape.borders.top.as_ref())
        .map(|s| pt_to_emu(s.thickness_pt).max(6350))
        .unwrap_or(0);

    let wsp_xml = build_wsp_xml(shape, None, cx, cy);

    format!(
        concat!(
            r#"<w:r>"#,
            r#"<w:drawing>"#,
            r#"<wp:inline distT="0" distB="0" distL="0" distR="0" "#,
            r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
            r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
            r#"xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" "#,
            r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
            r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">"#,
            r#"<wp:extent cx="{cx}" cy="{cy}"/>"#,
            r#"<wp:effectExtent l="{pad}" t="{pad}" r="{pad}" b="{pad}"/>"#,
            r#"<wp:docPr id="{id}" name="TypstShape_{id}"/>"#,
            r#"<wp:cNvGraphicFramePr/>"#,
            r#"<a:graphic>"#,
            r#"<a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">"#,
            r#"{wsp}"#,
            r#"</a:graphicData>"#,
            r#"</a:graphic>"#,
            r#"</wp:inline>"#,
            r#"</w:drawing>"#,
            r#"</w:r>"#
        ),
        cx = cx,
        cy = cy,
        pad = stroke_pad_emu,
        id = draw_id,
        wsp = wsp_xml
    )
}

pub(crate) fn effective_shape_dimensions(shape: &ShapeIr) -> (f64, f64) {
    match &shape.kind {
        ShapeKindIr::Line {
            x1_pt,
            y1_pt,
            x2_pt,
            y2_pt,
        } => {
            let dx = (x2_pt - x1_pt).abs();
            let dy = (y2_pt - y1_pt).abs();
            (
                shape.width_pt.max(dx).max(1.0),
                shape.height_pt.max(dy).max(1.0),
            )
        }
        _ => (shape.width_pt.max(1.0), shape.height_pt.max(1.0)),
    }
}

pub(crate) fn build_wsp_xml(
    shape: &ShapeIr,
    group_offset_emu: Option<(i64, i64)>,
    cx: i64,
    cy: i64,
) -> String {
    let sp_id = next_drawing_id();
    let (off_x, off_y) = group_offset_emu.unwrap_or((0, 0));

    let mut xfrm_attrs = String::new();
    if shape.rotation_deg.abs() > 0.01 {
        let rot = (shape.rotation_deg * 60000.0).round() as i64;
        xfrm_attrs.push_str(&format!(r#" rot="{rot}""#));
    }

    let (flip_h, flip_v) = match &shape.kind {
        ShapeKindIr::Line {
            x1_pt,
            y1_pt,
            x2_pt,
            y2_pt,
        } => (x2_pt < x1_pt, y2_pt < y1_pt),
        _ => (shape.scale_x < 0.0, shape.scale_y < 0.0),
    };
    if flip_h {
        xfrm_attrs.push_str(r#" flipH="1""#);
    }
    if flip_v {
        xfrm_attrs.push_str(r#" flipV="1""#);
    }

    let is_line_connector = matches!(shape.kind, ShapeKindIr::Line { .. }) && shape.body.is_empty();
    let c_nv_tag = if is_line_connector {
        "<wps:cNvCnPr/>"
    } else {
        r#"<wps:cNvSpPr/>"#
    };

    let geom_xml = match &shape.kind {
        ShapeKindIr::Rect => {
            if let Some(radius) = shape.corner_radius_pt
                && radius > 0.1
            {
                let min_side = shape.width_pt.min(shape.height_pt).max(1.0);
                let adj = ((radius / min_side) * 100000.0).clamp(0.0, 50000.0).round() as i64;
                format!(
                    r#"<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val {adj}"/></a:avLst></a:prstGeom>"#
                )
            } else {
                r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#.to_string()
            }
        }
        ShapeKindIr::Ellipse => r#"<a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom>"#.to_string(),
        ShapeKindIr::Line { .. } => {
            // For horizontal or vertical lines, ensure extent along the zero axis is at least 0
            r#"<a:prstGeom prst="line"><a:avLst/></a:prstGeom>"#.to_string()
        }
        ShapeKindIr::Polygon { points_pt } => {
            let mut segs = Vec::with_capacity(points_pt.len() + 1);
            for (idx, &(x_pt, y_pt)) in points_pt.iter().enumerate() {
                if idx == 0 {
                    segs.push(CurveSegmentIr::MoveTo { x_pt, y_pt });
                } else {
                    segs.push(CurveSegmentIr::LineTo { x_pt, y_pt });
                }
            }
            segs.push(CurveSegmentIr::Close);
            build_cust_geom_xml(&segs, cx, cy, 0.0, 0.0)
        }
        ShapeKindIr::CustomPath { segments } => build_cust_geom_xml(segments, cx, cy, 0.0, 0.0),
    };

    let fill_xml = if is_line_connector {
        String::new()
    } else if let Some(ref hex) = shape.fill {
        format!(r#"<a:solidFill><a:srgbClr val="{hex}"/></a:solidFill>"#)
    } else {
        r#"<a:noFill/>"#.to_string()
    };

    let effective_stroke = shape
        .stroke
        .as_ref()
        .or(shape.borders.top.as_ref())
        .or(shape.borders.left.as_ref())
        .or(shape.borders.bottom.as_ref())
        .or(shape.borders.right.as_ref());
    let ln_xml = build_ln_xml(effective_stroke);

    let (line_cx, line_cy) = if matches!(shape.kind, ShapeKindIr::Line { .. }) {
        match &shape.kind {
            ShapeKindIr::Line {
                x1_pt,
                y1_pt,
                x2_pt,
                y2_pt,
            } => (
                pt_to_emu((x2_pt - x1_pt).abs()),
                pt_to_emu((y2_pt - y1_pt).abs()),
            ),
            _ => (cx, cy),
        }
    } else {
        (cx, cy)
    };

    let txbx_and_body_pr = if shape.body.is_empty() {
        r#"<wps:bodyPr/>"#.to_string()
    } else {
        let l_ins = pt_to_emu(shape.inset.left.max(0.0));
        let t_ins = pt_to_emu(shape.inset.top.max(0.0));
        let r_ins = pt_to_emu(shape.inset.right.max(0.0));
        let b_ins = pt_to_emu(shape.inset.bottom.max(0.0));
        let anchor = if matches!(shape.kind, ShapeKindIr::Rect) {
            "t"
        } else {
            "ctr"
        };
        let inner_paras = render_textbox_blocks(&shape.body);
        format!(
            concat!(
                r#"<wps:txbx><w:txbxContent>{paras}</w:txbxContent></wps:txbx>"#,
                r#"<wps:bodyPr rot="0" vert="horz" wrap="square" "#,
                r#"lIns="{l}" tIns="{t}" rIns="{r}" bIns="{b}" anchor="{anchor}">"#,
                r#"<a:spAutoFit/>"#,
                r#"</wps:bodyPr>"#
            ),
            paras = inner_paras,
            l = l_ins,
            t = t_ins,
            r = r_ins,
            b = b_ins,
            anchor = anchor,
        )
    };

    format!(
        concat!(
            r#"<wps:wsp>"#,
            r#"<wps:cNvPr id="{id}" name="Shape_{id}"/>"#,
            r#"{cnv}"#,
            r#"<wps:spPr>"#,
            r#"<a:xfrm{xfrm_attrs}>"#,
            r#"<a:off x="{off_x}" y="{off_y}"/>"#,
            r#"<a:ext cx="{cx}" cy="{cy}"/>"#,
            r#"</a:xfrm>"#,
            r#"{geom}"#,
            r#"{fill}"#,
            r#"{ln}"#,
            r#"</wps:spPr>"#,
            r#"{body}"#,
            r#"</wps:wsp>"#
        ),
        id = sp_id,
        cnv = c_nv_tag,
        xfrm_attrs = xfrm_attrs,
        off_x = off_x,
        off_y = off_y,
        cx = line_cx,
        cy = line_cy,
        geom = geom_xml,
        fill = fill_xml,
        ln = ln_xml,
        body = txbx_and_body_pr,
    )
}
