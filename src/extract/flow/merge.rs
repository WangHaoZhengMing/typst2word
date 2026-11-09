//! Cross-page / cross-column styled container merging and orphaned list marker recovery.

use typst::foundations::Packed;
use typst::model::{EnumElem, ListElem};
use typst::pdf::{PdfMarkerTag, PdfMarkerTagKind};
use typst::text::TextElem;

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, EnumItemIr, HAlignIr, ListItemIr, TableRowIr};

impl<'a> ExtractorContext<'a> {
    /// Merge adjacent 1-cell styled container tables produced when a single
    /// `#block(stroke: ..., fill: ...)[...]` broke across columns or pages in Typst.
    pub(super) fn merge_split_styled_containers(blocks: &mut Vec<BlockNode>) {
        let mut i = 1;
        while i < blocks.len() {
            let can_merge = match (&blocks[i - 1], &blocks[i]) {
                (BlockNode::Table(a), BlockNode::Table(b)) => {
                    a.is_styled_container()
                        && b.is_styled_container()
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

    /// Reconstruct list or enum items when a `ListElem` or `EnumElem` inside a hard `Group`
    /// (`#block`) split across columns or pages, leaving orphaned `PdfMarkerTag` nodes.
    pub(crate) fn try_extract_orphaned_list_marker(
        &mut self,
        marker: &Packed<PdfMarkerTag>,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
    ) -> bool {
        match marker.kind {
            PdfMarkerTagKind::ListItemLabel => {
                let lbl = Self::nodes_to_plain_text(children).trim().to_string();
                if !lbl.is_empty() {
                    self.pending_list_marker = Some(lbl);
                }
                true
            }
            PdfMarkerTagKind::ListItemBody => {
                let marker_str = self
                    .pending_list_marker
                    .take()
                    .unwrap_or_else(|| "•".to_string());
                let item_blocks =
                    self.extract_blocks_from_nodes(children, list_level + 1, HAlignIr::Left);
                let is_enum = marker_str
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric());
                if is_enum {
                    let parsed_num = marker_str
                        .trim_matches(|c: char| !c.is_ascii_digit())
                        .parse::<u64>()
                        .ok();
                    let item = EnumItemIr {
                        number: parsed_num,
                        marker_text: marker_str,
                        blocks: item_blocks,
                    };
                    if let Some(BlockNode::Enum { items, level, .. }) = blocks.last_mut()
                        && *level == list_level
                    {
                        items.push(item);
                    } else {
                        let chain = self.style_chain();
                        let default_font_pt =
                            Self::resolve_length_pt(chain.get(TextElem::size).0, 11.0);
                        let indent_pt =
                            Self::resolve_length_pt(chain.get(EnumElem::indent), default_font_pt);
                        let body_indent_pt = Self::resolve_length_pt(
                            chain.get(EnumElem::body_indent),
                            default_font_pt,
                        );
                        self.report.lists_count += 1;
                        blocks.push(BlockNode::Enum {
                            items: vec![item],
                            tight: true,
                            level: list_level,
                            spacing_pt: None,
                            indent_pt,
                            hanging_pt: (11.5 + body_indent_pt).max(16.0),
                        });
                    }
                } else {
                    let item = ListItemIr {
                        marker: marker_str,
                        blocks: item_blocks,
                    };
                    if let Some(BlockNode::List { items, level, .. }) = blocks.last_mut()
                        && *level == list_level
                    {
                        items.push(item);
                    } else {
                        let chain = self.style_chain();
                        let default_font_pt =
                            Self::resolve_length_pt(chain.get(TextElem::size).0, 11.0);
                        let indent_pt =
                            Self::resolve_length_pt(chain.get(ListElem::indent), default_font_pt);
                        let body_indent_pt = Self::resolve_length_pt(
                            chain.get(ListElem::body_indent),
                            default_font_pt,
                        );
                        self.report.lists_count += 1;
                        blocks.push(BlockNode::List {
                            items: vec![item],
                            tight: true,
                            level: list_level,
                            spacing_pt: None,
                            indent_pt,
                            hanging_pt: (6.0 + body_indent_pt).max(13.0),
                        });
                    }
                }
                true
            }
            _ => false,
        }
    }
}
