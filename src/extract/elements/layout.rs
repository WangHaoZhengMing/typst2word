//! Layout container extractors (`AlignElem`, `PadElem`, `ColumnsElem`) and
//! group horizontal offset propagation.

use typst::foundations::{Packed, Smart, StyleChain};
use typst::layout::{AlignElem, ColumnsElem, GroupItem, PadElem, Point};
use typst::text::TextElem;

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, HAlignIr};

impl<'a> ExtractorContext<'a> {
    /// Propagate horizontal indentation when a laid-out `GroupItem` (such as an indented
    /// `#block(...)` without border/fill) is offset relative to the current column's left edge.
    pub(crate) fn apply_group_left_offset(
        &self,
        group: &GroupItem,
        pos: Point,
        list_level: usize,
        blocks: &mut [BlockNode],
    ) {
        if self.container_inset_pt > 0.1 || list_level > 0 {
            return;
        }
        let gx = pos.x.to_pt();
        let gw = group.frame.size().x.to_pt();
        let col_w = self.current_column_width_pt();
        let (count, gutter_pt) = self.active_columns;
        let left_bound = if gx < self.content_left_pt - 4.0 {
            0.0
        } else if count > 1 && gw <= col_w + 12.0 {
            let col_idx = (((gx - self.content_left_pt + 4.0).max(0.0))
                / (col_w + gutter_pt).max(1.0))
            .floor() as usize;
            self.content_left_pt + (col_idx.min(count - 1) as f64) * (col_w + gutter_pt)
        } else {
            self.content_left_pt
        };
        let group_left_offset = gx - left_bound;
        if (2.0..=48.0).contains(&group_left_offset) && gw > 20.0 && gw < col_w - 2.0 {
            for b in blocks {
                match b {
                    BlockNode::Paragraph { style, .. } => {
                        if matches!(style.align, HAlignIr::Left | HAlignIr::Justify)
                            && style.left_indent_pt < group_left_offset
                        {
                            style.left_indent_pt = group_left_offset;
                        }
                    }
                    BlockNode::List { indent_pt, .. } | BlockNode::Enum { indent_pt, .. } => {
                        if *indent_pt < group_left_offset {
                            *indent_pt = group_left_offset;
                        }
                    }
                    BlockNode::Table(t) => {
                        if t.left_indent_pt < group_left_offset {
                            t.left_indent_pt = group_left_offset;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub(crate) fn extract_align_container(
        &mut self,
        al: &Packed<AlignElem>,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
    ) {
        let (ha, _) = Self::alignment_to_ir(Smart::Custom(al.alignment.get(StyleChain::default())));
        let child_blocks = self.extract_blocks_from_nodes(children, list_level, ha);
        blocks.extend(child_blocks);
    }

    pub(crate) fn extract_pad_container(
        &mut self,
        pad: &Packed<PadElem>,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
        inherited_align: HAlignIr,
    ) {
        let chain = self.style_chain();
        let base_font_pt = Self::resolve_length_pt(chain.get(TextElem::size).0, 11.0);
        let pad_left_pt = Self::resolve_length_pt(pad.left.get(chain).abs, base_font_pt);
        let pad_right_pt = Self::resolve_length_pt(pad.right.get(chain).abs, base_font_pt);
        let mut child_blocks =
            self.extract_blocks_from_nodes(children, list_level, inherited_align);
        if pad_left_pt > 0.1 || pad_right_pt > 0.1 {
            for b in &mut child_blocks {
                match b {
                    BlockNode::Paragraph { style, .. } => {
                        if pad_left_pt > 0.1 {
                            style.left_indent_pt = pad_left_pt;
                        }
                        if pad_right_pt > 0.1 {
                            style.right_indent_pt = pad_right_pt;
                        }
                    }
                    BlockNode::List { indent_pt, .. } | BlockNode::Enum { indent_pt, .. } => {
                        *indent_pt += pad_left_pt;
                    }
                    BlockNode::Table(t) => {
                        t.left_indent_pt += pad_left_pt;
                    }
                    _ => {}
                }
            }
        }
        blocks.extend(child_blocks);
    }

    pub(crate) fn extract_columns_container(
        &mut self,
        cols_elem: &Packed<ColumnsElem>,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
        inherited_align: HAlignIr,
    ) {
        let chain = self.style_chain();
        let count = cols_elem.count.get(chain).get();
        let gutter_rel = cols_elem.gutter.get(chain);
        let base_font_pt = Self::resolve_length_pt(chain.get(TextElem::size).0, 11.0);
        let full_w = (self.content_right_pt - self.content_left_pt).max(100.0);
        let gutter_pt = (Self::resolve_length_pt(gutter_rel.abs, base_font_pt)
            + gutter_rel.rel.get() * full_w)
            .max(6.0);

        let prev_cols = self.active_columns;
        if count > 1 {
            self.active_columns = (count, gutter_pt);
        }
        let child_blocks = self.extract_blocks_from_nodes(children, list_level, inherited_align);
        self.active_columns = prev_cols;

        if count > 1 && self.page_columns <= 1 {
            blocks.push(BlockNode::Columns {
                count,
                gutter_pt,
                body: child_blocks,
            });
        } else {
            blocks.extend(child_blocks);
        }
    }
}
