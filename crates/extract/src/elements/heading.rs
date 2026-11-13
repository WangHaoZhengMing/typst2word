//! `title` / `heading` -> `BlockNode::Title` / `BlockNode::Heading`.

use typst::foundations::Packed;
use typst::introspection::Location;
use typst::layout::{BlockElem, BoxElem, Point};
use typst::model::HeadingElem;
use typst::pdf::ArtifactElem;
use typst::syntax::Span;
use typst::visualize::{CurveItem, Geometry};

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    BlockNode, BordersIr, ContainerStyleIr, DashPatternIr, HAlignIr, InlineNode, SemanticRoleIr,
    SidesPtIr, StrokeIr,
};

impl<'a> ExtractorContext<'a> {
    /// `TitleElem` -> `BlockNode::Title`.
    pub(crate) fn extract_title_block(
        &mut self,
        span: Span,
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
    ) {
        let id = self.alloc_node_with_fragments(
            span,
            Some(loc),
            Some(SemanticRoleIr::Title),
            children,
        );
        let align = self.infer_node_halign(children).unwrap_or(HAlignIr::Left);
        let content = self.extract_inlines_flat_with_bold(children, true);
        self.report.headings_count += 1;
        blocks.push(BlockNode::Title { id, content, align });
    }

    /// `HeadingElem` -> `BlockNode::Heading` (with numbering prefix, bookmark, and optional
    /// realized `#show heading: it => block(fill: ..., stroke: ...)[...]` dual-identity box style).
    pub(crate) fn extract_heading_block(
        &mut self,
        heading: &Packed<HeadingElem>,
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        inherited_align: HAlignIr,
    ) {
        let (level, outlined) = {
            let chain = self.scoped_style_chain(Some(loc));
            (heading.resolve_level(chain).get(), heading.outlined.get(chain))
        };
        let (box_style, content_nodes) = Self::extract_heading_box_style_and_nodes(children);
        let align = self
            .infer_node_halign(&content_nodes)
            .unwrap_or(inherited_align);
        let bookmark = self.bookmarks.get(&loc).cloned();
        let (numbering, content) =
            self.extract_heading_numbering_and_body(heading, Some(loc), &content_nodes);
        let id = self.alloc_node_with_fragments(
            heading.span(),
            Some(loc),
            Some(SemanticRoleIr::Heading {
                level,
                numbering: numbering.clone(),
                outlined,
                bookmark: bookmark.clone(),
            }),
            children,
        );
        self.report.headings_count += 1;
        blocks.push(BlockNode::Heading {
            id,
            level,
            numbering,
            content,
            bookmark,
            align,
            box_style,
        });
    }

