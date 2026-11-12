//! Block-level flow orchestration:
//! - `mod.rs`: walking `TreeNode` sequences and coalescing stray inline nodes into paragraphs
//! - `spacing.rs`: inter-block vertical spacing measurements and adjustments
//! - `merge.rs`: merging cross-page split styled container blocks and orphaned list markers

mod merge;
mod spacing;

use typst::foundations::StyleChain;
use typst::layout::{GridElem, GroupItem};
use typst::math::EquationElem;
use typst::model::{
    CiteGroup, DirectLinkElem, EmphElem, EnumElem, FigureElem, FootnoteElem, FootnoteEntry,
    HeadingElem, LinkElem, LinkMarker, ListElem, ParElem, RefElem, StrongElem, TableElem,
    TermsElem,
};
use typst::pdf::{ArtifactElem, PdfMarkerTag};
use typst::text::{
    HighlightElem, OverlineElem, RawElem, SmallcapsElem, StrikeElem, SubElem, SuperElem,
    UnderlineElem,
};
use typst::visualize::LineElem;

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use crate::inline::ActiveInlineState;
use ir::{BlockNode, HAlignIr, ParagraphStyleIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_blocks_from_nodes(
        &mut self,
        nodes: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Vec<BlockNode> {
        let mut blocks = Vec::new();
        let mut stray_inlines: Vec<TreeNode> = Vec::new();
        let mut block_source_spans: Vec<(
            usize,
            Option<(usize, f64, f64)>,
            Option<(usize, f64, f64)>,
        )> = Vec::new();

        for (idx, node) in nodes.iter().enumerate() {
            if self.is_footnote_separator_node(nodes, idx) {
                continue;
            }
            if matches!(blocks.last(), Some(BlockNode::CodeBlock { .. }))
                && stray_inlines.is_empty()
                && matches!(node, TreeNode::Element { elem, .. } if elem.is::<typst::text::LinebreakElem>())
            {
                continue;
            }
            if Self::is_stray_inline_node_at(nodes, idx, !stray_inlines.is_empty()) {
                stray_inlines.push(node.clone());
                continue;
            }
            if !stray_inlines.is_empty() {
                let is_raw_line = matches!(
                    node,
                    TreeNode::Element { elem, .. } if elem.is::<typst::text::RawLine>()
                );
                if is_raw_line {
                    let only_breaks = stray_inlines.iter().all(|n| {
                        matches!(n, TreeNode::Element { elem, .. } if elem.is::<typst::text::LinebreakElem>())
                    });
                    if !only_breaks {
                        let mut runs = Vec::new();
                        Self::collect_raw_line_runs(&stray_inlines, &mut runs);
                        if !runs.is_empty() {
                            if let Some(BlockNode::CodeBlock { lines, .. }) = blocks.last_mut() {
                                lines.push(runs);
                                Self::normalize_empty_code_line_metrics(lines);
                            } else {
                                let mut lines = vec![runs];
                                Self::normalize_empty_code_line_metrics(&mut lines);
                                blocks.push(BlockNode::CodeBlock {
                                    lang: None,
                                    lines,
                                    fill: None,
                                    stroke: None,
                                });
                            }
                        }
                    }
                    stray_inlines.clear();
                } else {
                    let first_y = Self::nodes_first_page_top_y(&stray_inlines);
                    let last_y = Self::nodes_last_page_bottom_y(&stray_inlines);
                    let before_len = blocks.len();
                    self.flush_stray_inlines(&mut stray_inlines, &mut blocks, inherited_align);
                    if blocks.len() > before_len {
                        block_source_spans.push((blocks.len() - 1, first_y, last_y));
                    }
                }
            }
            let before_len = blocks.len();
            self.extract_block_from_node(node, &mut blocks, list_level, inherited_align);
            if blocks.len() > before_len {
                let single_slice = std::slice::from_ref(node);
                let first_y = Self::nodes_first_page_top_y(single_slice);
                let last_y = Self::nodes_last_page_bottom_y(single_slice);
                block_source_spans.push((blocks.len() - 1, first_y, last_y));
            }
        }

        if !stray_inlines.is_empty() {
            let first_y = Self::nodes_first_page_top_y(&stray_inlines);
            let last_y = Self::nodes_last_page_bottom_y(&stray_inlines);
            let before_len = blocks.len();
            self.flush_stray_inlines(&mut stray_inlines, &mut blocks, inherited_align);
            if blocks.len() > before_len {
                block_source_spans.push((blocks.len() - 1, first_y, last_y));
            }
        }

        Self::record_inter_block_pdf_gaps(&mut blocks, &block_source_spans);
        blocks
    }

    fn node_contains_footnote_entry(node: &TreeNode) -> bool {
        match node {
            TreeNode::Element { elem, children, .. } => {
                elem.is::<FootnoteEntry>()
                    || children.iter().any(Self::node_contains_footnote_entry)
            }
            TreeNode::Group { children, .. } => {
                children.iter().any(Self::node_contains_footnote_entry)
            }
            _ => false,
        }
    }

    fn node_has_text_or_image(node: &TreeNode) -> bool {
        match node {
            TreeNode::Text { .. } | TreeNode::Image { .. } => true,
            TreeNode::Element { children, .. } | TreeNode::Group { children, .. } => {
                children.iter().any(Self::node_has_text_or_image)
            }
            _ => false,
        }
    }

    fn node_horizontal_rule_info(node: &TreeNode) -> (bool, bool) {
        match node {
            TreeNode::Shape { shape, span, .. } => {
                let sz = shape.geometry.bbox(None).size();
                let is_horiz = sz.y.to_pt().abs() <= 2.0 && sz.x.to_pt().abs() > 20.0;
                (is_horiz, is_horiz && span.is_detached())
            }
            TreeNode::Element { elem, children, .. } => {
                let is_artifact = elem.is::<ArtifactElem>();
                let is_line = elem.is::<LineElem>();
                let mut any_horiz = is_line;
                let mut any_detached = is_line && elem.span().is_detached();
                for ch in children {
                    let (ch_h, ch_d) = Self::node_horizontal_rule_info(ch);
                    any_horiz |= ch_h;
                    any_detached |= ch_d;
                }
                (any_horiz, any_horiz && (is_artifact || any_detached))
            }
            TreeNode::Group { children, .. } => {
                let mut any_horiz = false;
                let mut any_detached = false;
                for ch in children {
                    let (ch_h, ch_d) = Self::node_horizontal_rule_info(ch);
                    any_horiz |= ch_h;
                    any_detached |= ch_d;
                }
                (any_horiz, any_detached)
            }
            _ => (false, false),
        }
    }

    fn is_footnote_separator_node(&self, nodes: &[TreeNode], idx: usize) -> bool {
        let node = &nodes[idx];
        if let TreeNode::Element { elem, .. } = node
            && elem.is::<LineElem>()
            && nodes
                .get(idx + 1)
                .is_some_and(Self::node_contains_footnote_entry)
        {
            return true;
        }
        if self.footnote_top_y_by_page.is_empty() || Self::node_has_text_or_image(node) {
            return false;
        }
        let (has_horiz_rule, is_artifact_or_detached) = Self::node_horizontal_rule_info(node);
        if !has_horiz_rule || !is_artifact_or_detached {
            return false;
        }
        let slice = std::slice::from_ref(node);
        if let (Some((p1, top_y, _)), Some((p2, bot_y, _))) = (
            Self::nodes_first_page_top_y(slice),
            Self::nodes_last_page_bottom_y(slice),
        ) && p1 == p2
            && (bot_y - top_y).abs() <= 2.5
            && let Some(&fn_top_y) = self.footnote_top_y_by_page.get(&p1)
        {
            let gap_to_fn = fn_top_y - bot_y;
            if (-2.0..=28.0).contains(&gap_to_fn) {
                return true;
            }
        }
        false
    }

    pub(crate) fn is_stray_inline_node_at(
        nodes: &[TreeNode],
        idx: usize,
        has_preceding_inline: bool,
    ) -> bool {
        let node = &nodes[idx];
        if Self::is_stray_inline_node(node) {
            return true;
        }
        if let TreeNode::Group {
            group, children, ..
        } = node
            && Self::is_inline_box_group(group, children)
        {
            let prev_same_line = has_preceding_inline
                && idx > 0
                && Self::nodes_share_horizontal_line(&nodes[idx - 1], node);
            let next_same_line = nodes.get(idx + 1).is_some_and(|n| {
                (Self::is_stray_inline_node(n)
                    || matches!(
                        n,
                        TreeNode::Group {
                            group: g,
                            children: c,
                            ..
                        } if Self::is_inline_box_group(g, c)
                    ))
                    && Self::nodes_share_horizontal_line(node, n)
            });
            if prev_same_line || next_same_line || group.frame.size().y.to_pt() <= 4.0 {
                return true;
            }
        }
        if let Some((x, y)) = Self::small_inline_shape_pos(node) {
            let prev_same_row = idx > 0
                && has_preceding_inline
                && Self::small_inline_shape_pos(&nodes[idx - 1])
                    .is_some_and(|(px, py)| (y - py).abs() <= 10.0 && (x - px).abs() > 1.0);
            let next_same_row = nodes
                .get(idx + 1)
                .and_then(Self::small_inline_shape_pos)
                .is_some_and(|(nx, ny)| (y - ny).abs() <= 10.0 && (nx - x).abs() > 1.0);
            if prev_same_row || next_same_row {
                return true;
            }
        }
        false
    }

    fn small_inline_shape_pos(node: &TreeNode) -> Option<(f64, f64)> {
        if let TreeNode::Element {
            elem,
            pos,
            children,
            ..
        } = node
            && Self::is_native_shape_elem(elem)
            && !elem.is::<LineElem>()
        {
            for ch in children {
                if let TreeNode::Shape { shape, .. } = ch {
                    let sz = shape.geometry.bbox(None).size();
                    if sz.x.to_pt().abs() <= 26.0 && sz.y.to_pt().abs() <= 26.0 {
                        return Some((pos.x.to_pt(), pos.y.to_pt()));
                    }
                }
            }
        }
        None
    }

    fn nodes_share_horizontal_line(a: &TreeNode, b: &TreeNode) -> bool {
        let (ax, ay, aw, ah) = Self::estimate_nodes_bbox(std::slice::from_ref(a));
        let (bx, by, bw, bh) = Self::estimate_nodes_bbox(std::slice::from_ref(b));
        if aw <= 0.0 || bw <= 0.0 || ah <= 0.0 || bh <= 0.0 {
            return false;
        }
        let y_overlap = (ay + ah).min(by + bh) - ay.max(by);
        let min_h = ah.min(bh);
        let x_overlap = (ax + aw).min(bx + bw) - ax.max(bx);
        let min_w = aw.min(bw);
        y_overlap >= (min_h * 0.4).max(2.5) && (ah > 28.0 || bh > 28.0 || x_overlap <= min_w * 0.5)
    }

    pub(crate) fn is_inline_box_group(group: &GroupItem, children: &[TreeNode]) -> bool {
        let sz = group.frame.size();
        if sz.y.to_pt() > 26.0 || sz.x.to_pt() <= 0.0 || sz.x.to_pt() > 300.0 {
            return false;
        }
        !(Self::is_cetz_group_nodes(children)
            || Self::is_direct_table_or_code_container(children)
            || Self::contains_block_elements(children))
    }

    fn contains_block_elements(nodes: &[TreeNode]) -> bool {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if elem.is::<ParElem>()
                        || elem.is::<HeadingElem>()
                        || elem.is::<TableElem>()
                        || elem.is::<GridElem>()
                        || elem.is::<ListElem>()
                        || elem.is::<EnumElem>()
                        || elem.is::<TermsElem>()
                        || elem.is::<FigureElem>()
                        || elem.is::<LineElem>()
                        || elem.is::<PdfMarkerTag>()
                        || (elem.is::<EquationElem>()
                            && elem
                                .to_packed::<EquationElem>()
                                .unwrap()
                                .block
                                .get(StyleChain::default()))
                        || (elem.is::<RawElem>()
                            && elem
                                .to_packed::<RawElem>()
                                .unwrap()
                                .block
                                .get(StyleChain::default()))
                    {
                        return true;
                    }
                    if Self::contains_block_elements(children) {
                        return true;
                    }
                }
                TreeNode::Group { children, .. } => {
                    if Self::contains_block_elements(children) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    pub(crate) fn is_stray_inline_node(node: &TreeNode) -> bool {
        match node {
            TreeNode::Text { .. } | TreeNode::Link { .. } => true,
            TreeNode::Element { elem, .. } => {
                elem.is::<typst::introspection::CounterDisplayElem>() || elem.is::<StrongElem>()
                    || elem.is::<EmphElem>()
                    || elem.is::<UnderlineElem>()
                    || elem.is::<StrikeElem>()
                    || elem.is::<OverlineElem>()
                    || elem.is::<HighlightElem>()
                    || elem.is::<SubElem>()
                    || elem.is::<SuperElem>()
                    || elem.is::<SmallcapsElem>()
                    || elem.is::<LinkMarker>()
                    || elem.is::<LinkElem>()
                    || elem.is::<DirectLinkElem>()
                    || elem.is::<RefElem>()
                    || elem.is::<CiteGroup>()
                    || elem.is::<FootnoteElem>()
                    || (elem.is::<EquationElem>()
                        && !elem
                            .to_packed::<EquationElem>()
                            .unwrap()
                            .block
                            .get(StyleChain::default()))
                    || (elem.is::<RawElem>()
                        && !elem
                            .to_packed::<RawElem>()
                            .unwrap()
                            .block
                            .get(StyleChain::default()))
                    || elem.is::<typst::text::LinebreakElem>()
            }
            _ => false,
        }
    }

    pub(crate) fn flush_stray_inlines(
        &mut self,
        stray: &mut Vec<TreeNode>,
        blocks: &mut Vec<BlockNode>,
        align: HAlignIr,
    ) {
        let inferred_align = self.infer_node_halign(stray).unwrap_or(align);
        let mut eff_align = if inferred_align == HAlignIr::Left && align != HAlignIr::Left {
            align
        } else {
            inferred_align
        };

        let mut line_spans: Vec<(f64, f64, f64)> = Vec::new();
        Self::collect_line_x_spans(stray, typst::layout::Point::zero(), &mut line_spans);
        line_spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let (vis_first, vis_hang) = if line_spans.len() >= 2 {
            Self::infer_paragraph_line_indents(stray)
        } else {
            (0.0, 0.0)
        };

        let mut first_line_indent_pt = 0.0;
        let mut hanging_indent_pt = 0.0;
        let mut left_indent_pt = 0.0;
        let mut right_indent_pt = 0.0;
        let mut line_spacing_pt = None;

        let chain = self.style_chain();
        let chain_justify = chain.get(ParElem::justify);
        let chain_linebreaks = match chain.get(ParElem::linebreaks) {
            typst::foundations::Smart::Custom(typst::model::Linebreaks::Optimized) => {
                ir::LinebreaksIr::Optimized
            }
            typst::foundations::Smart::Custom(typst::model::Linebreaks::Simple) => {
                ir::LinebreaksIr::Simple
            }
            typst::foundations::Smart::Auto => ir::LinebreaksIr::Auto,
        };

        if line_spans.len() >= 2 {
            let non_final = &line_spans[..line_spans.len() - 1];
            let r0 = non_final[0].2;
            let min_flush_span = if chain_justify { 100.0 } else { 200.0 };
            if eff_align == HAlignIr::Left {
                let multi_flush = non_final.len() >= 2
                    && non_final.iter().all(|(_, _, rx)| (*rx - r0).abs() <= 1.2)
                    && (r0 - non_final[0].1) >= min_flush_span;
                let expected_right = self
                    .active_group_bounds_pt
                    .map(|(_, gr)| gr)
                    .unwrap_or(self.content_right_pt);
                let right_margin_flush = (expected_right - r0).abs() <= 1.5
                    && (r0 - non_final[0].1) >= min_flush_span;
                if multi_flush || right_margin_flush {
                    eff_align = HAlignIr::Justify;
                }
            }
            if matches!(eff_align, HAlignIr::Left | HAlignIr::Justify) {
                if vis_first > 2.0 {
                    first_line_indent_pt = vis_first;
                } else if vis_hang > 2.0 {
                    hanging_indent_pt = vis_hang;
                }
                if self.container_inset_pt <= 0.1 {
                    let base_l = if vis_first > 2.0 {
                        line_spans[1].1
                    } else {
                        line_spans[0].1
                    };
                    let col_w = self.current_column_width_pt();
                    let (left_bound, right_bound) =
                        if let Some((gl, gr)) = self.active_group_bounds_pt {
                            (gl, gr)
                        } else if base_l >= self.content_left_pt - 4.0 {
                            (self.content_left_pt, self.content_right_pt)
                        } else {
                            (0.0, col_w)
                        };
                    let vis_left = (base_l - left_bound).max(0.0);
                    if (2.0..=48.0).contains(&vis_left) {
                        left_indent_pt = vis_left;
                    }
                    if eff_align == HAlignIr::Justify && r0 <= right_bound + 4.0 {
                        let r_gap = (right_bound - r0).max(0.0);
                        if (2.0..=48.0).contains(&r_gap) {
                            right_indent_pt = r_gap;
                        }
                    }
                }
            }
            let default_font_pt =
                Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
            line_spacing_pt = Some(Self::resolve_length_pt(
                chain.get(ParElem::leading),
                default_font_pt,
            ));
        }

        let mut inlines = Vec::new();
        let mut state = ActiveInlineState::default();
        let mut last_y: Option<f64> = None;
        let mut last_x_end: Option<f64> = None;
        self.extract_inlines_from_nodes(
            stray,
            &mut inlines,
            &mut state,
            &mut last_y,
            &mut last_x_end,
        );
        stray.clear();
        if !inlines.is_empty() {
            self.report.paragraphs_count += 1;
            let mut style = ParagraphStyleIr::default()
                .with_align(eff_align)
                .with_spacing(0.0, 2.0);
            style.first_line_indent_pt = first_line_indent_pt;
            style.hanging_indent_pt = hanging_indent_pt;
            style.left_indent_pt = left_indent_pt;
            style.right_indent_pt = right_indent_pt;
            style.line_spacing_pt = line_spacing_pt;
            style.justify = eff_align == HAlignIr::Justify;
            style.linebreaks = chain_linebreaks;
            blocks.push(BlockNode::Paragraph {
                content: inlines,
                style,
                bookmark: None,
            });
        }
    }
}
