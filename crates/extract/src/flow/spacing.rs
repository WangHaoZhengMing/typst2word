//! Raw inter-block PDF vertical span measurements (`FlowMeasureIr` / `BlockPdfSpanIr`).
//!
//! Records physical `(page_idx, top_y_pt, bottom_y_pt)` spans on `ParagraphStyleIr::flow_measure`
//! without embedding Word-specific spacing thresholds.

use crate::ExtractorContext;
use crate::frame_tree::{TreeNode, decode_page_and_local_y};
use ir::{BlockNode, BlockPdfSpanIr, NextBlockKindIr, NextBlockMeasureIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn nodes_first_page_top_y(nodes: &[TreeNode]) -> Option<(usize, f64, f64)> {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let raw_y = (pos.y - text.size * 0.8).to_pt();
                    return Some(decode_page_and_local_y(raw_y, pos.x.to_pt()));
                }
                TreeNode::Group { group, pos, .. } => {
                    if group.frame.size().x.to_pt() <= 0.0 && group.frame.size().y.to_pt() <= 0.0 {
                        continue;
                    }
                    return Some(decode_page_and_local_y(pos.y.to_pt(), pos.x.to_pt()));
                }
                TreeNode::Shape { shape, pos, .. } => {
                    let b = shape.geometry.bbox(None);
                    let raw_y = (pos.y + b.min.y).to_pt();
                    return Some(decode_page_and_local_y(raw_y, pos.x.to_pt()));
                }
                TreeNode::Image { pos, .. } => {
                    return Some(decode_page_and_local_y(pos.y.to_pt(), pos.x.to_pt()));
                }
                TreeNode::Element { elem, children, .. } => {
                    if Self::is_page_furniture_elem(elem) {
                        continue;
                    }
                    if let Some(found) = Self::nodes_first_page_top_y(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(crate) fn nodes_last_page_bottom_y(nodes: &[TreeNode]) -> Option<(usize, f64, f64)> {
        for node in nodes.iter().rev() {
            match node {
                TreeNode::Text { text, pos } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let raw_y = (pos.y + text.size * 0.2).to_pt();
                    return Some(decode_page_and_local_y(raw_y, pos.x.to_pt()));
                }
                TreeNode::Group { group, pos, .. } => {
                    if group.frame.size().x.to_pt() <= 0.0 && group.frame.size().y.to_pt() <= 0.0 {
                        continue;
                    }
                    let raw_y = (pos.y + group.frame.size().y).to_pt();
                    return Some(decode_page_and_local_y(raw_y, pos.x.to_pt()));
                }
                TreeNode::Shape { shape, pos, .. } => {
                    let b = shape.geometry.bbox(None);
                    let raw_y = (pos.y + b.max.y).to_pt();
                    return Some(decode_page_and_local_y(raw_y, pos.x.to_pt()));
                }
                TreeNode::Image { size, pos, .. } => {
                    let raw_y = (pos.y + size.y).to_pt();
                    return Some(decode_page_and_local_y(raw_y, pos.x.to_pt()));
                }
                TreeNode::Element { elem, children, .. } => {
                    if Self::is_page_furniture_elem(elem) {
                        continue;
                    }
                    if let Some(found) = Self::nodes_last_page_bottom_y(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(crate) fn record_trailing_transition_on_last_block(
        blocks: &mut [BlockNode],
        span_a: BlockPdfSpanIr,
        measure: NextBlockMeasureIr,
    ) {
        if let Some(last) = blocks.last_mut() {
            match last {
                BlockNode::Paragraph { style, .. } => {
                    style
                        .flow_measure
                        .trailing_transitions
                        .push((span_a, measure));
                }
                BlockNode::Enum { items, .. } => {
                    if let Some(last_item) = items.last_mut() {
                        Self::record_trailing_transition_on_last_block(
                            &mut last_item.blocks,
                            span_a,
                            measure,
                        );
                    }
                }
                BlockNode::List { items, .. } => {
                    if let Some(last_item) = items.last_mut() {
                        Self::record_trailing_transition_on_last_block(
                            &mut last_item.blocks,
                            span_a,
                            measure,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    #[allow(clippy::type_complexity)]
    pub(super) fn record_inter_block_pdf_gaps(
        blocks: &mut [BlockNode],
        spans: &[(usize, Option<(usize, f64, f64)>, Option<(usize, f64, f64)>)],
    ) {
        for &(idx, start, end) in spans {
            if let Some(BlockNode::Paragraph { style, .. }) = blocks.get_mut(idx) {
                style.flow_measure.self_span = BlockPdfSpanIr::from_start_end(start, end);
            }
        }

        for win in spans.windows(2) {
            let (idx_a, start_a, end_a) = win[0];
            let (idx_b, start_b, end_b) = win[1];
            if idx_a >= blocks.len() || idx_b >= blocks.len() || idx_b != idx_a + 1 {
                continue;
            }
            let span_a = BlockPdfSpanIr::from_start_end(start_a, end_a);
            let span_b = BlockPdfSpanIr::from_start_end(start_b, end_b);
            let font_a = Self::block_primary_font_size_pt(&blocks[idx_a]);
            let font_b = Self::block_primary_font_size_pt(&blocks[idx_b]);
            let kind = match &blocks[idx_b] {
                BlockNode::List { tight: true, .. } | BlockNode::Enum { tight: true, .. } => {
                    NextBlockKindIr::TightList
                }
                BlockNode::Paragraph { .. } => NextBlockKindIr::Paragraph,
                BlockNode::Heading { .. }
                    if matches!(blocks.get(idx_b + 1), Some(BlockNode::Paragraph { .. })) =>
                {
                    NextBlockKindIr::HeadingFollowedByParagraph
                }
                _ => NextBlockKindIr::Other,
            };
            let measure = NextBlockMeasureIr {
                kind,
                span: span_b,
                prev_font_pt: font_a,
                next_font_pt: font_b,
            };
            if let BlockNode::Paragraph { style, .. } = &mut blocks[idx_a] {
                style.flow_measure.self_span = span_a;
                style.flow_measure.next = Some(measure);
            } else {
                Self::record_trailing_transition_on_last_block(
                    &mut blocks[..=idx_a],
                    span_a,
                    measure,
                );
            }
        }
    }

    fn block_primary_font_size_pt(block: &BlockNode) -> f64 {
        match block {
            BlockNode::Paragraph { content, .. } | BlockNode::Heading { content, .. } => content
                .iter()
                .find_map(|inl| match inl {
                    ir::InlineNode::Text(r) => r.font_size_pt,
                    ir::InlineNode::EquationInline(eq) => eq.font_size_pt,
                    _ => None,
                })
                .unwrap_or(11.0),
            BlockNode::EquationBlock(eq) => eq.font_size_pt.unwrap_or(11.0),
            _ => 11.0,
        }
    }
}
