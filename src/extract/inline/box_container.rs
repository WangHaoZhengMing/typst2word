//! Inline `#box(...)` container extraction:
//! - Empty bottom-stroked boxes (`#box(width: 4em, stroke: (bottom: 0.5pt))`) -> underlined blank run or shape
//! - Non-empty bordered/shaded inline boxes (`#box([答案], stroke: 1pt, fill: ...)`) -> inline runs with border/highlight

use typst::layout::{GroupItem, Point};
use typst::pdf::ArtifactElem;
use typst::visualize::Geometry;

use crate::color::paint_to_hex;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::extract::inline::ActiveInlineState;
use crate::ir::{InlineNode, StrokeIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn try_extract_inline_box_group(
        &mut self,
        group: &GroupItem,
        pos: Point,
        children: &[TreeNode],
        out: &mut Vec<InlineNode>,
        state: &mut ActiveInlineState,
        _last_y: &mut Option<f64>,
        last_x_end: &mut Option<f64>,
    ) -> bool {
        let mut bg_shapes = Vec::new();
        let mut content_children = Vec::new();
        let mut shape_nodes = Vec::new();
        for child in children {
            match child {
                TreeNode::Shape {
                    shape,
                    span,
                    pos: spos,
                } => {
                    bg_shapes.push((*spos, shape.clone(), *span));
                    shape_nodes.push(child.clone());
                }
                TreeNode::Element {
                    elem,
                    children: achildren,
                    pos: apos,
                    ..
                } if elem.is::<ArtifactElem>() => {
                    for ac in achildren {
                        if let TreeNode::Shape {
                            shape,
                            span,
                            pos: spos,
                        } = ac
                        {
                            bg_shapes.push((*apos + *spos, shape.clone(), *span));
                        }
                    }
                    shape_nodes.push(child.clone());
                }
                other => content_children.push(other.clone()),
            }
        }

        if bg_shapes.is_empty() {
            return false;
        }

        let inner_text = Self::nodes_to_plain_text(&content_children);
        if inner_text.trim().is_empty() {
            // Empty / whitespace-only inline `#box(width: 4em, stroke: (bottom: 0.5pt))`:
            // emit only the shape so whitespace doesn't offset the underline!
            let mut inner_y = None;
            let mut inner_x = None;
            self.extract_inlines_from_nodes(&shape_nodes, out, state, &mut inner_y, &mut inner_x);
            *last_x_end = Some(pos.x.to_pt() + group.frame.size().x.to_pt());
            return true;
        }

        // Non-empty inline `#box`: distinguish a thin bottom-underline strip from a 4-sided box fill/stroke.
        let is_bottom_line_only = bg_shapes.len() == 1 && {
            let b = bg_shapes[0].1.geometry.bbox(None).size();
            matches!(bg_shapes[0].1.geometry, Geometry::Line(_))
                || (b.y.to_pt().abs() <= 2.5 && b.x.to_pt().abs() >= 6.0)
        };
        let prev_hl = state.highlight.clone();
        let prev_bdr = state.border.clone();
        let prev_ul = state.underline.clone();
        if is_bottom_line_only {
            let s = &bg_shapes[0].1;
            let st = s.stroke.as_ref().map(Self::fixed_stroke_to_ir).or_else(|| {
                let color = s
                    .fill
                    .as_ref()
                    .and_then(paint_to_hex)
                    .unwrap_or_else(|| "000000".to_string());
                let th = s.geometry.bbox(None).size().y.to_pt().abs().clamp(0.5, 2.5);
                Some(StrokeIr {
                    color,
                    thickness_pt: th,
                    ..Default::default()
                })
            });
            state.underline = st;
        } else {
            let box_fill = bg_shapes
                .iter()
                .find_map(|(_, s, _)| s.fill.as_ref().and_then(paint_to_hex));
            let box_stroke = bg_shapes
                .iter()
                .find_map(|(_, s, _)| s.stroke.as_ref().map(Self::fixed_stroke_to_ir));
            if let Some(f) = box_fill {
                state.highlight = Some(f);
            }
            if let Some(st) = box_stroke {
                state.border = Some(st);
            }
        }
        let mut inner_y = None;
        let mut inner_x = None;
        self.extract_inlines_from_nodes(&content_children, out, state, &mut inner_y, &mut inner_x);
        state.highlight = prev_hl;
        state.border = prev_bdr;
        state.underline = prev_ul;
        *last_x_end = Some(pos.x.to_pt() + group.frame.size().x.to_pt());
        true
    }
}
