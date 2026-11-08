//! Native Microsoft Word DrawingML (`<wps:wsp>` and `<wpg:wgp>`) generator for:
//! 1. Typst shapes (`rect`, `square`, `circle`, `ellipse`, `line`, `polygon`, `path`, `curve`),
//!    including shapes containing rich text/formulas inside `<wps:txbx><w:txbxContent>`.
//! 2. CeTZ vector canvases (`cetz.canvas`) converted into native WordprocessingGroup
//!    (`<wpg:wgp>`) vector graphics with custom Bézier/line paths (`<a:custGeom>`) and
//!    positioned text/formula labels.

use std::sync::atomic::{AtomicU32, Ordering};

use crate::ir::{
    BlockNode, CetzCanvasIr, CetzLabelDrawableIr, CetzPathDrawableIr, CurveSegmentIr,
    DashPatternIr, HAlignIr, InlineNode, LineCapIr, LineJoinIr, ScriptKindIr, ShapeIr, ShapeKindIr,
    StrokeIr, TextRunIr,
};

static NEXT_DOC_PR_ID: AtomicU32 = AtomicU32::new(1000);

fn next_drawing_id() -> u32 {
    NEXT_DOC_PR_ID.fetch_add(1, Ordering::Relaxed)
}

/// Convert points (`pt`, 1/72 inch) to DrawingML EMUs (`1 pt = 12700 EMU`).
#[inline]
pub fn pt_to_emu(pt: f64) -> i64 {
    if !pt.is_finite() {
        return 0;
    }
    (pt * 12700.0).round() as i64
}

/// Convert points (`pt`, 1/72 inch) to OpenXML twentieths of a point (`dxa` / twips, `1 pt = 20 dxa`).
#[inline]
pub fn pt_to_dxa(pt: f64) -> i64 {
    if !pt.is_finite() {
        return 0;
    }
    (pt * 20.0).round() as i64
}

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

