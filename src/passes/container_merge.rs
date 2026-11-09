//! Pass 4 (`PipelinePassKind::ContainerAndPageBreakMerge`):
//! Merges cross-page / cross-column split styled containers (`TableKindIr::StyledContainer`),
//! stitches split lists/enums across container boundaries, and cleans redundant `PageBreak` blocks.

use crate::ir::{BlockNode, DocumentIr, TableKindIr, TableRowIr};
use crate::passes::visit_document_block_lists_mut;

/// Run Pass 4 (`ContainerAndPageBreakMerge`) on `doc`.
pub fn run_container_merge_pass(doc: &mut DocumentIr) {
    visit_document_block_lists_mut(doc, &mut merge_split_styled_containers);
    clean_redundant_pagebreaks(&mut doc.blocks);
}

/// Merge adjacent 1-cell styled container tables produced when a single
/// `#block(stroke: ..., fill: ...)[...]` broke across columns or pages in Typst.
pub fn merge_split_styled_containers(blocks: &mut Vec<BlockNode>) {
    let mut i = 1;
    while i < blocks.len() {
        let can_merge = match (&blocks[i - 1], &blocks[i]) {
            (BlockNode::Table(a), BlockNode::Table(b)) => {
                a.kind == TableKindIr::StyledContainer
                    && b.kind == TableKindIr::StyledContainer
                    && a.rows.len() == 1
                    && b.rows.len() == 1
                    && a.rows[0].cells.len() == 1
                    && b.rows[0].cells.len() == 1
            }
            _ => false,
        };
        if !can_merge {
            i += 1;
            continue;
        }

        let BlockNode::Table(mut b) = blocks.remove(i) else {
            unreachable!()
        };
        if let BlockNode::Table(a) = &mut blocks[i - 1] {
            let b_cell = &mut b.rows[0].cells[0];
            let mut b_blocks = std::mem::take(&mut b_cell.blocks);
            let starts_with_single_list_item = matches!(
                b_blocks.first(),
                Some(BlockNode::List { items, .. }) if items.len() == 1
            );
            if starts_with_single_list_item && b_blocks.len() > 1 {
                let a_inset_l = a.rows[0].cells[0].inset.left;
                let a_inset_r = a.rows[0].cells[0].inset.right;
                a.rows[0].cells[0].inset.bottom = 16.0;
                let first_blk = b_blocks.remove(0);
                let mut row1_cell = b_cell.clone();
                row1_cell.borders.bottom = None;
                row1_cell.inset.top = 1.5;
                row1_cell.inset.bottom = 0.0;
                row1_cell.inset.left = a_inset_l;
                row1_cell.inset.right = a_inset_r;
                row1_cell.blocks = vec![first_blk];
                let mut row2_cell = b_cell.clone();
                row2_cell.borders.top = None;
                row2_cell.inset.top = 0.0;
                row2_cell.inset.bottom += 11.0;
                row2_cell.inset.left = a_inset_l;
                row2_cell.inset.right = a_inset_r;
                row2_cell.blocks = b_blocks;
                a.rows.push(TableRowIr {
                    height_pt: Some(1.0),
                    is_header: false,
                    cells: vec![row1_cell],
                });
                a.rows.push(TableRowIr {
                    height_pt: None,
                    is_header: false,
                    cells: vec![row2_cell],
                });
            } else {
                let a_blocks = &mut a.rows[0].cells[0].blocks;
                if let (
                    Some(BlockNode::Enum {
                        items: a_items,
                        level: a_lvl,
                        ..
                    }),
                    Some(BlockNode::Enum { level: b_lvl, .. }),
                ) = (a_blocks.last_mut(), b_blocks.first())
                    && *a_lvl == *b_lvl
                {
                    if let BlockNode::Enum { items: b_items, .. } = b_blocks.remove(0) {
                        a_items.extend(b_items);
                    }
                } else if let (
                    Some(BlockNode::List {
                        items: a_items,
                        level: a_lvl,
                        ..
                    }),
                    Some(BlockNode::List { level: b_lvl, .. }),
                ) = (a_blocks.last_mut(), b_blocks.first())
                    && *a_lvl == *b_lvl
                {
                    if let BlockNode::List { items: b_items, .. } = b_blocks.remove(0) {
                        a_items.extend(b_items);
                    }
                }
                a_blocks.extend(b_blocks);
            }
        }
    }
}

/// Trim leading/trailing `PageBreak` blocks and collapse consecutive `PageBreak`s.
pub fn clean_redundant_pagebreaks(blocks: &mut Vec<BlockNode>) {
    while matches!(blocks.first(), Some(BlockNode::PageBreak)) {
        blocks.remove(0);
    }
    while matches!(blocks.last(), Some(BlockNode::PageBreak)) {
        blocks.pop();
    }
    let mut i = 1;
    while i < blocks.len() {
        if matches!(blocks[i], BlockNode::PageBreak)
            && matches!(blocks[i - 1], BlockNode::PageBreak)
        {
            blocks.remove(i);
        } else {
            i += 1;
        }
    }
}
