//! Native geometric shapes (`rect`, `square`, `circle`, `ellipse`, `line`, `polygon`,
//! `curve`) and geometric transforms (`rotate`, `scale`, `move`) -> `ShapeIr`.

pub(crate) mod raw;

use typst::foundations::{Content, Smart, StyleChain};
use typst::layout::{Point, RotateElem, ScaleAmount, ScaleElem, Size};
use typst::syntax::Span;
use typst::visualize::{
    CircleElem, CurveElem, CurveItem, EllipseElem, FillRule, Geometry, LineElem, Paint,
    PolygonElem, RectElem, Shape, SquareElem,
};

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    BlockNode, BordersIr, DashPatternIr, HAlignIr, ShapeIr, ShapeKindIr, SidesPtIr, StrokeIr,
};

impl<'a> ExtractorContext<'a> {
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
                    placed_offset_pt: None,
                });
            }
        }

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
            let mut segments = Vec::new();
            for (spos, sh, _) in &shape_items {
                Self::geometry_to_curve_segments(
                    &sh.geometry,
                    *spos,
                    bbox_x,
                    bbox_y,
                    &mut segments,
                );
            }
            (
                ShapeKindIr::CustomPath { segments },
                None,
                SidesPtIr::default(),
            )
        }
        ;

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
            placed_offset_pt: None,
        })
    }
}
