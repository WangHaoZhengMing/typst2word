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
    CiteGroup, DirectLinkElem, EmphElem, EnumElem, FigureElem, FootnoteElem, HeadingElem, LinkElem,
    LinkMarker, ListElem, ParElem, RefElem, StrongElem, TableElem, TermsElem,
};
use typst::pdf::PdfMarkerTag;
use typst::text::{
    HighlightElem, OverlineElem, RawElem, SmallcapsElem, StrikeElem, SubElem, SuperElem,
    UnderlineElem,
};
use typst::visualize::LineElem;

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::extract::inline::ActiveInlineState;
use crate::ir::{BlockNode, HAlignIr, ParagraphStyleIr};

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
            if Self::is_stray_inline_node_at(nodes, idx, !stray_inlines.is_empty()) {
                stray_inlines.push(node.clone());
                continue;
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
            let next_is_inline = nodes.get(idx + 1).is_some_and(|n| {
                Self::is_stray_inline_node(n)
                    || matches!(
                        n,
                        TreeNode::Group {
                            group: g,
                            children: c,
                            ..
                        } if Self::is_inline_box_group(g, c)
                    )
            });
            if has_preceding_inline || next_is_inline || group.frame.size().y.to_pt() <= 4.0 {
                return true;
            }
        }
        false
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
                elem.is::<StrongElem>()
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

        if line_spans.len() >= 2 {
            let non_final = &line_spans[..line_spans.len() - 1];
            let r0 = non_final[0].2;
            if eff_align == HAlignIr::Left {
                let multi_flush = non_final.len() >= 2
                    && non_final.iter().all(|(_, _, rx)| (*rx - r0).abs() <= 1.2)
                    && (r0 - non_final[0].1) >= 200.0;
                let right_margin_flush = (self.content_right_pt - r0).abs() <= 1.5
                    && (r0 - non_final[0].1) >= 200.0;
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
                    let (left_bound, right_bound) = if base_l >= self.content_left_pt - 4.0 {
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
            let chain = self.style_chain();
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
            blocks.push(BlockNode::Paragraph {
                content: inlines,
                style,
                bookmark: None,
            });
        }
    }
}
