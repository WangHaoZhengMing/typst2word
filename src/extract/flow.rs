//! Block-level flow orchestration:
//! - Walking `TreeNode` sequences and coalescing stray inline nodes into paragraphs
//! - Merging cross-column / cross-page split styled container blocks (`is_styled_container`)
//! - Recovering orphaned cross-page list/enum markers (`PdfMarkerTag`)

use typst::foundations::{Packed, StyleChain};
use typst::math::EquationElem;
use typst::model::{
    CiteGroup, DirectLinkElem, EmphElem, EnumElem, FootnoteElem, LinkElem, LinkMarker, ListElem,
    RefElem, StrongElem,
};
use typst::pdf::{PdfMarkerTag, PdfMarkerTagKind};
use typst::text::{
    HighlightElem, OverlineElem, RawElem, SmallcapsElem, StrikeElem, SubElem, SuperElem, TextElem,
    UnderlineElem,
};

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::extract::inline::ActiveInlineState;
use crate::ir::{BlockNode, EnumItemIr, HAlignIr, ListItemIr, ParagraphStyleIr, TableRowIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_blocks_from_nodes(
        &mut self,
        nodes: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Vec<BlockNode> {
        let mut blocks = Vec::new();
        let mut stray_inlines: Vec<TreeNode> = Vec::new();

        for node in nodes {
            if Self::is_stray_inline_node(node) {
                stray_inlines.push(node.clone());
                continue;
            }
            if !stray_inlines.is_empty() {
                self.flush_stray_inlines(&mut stray_inlines, &mut blocks, inherited_align);
            }
            self.extract_block_from_node(node, &mut blocks, list_level, inherited_align);
        }

        if !stray_inlines.is_empty() {
            self.flush_stray_inlines(&mut stray_inlines, &mut blocks, inherited_align);
        }

        Self::merge_split_styled_containers(&mut blocks);
        blocks
    }

    /// Merge adjacent 1-cell styled container tables produced when a single
    /// `#block(stroke: ..., fill: ...)[...]` broke across columns or pages in Typst.
    fn merge_split_styled_containers(blocks: &mut Vec<BlockNode>) {
        let mut i = 1;
        while i < blocks.len() {
            let can_merge = match (&blocks[i - 1], &blocks[i]) {
                (BlockNode::Table(a), BlockNode::Table(b)) => {
                    a.is_styled_container
                        && b.is_styled_container
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
        let eff_align = if inferred_align == HAlignIr::Left && align != HAlignIr::Left {
            align
        } else {
            inferred_align
        };
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
        Self::merge_adjacent_text_runs(&mut inlines);
        stray.clear();
        if !inlines.is_empty() {
            self.report.paragraphs_count += 1;
            blocks.push(BlockNode::Paragraph {
                content: inlines,
                style: ParagraphStyleIr {
                    align: eff_align,
                    space_after_pt: 2.0,
                    ..Default::default()
                },
                bookmark: None,
            });
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
