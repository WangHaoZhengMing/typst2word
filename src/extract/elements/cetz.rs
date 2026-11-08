//! CeTZ canvases (`cetz.canvas(...)`) -> `CetzCanvasIr`.

use typst::foundations::StyleChain;
use typst::introspection::Location;
use typst::layout::{Abs, ColbreakElem, LayoutElem, PagebreakElem, Point, RotateElem, Size};
use typst::math::EquationElem;
use typst::model::{HeadingElem, ParElem};
use typst::visualize::{CurveElem, FillRule, Geometry};

use crate::color::paint_to_hex;
use crate::compiler::compile::is_cetz_span;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::extract::inline::ActiveInlineState;
use crate::ir::{
    CetzCanvasIr, CetzLabelDrawableIr, CetzPathDrawableIr, ColorHex, HAlignIr, InlineNode,
};

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // CeTZ Canvas Extraction (`cetz.canvas(...)`)
    // =========================================================================
    pub(crate) fn contains_cetz_shapes_recursive(nodes: &[TreeNode]) -> bool {
        for node in nodes {
            match node {
                TreeNode::Shape { span, .. } if is_cetz_span(*span) => return true,
                TreeNode::Element { elem, children, .. } => {
                    if is_cetz_span(elem.span()) && elem.is::<CurveElem>() {
                        return true;
                    }
                    if Self::contains_cetz_shapes_recursive(children) {
                        return true;
                    }
                }
                TreeNode::Group { children, .. } => {
                    if Self::contains_cetz_shapes_recursive(children) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    pub(crate) fn is_cetz_group_nodes(nodes: &[TreeNode]) -> bool {
        // A group is a CeTZ canvas box only if it directly contains CeTZ `Shape` items
        // and is not an outer multi-column/flow container holding `HeadingElem`, `ColbreakElem`,
        // `LayoutElem`, or `context`.
        let has_direct_cetz_shape = nodes
            .iter()
            .any(|n| matches!(n, TreeNode::Shape { span, .. } if is_cetz_span(*span)));
        if !has_direct_cetz_shape {
            return false;
        }
        let has_flow_elements = nodes.iter().any(|n| match n {
            TreeNode::Element { elem, .. } => {
                elem.is::<HeadingElem>()
                    || elem.is::<ColbreakElem>()
                    || elem.is::<PagebreakElem>()
                    || elem.is::<LayoutElem>()
                    || elem.elem().name() == "context"
            }
            _ => false,
        });
        !has_flow_elements
    }

    pub(crate) fn extract_cetz_canvas(
        &mut self,
        nodes: &[TreeNode],
        explicit_box_size: Option<Size>,
        align: HAlignIr,
    ) -> Option<CetzCanvasIr> {
        // Locate the inner `box(width: width, height: height, ...)` group if `nodes` is the outer `LayoutElem`.
        if explicit_box_size.is_none() {
            if let Some((box_size, box_children)) = Self::find_cetz_box_group(nodes) {
                return self.extract_cetz_canvas(&box_children, Some(box_size), align);
            }
        }

        let mut paths: Vec<CetzPathDrawableIr> = Vec::new();
        let mut labels: Vec<CetzLabelDrawableIr> = Vec::new();
        let mut background: Option<ColorHex> = None;

        self.collect_cetz_drawables(
            nodes,
            Point::zero(),
            0.0,
            explicit_box_size,
            &mut background,
            &mut paths,
            &mut labels,
        );

        if paths.is_empty() && labels.is_empty() {
            return None;
        }

        // Normalize coordinates if the CeTZ box was soft-flattened into page coordinates.
        let min_px = paths
            .iter()
            .map(|p| p.x_pt)
            .chain(labels.iter().map(|l| l.x_pt))
            .fold(f64::INFINITY, f64::min);
        let min_py = paths
            .iter()
            .map(|p| p.y_pt)
            .chain(labels.iter().map(|l| l.y_pt))
            .fold(f64::INFINITY, f64::min);
        if min_px.is_finite() && min_py.is_finite() {
            let should_shift = match explicit_box_size {
                Some(sz) => min_px > sz.x.to_pt() * 0.4 || min_py > sz.y.to_pt() * 0.4,
                None => min_px > 15.0 || min_py > 15.0,
            };
            if should_shift {
                for p in &mut paths {
                    p.x_pt = (p.x_pt - min_px).max(0.0);
                    p.y_pt = (p.y_pt - min_py).max(0.0);
                }
                for l in &mut labels {
                    l.x_pt = (l.x_pt - min_px).max(0.0);
                    l.y_pt = (l.y_pt - min_py).max(0.0);
                }
            }
        }

        let (width_pt, height_pt) = if let Some(sz) = explicit_box_size {
            (sz.x.to_pt().max(10.0), sz.y.to_pt().max(10.0))
        } else {
            let max_px = paths
                .iter()
                .map(|p| p.x_pt + p.width_pt)
                .fold(10.0_f64, f64::max);
            let max_py = paths
                .iter()
                .map(|p| p.y_pt + p.height_pt)
                .fold(10.0_f64, f64::max);
            let max_lx = labels
                .iter()
                .map(|l| l.x_pt + l.width_pt)
                .fold(max_px, f64::max);
            let max_ly = labels
                .iter()
                .map(|l| l.y_pt + l.height_pt)
                .fold(max_py, f64::max);
            (max_lx, max_ly)
        };

        Some(CetzCanvasIr {
            width_pt,
            height_pt,
            background,
            paths,
            labels,
            svg_fallback: None,
            align,
        })
    }

    pub(crate) fn find_cetz_box_group(nodes: &[TreeNode]) -> Option<(Size, Vec<TreeNode>)> {
        for node in nodes {
            match node {
                TreeNode::Group {
                    group, children, ..
                } => {
                    if Self::is_cetz_group_nodes(children) {
                        return Some((group.frame.size(), children.clone()));
                    }
                    if let Some(found) = Self::find_cetz_box_group(children) {
                        return Some(found);
                    }
                }
                TreeNode::Element { children, .. } => {
                    if let Some(found) = Self::find_cetz_box_group(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn collect_cetz_drawables(
        &mut self,
        nodes: &[TreeNode],
        offset: Point,
        rotation_deg: f64,
        box_size: Option<Size>,
        background: &mut Option<ColorHex>,
        paths: &mut Vec<CetzPathDrawableIr>,
        labels: &mut Vec<CetzLabelDrawableIr>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Shape { shape, pos, .. } => {
                    let abs_pos = offset + *pos;
                    // Check if this is the canvas background rectangle (`Geometry::Rect` matching `box_size`).
                    if let Geometry::Rect(rect_sz) = shape.geometry {
                        if let Some(bsz) = box_size {
                            if (rect_sz.x - bsz.x).abs() < Abs::pt(0.5)
                                && (rect_sz.y - bsz.y).abs() < Abs::pt(0.5)
                                && shape.stroke.is_none()
                                && background.is_none()
                            {
                                *background = shape.fill.as_ref().and_then(paint_to_hex);
                                continue;
                            }
                        }
                    }

                    if shape.fill.is_none() && shape.stroke.is_none() {
                        continue;
                    }

                    let bbox = shape.geometry.bbox(None);
                    let min_x = bbox.min.x.to_pt().min(0.0);
                    let min_y = bbox.min.y.to_pt().min(0.0);
                    let w_pt = (bbox.max.x.to_pt() - min_x).max(0.5);
                    let h_pt = (bbox.max.y.to_pt() - min_y).max(0.5);

                    let mut segments = Vec::new();
                    Self::geometry_to_curve_segments(
                        &shape.geometry,
                        Point::zero(),
                        min_x,
                        min_y,
                        &mut segments,
                    );

                    if !segments.is_empty() {
                        paths.push(CetzPathDrawableIr {
                            x_pt: abs_pos.x.to_pt() + min_x,
                            y_pt: abs_pos.y.to_pt() + min_y,
                            width_pt: w_pt,
                            height_pt: h_pt,
                            segments,
                            fill: shape.fill.as_ref().and_then(paint_to_hex),
                            fill_rule_even_odd: shape.fill_rule == FillRule::EvenOdd,
                            stroke: shape.stroke.as_ref().map(Self::fixed_stroke_to_ir),
                        });
                    }
                }
                TreeNode::Element { elem, children, .. } => {
                    let mut next_rot = rotation_deg;
                    if let Some(rot) = elem.to_packed::<RotateElem>() {
                        next_rot += rot.angle.get(StyleChain::default()).to_deg();
                    }

                    // If this element is an inline equation `$...$` or paragraph inside `cetz.draw.content`,
                    // extract it as a positioned `CetzLabelDrawableIr`!
                    if elem.is::<EquationElem>() || elem.is::<ParElem>() {
                        if !Self::contains_cetz_shapes_recursive(children) {
                            let inlines = if let Some(eq) = elem.to_packed::<EquationElem>() {
                                vec![InlineNode::EquationInline(self.extract_equation(
                                    eq,
                                    elem.location().unwrap_or_else(|| Location::new(0)),
                                    children,
                                ))]
                            } else {
                                self.extract_inlines_flat(children)
                            };
                            let has_horiz_jump = inlines
                                .iter()
                                .any(|n| matches!(n, InlineNode::HorizontalSpace { .. }));
                            if !inlines.is_empty() && !has_horiz_jump {
                                let (bx, by, bw, bh) = Self::estimate_nodes_bbox(children);
                                let lx = offset.x.to_pt() + bx;
                                let ly = offset.y.to_pt() + by;
                                labels.push(CetzLabelDrawableIr {
                                    x_pt: lx,
                                    y_pt: ly,
                                    width_pt: bw.max(18.0),
                                    height_pt: bh.max(14.0),
                                    rotation_deg: next_rot,
                                    content: inlines,
                                });
                                continue;
                            }
                        }
                    }

                    self.collect_cetz_drawables(
                        children, offset, next_rot, box_size, background, paths, labels,
                    );
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    let abs_pos =
                        offset + *pos + Point::new(group.transform.tx, group.transform.ty);
                    // Check if this group is a placed label group (contains text/equations and NO CeTZ curves).
                    if !Self::contains_cetz_shapes_recursive(children) {
                        let inlines = self.extract_inlines_flat(children);
                        let has_horiz_jump = inlines
                            .iter()
                            .any(|n| matches!(n, InlineNode::HorizontalSpace { .. }));
                        if !inlines.is_empty() && !has_horiz_jump {
                            let gsize = group.frame.size();
                            labels.push(CetzLabelDrawableIr {
                                x_pt: abs_pos.x.to_pt(),
                                y_pt: abs_pos.y.to_pt(),
                                width_pt: gsize.x.to_pt().max(18.0),
                                height_pt: gsize.y.to_pt().max(14.0),
                                rotation_deg,
                                content: inlines,
                            });
                            continue;
                        }
                    }
                    self.collect_cetz_drawables(
                        children,
                        abs_pos,
                        rotation_deg,
                        box_size,
                        background,
                        paths,
                        labels,
                    );
                }
                TreeNode::Text { text, pos } => {
                    let abs_pos = offset + *pos;
                    let mut run = Self::text_item_to_run(text, &ActiveInlineState::default());
                    run.text = run.text.trim().to_string();
                    if !run.text.is_empty() {
                        labels.push(CetzLabelDrawableIr {
                            x_pt: abs_pos.x.to_pt(),
                            y_pt: (abs_pos.y - text.size * 0.75).to_pt().max(0.0),
                            width_pt: text.width().to_pt().max(18.0),
                            height_pt: (text.size.to_pt() * 1.3).max(14.0),
                            rotation_deg,
                            content: vec![InlineNode::Text(run)],
                        });
                    }
                }
                _ => {}
            }
        }
    }
}