    /// Inspect a heading's realized subtree for post-`show` container background/borders
    /// (`#show heading: it => block(fill: ..., stroke: ...)[#it.body]`), returning the
    /// extracted [`ContainerStyleIr`] and the clean inner content nodes without background shapes.
    fn extract_heading_box_style_and_nodes(
        children: &[TreeNode],
    ) -> (Option<ContainerStyleIr>, Vec<TreeNode>) {
        let mut target = children;
        let mut frame_w = 0.0_f64;
        let mut frame_h = 0.0_f64;
        while target.len() == 1 {
            match &target[0] {
                TreeNode::Group {
                    group,
                    children: gchildren,
                    ..
                } => {
                    frame_w = group.frame.size().x.to_pt();
                    frame_h = group.frame.size().y.to_pt();
                    target = gchildren;
                }
                TreeNode::Element {
                    elem,
                    children: echildren,
                    ..
                } if elem.is::<BlockElem>() || elem.is::<BoxElem>() => {
                    target = echildren;
                }
                _ => break,
            }
        }

        let mut bg_shapes = Vec::new();
        let mut content_nodes = Vec::new();
        for child in target {
            match child {
                TreeNode::Shape { shape, pos, .. } => {
                    bg_shapes.push((*pos, shape.clone()));
                }
                TreeNode::Element {
                    elem,
                    children: achildren,
                    pos,
                    ..
                } if elem.is::<ArtifactElem>() => {
                    for ac in achildren {
                        if let TreeNode::Shape {
                            shape, pos: spos, ..
                        } = ac
                        {
                            bg_shapes.push((*pos + *spos, shape.clone()));
                        }
                    }
                }
                other => content_nodes.push(other.clone()),
            }
        }

        if bg_shapes.is_empty() || content_nodes.is_empty() {
            return (None, children.to_vec());
        }

        let mut max_w_pt = frame_w;
        let mut max_h_pt = frame_h;
        let mut main_fill = None;
        let mut stroke = None;
        let mut top_border: Option<StrokeIr> = None;
        let mut bottom_border: Option<StrokeIr> = None;
        let mut left_border: Option<StrokeIr> = None;
        let mut right_border: Option<StrokeIr> = None;
        let min_shape_x = bg_shapes
            .iter()
            .map(|(p, _)| p.x.to_pt())
            .reduce(f64::min)
            .unwrap_or(0.0);

        for (spos, s) in &bg_shapes {
            let b = s.geometry.bbox(None);
            let sw = b.size().x.to_pt().abs();
            let sh = b.size().y.to_pt().abs();
            max_w_pt = max_w_pt.max(sw);
            max_h_pt = max_h_pt.max(sh);
            if sw > 12.0 && sh > 6.0 {
                if let Some(f) = s.fill.as_ref().and_then(paint_to_hex) {
                    main_fill = Some(f);
                }
                if stroke.is_none() {
                    stroke = s.stroke.as_ref().map(Self::fixed_stroke_to_ir);
                }
            } else if sw > 12.0 && sh <= 6.0 {
                let border_stroke = s
                    .stroke
                    .as_ref()
                    .map(Self::fixed_stroke_to_ir)
                    .or_else(|| {
                        s.fill.as_ref().and_then(paint_to_hex).map(|c| StrokeIr {
                            color: c,
                            thickness_pt: sh.max(0.5),
                            dash: DashPatternIr::Solid,
                            ..Default::default()
                        })
                    });
                if let Some(st) = border_stroke {
                    let sy = spos.y.to_pt() + b.min.y.to_pt();
                    if sy <= 3.0 {
                        top_border = Some(st);
                    } else {
                        bottom_border = Some(st);
                    }
                }
            } else if (0.3..=12.0).contains(&sw) && sh > 6.0 {
                let border_stroke = s
                    .stroke
                    .as_ref()
                    .map(Self::fixed_stroke_to_ir)
                    .or_else(|| {
                        s.fill.as_ref().and_then(paint_to_hex).map(|c| StrokeIr {
                            color: c,
                            thickness_pt: sw.max(0.5),
                            dash: DashPatternIr::Solid,
                            ..Default::default()
                        })
                    });
                if let Some(st) = border_stroke {
                    if (spos.x.to_pt() - min_shape_x).abs() <= 4.0 {
                        left_border = Some(st);
                    } else {
                        right_border = Some(st);
                    }
                }
            }
        }

        let has_partial = top_border.is_some()
            || bottom_border.is_some()
            || left_border.is_some()
            || right_border.is_some();
        if main_fill.is_none() && stroke.is_none() && !has_partial {
            return (None, children.to_vec());
        }

        let has_sides = main_fill.is_some()
            || stroke.is_some()
            || left_border.is_some()
            || right_border.is_some();

        let borders = if has_partial && stroke.is_none() {
            BordersIr {
                top: top_border,
                bottom: bottom_border,
                left: left_border,
                right: right_border,
            }
        } else {
            BordersIr {
                top: top_border.or_else(|| stroke.clone()),
                bottom: bottom_border.or_else(|| stroke.clone()),
                left: left_border.or_else(|| stroke.clone()),
                right: right_border.or_else(|| stroke.clone()),
            }
        };

        let mut line_starts = Vec::new();
        Self::collect_line_left_edges(&content_nodes, Point::zero(), &mut line_starts);
        let h_pad = if has_sides {
            line_starts
                .iter()
                .map(|(_, lx)| if min_shape_x > 10.0 { *lx - min_shape_x } else { *lx })
                .filter(|&lx| lx >= 2.0)
                .reduce(f64::min)
                .map(|lx| lx.clamp(4.0, 24.0))
                .unwrap_or(6.0)
        } else {
            0.0
        };
        let v_pad = if has_sides { h_pad.clamp(4.0, 12.0) } else { 2.0 };
        let inset = SidesPtIr {
            top: if has_sides || borders.top.is_some() { v_pad } else { 0.0 },
            bottom: if has_sides || borders.bottom.is_some() { v_pad } else { 0.0 },
            left: h_pad,
            right: h_pad,
        };

        let corner_radius_pt = bg_shapes.iter().find_map(|(_, s)| {
            if let Geometry::Curve(curve) = &s.geometry
                && let Some(CurveItem::Move(p)) = curve.0.first()
            {
                let rx = p.x.to_pt();
                let ry = p.y.to_pt();
                if rx > 0.5 && ry.abs() < 0.5 && rx <= max_w_pt.max(20.0) * 0.5 {
                    return Some(rx);
                } else if ry > 0.5 && rx.abs() < 0.5 && ry <= max_h_pt.max(20.0) * 0.5 {
                    return Some(ry);
                }
            }
            None
        });

        (
            Some(ContainerStyleIr {
                fill: main_fill,
                borders,
                corner_radius_pt,
                inset,
            }),
            content_nodes,
        )
    }

