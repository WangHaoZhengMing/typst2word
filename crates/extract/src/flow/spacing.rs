//! Reference layout vertical span measurements (`FlowReferenceIr` / `BlockLayoutSpanIr`).
//!
//! Records physical `(page_idx, top_y_pt, bottom_y_pt)` spans into `DocumentIr::reference_layout`
//! keyed by `NodeId` without embedding Word-specific spacing thresholds on `ParagraphStyleIr`.

use typst::syntax::Span;

use crate::ExtractorContext;
use crate::frame_tree::{TreeNode, decode_page_and_local_y};
use ir::{BlockLayoutSpanIr, BlockNode, NextBlockKindIr, NextBlockLayoutMeasureIr, SemanticRoleIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn nodes_first_page_top_y(nodes: &[TreeNode]) -> Option<(usize, f64, f64)> {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos, .. } => {
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
                TreeNode::Text { text, pos, .. } => {
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
        &mut self,
        blocks: &mut [BlockNode],
        span_a: BlockLayoutSpanIr,
        measure: NextBlockLayoutMeasureIr,
    ) {
        if let Some(last) = blocks.last_mut() {
            match last {
                BlockNode::Paragraph { id, .. } => {
                    if id.is_unset() {
                        *id = self.alloc_node_id(
                            Span::detached(),
                            None,
                            Some(SemanticRoleIr::Paragraph),
                        );
                    }
                    self.reference_layout
                        .entry(*id)
                        .or_default()
                        .flow
                        .get_or_insert_with(Default::default)
                        .trailing_transitions
                        .push((span_a, measure));
                }
                BlockNode::Enum { items, .. } => {
                    if let Some(last_item) = items.last_mut() {
                        self.record_trailing_transition_on_last_block(
                            &mut last_item.blocks,
                            span_a,
                            measure,
                        );
                    }
                }
                BlockNode::List { items, .. } => {
                    if let Some(last_item) = items.last_mut() {
                        self.record_trailing_transition_on_last_block(
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
        &mut self,
        blocks: &mut [BlockNode],
        spans: &[(usize, Option<(usize, f64, f64)>, Option<(usize, f64, f64)>)],
    ) {
        for &(idx, start, end) in spans {
            if let Some(BlockNode::Paragraph { id, .. }) = blocks.get_mut(idx) {
                if id.is_unset() {
                    *id = self.alloc_node_id(
                        Span::detached(),
                        None,
                        Some(SemanticRoleIr::Paragraph),
                    );
                }
                let span = BlockLayoutSpanIr::from_start_end(start, end);
                let entry = self.reference_layout.entry(*id).or_default();
                if matches!(entry.span, BlockLayoutSpanIr::Unmeasured) {
                    entry.span = span;
                }
                entry.flow.get_or_insert_with(Default::default).self_span = span;
            }
        }

        for win in spans.windows(2) {
            let (idx_a, start_a, end_a) = win[0];
            let (idx_b, start_b, end_b) = win[1];
            if idx_a >= blocks.len() || idx_b >= blocks.len() || idx_b != idx_a + 1 {
                continue;
            }
            let span_a = BlockLayoutSpanIr::from_start_end(start_a, end_a);
            let span_b = BlockLayoutSpanIr::from_start_end(start_b, end_b);
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
            let measure = NextBlockLayoutMeasureIr {
                kind,
                span: span_b,
                prev_font_pt: font_a,
                next_font_pt: font_b,
            };
            if let BlockNode::Paragraph { id, .. } = &mut blocks[idx_a] {
                if id.is_unset() {
                    *id = self.alloc_node_id(
                        Span::detached(),
                        None,
                        Some(SemanticRoleIr::Paragraph),
                    );
                }
                let flow = self
                    .reference_layout
                    .entry(*id)
                    .or_default()
                    .flow
                    .get_or_insert_with(Default::default);
                flow.self_span = span_a;
                flow.next = Some(measure);
            } else {
                self.record_trailing_transition_on_last_block(
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
