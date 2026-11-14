//! Raw `Shape` item extraction and node partitioning helpers.

use typst::layout::{Point, Size};
use typst::pdf::ArtifactElem;
use typst::syntax::Span;
use typst::visualize::{FillRule, Geometry, Paint, Shape};

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{BlockNode, BordersIr, HAlignIr, SemanticRoleIr, ShapeIr, ShapeKindIr, SidesPtIr};

impl<'a> ExtractorContext<'a> {
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

        let id = self.alloc_node_id(span, None, Some(SemanticRoleIr::Shape));
        Some(ShapeIr {
            id,
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
            placed_offset_pt: None,
            baseline_shift_pt: None,
        })
    }
}
