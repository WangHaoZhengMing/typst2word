//! Recursive drawable path and text label collection for CeTZ canvases.

use typst::foundations::StyleChain;
use typst::introspection::Location;
use typst::layout::{Abs, Point, RotateElem, Size};
use typst::math::EquationElem;
use typst::model::ParElem;
use typst::visualize::{FillRule, Geometry};

use crate::color::paint_to_hex;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::extract::inline::ActiveInlineState;
use crate::ir::{CetzLabelDrawableIr, CetzPathDrawableIr, ColorHex, InlineNode};

impl<'a> ExtractorContext<'a> {
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

                    if elem.is::<EquationElem>() || elem.is::<ParElem>() {
                        if !Self::contains_any_drawable_shapes_recursive(children) {
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
                    if !Self::contains_any_drawable_shapes_recursive(children) {
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
                    } else if !Self::contains_cetz_shapes_recursive(children) {
                        let mut shape_nodes = Vec::new();
                        let mut text_nodes = Vec::new();
                        for ch in children {
                            match ch {
                                TreeNode::Shape { .. } => shape_nodes.push(ch.clone()),
                                TreeNode::Element { elem, .. }
                                    if elem.is::<typst::pdf::ArtifactElem>() =>
                                {
                                    shape_nodes.push(ch.clone());
                                }
                                other => text_nodes.push(other.clone()),
                            }
                        }
                        if !shape_nodes.is_empty()
                            && !text_nodes.is_empty()
                            && !Self::contains_any_drawable_shapes_recursive(&text_nodes)
                        {
                            self.collect_cetz_drawables(
                                &shape_nodes,
                                abs_pos,
                                rotation_deg,
                                box_size,
                                background,
                                paths,
                                labels,
                            );
                            let inlines = self.extract_inlines_flat(&text_nodes);
                            if !inlines.is_empty() {
                                let (bx, by, bw, bh) = Self::estimate_nodes_bbox(&text_nodes);
                                labels.push(CetzLabelDrawableIr {
                                    x_pt: abs_pos.x.to_pt() + bx,
                                    y_pt: abs_pos.y.to_pt() + by,
                                    width_pt: bw.max(18.0),
                                    height_pt: bh.max(14.0),
                                    rotation_deg,
                                    content: inlines,
                                });
                            }
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
