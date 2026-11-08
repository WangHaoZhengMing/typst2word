//! Native shapes (`rect`, `square`, `circle`, `ellipse`, `line`, `polygon`, `path`,
//! `curve`), transformed shapes and styled `block`/`box` containers -> `ShapeIr`.

use typst::foundations::{Content, Smart, StyleChain};
use typst::layout::{
    GridCell, GroupItem, Point, RotateElem, ScaleAmount, ScaleElem, Size,
};
use typst::model::TableCell;
use typst::pdf::ArtifactElem;
use typst::syntax::Span;
use typst::text::RawElem;
use typst::visualize::{
    CircleElem, CurveElem, CurveItem, EllipseElem, FillRule, Geometry, LineElem, Paint,
    PolygonElem, RectElem, Shape, SquareElem,
};

use crate::color::paint_to_hex;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{
    BlockNode, BordersIr, DashPatternIr, HAlignIr, ShapeIr, ShapeKindIr, SidesPtIr, StrokeIr,
    TableCellIr, TableIr, TableRowIr, VAlignIr,
};

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Native Shapes (`RectElem`, `SquareElem`, `CircleElem`, `EllipseElem`,
    // `LineElem`, `PolygonElem`, `CurveElem`) & CeTZ Canvases
    // =========================================================================
    pub(crate) fn is_native_shape_elem(elem: &Content) -> bool {
        elem.is::<RectElem>()
            || elem.is::<SquareElem>()
            || elem.is::<CircleElem>()
            || elem.is::<EllipseElem>()
            || elem.is::<PolygonElem>()
            || elem.is::<CurveElem>()
            || elem.is::<LineElem>()
    }

    pub(crate) fn try_extract_transformed_shape(
        &mut self,
        elem: &Content,
        pos: Point,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Option<ShapeIr> {
        let mut rot_deg = 0.0;
        let mut scale = (1.0, 1.0);

        if let Some(rot) = elem.to_packed::<RotateElem>() {
            rot_deg = rot.angle.get(StyleChain::default()).to_deg();
        }
        if let Some(sc) = elem.to_packed::<ScaleElem>() {
            if let Smart::Custom(ScaleAmount::Ratio(rx)) = sc.x.get(StyleChain::default()) {
                scale.0 = rx.get();
            }
            if let Smart::Custom(ScaleAmount::Ratio(ry)) = sc.y.get(StyleChain::default()) {
                scale.1 = ry.get();
            }
        }

        // Look for an inner shape element inside `children`.
        for child in children {
            match child {
                TreeNode::Element {
                    elem: inner_elem,
                    pos: inner_pos,
                    children: inner_children,
                    ..
                } if Self::is_native_shape_elem(inner_elem) => {
                    return self.extract_native_shape_elem(
                        inner_elem,
                        *inner_pos,
                        inner_children,
                        list_level,
                        inherited_align,
                        rot_deg,
                        scale,
                    );
                }
                TreeNode::Group {
                    children: gchildren,
                    ..
                } => {
                    for gchild in gchildren {
                        if let TreeNode::Element {
                            elem: inner_elem,
                            pos: inner_pos,
                            children: inner_children,
                            ..
                        } = gchild
                            && Self::is_native_shape_elem(inner_elem)
                        {
                            return self.extract_native_shape_elem(
                                inner_elem,
                                *inner_pos,
                                inner_children,
                                list_level,
                                inherited_align,
                                rot_deg,
                                scale,
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        let _ = pos;
        None
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn extract_native_shape_elem(
        &mut self,
        elem: &Content,
        _pos: Point,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
        rotation_deg: f64,
        scale: (f64, f64),
    ) -> Option<ShapeIr> {
        // Collect all direct `Shape` items (background fill & border strokes) and any inner content nodes.
        let mut shape_items: Vec<(Point, Shape, Span)> = Vec::new();
        let mut body_tree_nodes: Vec<TreeNode> = Vec::new();
        let mut group_size: Option<Size> = None;

        Self::partition_shape_and_body_nodes(
            children,
            Point::zero(),
            &mut shape_items,
            &mut body_tree_nodes,
            &mut group_size,
        );

        let default_body_align = if elem.is::<RectElem>() || elem.is::<SquareElem>() {
            HAlignIr::Left
        } else {
            HAlignIr::Center
        };
        let body_blocks =
            self.extract_blocks_from_nodes(&body_tree_nodes, list_level, default_body_align);

        // Check for unsupported tiling paint on the element or its shapes.
        for (_, sh, sp) in &shape_items {
            if let Some(Paint::Tiling(_)) = &sh.fill {
                let unsup = self.record_unsupported_paint(elem.elem().name(), *sp);
                let mut body = body_blocks;
                body.insert(0, BlockNode::UnsupportedBlock(unsup));
                return Some(ShapeIr {
                    kind: ShapeKindIr::Rect,
                    width_pt: group_size.map(|s| s.x.to_pt()).unwrap_or(100.0),
                    height_pt: group_size.map(|s| s.y.to_pt()).unwrap_or(36.0),
                    fill: Some("FFF5F5".to_string()),
                    fill_rule_even_odd: false,
                    stroke: Some(StrokeIr {
                        color: "D32F2F".to_string(),
                        thickness_pt: 1.0,
                        dash: DashPatternIr::Dashed,
                        ..Default::default()
                    }),
                    borders: BordersIr::default(),
                    corner_radius_pt: Some(4.0),
                    inset: SidesPtIr::default(),
                    rotation_deg,
                    scale_x: scale.0,
                    scale_y: scale.1,
                    body,
                    align: inherited_align,
                });
            }
        }

        // Determine shape dimensions from `group_size` or `shape_items` bounding box.
        let (bbox_x, bbox_y, bbox_w, bbox_h) = Self::shapes_bbox(&shape_items);
        let width_pt = group_size
            .map(|s| s.x.to_pt())
            .unwrap_or(bbox_w)
            .max(bbox_w);
        let height_pt = group_size
            .map(|s| s.y.to_pt())
            .unwrap_or(bbox_h)
            .max(bbox_h);

        let fill = shape_items
            .iter()
            .find_map(|(_, s, _)| s.fill.as_ref().and_then(paint_to_hex));
        let fill_rule_even_odd = shape_items
            .first()
            .is_some_and(|(_, s, _)| s.fill_rule == FillRule::EvenOdd);
        let stroke = shape_items
            .iter()
            .find_map(|(_, s, _)| s.stroke.as_ref().map(Self::fixed_stroke_to_ir));

        // Classify shape kind from the Typst element type.
        let (kind, corner_radius_pt, inset) = if let Some(rect) = elem.to_packed::<RectElem>() {
            let rad = Self::corners_radius_to_pt(
                &rect.radius.get(StyleChain::default()),
                width_pt.min(height_pt),
            );
            let ins =
                Self::sides_rel_length_to_pt(&Smart::Custom(rect.inset.get(StyleChain::default())));
            (ShapeKindIr::Rect, rad, ins)
        } else if let Some(sq) = elem.to_packed::<SquareElem>() {
            let rad = Self::corners_radius_to_pt(
                &sq.radius.get(StyleChain::default()),
                width_pt.min(height_pt),
            );
            let ins =
                Self::sides_rel_length_to_pt(&Smart::Custom(sq.inset.get(StyleChain::default())));
            (ShapeKindIr::Rect, rad, ins)
        } else if let Some(cir) = elem.to_packed::<CircleElem>() {
            let ins =
                Self::sides_rel_length_to_pt(&Smart::Custom(cir.inset.get(StyleChain::default())));
            (ShapeKindIr::Ellipse, None, ins)
        } else if let Some(ell) = elem.to_packed::<EllipseElem>() {
            let ins =
                Self::sides_rel_length_to_pt(&Smart::Custom(ell.inset.get(StyleChain::default())));
            (ShapeKindIr::Ellipse, None, ins)
        } else if elem.is::<LineElem>() {
            if let Some((
                lpos,
                Shape {
                    geometry: Geometry::Line(end),
                    ..
                },
                _,
            )) = shape_items.first()
            {
                let x1 = lpos.x.to_pt() - bbox_x;
                let y1 = lpos.y.to_pt() - bbox_y;
                let x2 = x1 + end.x.to_pt();
                let y2 = y1 + end.y.to_pt();
                (
                    ShapeKindIr::Line {
                        x1_pt: x1,
                        y1_pt: y1,
                        x2_pt: x2,
                        y2_pt: y2,
                    },
                    None,
                    SidesPtIr::default(),
                )
            } else {
                (
                    ShapeKindIr::Line {
                        x1_pt: 0.0,
                        y1_pt: 0.0,
                        x2_pt: width_pt.max(30.0),
                        y2_pt: 0.0,
                    },
                    None,
                    SidesPtIr::default(),
                )
            }
        } else if elem.is::<PolygonElem>() {
            let mut pts = Vec::new();
            if let Some((
                spos,
                Shape {
                    geometry: Geometry::Curve(curve),
                    ..
                },
                _,
            )) = shape_items.first()
            {
                for item in &curve.0 {
                    match item {
                        CurveItem::Move(p) | CurveItem::Line(p) => {
                            pts.push((
                                (spos.x + p.x).to_pt() - bbox_x,
                                (spos.y + p.y).to_pt() - bbox_y,
                            ));
                        }
                        _ => {}
                    }
                }
            }
            (
                ShapeKindIr::Polygon { points_pt: pts },
                None,
                SidesPtIr::default(),
            )
        } else {
            // `CurveElem` (Typst 0.15 removed the deprecated `path`)
            let mut segments = Vec::new();
            let off_x = bbox_x;
            let off_y = bbox_y;
            for (spos, sh, _) in &shape_items {
                Self::geometry_to_curve_segments(&sh.geometry, *spos, off_x, off_y, &mut segments);
            }
            (
                ShapeKindIr::CustomPath { segments },
                None,
                SidesPtIr::default(),
            )
        };

        Some(ShapeIr {
            kind,
            width_pt: width_pt.max(1.0),
            height_pt: height_pt.max(1.0),
            fill,
            fill_rule_even_odd,
            stroke: stroke.clone(),
            borders: BordersIr::splat(stroke),
            corner_radius_pt,
            inset,
            rotation_deg,
            scale_x: scale.0,
            scale_y: scale.1,
            body: body_blocks,
            align: inherited_align,
        })
    }

    pub(crate) fn partition_shape_and_body_nodes(
        nodes: &[TreeNode],
        offset: Point,
        shape_items: &mut Vec<(Point, Shape, Span)>,
        body_nodes: &mut Vec<TreeNode>,
        group_size: &mut Option<Size>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Shape { shape, span, pos } => {
                    shape_items.push((offset + *pos, shape.clone(), *span));
                }
                TreeNode::Element {
                    elem,
                    children,
                    pos,
                    ..
                } => {
                    // `fill_and_stroke` wraps shape borders/backgrounds in `ArtifactElem(Other)`.
                    if elem.is::<ArtifactElem>() {
                        Self::partition_shape_and_body_nodes(
                            children,
                            offset + *pos,
                            shape_items,
                            body_nodes,
                            group_size,
                        );
                    } else {
                        body_nodes.push(node.clone());
                    }
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    if group_size.is_none() {
                        *group_size = Some(group.frame.size());
                    }
                    // If this group is the direct container of the shape (has background `Shape` items),
                    // partition its children; otherwise treat it as body content.
                    if children.iter().any(|c| {
                        matches!(c, TreeNode::Shape { .. })
                            || matches!(c, TreeNode::Element { elem, .. } if elem.is::<ArtifactElem>())
                    }) && shape_items.is_empty()
                    {
                        Self::partition_shape_and_body_nodes(
                            children,
                            offset + *pos,
                            shape_items,
                            body_nodes,
                            group_size,
                        );
                    } else {
                        body_nodes.push(node.clone());
                    }
                }
                other => {
                    body_nodes.push(other.clone());
                }
            }
        }
    }

    pub(crate) fn try_extract_styled_container_group(
        &mut self,
        group: &GroupItem,
        _pos: Point,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Option<BlockNode> {
        // Check if this group starts with background/border `Shape` items (e.g. `#block(fill: ..., stroke: ...)[...]`)
        // AND is not a raw table/grid cell or CeTZ canvas.
        if Self::is_direct_table_or_code_container(children) || Self::is_cetz_group_nodes(children)
        {
            return None;
        }

        let mut bg_shapes = Vec::new();
        let mut content_nodes = Vec::new();
        for child in children {
            match child {
                TreeNode::Shape { shape, span, pos } => {
                    bg_shapes.push((*pos, shape.clone(), *span));
                }
                TreeNode::Element {
                    elem,
                    children: achildren,
                    pos,
                    ..
                } if elem.is::<ArtifactElem>() => {
                    for ac in achildren {
                        if let TreeNode::Shape {
                            shape,
                            span,
                            pos: spos,
                        } = ac
                        {
                            bg_shapes.push((*pos + *spos, shape.clone(), *span));
                        }
                    }
                }
                other => content_nodes.push(other.clone()),
            }
        }

        if bg_shapes.is_empty() || content_nodes.is_empty() {
            return None;
        }

        let fill = bg_shapes
            .iter()
            .find_map(|(_, s, _)| s.fill.as_ref().and_then(paint_to_hex));
        let stroke = bg_shapes
            .iter()
            .find_map(|(_, s, _)| s.stroke.as_ref().map(Self::fixed_stroke_to_ir));
        if fill.is_none() && stroke.is_none() {
            return None;
        }

        let mut line_starts = Vec::new();
        Self::collect_line_left_edges(&content_nodes, Point::zero(), &mut line_starts);
        let inferred_pad = line_starts
            .first()
            .map(|(_, lx)| lx.clamp(4.0, 24.0))
            .unwrap_or(6.0);
        let v_pad = (inferred_pad * 0.65).clamp(4.0, 12.0);

        let prev_inset = self.container_inset_pt;
        self.container_inset_pt += 2.0 * inferred_pad;
        let body_blocks =
            self.extract_blocks_from_nodes(&content_nodes, list_level, inherited_align);
        self.container_inset_pt = prev_inset;

        if body_blocks.is_empty() {
            return None;
        }

        let size = group.frame.size();
        let width_pt = size.x.to_pt().max(40.0);
        let inset = SidesPtIr {
            top: v_pad,
            bottom: v_pad,
            left: inferred_pad,
            right: inferred_pad,
        };

        // If the styled `#block(stroke: ..., fill: ...)[...]` contains tables, lists, enums,
        // figures, rules, CeTZ canvases, or more than 3 blocks, represent it as a breakable
        // 1-cell bordered `BlockNode::Table` so all inner blocks render natively and flow
        // across columns/pages in Word!
        let needs_table_container = body_blocks.iter().any(|b| {
            !matches!(
                b,
                BlockNode::Paragraph { .. }
                    | BlockNode::Heading { .. }
                    | BlockNode::EquationBlock(_)
                    | BlockNode::CodeBlock { .. }
            )
        }) || body_blocks.len() > 3;

        if needs_table_container {
            self.report.tables_count += 1;
            return Some(BlockNode::Table(TableIr {
                is_semantic_table: false,
                col_widths_pt: vec![width_pt],
                row_heights_pt: vec![0.0],
                header_rows: 0,
                rows: vec![TableRowIr {
                    height_pt: None,
                    is_header: false,
                    cells: vec![TableCellIr {
                        x: 0,
                        y: 0,
                        colspan: 1,
                        rowspan: 1,
                        vmerge: None,
                        hmerged_covered: false,
                        fill,
                        borders: BordersIr::splat(stroke),
                        inset,
                        halign: inherited_align,
                        valign: VAlignIr::Top,
                        is_header_cell: false,
                        blocks: body_blocks,
                    }],
                }],
                align: inherited_align,
                left_indent_pt: 0.0,
                summary: Some("__styled_container__".to_string()),
            }));
        }

        let corner_radius_pt = bg_shapes.iter().find_map(|(_, s, _)| {
            if let Geometry::Curve(curve) = &s.geometry
                && let Some(CurveItem::Move(p)) = curve.0.first()
            {
                let rx = p.x.to_pt();
                let ry = p.y.to_pt();
                if rx > 0.5 && ry.abs() < 0.5 && rx <= size.x.to_pt() * 0.5 {
                    return Some(rx);
                } else if ry > 0.5 && rx.abs() < 0.5 && ry <= size.y.to_pt() * 0.5 {
                    return Some(ry);
                }
            }
            None
        });

        self.report.shapes_count += 1;
        Some(BlockNode::ShapeBlock(ShapeIr {
            kind: ShapeKindIr::Rect,
            width_pt,
            height_pt: size.y.to_pt().max(20.0),
            fill,
            fill_rule_even_odd: false,
            stroke: stroke.clone(),
            borders: BordersIr::splat(stroke),
            corner_radius_pt,
            inset,
            rotation_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            body: body_blocks,
            align: inherited_align,
        }))
    }

    pub(crate) fn is_direct_table_or_code_container(nodes: &[TreeNode]) -> bool {
        for node in nodes {
            if let TreeNode::Element { elem, .. } = node
                && (elem.is::<TableCell>() || elem.is::<GridCell>() || elem.is::<RawElem>())
            {
                return true;
            }
        }
        false
    }

    pub(crate) fn extract_raw_shape_item(
        &mut self,
        shape: &Shape,
        span: Span,
        pos: Point,
        body: Vec<BlockNode>,
        align: HAlignIr,
    ) -> Option<ShapeIr> {
        if let Some(Paint::Tiling(_)) = &shape.fill {
            self.record_unsupported_paint("shape.fill", span);
        }
        let fill = shape.fill.as_ref().and_then(paint_to_hex);
        let stroke = shape.stroke.as_ref().map(Self::fixed_stroke_to_ir);
        let bbox = shape.geometry.bbox(None);
        let width_pt = bbox.size().x.to_pt().abs().max(1.0);
        let height_pt = bbox.size().y.to_pt().abs().max(1.0);

        let kind = match &shape.geometry {
            Geometry::Rect(_) => ShapeKindIr::Rect,
            Geometry::Line(end) => ShapeKindIr::Line {
                x1_pt: pos.x.to_pt(),
                y1_pt: pos.y.to_pt(),
                x2_pt: pos.x.to_pt() + end.x.to_pt(),
                y2_pt: pos.y.to_pt() + end.y.to_pt(),
            },
            Geometry::Curve(_) => {
                let mut segments = Vec::new();
                Self::geometry_to_curve_segments(
                    &shape.geometry,
                    Point::zero(),
                    bbox.min.x.to_pt(),
                    bbox.min.y.to_pt(),
                    &mut segments,
                );
                ShapeKindIr::CustomPath { segments }
            }
        };

        Some(ShapeIr {
            kind,
            width_pt,
            height_pt,
            fill,
            fill_rule_even_odd: shape.fill_rule == FillRule::EvenOdd,
            stroke: stroke.clone(),
            borders: BordersIr::splat(stroke),
            corner_radius_pt: None,
            inset: SidesPtIr::default(),
            rotation_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            body,
            align,
        })
    }
}
