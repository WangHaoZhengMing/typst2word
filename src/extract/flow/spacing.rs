//! Inter-block PDF vertical gap measurements (`InterBlockFlowIr` and `extra_trailing_space_pt`).

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, InterBlockFlowIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn nodes_first_page_top_y(nodes: &[TreeNode]) -> Option<(usize, f64, f64)> {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let raw_y = (pos.y - text.size * 0.8).to_pt().max(0.0);
                    let page_idx = (raw_y / 10000.0).floor() as usize;
                    let local_y = raw_y - page_idx as f64 * 10000.0;
                    return Some((page_idx, local_y, pos.x.to_pt()));
                }
                TreeNode::Group { group, pos, .. } => {
                    if group.frame.size().x.to_pt() <= 0.0 && group.frame.size().y.to_pt() <= 0.0 {
                        continue;
                    }
                    let raw_y = pos.y.to_pt().max(0.0);
                    let page_idx = (raw_y / 10000.0).floor() as usize;
                    let local_y = raw_y - page_idx as f64 * 10000.0;
                    return Some((page_idx, local_y, pos.x.to_pt()));
                }
                TreeNode::Shape { shape, pos, .. } => {
                    let b = shape.geometry.bbox(None);
                    let raw_y = (pos.y + b.min.y).to_pt().max(0.0);
                    let page_idx = (raw_y / 10000.0).floor() as usize;
                    let local_y = raw_y - page_idx as f64 * 10000.0;
                    return Some((page_idx, local_y, pos.x.to_pt()));
                }
                TreeNode::Image { pos, .. } => {
                    let raw_y = pos.y.to_pt().max(0.0);
                    let page_idx = (raw_y / 10000.0).floor() as usize;
                    let local_y = raw_y - page_idx as f64 * 10000.0;
                    return Some((page_idx, local_y, pos.x.to_pt()));
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
                    let raw_y = (pos.y + text.size * 0.2).to_pt().max(0.0);
                    let page_idx = (raw_y / 10000.0).floor() as usize;
                    let local_y = raw_y - page_idx as f64 * 10000.0;
                    return Some((page_idx, local_y, pos.x.to_pt()));
                }
                TreeNode::Group { group, pos, .. } => {
                    if group.frame.size().x.to_pt() <= 0.0 && group.frame.size().y.to_pt() <= 0.0 {
                        continue;
                    }
                    let raw_y = (pos.y + group.frame.size().y).to_pt().max(0.0);
                    let page_idx = (raw_y / 10000.0).floor() as usize;
                    let local_y = raw_y - page_idx as f64 * 10000.0;
                    return Some((page_idx, local_y, pos.x.to_pt()));
                }
                TreeNode::Shape { shape, pos, .. } => {
                    let b = shape.geometry.bbox(None);
                    let raw_y = (pos.y + b.max.y).to_pt().max(0.0);
                    let page_idx = (raw_y / 10000.0).floor() as usize;
                    let local_y = raw_y - page_idx as f64 * 10000.0;
                    return Some((page_idx, local_y, pos.x.to_pt()));
                }
                TreeNode::Image { size, pos, .. } => {
                    let raw_y = (pos.y + size.y).to_pt().max(0.0);
                    let page_idx = (raw_y / 10000.0).floor() as usize;
                    let local_y = raw_y - page_idx as f64 * 10000.0;
                    return Some((page_idx, local_y, pos.x.to_pt()));
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

    pub(crate) fn add_extra_space_after_to_last_block(blocks: &mut [BlockNode], extra_pt: f64) {
        if let Some(last) = blocks.last_mut() {
            match last {
                BlockNode::Paragraph { style, .. } => {
                    style.extra_trailing_space_pt += extra_pt;
                }
                BlockNode::Enum { items, .. } => {
                    if let Some(last_item) = items.last_mut() {
                        Self::add_extra_space_after_to_last_block(&mut last_item.blocks, extra_pt);
                    }
                }
                BlockNode::List { items, .. } => {
                    if let Some(last_item) = items.last_mut() {
                        Self::add_extra_space_after_to_last_block(&mut last_item.blocks, extra_pt);
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
        for win in spans.windows(2) {
            let (idx_a, _, end_a) = win[0];
            let (idx_b, start_b, _) = win[1];
            if idx_a >= blocks.len() || idx_b >= blocks.len() || idx_b != idx_a + 1 {
                continue;
            }
            let (Some((page_a, bottom_a, _x_a)), Some((page_b, top_b, _x_b))) = (end_a, start_b)
            else {
                continue;
            };
            if page_b == page_a + 1
                && bottom_a > 100.0
                && bottom_a < 560.0
                && top_b < 110.0
                && matches!(&blocks[idx_a], BlockNode::Paragraph { .. })
                && (matches!(&blocks[idx_b], BlockNode::Paragraph { .. })
                    || (matches!(&blocks[idx_b], BlockNode::Heading { .. })
                        && matches!(blocks.get(idx_b + 1), Some(BlockNode::Paragraph { .. }))))
            {
                let extra_pt = (755.0 - bottom_a).max(0.0);
                Self::add_extra_space_after_to_last_block(&mut blocks[..=idx_a], extra_pt);
                continue;
            }
            if page_a != page_b || top_b < bottom_a - 2.0 {
                continue;
            }
            let gap_pt = top_b - bottom_a;
            let font_a = Self::block_primary_font_size_pt(&blocks[idx_a]);
            let font_b = Self::block_primary_font_size_pt(&blocks[idx_b]);
            let next_is_tight_list = matches!(
                &blocks[idx_b],
                BlockNode::List { tight: true, .. } | BlockNode::Enum { tight: true, .. }
            );
            let next_is_paragraph = matches!(&blocks[idx_b], BlockNode::Paragraph { .. });
            if next_is_tight_list && gap_pt <= 10.0 {
                if let BlockNode::Paragraph { style, .. } = &mut blocks[idx_a] {
                    style.inter_block_flow = InterBlockFlowIr::BeforeTightList { gap_pt };
                }
            } else if next_is_paragraph
                && (font_a >= 15.0 || font_b >= 15.0)
                && gap_pt > 10.0
                && gap_pt < 60.0
            {
                if let BlockNode::Paragraph { style, .. } = &mut blocks[idx_a] {
                    style.inter_block_flow = InterBlockFlowIr::SamePageParagraphGap {
                        gap_pt,
                        prev_font_pt: font_a,
                        next_font_pt: font_b,
                    };
                }
            } else if gap_pt > 48.0 && gap_pt < 500.0 {
                Self::add_extra_space_after_to_last_block(
                    &mut blocks[..=idx_a],
                    (gap_pt - 12.0).max(0.0),
                );
            }
        }
    }

    fn block_primary_font_size_pt(block: &BlockNode) -> f64 {
        match block {
            BlockNode::Paragraph { content, .. } | BlockNode::Heading { content, .. } => content
                .iter()
                .find_map(|inl| match inl {
                    crate::ir::InlineNode::Text(r) => r.font_size_pt,
                    crate::ir::InlineNode::EquationInline(eq) => eq.font_size_pt,
                    _ => None,
                })
                .unwrap_or(11.0),
            BlockNode::EquationBlock(eq) => eq.font_size_pt.unwrap_or(11.0),
            _ => 11.0,
        }
    }
}
