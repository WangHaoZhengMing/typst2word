//! Semantic inline `#box(...)` (`BoxElem`) extraction:
//! - Materialized `Packed<BoxElem>` + `StyleChain` -> exact `fill`, 4-side `borders`, `inset` (+ `outset`),
//!   `radius`, and `baseline` without reverse-engineering laid-out geometry.
//! - Unstyled wrapper boxes (`#box(baseline: 30%, circle(...))`) -> propagate `baseline_shift_pt`.
//! - Bottom-stroked blanks (`#box(width: 96pt, stroke: (bottom: 0.6pt))[...]`) -> underlined inline runs.
//! - Tight 1-line pills (`#box(fill: ..., inset: (x: 4pt, y: 1pt))[ACTIVE]`) -> inline `<w:shd>`/`<w:bdr>` runs.
//! - Spatial boxes (rounded, padded, partial-border, or multi-line) -> `InlineNode::ShapeInline(ShapeIr)`.

use typst::foundations::{Packed, Resolve};
use typst::introspection::Location;
use typst::layout::{Abs, BoxElem, GroupItem, Point};
use typst::pdf::ArtifactElem;
use typst::syntax::Span;
use typst::visualize::{CurveItem, Geometry, Paint};

use crate::ExtractorContext;
use crate::color::paint_to_hex;
use crate::frame_tree::TreeNode;
use crate::inline::ActiveInlineState;
use ir::{
    BlockNode, BordersIr, ColorHex, HAlignIr, InlineNode, SemanticRoleIr, ShapeIr, ShapeKindIr,
    SidesPtIr, StrokeIr,
};

/// Exact resolved visual/geometric properties of a Typst `#box(...)`.
struct ResolvedBoxProps {
    width_pt: f64,
    height_pt: f64,
    fill: Option<ColorHex>,
    borders: BordersIr,
    uniform_stroke: Option<StrokeIr>,
    corner_radius_pt: Option<f64>,
    inset: SidesPtIr,
    baseline_shift_pt: Option<f64>,
}

