//! CeTZ canvases -> `<wpg:wgp>` group of path shapes and positioned text/formula labels.

use crate::docx::drawingml::geometry::{build_cust_geom_xml, compute_segments_bbox};
use crate::docx::drawingml::line::build_ln_xml;
use crate::docx::drawingml::next_drawing_id;
use crate::docx::drawingml::shape::build_wsp_xml;
use crate::docx::drawingml::textbox::render_inline_nodes_as_txbx_para;
use crate::ir::{
    CetzCanvasIr, CetzLabelDrawableIr, CetzPathDrawableIr, HAlignIr, ShapeIr, ShapeKindIr,
};
use crate::units::pt_to_emu;

/// Build a `<w:r><w:drawing>...</w:drawing></w:r>` XML fragment for a `CetzCanvasIr`
/// using a native WordprocessingGroup (`<wpg:wgp>`).
pub fn cetz_canvas_to_drawingml_run(canvas: &CetzCanvasIr) -> String {
    let group_id = next_drawing_id();
    let cx = pt_to_emu(canvas.width_pt.max(10.0)).max(12700);
    let cy = pt_to_emu(canvas.height_pt.max(10.0)).max(12700);

    let mut children_xml = String::new();

    // 1. Optional canvas background rectangle
    if let Some(ref bg_hex) = canvas.background {
        let bg_shape = ShapeIr {
            kind: ShapeKindIr::Rect,
            width_pt: canvas.width_pt,
            height_pt: canvas.height_pt,
            fill: Some(bg_hex.clone()),
            fill_rule_even_odd: false,
            stroke: None,
            borders: Default::default(),
            corner_radius_pt: None,
            inset: Default::default(),
            rotation_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            body: Vec::new(),
            align: HAlignIr::Left,
        };
        children_xml.push_str(&build_wsp_xml(&bg_shape, Some((0, 0)), cx, cy));
    }

    // 2. Vector path/curve drawables
    for path_drawable in &canvas.paths {
        children_xml.push_str(&build_cetz_path_wsp(path_drawable));
    }

    // 3. Positioned text/formula label drawables
    for label_drawable in &canvas.labels {
        children_xml.push_str(&build_cetz_label_wsp(label_drawable));
    }

    // Ensure `<wpg:wgp>` has at least one child shape so Word never complains about an empty group
    if children_xml.is_empty() {
        let empty_shape = ShapeIr {
            kind: ShapeKindIr::Rect,
            width_pt: canvas.width_pt.max(1.0),
            height_pt: canvas.height_pt.max(1.0),
            fill: None,
            fill_rule_even_odd: false,
            stroke: None,
            borders: Default::default(),
            corner_radius_pt: None,
            inset: Default::default(),
            rotation_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            body: Vec::new(),
            align: HAlignIr::Left,
        };
        children_xml.push_str(&build_wsp_xml(&empty_shape, Some((0, 0)), cx, cy));
    }

    format!(
        concat!(
            r#"<w:r>"#,
            r#"<w:drawing>"#,
            r#"<wp:inline distT="0" distB="0" distL="0" distR="0" "#,
            r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
            r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
            r#"xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" "#,
            r#"xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" "#,
            r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
            r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">"#,
            r#"<wp:extent cx="{cx}" cy="{cy}"/>"#,
            r#"<wp:effectExtent l="12700" t="12700" r="12700" b="12700"/>"#,
            r#"<wp:docPr id="{id}" name="CeTZCanvas_{id}"/>"#,
            r#"<wp:cNvGraphicFramePr/>"#,
            r#"<a:graphic>"#,
            r#"<a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup">"#,
            r#"<wpg:wgp>"#,
            r#"<wpg:cNvGrpSpPr/>"#,
            r#"<wpg:grpSpPr>"#,
            r#"<a:xfrm>"#,
            r#"<a:off x="0" y="0"/>"#,
            r#"<a:ext cx="{cx}" cy="{cy}"/>"#,
            r#"<a:chOff x="0" y="0"/>"#,
            r#"<a:chExt cx="{cx}" cy="{cy}"/>"#,
            r#"</a:xfrm>"#,
            r#"</wpg:grpSpPr>"#,
            r#"{children}"#,
            r#"</wpg:wgp>"#,
            r#"</a:graphicData>"#,
            r#"</a:graphic>"#,
            r#"</wp:inline>"#,
            r#"</w:drawing>"#,
            r#"</w:r>"#
        ),
        cx = cx,
        cy = cy,
        id = group_id,
        children = children_xml
    )
}

