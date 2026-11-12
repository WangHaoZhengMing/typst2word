//! Typst geometry / sizes -> IR geometry (curve segments, bounding boxes, radii, insets).

use typst::foundations::Smart;
use typst::layout::{Corners, Length, Point, Rel, Sides, Size};
use typst::syntax::Span;
use typst::visualize::{CurveItem, Geometry, Shape};

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{CurveSegmentIr, SidesPtIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn geometry_to_curve_segments(
        geom: &Geometry,
        offset: Point,
        origin_x_pt: f64,
        origin_y_pt: f64,
        out: &mut Vec<CurveSegmentIr>,
    ) {
        let ox = offset.x.to_pt() - origin_x_pt;
        let oy = offset.y.to_pt() - origin_y_pt;
        match geom {
            Geometry::Line(end) => {
                out.push(CurveSegmentIr::MoveTo { x_pt: ox, y_pt: oy });
                out.push(CurveSegmentIr::LineTo {
                    x_pt: ox + end.x.to_pt(),
                    y_pt: oy + end.y.to_pt(),
                });
            }
            Geometry::Rect(sz) => {
                let w = sz.x.to_pt();
                let h = sz.y.to_pt();
                out.push(CurveSegmentIr::MoveTo { x_pt: ox, y_pt: oy });
                out.push(CurveSegmentIr::LineTo {
                    x_pt: ox + w,
                    y_pt: oy,
                });
                out.push(CurveSegmentIr::LineTo {
                    x_pt: ox + w,
                    y_pt: oy + h,
                });
                out.push(CurveSegmentIr::LineTo {
                    x_pt: ox,
                    y_pt: oy + h,
                });
                out.push(CurveSegmentIr::Close);
            }
            Geometry::Curve(curve) => {
                for item in &curve.0 {
                    match item {
                        CurveItem::Move(p) => out.push(CurveSegmentIr::MoveTo {
                            x_pt: ox + p.x.to_pt(),
                            y_pt: oy + p.y.to_pt(),
                        }),
                        CurveItem::Line(p) => out.push(CurveSegmentIr::LineTo {
                            x_pt: ox + p.x.to_pt(),
                            y_pt: oy + p.y.to_pt(),
                        }),
                        CurveItem::Cubic(c1, c2, p) => out.push(CurveSegmentIr::CubicTo {
                            cx1_pt: ox + c1.x.to_pt(),
                            cy1_pt: oy + c1.y.to_pt(),
                            cx2_pt: ox + c2.x.to_pt(),
                            cy2_pt: oy + c2.y.to_pt(),
                            x_pt: ox + p.x.to_pt(),
                            y_pt: oy + p.y.to_pt(),
                        }),
                        CurveItem::Close => out.push(CurveSegmentIr::Close),
                    }
                }
            }
        }
    }

    pub(crate) fn shapes_bbox(shapes: &[(Point, Shape, Span)]) -> (f64, f64, f64, f64) {
        if shapes.is_empty() {
            return (0.0, 0.0, 45.0, 30.0);
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (pos, sh, _) in shapes {
            let b = sh.geometry.bbox(None);
            min_x = min_x.min(pos.x.to_pt() + b.min.x.to_pt());
            min_y = min_y.min(pos.y.to_pt() + b.min.y.to_pt());
            max_x = max_x.max(pos.x.to_pt() + b.max.x.to_pt());
            max_y = max_y.max(pos.y.to_pt() + b.max.y.to_pt());
        }
        if !min_x.is_finite() || !max_x.is_finite() {
            return (0.0, 0.0, 45.0, 30.0);
        }
        (
            min_x,
            min_y,
            (max_x - min_x).max(1.0),
            (max_y - min_y).max(1.0),
        )
    }

    pub(crate) fn corners_radius_to_pt(
        corners: &Corners<Option<Rel<Length>>>,
        min_side_pt: f64,
    ) -> Option<f64> {
        let resolve_one = |opt: &Option<Rel<Length>>| -> f64 {
            match opt {
                Some(rel) => rel.abs.abs.to_pt() + rel.rel.get() * (min_side_pt * 0.5),
                None => 0.0,
            }
        };
        let r = resolve_one(&corners.top_left)
            .max(resolve_one(&corners.top_right))
            .max(resolve_one(&corners.bottom_left))
            .max(resolve_one(&corners.bottom_right));
        if r > 0.1 { Some(r) } else { None }
    }

    pub(crate) fn sides_rel_length_to_pt(sides: &Smart<Sides<Option<Rel<Length>>>>) -> SidesPtIr {
        Self::sides_rel_length_to_pt_with_default(sides, SidesPtIr::default())
    }

    pub(crate) fn sides_rel_length_to_pt_with_default(
        sides: &Smart<Sides<Option<Rel<Length>>>>,
        default: SidesPtIr,
    ) -> SidesPtIr {
        let Smart::Custom(s) = sides else {
            return default;
        };
        let get =
            |opt: &Option<Rel<Length>>, def: f64| opt.map_or(def, |r| r.abs.abs.to_pt().max(0.0));
        SidesPtIr {
            top: get(&s.top, default.top),
            bottom: get(&s.bottom, default.bottom),
            left: get(&s.left, default.left),
            right: get(&s.right, default.right),
        }
    }

    pub(crate) fn find_group_size(nodes: &[TreeNode]) -> Size {
        for node in nodes {
            if let TreeNode::Group { group, .. } = node {
                return group.frame.size();
            }
        }
        Size::zero()
    }

    pub(crate) fn estimate_nodes_bbox(nodes: &[TreeNode]) -> (f64, f64, f64, f64) {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        Self::accumulate_nodes_bbox(
            nodes,
            Point::zero(),
            &mut min_x,
            &mut min_y,
            &mut max_x,
            &mut max_y,
        );

        if !min_x.is_finite() || !max_x.is_finite() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        (
            min_x,
            min_y,
            (max_x - min_x).max(0.0),
            (max_y - min_y).max(0.0),
        )
    }

    pub(crate) fn accumulate_nodes_bbox(
        nodes: &[TreeNode],
        offset: Point,
        min_x: &mut f64,
        min_y: &mut f64,
        max_x: &mut f64,
        max_y: &mut f64,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos } => {
                    let x = (offset.x + pos.x).to_pt();
                    let y = (offset.y + pos.y - text.size).to_pt();
                    *min_x = min_x.min(x);
                    *min_y = min_y.min(y);
                    *max_x = max_x.max(x + text.width().to_pt());
                    *max_y = max_y.max(y + text.size.to_pt() * 1.25);
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    let gx = (offset.x + pos.x).to_pt();
                    let gy = (offset.y + pos.y).to_pt();
                    let gsz = group.frame.size();
                    if gsz.x.to_pt() > 0.0 && gsz.y.to_pt() > 0.0 {
                        *min_x = min_x.min(gx);
                        *min_y = min_y.min(gy);
                        *max_x = max_x.max(gx + gsz.x.to_pt());
                        *max_y = max_y.max(gy + gsz.y.to_pt());
                    } else {
                        Self::accumulate_nodes_bbox(
                            children,
                            offset + *pos,
                            min_x,
                            min_y,
                            max_x,
                            max_y,
                        );
                    }
                }
                TreeNode::Element { children, .. } => {
                    Self::accumulate_nodes_bbox(children, offset, min_x, min_y, max_x, max_y);
                }
                TreeNode::Image { size, pos, .. } => {
                    let x = (offset.x + pos.x).to_pt();
                    let y = (offset.y + pos.y).to_pt();
                    *min_x = min_x.min(x);
                    *min_y = min_y.min(y);
                    *max_x = max_x.max(x + size.x.to_pt());
                    *max_y = max_y.max(y + size.y.to_pt());
                }
                TreeNode::Shape { shape, pos, .. } => {
                    let b = shape.geometry.bbox(None);
                    let x = (offset.x + pos.x + b.min.x).to_pt();
                    let y = (offset.y + pos.y + b.min.y).to_pt();
                    *min_x = min_x.min(x);
                    *min_y = min_y.min(y);
                    *max_x = max_x.max(x + b.size().x.to_pt());
                    *max_y = max_y.max(y + b.size().y.to_pt());
                }
                _ => {}
            }
        }
    }
}