impl<'a> ExtractorContext<'a> {
    /// Extract a materialized `Packed<BoxElem>` directly from its resolved `StyleChain`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn extract_inline_box_element(
        &mut self,
        box_elem: &Packed<BoxElem>,
        loc: Location,
        pos: Point,
        children: &[TreeNode],
        out: &mut Vec<InlineNode>,
        state: &mut ActiveInlineState,
        last_y: &mut Option<f64>,
        last_x_end: &mut Option<f64>,
    ) {
        // Locate the laid-out child `Group` frame if Typst wrapped this `#box` in a group.
        let group_entry = children.iter().find_map(|c| match c {
            TreeNode::Group {
                group,
                pos: gpos,
                children: gchildren,
            } => Some((group, *gpos, gchildren.as_slice())),
            _ => None,
        });

        let (box_pos, box_w_pt, box_h_pt) = if let Some((group, gpos, _)) = group_entry {
            (gpos, group.frame.size().x.to_pt(), group.frame.size().y.to_pt())
        } else {
            let (_, _, w, h) = Self::estimate_nodes_bbox(children);
            (pos, w.max(0.0), h.max(0.0))
        };

        // Resolve all `BoxElem` fields while borrowing `chain`, then drop `chain` before `&mut self` calls.
        let (has_tiling_fill, props) = {
            let chain = self.scoped_style_chain(Some(loc));
            let has_tiling_fill = matches!(box_elem.fill.get_ref(chain), Some(Paint::Tiling(_)));
            let fill = box_elem.fill.get_ref(chain).as_ref().and_then(paint_to_hex);

            let resolved_stroke = box_elem.stroke.resolve(chain);
            let side_to_stroke = |side: &Option<Option<typst::visualize::Stroke<Abs>>>| {
                side.as_ref()
                    .and_then(|o| o.as_ref())
                    .map(Self::abs_stroke_to_ir)
            };
            let borders = BordersIr {
                top: side_to_stroke(&resolved_stroke.top),
                bottom: side_to_stroke(&resolved_stroke.bottom),
                left: side_to_stroke(&resolved_stroke.left),
                right: side_to_stroke(&resolved_stroke.right),
            };
            let uniform_stroke = if borders.top.is_some()
                && borders.top == borders.bottom
                && borders.top == borders.left
                && borders.top == borders.right
            {
                borders.top.clone()
            } else {
                None
            };

            let baseline_pos = box_elem.baseline.get(chain);
            let shift_pt = baseline_pos
                .shift()
                .resolve(chain)
                .relative_to(Abs::pt(box_h_pt.max(0.0)))
                .to_pt();
            let at_pt = match baseline_pos.at() {
                typst::foundations::Smart::Auto => 0.0,
                typst::foundations::Smart::Custom(typst::layout::VAlignment::Bottom) => 0.0,
                typst::foundations::Smart::Custom(typst::layout::VAlignment::Horizon) => {
                    box_h_pt.max(0.0) * 0.5
                }
                typst::foundations::Smart::Custom(typst::layout::VAlignment::Top) => {
                    box_h_pt.max(0.0)
                }
            };
            let resolved_baseline = at_pt + shift_pt;
            let baseline_shift_pt = (resolved_baseline.abs() > 0.1).then_some(resolved_baseline);

            let resolved_inset = box_elem.inset.resolve(chain);
            let resolved_outset = box_elem.outset.resolve(chain);
            let resolve_h = |opt: Option<typst::layout::Rel<Abs>>| {
                opt.map_or(0.0, |r| {
                    r.relative_to(Abs::pt(box_w_pt.max(0.0))).to_pt().max(0.0)
                })
            };
            let resolve_v = |opt: Option<typst::layout::Rel<Abs>>| {
                opt.map_or(0.0, |r| {
                    r.relative_to(Abs::pt(box_h_pt.max(0.0))).to_pt().max(0.0)
                })
            };
            let outset = SidesPtIr {
                top: resolve_v(resolved_outset.top),
                bottom: resolve_v(resolved_outset.bottom),
                left: resolve_h(resolved_outset.left),
                right: resolve_h(resolved_outset.right),
            };
            let inset = SidesPtIr {
                top: resolve_v(resolved_inset.top) + outset.top,
                bottom: resolve_v(resolved_inset.bottom) + outset.bottom,
                left: resolve_h(resolved_inset.left) + outset.left,
                right: resolve_h(resolved_inset.right) + outset.right,
            };

            let visual_w_pt = box_w_pt + outset.left + outset.right;
            let visual_h_pt = box_h_pt + outset.top + outset.bottom;
            let min_side_pt = visual_w_pt.min(visual_h_pt).max(0.0);
            let resolved_radius = box_elem.radius.resolve(chain);
            let max_r = [
                resolved_radius.top_left,
                resolved_radius.top_right,
                resolved_radius.bottom_left,
                resolved_radius.bottom_right,
            ]
            .into_iter()
            .flatten()
            .map(|r| r.relative_to(Abs::pt(min_side_pt)).to_pt())
            .fold(0.0_f64, f64::max);
            let corner_radius_pt = (max_r > 0.1).then_some(max_r.min(min_side_pt * 0.5));

            (
                has_tiling_fill,
                ResolvedBoxProps {
                    width_pt: visual_w_pt,
                    height_pt: visual_h_pt,
                    fill,
                    borders,
                    uniform_stroke,
                    corner_radius_pt,
                    inset,
                    baseline_shift_pt,
                },
            )
        };

        if has_tiling_fill {
            let unsup = self.record_unsupported_paint("box.fill", box_elem.span());
            out.push(InlineNode::UnsupportedInline(unsup));
        }

        // 1. Unstyled wrapper `#box(...)` (no fill and no borders, e.g. `#box(baseline: 30%, circle(...))`
        //    or `#box(image(...))`): extract inner inlines normally and propagate `baseline_shift_pt`.
        if props.fill.is_none() && !props.borders.has_any() {
            let start_idx = out.len();
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            if let Some(shift) = props.baseline_shift_pt {
                Self::apply_box_baseline_shift_to_inlines(&mut out[start_idx..], shift);
            }
            return;
        }

        // Preserve horizontal gap before an inline box sitting on the same line as preceding text.
        if let (Some(prev_y), Some(prev_x_end)) = (*last_y, *last_x_end)
            && (box_pos.y.to_pt() - prev_y).abs() <= 12.0
        {
            let gap_pt = box_pos.x.to_pt() - prev_x_end;
            if gap_pt > 2.0 {
                out.push(InlineNode::HorizontalSpace { width_pt: gap_pt });
            }
        }

        let raw_inner = group_entry.map_or(children, |(_, _, gc)| gc);
        let (content_children, shape_nodes) = Self::split_box_bg_and_content(raw_inner);
        self.emit_resolved_inline_box(
            box_elem.span(),
            Some(loc),
            box_pos,
            children,
            &content_children,
            &shape_nodes,
            props,
            out,
            state,
            last_y,
            last_x_end,
        );
    }

    /// Fallback for untagged `GroupItem` frames carrying background/border shapes without an outer `BoxElem`.
    pub(crate) fn try_extract_inline_box_group(
        &mut self,
        group: &GroupItem,
        pos: Point,
        children: &[TreeNode],
        out: &mut Vec<InlineNode>,
        state: &mut ActiveInlineState,
        last_y: &mut Option<f64>,
        last_x_end: &mut Option<f64>,
    ) -> bool {
        let mut bg_shapes = Vec::new();
        for child in children {
            match child {
                TreeNode::Shape {
                    shape,
                    span,
                    pos: spos,
                } => bg_shapes.push((*spos, shape.clone(), *span)),
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
                }
                _ => {}
            }
        }
        if bg_shapes.is_empty() {
            return false;
        }

        let box_w_pt = group.frame.size().x.to_pt();
        let box_h_pt = group.frame.size().y.to_pt();
        let (content_children, shape_nodes) = Self::split_box_bg_and_content(children);

        let is_bottom_line_only = bg_shapes.len() == 1 && {
            let b = bg_shapes[0].1.geometry.bbox(None).size();
            matches!(bg_shapes[0].1.geometry, Geometry::Line(_))
                || (b.y.to_pt().abs() <= 2.5 && b.x.to_pt().abs() >= 6.0)
        };
        let (fill, borders, uniform_stroke) = if is_bottom_line_only {
            let s = &bg_shapes[0].1;
            let bottom = s.stroke.as_ref().map(Self::fixed_stroke_to_ir).or_else(|| {
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
            (
                None,
                BordersIr {
                    bottom,
                    ..Default::default()
                },
                None,
            )
        } else {
            let f = bg_shapes
                .iter()
                .find_map(|(_, s, _)| s.fill.as_ref().and_then(paint_to_hex));
            let st = bg_shapes
                .iter()
                .find_map(|(_, s, _)| s.stroke.as_ref().map(Self::fixed_stroke_to_ir));
            (f, BordersIr::splat(st.clone()), st)
        };

        let corner_radius_pt = bg_shapes.iter().find_map(|(_, s, _)| {
            if let Geometry::Curve(curve) = &s.geometry
                && let Some(CurveItem::Move(p)) = curve.0.first()
            {
                let rx = p.x.to_pt();
                let ry = p.y.to_pt();
                if rx > 0.5 && ry.abs() < 0.5 && rx <= box_w_pt * 0.5 {
                    return Some(rx);
                } else if ry > 0.5 && rx.abs() < 0.5 && ry <= box_h_pt * 0.5 {
                    return Some(ry);
                }
            }
            None
        });
        let (min_x, _, _, _) = Self::estimate_nodes_bbox(&content_children);
        let pad = if min_x.is_finite() {
            min_x.clamp(0.0, 24.0)
        } else {
            0.0
        };
        let span = bg_shapes
            .first()
            .map(|(_, _, sp)| *sp)
            .unwrap_or_else(Span::detached);
        let props = ResolvedBoxProps {
            width_pt: box_w_pt,
            height_pt: box_h_pt,
            fill,
            borders,
            uniform_stroke,
            corner_radius_pt,
            inset: SidesPtIr {
                top: pad.min(12.0),
                bottom: pad.min(12.0),
                left: pad,
                right: pad,
            },
            baseline_shift_pt: None,
        };
        self.emit_resolved_inline_box(
            span,
            None,
            pos,
            children,
            &content_children,
            &shape_nodes,
            props,
            out,
            state,
            last_y,
            last_x_end,
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_resolved_inline_box(
        &mut self,
        span: Span,
        loc_opt: Option<Location>,
        pos: Point,
        all_nodes: &[TreeNode],
        content_children: &[TreeNode],
        shape_nodes: &[TreeNode],
        props: ResolvedBoxProps,
        out: &mut Vec<InlineNode>,
        state: &mut ActiveInlineState,
        last_y: &mut Option<f64>,
        last_x_end: &mut Option<f64>,
    ) {
        let is_bottom_line_only = props.fill.is_none()
            && props.borders.bottom.is_some()
            && props.borders.top.is_none()
            && props.borders.left.is_none()
            && props.borders.right.is_none();

        let inner_text = Self::nodes_to_plain_text(content_children);
        if inner_text.trim().is_empty() {
            if is_bottom_line_only
                && let Some(ul) = props.borders.bottom
            {
                let sz_pt = 9.0;
                let space_w_pt = sz_pt * 0.25;
                let count = (props.width_pt / space_w_pt).round().max(1.0) as usize;
                out.push(InlineNode::Text(ir::TextRunIr {
                    text: "\u{00A0}".repeat(count),
                    font_family: Some("Times New Roman".to_string()),
                    font_size_pt: Some(sz_pt),
                    underline: Some(ul),
                    ..Default::default()
                }));
                *last_x_end = Some(pos.x.to_pt() + props.width_pt);
                return;
            }
            let mut inner_y = None;
            let mut inner_x = None;
            self.extract_inlines_from_nodes(shape_nodes, out, state, &mut inner_y, &mut inner_x);
            *last_x_end = Some(pos.x.to_pt() + props.width_pt);
            return;
        }

        // Route between spatial container boxes (`InlineNode::ShapeInline`) and tight 1-line badges:
        // - Bottom-underline blanks stay inline runs with `<w:u>`.
        // - Tight 1-line pills (no radius, small inset, uniform/no stroke) stay inline `<w:shd>`/`<w:bdr>` runs.
        // - Rounded, padded, partial-border, or multi-line boxes become `InlineNode::ShapeInline(ShapeIr)`.
        if !is_bottom_line_only {
            let (distinct_lines, font_sz) = Self::count_inline_box_lines(content_children);
            let has_partial_border = props.uniform_stroke.is_none() && props.borders.has_any();
            let is_spatial_box = distinct_lines >= 2
                || has_partial_border
                || props.corner_radius_pt.is_some_and(|r| r >= 1.5)
                || props.inset.top >= 4.0
                || props.inset.bottom >= 4.0
                || props.inset.left >= 6.0
                || props.inset.right >= 6.0
                || props.height_pt > font_sz * 1.65;

            if is_spatial_box && (props.fill.is_some() || props.borders.has_any()) {
                let id = self.alloc_node_with_fragments(
                    span,
                    loc_opt,
                    Some(SemanticRoleIr::Container),
                    all_nodes,
                );

                // Guard `container_inset_pt` so inner paragraphs do not treat the box's own
                // horizontal `inset` as an extra `left_indent_pt`.
                let prev_inset = self.container_inset_pt;
                let prev_gw = self.active_group_bounds_pt;
                self.container_inset_pt += props.inset.left + props.inset.right + 3.0;
                self.active_group_bounds_pt = Some((0.0, props.width_pt));
                self.parent_node_stack.push(id);
                let mut body_blocks =
                    self.extract_blocks_from_nodes(content_children, 0, HAlignIr::Left);
                self.parent_node_stack.pop();
                self.active_group_bounds_pt = prev_gw;
                self.container_inset_pt = prev_inset;

                if !body_blocks.is_empty() {
                    if let Some(BlockNode::Paragraph { style, .. }) = body_blocks.last_mut() {
                        style.space_after_pt = 0.0;
                    }
                    self.report.shapes_count += 1;
                    out.push(InlineNode::ShapeInline(ShapeIr {
                        id,
                        kind: ShapeKindIr::Rect,
                        width_pt: props.width_pt,
                        height_pt: props.height_pt,
                        fill: props.fill,
                        fill_rule_even_odd: false,
                        stroke: props.uniform_stroke,
                        borders: props.borders,
                        corner_radius_pt: props.corner_radius_pt,
                        inset: props.inset,
                        rotation_deg: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        body: body_blocks,
                        align: HAlignIr::Left,
                        placed_offset_pt: None,
                        baseline_shift_pt: props.baseline_shift_pt,
                    }));
                    *last_y = Some(pos.y.to_pt());
                    *last_x_end = Some(pos.x.to_pt() + props.width_pt);
                    return;
                }
            }
        }

        // Tight single-line pill or bottom-underlined fill-in-the-blank field.
        let prev_hl = state.highlight.clone();
        let prev_bdr = state.border.clone();
        let prev_ul = state.underline.clone();
        if is_bottom_line_only {
            state.underline = props.borders.bottom.clone();
        } else {
            if let Some(f) = props.fill {
                state.highlight = Some(f);
            }
            if let Some(st) = props.uniform_stroke {
                state.border = Some(st);
            }
        }

        let start_idx = out.len();
        let mut inner_y = None;
        let mut inner_x = None;
        if is_bottom_line_only
            && let Some(ref ul) = props.borders.bottom
        {
            let (min_x, _min_y, text_w, _text_h) = Self::estimate_nodes_bbox(content_children);
            let mut box_inlines = Vec::new();
            self.extract_inlines_from_nodes(
                content_children,
                &mut box_inlines,
                state,
                &mut inner_y,
                &mut inner_x,
            );
            let sz_pt = box_inlines
                .iter()
                .find_map(|n| match n {
                    InlineNode::Text(r) => r.font_size_pt,
                    _ => None,
                })
                .unwrap_or(9.0);
            let space_w_pt = (sz_pt * 0.25).max(1.5);
            if min_x.is_finite() && min_x > 1.5 {
                let lead_count = (min_x / space_w_pt).round().max(1.0) as usize;
                out.push(InlineNode::Text(ir::TextRunIr {
                    text: "\u{00A0}".repeat(lead_count),
                    font_family: Some("Times New Roman".to_string()),
                    font_size_pt: Some(sz_pt),
                    underline: Some(ul.clone()),
                    ..Default::default()
                }));
            }
            out.extend(box_inlines);
            let used_end_x = if min_x.is_finite() {
                (min_x + text_w).max(0.0)
            } else {
                text_w.max(0.0)
            };
            if props.width_pt - used_end_x > 1.5 {
                let trail_count =
                    ((props.width_pt - used_end_x) / space_w_pt).round().max(1.0) as usize;
                out.push(InlineNode::Text(ir::TextRunIr {
                    text: "\u{00A0}".repeat(trail_count),
                    font_family: Some("Times New Roman".to_string()),
                    font_size_pt: Some(sz_pt),
                    underline: Some(ul.clone()),
                    ..Default::default()
                }));
            }
        } else {
            self.extract_inlines_from_nodes(
                content_children,
                out,
                state,
                &mut inner_y,
                &mut inner_x,
            );
        }

        if let Some(shift) = props.baseline_shift_pt {
            Self::apply_box_baseline_shift_to_inlines(&mut out[start_idx..], shift);
        }

        state.highlight = prev_hl;
        state.border = prev_bdr;
        state.underline = prev_ul;
        *last_x_end = Some(pos.x.to_pt() + props.width_pt);
    }

    fn split_box_bg_and_content(children: &[TreeNode]) -> (Vec<TreeNode>, Vec<TreeNode>) {
        let mut content_children = Vec::new();
        let mut shape_nodes = Vec::new();
        for child in children {
            match child {
                TreeNode::Shape { .. } => shape_nodes.push(child.clone()),
                TreeNode::Element { elem, .. } if elem.is::<ArtifactElem>() => {
                    shape_nodes.push(child.clone());
                }
                other => content_children.push(other.clone()),
            }
        }
        (content_children, shape_nodes)
    }

    fn count_inline_box_lines(nodes: &[TreeNode]) -> (usize, f64) {
        let mut ys = Vec::new();
        let mut first_sz = None;
        Self::collect_box_line_ys(nodes, Point::zero(), &mut ys, &mut first_sz);
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let font_sz = first_sz.unwrap_or(11.0).max(6.0);
        let mut distinct = 0usize;
        let mut prev_y: Option<f64> = None;
        for y in ys {
            if prev_y.is_none_or(|py| (y - py).abs() > font_sz * 0.45) {
                distinct += 1;
                prev_y = Some(y);
            }
        }
        (distinct, font_sz)
    }

    fn collect_box_line_ys(
        nodes: &[TreeNode],
        offset: Point,
        ys: &mut Vec<f64>,
        first_sz: &mut Option<f64>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos, .. } if !text.text.trim().is_empty() => {
                    if first_sz.is_none() {
                        *first_sz = Some(text.size.to_pt());
                    }
                    ys.push((offset + *pos).y.to_pt());
                }
                TreeNode::Element { children, .. } => {
                    Self::collect_box_line_ys(children, offset, ys, first_sz);
                }
                TreeNode::Group { children, pos, .. } => {
                    Self::collect_box_line_ys(children, offset + *pos, ys, first_sz);
                }
                _ => {}
            }
        }
    }

    fn apply_box_baseline_shift_to_inlines(inlines: &mut [InlineNode], shift_pt: f64) {
        for inline in inlines {
            match inline {
                InlineNode::ShapeInline(shape) if shape.baseline_shift_pt.is_none() => {
                    shape.baseline_shift_pt = Some(shift_pt);
                }
                InlineNode::Text(run) => {
                    // In `TextRunIr`, positive `baseline_shift_pt` follows Typst `text(baseline: ...)`
                    // (downward shift), matching `BoxElem::baseline`.
                    run.baseline_shift_pt = Some(run.baseline_shift_pt.unwrap_or(0.0) + shift_pt);
                }
                InlineNode::Link { content, .. }
                | InlineNode::Ref { content, .. }
                | InlineNode::Citation { content, .. } => {
                    Self::apply_box_baseline_shift_to_inlines(content, shift_pt);
                }
                _ => {}
            }
        }
    }
}