pub(crate) fn build_cetz_path_wsp(drawable: &CetzPathDrawableIr) -> String {
    let sp_id = next_drawing_id();

    // Compute exact bounding box of segments so negative relative coordinates inside
    // a CeTZ path are shifted into non-negative local path coordinates `[0..w_emu, 0..h_emu]`.
    let (min_x_pt, min_y_pt, max_x_pt, max_y_pt) = compute_segments_bbox(&drawable.segments);
    let local_origin_x_pt = min_x_pt.min(0.0);
    let local_origin_y_pt = min_y_pt.min(0.0);

    let eff_x_pt = drawable.x_pt + local_origin_x_pt;
    let eff_y_pt = drawable.y_pt + local_origin_y_pt;
    let eff_w_pt = (max_x_pt - local_origin_x_pt)
        .max(drawable.width_pt)
        .max(0.5);
    let eff_h_pt = (max_y_pt - local_origin_y_pt)
        .max(drawable.height_pt)
        .max(0.5);

    let off_x = pt_to_emu(eff_x_pt).max(0);
    let off_y = pt_to_emu(eff_y_pt).max(0);
    let cx = pt_to_emu(eff_w_pt).max(1270);
    let cy = pt_to_emu(eff_h_pt).max(1270);

    let geom_xml = build_cust_geom_xml(
        &drawable.segments,
        cx,
        cy,
        -local_origin_x_pt,
        -local_origin_y_pt,
    );

    let fill_xml = if let Some(ref hex) = drawable.fill {
        format!(r#"<a:solidFill><a:srgbClr val="{hex}"/></a:solidFill>"#)
    } else {
        r#"<a:noFill/>"#.to_string()
    };

    let ln_xml = build_ln_xml(drawable.stroke.as_ref());

    format!(
        concat!(
            r#"<wps:wsp>"#,
            r#"<wps:cNvPr id="{id}" name="CeTZPath_{id}"/>"#,
            r#"<wps:cNvSpPr/>"#,
            r#"<wps:spPr>"#,
            r#"<a:xfrm>"#,
            r#"<a:off x="{off_x}" y="{off_y}"/>"#,
            r#"<a:ext cx="{cx}" cy="{cy}"/>"#,
            r#"</a:xfrm>"#,
            r#"{geom}"#,
            r#"{fill}"#,
            r#"{ln}"#,
            r#"</wps:spPr>"#,
            r#"<wps:bodyPr/>"#,
            r#"</wps:wsp>"#
        ),
        id = sp_id,
        off_x = off_x,
        off_y = off_y,
        cx = cx,
        cy = cy,
        geom = geom_xml,
        fill = fill_xml,
        ln = ln_xml,
    )
}

pub(crate) fn build_cetz_label_wsp(label: &CetzLabelDrawableIr) -> String {
    let sp_id = next_drawing_id();
    // Give the label textbox generous width/height so Word never wraps single-line labels
    let w_pt = (label.width_pt * 2.0 + 48.0).max(72.0);
    let h_pt = (label.height_pt + 14.0).max(28.0);
    let off_x = pt_to_emu((label.x_pt - 2.0).max(0.0));
    let off_y = pt_to_emu((label.y_pt - 2.0).max(0.0));
    let cx = pt_to_emu(w_pt);
    let cy = pt_to_emu(h_pt);

    let mut xfrm_attrs = String::new();
    if label.rotation_deg.abs() > 0.01 {
        let rot = (label.rotation_deg * 60000.0).round() as i64;
        xfrm_attrs.push_str(&format!(r#" rot="{rot}""#));
    }

    let nobreak_content: Vec<crate::ir::InlineNode> = label
        .content
        .iter()
        .map(|node| match node {
            crate::ir::InlineNode::Text(run) => {
                let mut r = run.clone();
                r.text = r.text.replace(' ', "\u{00A0}");
                crate::ir::InlineNode::Text(r)
            }
            other => other.clone(),
        })
        .collect();
    let para_xml = render_inline_nodes_as_txbx_para(&nobreak_content, HAlignIr::Left);

    format!(
        concat!(
            r#"<wps:wsp>"#,
            r#"<wps:cNvPr id="{id}" name="CeTZLabel_{id}"/>"#,
            r#"<wps:cNvSpPr txBox="1"/>"#,
            r#"<wps:spPr>"#,
            r#"<a:xfrm{xfrm_attrs}>"#,
            r#"<a:off x="{off_x}" y="{off_y}"/>"#,
            r#"<a:ext cx="{cx}" cy="{cy}"/>"#,
            r#"</a:xfrm>"#,
            r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#,
            r#"<a:noFill/>"#,
            r#"<a:ln><a:noFill/></a:ln>"#,
            r#"</wps:spPr>"#,
            r#"<wps:txbx><w:txbxContent>{para}</w:txbxContent></wps:txbx>"#,
            r#"<wps:bodyPr rot="0" vert="horz" wrap="none" lIns="0" tIns="0" rIns="0" bIns="0" anchor="t">"#,
            r#"<a:noAutofit/>"#,
            r#"</wps:bodyPr>"#,
            r#"</wps:wsp>"#
        ),
        id = sp_id,
        xfrm_attrs = xfrm_attrs,
        off_x = off_x,
        off_y = off_y,
        cx = cx,
        cy = cy,
        para = para_xml,
    )
}
