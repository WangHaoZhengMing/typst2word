//! Layout container extractors (`AlignElem`, `PadElem`, `ColumnsElem`) and
//! group horizontal offset propagation.

use typst::foundations::{Packed, Smart};
use typst::introspection::Location;
use typst::layout::{AlignElem, ColumnsElem, GroupItem, PadElem, Point};
use typst::text::TextElem;

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{BlockNode, HAlignIr, SemanticRoleIr};

impl<'a> ExtractorContext<'a> {
    /// Propagate horizontal offset when a laid-out `GroupItem` (such as an indented/centered
    /// `#block(...)` or a full-bleed `#pad(x: -2em)` header/footer bar) is offset relative to
    /// the current column's left edge.
    pub(crate) fn apply_group_left_offset(
        &self,
        group: &GroupItem,
        pos: Point,
        list_level: usize,
        blocks: &mut [BlockNode],
    ) {
        if self.scope.container_inset_pt > 0.1 || list_level > 0 {
            return;
        }
        let gx = pos.x.to_pt();
        let gw = group.frame.size().x.to_pt();
        let col_w = self.current_column_width_pt();
        let (count, gutter_pt) = self.active_columns;
        let left_bound = if let Some((gl, gr)) = self.scope.active_group_bounds_pt {
            // When an enclosing `BlockElem(inset: ...)` or `PadElem` already narrowed `(gl, gr)`
            // inside `group`'s own unpadded outer frame `[gx, gx + gw]`, `apply_padding_to_child_blocks`
            // applies that inset once when leaving the outer element; skip here to avoid double-counting.
            if gx <= gl - 0.5 && gx + gw >= gr + 0.5 {
                return;
            }
            gl
        } else if count > 1 && gx >= self.content_left_pt - 4.0 && gw <= col_w + 12.0 {
            let col_idx = (((gx - self.content_left_pt + 4.0).max(0.0))
                / (col_w + gutter_pt).max(1.0))
            .floor() as usize;
            self.content_left_pt + (col_idx.min(count - 1) as f64) * (col_w + gutter_pt)
        } else {
            self.content_left_pt
        };
        let group_left_offset = gx - left_bound;
        if group_left_offset < -1.5 {
            // Negative group outdent (e.g. `#pad(x: -2em)` full-bleed slide header/footer bars).
            for b in blocks {
                match b {
                    BlockNode::Paragraph { style, .. } => {
                        if matches!(style.align, HAlignIr::Left | HAlignIr::Justify)
                            && style.left_indent_pt > group_left_offset
                        {
                            style.left_indent_pt = group_left_offset;
                        }
                    }
                    BlockNode::Table(t) => {
                        if t.align == HAlignIr::Left && t.left_indent_pt > group_left_offset {
                            t.left_indent_pt = group_left_offset;
                        }
                    }
                    BlockNode::Container(c) => {
                        if c.left_indent_pt > group_left_offset {
                            c.left_indent_pt = group_left_offset;
                        }
                    }
                    BlockNode::ShapeBlock(sh) => {
                        sh.placed_offset_pt = match sh.placed_offset_pt {
                            Some((ox, oy)) => Some((ox.min(group_left_offset), oy)),
                            None => Some((group_left_offset, 0.0)),
                        };
                    }
                    BlockNode::Heading {
                        align,
                        left_indent_pt,
                        ..
                    }
                    | BlockNode::HorizontalRule {
                        align,
                        left_indent_pt,
                        ..
                    } => {
                        if *align == HAlignIr::Left && *left_indent_pt > group_left_offset {
                            *left_indent_pt = group_left_offset;
                        }
                    }
                    _ => {}
                }
            }
        } else if group_left_offset >= 2.0
            && gw > 20.0
            && gw < col_w - 2.0
            && group_left_offset <= col_w - gw + 6.0
        {
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
                        if t.align == HAlignIr::Left && t.left_indent_pt < group_left_offset {
                            t.left_indent_pt = group_left_offset;
                        }
                    }
                    BlockNode::Container(c) => {
                        if c.left_indent_pt < group_left_offset {
                            c.left_indent_pt = group_left_offset;
                        }
                    }
                    BlockNode::Heading {
                        align,
                        left_indent_pt,
                        ..
                    }
                    | BlockNode::HorizontalRule {
                        align,
                        left_indent_pt,
                        ..
                    } => {
                        if *align == HAlignIr::Left && *left_indent_pt < group_left_offset {
                            *left_indent_pt = group_left_offset;
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
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
    ) {
        let chain = self.scoped_style_chain(Some(loc));
        let (ha, _) = Self::alignment_to_ir(Smart::Custom(al.alignment.get(chain)));
        let child_blocks = self.extract_blocks_from_nodes(children, list_level, ha);
        blocks.extend(child_blocks);
    }

    pub(crate) fn extract_pad_container(
        &mut self,
        pad: &Packed<PadElem>,
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
        inherited_align: HAlignIr,
    ) {
        let chain = self.scoped_style_chain(Some(loc));
        let base_font_pt = Self::resolve_length_pt(chain.get(TextElem::size).0, 11.0);
        let (cur_l, cur_r) = self
            .scope
            .active_group_bounds_pt
            .unwrap_or((self.content_left_pt, self.content_right_pt));
        let avail_w = (cur_r - cur_l).max(20.0);
        let avail_h = self.current_page_content_height_pt();
        let left_rel = pad.left.get(chain);
        let right_rel = pad.right.get(chain);
        let top_rel = pad.top.get(chain);
        let bottom_rel = pad.bottom.get(chain);
        let pad_left_pt =
            Self::resolve_length_pt(left_rel.abs, base_font_pt) + left_rel.rel.get() * avail_w;
        let pad_right_pt =
            Self::resolve_length_pt(right_rel.abs, base_font_pt) + right_rel.rel.get() * avail_w;
        let pad_top_pt =
            Self::resolve_length_pt(top_rel.abs, base_font_pt) + top_rel.rel.get() * avail_h;
        let pad_bottom_pt = Self::resolve_length_pt(bottom_rel.abs, base_font_pt)
            + bottom_rel.rel.get() * avail_h;

        // Narrow `active_group_bounds_pt` while extracting children so nested bounding-box
        // indentation and `1fr` table column sizing measure relative to the padded content box.
        let mut child_blocks = self.with_scope(|ctx| {
            if pad_left_pt.abs() > 0.1 || pad_right_pt.abs() > 0.1 {
                let nl = cur_l + pad_left_pt;
                let nr = (cur_r - pad_right_pt).max(nl + 20.0);
                ctx.scope.active_group_bounds_pt = Some((nl, nr));
            }
            ctx.extract_blocks_from_nodes(children, list_level, inherited_align)
        });

        Self::apply_padding_to_child_blocks(
            &mut child_blocks,
            pad_left_pt,
            pad_right_pt,
            pad_top_pt,
            pad_bottom_pt,
        );
        blocks.extend(child_blocks);
    }

    /// Propagate outer container padding (`#pad` or unstyled `#block(inset: ...)`) onto child blocks.
    pub(crate) fn apply_padding_to_child_blocks(
        child_blocks: &mut [BlockNode],
        pad_left_pt: f64,
        pad_right_pt: f64,
        pad_top_pt: f64,
        pad_bottom_pt: f64,
    ) {
        if pad_left_pt.abs() <= 0.1
            && pad_right_pt.abs() <= 0.1
            && pad_top_pt.abs() <= 0.1
            && pad_bottom_pt.abs() <= 0.1
        {
            return;
        }
        let last_idx = child_blocks.len().saturating_sub(1);
        for (idx, b) in child_blocks.iter_mut().enumerate() {
            match b {
                BlockNode::Paragraph { style, .. } => {
                    if pad_left_pt.abs() > 0.1 {
                        style.left_indent_pt += pad_left_pt;
                    }
                    if pad_right_pt.abs() > 0.1 {
                        style.right_indent_pt += pad_right_pt;
                    }
                    if idx == 0 && pad_top_pt > 0.1 {
                        style.space_before_pt += pad_top_pt;
                    }
                    if idx == last_idx && pad_bottom_pt > 0.1 {
                        style.pad_bottom_pt += pad_bottom_pt;
                    }
                }
                BlockNode::List { indent_pt, .. } | BlockNode::Enum { indent_pt, .. } => {
                    if pad_left_pt > 0.1 {
                        *indent_pt += pad_left_pt;
                    }
                }
                BlockNode::Table(t) => {
                    if pad_left_pt.abs() > 0.1 {
                        t.left_indent_pt += pad_left_pt;
                    }
                }
                BlockNode::Container(c) => {
                    if pad_left_pt.abs() > 0.1 {
                        c.left_indent_pt += pad_left_pt;
                    }
                }
                BlockNode::ShapeBlock(sh) => {
                    let dy = if idx == 0 { pad_top_pt } else { 0.0 };
                    if pad_left_pt.abs() > 0.1 || dy > 0.1 {
                        sh.placed_offset_pt = match sh.placed_offset_pt {
                            Some((ox, oy)) => Some((ox + pad_left_pt, oy + dy)),
                            None => Some((pad_left_pt, dy)),
                        };
                    }
                }
                BlockNode::Heading {
                    align,
                    left_indent_pt,
                    ..
                }
                | BlockNode::HorizontalRule {
                    align,
                    left_indent_pt,
                    ..
                } => {
                    if *align == HAlignIr::Left && pad_left_pt.abs() > 0.1 {
                        *left_indent_pt += pad_left_pt;
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn extract_columns_container(
        &mut self,
        cols_elem: &Packed<ColumnsElem>,
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
        inherited_align: HAlignIr,
    ) {
        let id = self.alloc_node_with_fragments(
            cols_elem.span(),
            Some(loc),
            Some(SemanticRoleIr::Columns),
            children,
        );
        self.parent_node_stack.push(id);
        let chain = self.scoped_style_chain(Some(loc));
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
        self.parent_node_stack.pop();

        if count > 1 && self.page_columns <= 1 {
            blocks.push(BlockNode::Columns {
                id,
                count,
                gutter_pt,
                body: child_blocks,
            });
        } else {
            blocks.extend(child_blocks);
        }
    }
}