fn effective_shape_dimensions(shape: &ShapeIr) -> (f64, f64) {
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

fn build_wsp_xml(
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
        let inner_paras = render_textbox_blocks(&shape.body);
        format!(
            concat!(
                r#"<wps:txbx><w:txbxContent>{paras}</w:txbxContent></wps:txbx>"#,
                r#"<wps:bodyPr rot="0" vert="horz" wrap="square" "#,
                r#"lIns="{l}" tIns="{t}" rIns="{r}" bIns="{b}" anchor="ctr">"#,
                r#"<a:spAutoFit/>"#,
                r#"</wps:bodyPr>"#
            ),
            paras = inner_paras,
            l = l_ins,
            t = t_ins,
            r = r_ins,
            b = b_ins,
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

fn build_cetz_path_wsp(drawable: &CetzPathDrawableIr) -> String {
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

fn build_cetz_label_wsp(label: &CetzLabelDrawableIr) -> String {
    let sp_id = next_drawing_id();
    // Give the label textbox generous width/height so Word never wraps single-line labels
    let w_pt = (label.width_pt + 16.0).max(36.0);
    let h_pt = (label.height_pt + 8.0).max(18.0);
    let off_x = pt_to_emu((label.x_pt - 2.0).max(0.0));
    let off_y = pt_to_emu((label.y_pt - 2.0).max(0.0));
    let cx = pt_to_emu(w_pt);
    let cy = pt_to_emu(h_pt);

    let mut xfrm_attrs = String::new();
    if label.rotation_deg.abs() > 0.01 {
        let rot = (label.rotation_deg * 60000.0).round() as i64;
        xfrm_attrs.push_str(&format!(r#" rot="{rot}""#));
    }

    let para_xml = render_inline_nodes_as_txbx_para(&label.content, HAlignIr::Left);

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

fn compute_segments_bbox(segments: &[CurveSegmentIr]) -> (f64, f64, f64, f64) {
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

fn build_cust_geom_xml(
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

fn build_ln_xml(stroke: Option<&StrokeIr>) -> String {
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
        LineJoinIr::Miter => r#"<a:miter lim="800000"/>"#,
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

fn render_textbox_blocks(blocks: &[BlockNode]) -> String {
    let mut out = String::new();
    for block in blocks {
        match block {
            BlockNode::Paragraph { content, style, .. } => {
                out.push_str(&render_inline_nodes_as_txbx_para(content, style.align));
            }
            BlockNode::Heading { content, align, .. } | BlockNode::Title { content, align } => {
                out.push_str(&render_inline_nodes_as_txbx_para(content, *align));
            }
            BlockNode::EquationBlock(eq) => {
                out.push_str(&format!(
                    r#"<w:p><w:pPr><w:jc w:val="center"/><w:spacing w:before="0" w:after="0"/></w:pPr>{}</w:p>"#,
                    eq.omml_xml
                ));
            }
            BlockNode::List { items, .. } => {
                for item in items {
                    out.push_str(&render_textbox_blocks(&item.blocks));
                }
            }
            BlockNode::Enum { items, .. } => {
                for item in items {
                    out.push_str(&render_textbox_blocks(&item.blocks));
                }
            }
            BlockNode::UnsupportedBlock(u) => {
                let run = TextRunIr {
                    text: u.display_badge(),
                    color: Some("B91C1C".to_string()),
                    bold: true,
                    ..Default::default()
                };
                out.push_str(&render_inline_nodes_as_txbx_para(
                    &[InlineNode::Text(run)],
                    HAlignIr::Left,
                ));
            }
            _ => {}
        }
    }
    if out.is_empty() {
        out.push_str(r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0"/></w:pPr></w:p>"#);
    }
    out
}

fn render_inline_nodes_as_txbx_para(inlines: &[InlineNode], align: HAlignIr) -> String {
    let jc = match align {
        HAlignIr::Left => "left",
        HAlignIr::Center => "center",
        HAlignIr::Right => "right",
        HAlignIr::Justify => "both",
    };
    let mut runs_xml = String::new();
    render_inline_nodes_to_wml(inlines, &mut runs_xml);
    format!(
        r#"<w:p><w:pPr><w:jc w:val="{jc}"/><w:spacing w:before="0" w:after="0"/></w:pPr>{runs_xml}</w:p>"#
    )
}

fn render_inline_nodes_to_wml(inlines: &[InlineNode], out: &mut String) {
    for node in inlines {
        match node {
            InlineNode::Text(run) => {
                out.push_str(&render_text_run_wml(run));
            }
            InlineNode::EquationInline(eq) => {
                out.push_str(&eq.omml_xml);
            }
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. } => {
                render_inline_nodes_to_wml(content, out);
            }
            InlineNode::LineBreak => {
                out.push_str("<w:r><w:br/></w:r>");
            }
            InlineNode::Tab => {
                out.push_str("<w:r><w:tab/></w:r>");
            }
            InlineNode::HorizontalSpace { .. } => {
                out.push_str(r#"<w:r><w:t xml:space="preserve"> </w:t></w:r>"#);
            }
            InlineNode::UnsupportedInline(u) => {
                let badge_run = TextRunIr {
                    text: u.display_badge(),
                    color: Some("B91C1C".to_string()),
                    bold: true,
                    ..Default::default()
                };
                out.push_str(&render_text_run_wml(&badge_run));
            }
            _ => {}
        }
    }
}

fn render_text_run_wml(run: &TextRunIr) -> String {
    if run.text.is_empty() {
        return String::new();
    }
    let mut rpr = String::new();
    if let Some(ref font) = run.font_family {
        let esc = escape_xml(font);
        rpr.push_str(&format!(
            r#"<w:rFonts w:ascii="{esc}" w:hAnsi="{esc}" w:eastAsia="{esc}" w:cs="{esc}"/>"#
        ));
    }
    if run.bold {
        rpr.push_str("<w:b/><w:bCs/>");
    }
    if run.italic {
        rpr.push_str("<w:i/><w:iCs/>");
    }
    if run.smallcaps {
        rpr.push_str("<w:smallCaps/>");
    }
    if run.underline.is_some() {
        rpr.push_str(r#"<w:u w:val="single"/>"#);
    }
    if run.strike.is_some() {
        rpr.push_str("<w:strike/>");
    }
    if let Some(ref color) = run.color {
        rpr.push_str(&format!(r#"<w:color w:val="{color}"/>"#));
    }
    if let Some(ref hl) = run.highlight {
        rpr.push_str(&format!(
            r#"<w:shd w:val="clear" w:color="auto" w:fill="{hl}"/>"#
        ));
    }
    if let Some(sz_pt) = run.font_size_pt {
        let half_pts = (sz_pt * 2.0).round() as u32;
        if half_pts > 0 {
            rpr.push_str(&format!(
                r#"<w:sz w:val="{half_pts}"/><w:szCs w:val="{half_pts}"/>"#
            ));
        }
    }
    match run.script {
        ScriptKindIr::Normal => {}
        ScriptKindIr::Sub => rpr.push_str(r#"<w:vertAlign w:val="subscript"/>"#),
        ScriptKindIr::Super => rpr.push_str(r#"<w:vertAlign w:val="superscript"/>"#),
    }

    let rpr_block = if rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{rpr}</w:rPr>")
    };

    format!(
        r#"<w:r>{rpr_block}<w:t xml:space="preserve">{}</w:t></w:r>"#,
        escape_xml(&run.text)
    )
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