    // =========================================================================
    // Headings, Lists, Enums, Terms
    // =========================================================================
    pub(crate) fn extract_heading_numbering_and_body(
        &mut self,
        heading: &Packed<HeadingElem>,
        loc_opt: Option<Location>,
        children: &[TreeNode],
    ) -> (Option<String>, Vec<InlineNode>) {
        let all_inlines = self.extract_inlines_flat_with_bold(children, true);
        let chain = self.scoped_style_chain(loc_opt);
        if heading.numbering.get_ref(chain).is_none() {
            return (None, all_inlines);
        }

        // First check synthesized `heading.numbers` if populated by Typst synthesis.
        if let Some(ref synth_eco) = heading.numbers {
            let synth_num = synth_eco.trim();
            if !synth_num.is_empty() {
                if let Some(stripped) =
                    Self::strip_leading_prefix_from_inlines(&all_inlines, synth_num)
                {
                    if !stripped.is_empty() {
                        return (Some(synth_num.to_string()), stripped);
                    }
                }
            }
        }

        // Fallback: look for the first `HorizontalSpace` (or space run after numbering text).
        for (idx, inline) in all_inlines.iter().enumerate() {
            if matches!(inline, InlineNode::HorizontalSpace { .. }) && idx > 0 {
                let prefix_str = Self::inlines_to_plain_text(&all_inlines[..idx])
                    .trim()
                    .to_string();
                let body_inlines = all_inlines[idx + 1..].to_vec();
                if !prefix_str.is_empty() && !body_inlines.is_empty() {
                    return (Some(prefix_str), body_inlines);
                }
            }
        }

        (None, all_inlines)
    }

    pub(crate) fn strip_leading_prefix_from_inlines(
        inlines: &[InlineNode],
        prefix: &str,
    ) -> Option<Vec<InlineNode>> {
        let mut rem_prefix = prefix;
        let mut out = Vec::new();
        let mut prefix_done = false;

        for inline in inlines {
            if prefix_done {
                if out.is_empty() && matches!(inline, InlineNode::HorizontalSpace { .. }) {
                    continue;
                }
                if out.is_empty() {
                    if let InlineNode::Text(r) = inline {
                        let trimmed = r.text.trim_start();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if trimmed.len() != r.text.len() {
                            let mut nr = r.clone();
                            nr.text = trimmed.to_string();
                            out.push(InlineNode::Text(nr));
                            continue;
                        }
                    }
                }
                out.push(inline.clone());
                continue;
            }

            match inline {
                InlineNode::Text(r) => {
                    let t = r.text.as_str();
                    if let Some(rest_run) = t.strip_prefix(rem_prefix) {
                        rem_prefix = "";
                        prefix_done = true;
                        let trimmed_rest = rest_run.trim_start();
                        if !trimmed_rest.is_empty() {
                            let mut nr = r.clone();
                            nr.text = trimmed_rest.to_string();
                            out.push(InlineNode::Text(nr));
                        }
                    } else if let Some(rest_pref) = rem_prefix.strip_prefix(t) {
                        rem_prefix = rest_pref;
                        if rem_prefix.is_empty() {
                            prefix_done = true;
                        }
                    } else {
                        return None;
                    }
                }
                InlineNode::HorizontalSpace { .. } if rem_prefix.is_empty() => {
                    prefix_done = true;
                }
                _ => return None,
            }
        }

        if prefix_done { Some(out) } else { None }
    }
}
