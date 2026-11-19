//! Layout container extractors (`AlignElem`, `PadElem`, `ColumnsElem`, `StackElem`) and
//! group horizontal offset propagation.

use typst::foundations::{Packed, Smart};
use typst::introspection::Location;
use typst::layout::{AlignElem, ColumnsElem, Dir, GroupItem, HElem, HideElem, PadElem, Point, StackElem};
use typst::text::TextElem;

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    BlockNode, BordersIr, HAlignIr, SemanticRoleIr, SidesPtIr, TableCellIr, TableIr, TableKindIr,
    TableRowIr, VAlignIr,
};

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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn extract_stack_container(
        &mut self,
        stack: &Packed<StackElem>,
        loc: Location,
        pos: Point,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
        inherited_align: HAlignIr,
    ) {
        let chain = self.scoped_style_chain(Some(loc));
        let dir = stack.dir.get(chain);

        match dir {
            Dir::TTB => {
                let child_blocks =
                    self.extract_blocks_from_nodes(children, list_level, inherited_align);
                blocks.extend(child_blocks);
            }
            Dir::BTT => {
                // `dir: btt` lays out child `0` at the bottom and the final child at the top;
                // sort the stack's frame items top-to-bottom by laid-out Y so Word block flow
                // and inter-block gap measurement see true page order.
                if let [TreeNode::Group {
                    group,
                    pos: gpos,
                    children: gchildren,
                }] = children
                    && gchildren.len() > 1
                {
                    let mut sorted = gchildren.clone();
                    sorted.sort_by(|a, b| {
                        let (_, ay, _, _) = Self::estimate_nodes_bbox(std::slice::from_ref(a));
                        let (_, by, _, _) = Self::estimate_nodes_bbox(std::slice::from_ref(b));
                        ay.partial_cmp(&by).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let rewrapped = TreeNode::Group {
                        group: group.clone(),
                        pos: *gpos,
                        children: sorted,
                    };
                    let child_blocks = self.extract_blocks_from_nodes(
                        std::slice::from_ref(&rewrapped),
                        list_level,
                        inherited_align,
                    );
                    blocks.extend(child_blocks);
                } else {
                    let child_blocks =
                        self.extract_blocks_from_nodes(children, list_level, inherited_align);
                    blocks.extend(child_blocks);
                }
            }
            Dir::LTR | Dir::RTL => {
                let (stack_pos, raw_items, local_offset) = match children {
                    [TreeNode::Group {
                        pos: gpos,
                        children: gchildren,
                        ..
                    }] => (*gpos, gchildren.as_slice(), self.scope.group_origin_offset + *gpos),
                    _ => (pos, children, self.scope.group_origin_offset),
                };

                // Preserve single-paragraph inline horizontal stacks (e.g. `#stack(dir: ltr, spacing: 12pt, [Alpha], [Beta], [Gamma])`)
                // when all children are stray inline items in LTR order.
                let all_stray_inline = dir == Dir::LTR
                    && !raw_items.is_empty()
                    && raw_items
                        .iter()
                        .enumerate()
                        .all(|(i, _)| Self::is_stray_inline_node_at(raw_items, i, i > 0));
                if all_stray_inline {
                    let child_blocks =
                        self.extract_blocks_from_nodes(children, list_level, inherited_align);
                    blocks.extend(child_blocks);
                    return;
                }

                // Group visible items into horizontal stack slots, coalescing only adjacent
                // inline runs that belong to the same inline child (`gap_x < 2.0pt` without `HElem`).
                let mut slots: Vec<(f64, f64, Vec<TreeNode>)> = Vec::new();
                let mut saw_spacer = false;
                for item in raw_items {
                    if let TreeNode::Element { elem, .. } = item {
                        if Self::is_page_furniture_elem(elem) || elem.is::<HideElem>() {
                            continue;
                        }
                        if elem.is::<HElem>() {
                            saw_spacer = true;
                            continue;
                        }
                    }
                    let (ix, _, iw, ih) = Self::estimate_nodes_bbox(std::slice::from_ref(item));
                    if iw <= 0.0 && ih <= 0.0 {
                        continue;
                    }
                    let is_inline = Self::is_stray_inline_node(item);
                    if is_inline
                        && !saw_spacer
                        && let Some((last_x, last_w, last_nodes)) = slots.last_mut()
                        && last_nodes.iter().all(Self::is_stray_inline_node)
                    {
                        let prev_right = *last_x + *last_w;
                        if (ix - prev_right).abs() < 2.0 {
                            let new_min_x = last_x.min(ix);
                            let new_max_x = prev_right.max(ix + iw);
                            *last_x = new_min_x;
                            *last_w = (new_max_x - new_min_x).max(1.0);
                            last_nodes.push(item.clone());
                            continue;
                        }
                    }
                    saw_spacer = false;
                    slots.push((ix, iw.max(1.0), vec![item.clone()]));
                }

                if slots.len() <= 1 {
                    let child_blocks =
                        self.extract_blocks_from_nodes(children, list_level, inherited_align);
                    blocks.extend(child_blocks);
                    return;
                }

                // Sort slots left-to-right by laid-out `x` (automatically handling `Dir::RTL` too).
                slots.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

                let stack_id = self.alloc_node_with_fragments(
                    stack.span(),
                    Some(loc),
                    Some(SemanticRoleIr::Grid),
                    children,
                );
                self.parent_node_stack.push(stack_id);

                let mut extracted_slots: Vec<(f64, f64, f64, Vec<BlockNode>)> =
                    Vec::with_capacity(slots.len());
                for (sx, sw, snodes) in &slots {
                    let cell_blocks = self.with_scope(|ctx| {
                        ctx.scope.group_origin_offset = local_offset;
                        ctx.scope.active_group_bounds_pt = Some((*sx, *sx + sw.max(20.0)));
                        ctx.scope.container_inset_pt = 0.0;
                        ctx.extract_blocks_from_nodes(snodes, list_level, HAlignIr::Left)
                    });
                    if cell_blocks.is_empty() {
                        continue;
                    }
                    let stroke_pad_pt = match cell_blocks.as_slice() {
                        [BlockNode::ShapeBlock(sh)] => sh
                            .stroke
                            .as_ref()
                            .or(sh.borders.top.as_ref())
                            .map_or(0.0, |s| s.thickness_pt.max(0.5)),
                        _ => 0.0,
                    };
                    let eff_w = match cell_blocks.as_slice() {
                        [BlockNode::ShapeBlock(sh)] => sh.width_pt.max(*sw),
                        _ => *sw,
                    };
                    extracted_slots.push((*sx, eff_w, stroke_pad_pt, cell_blocks));
                }

                self.parent_node_stack.pop();

                if extracted_slots.len() <= 1 {
                    for (_, _, _, mut cb) in extracted_slots {
                        blocks.append(&mut cb);
                    }
                    return;
                }

                let n = extracted_slots.len();
                let mut col_widths_pt = Vec::with_capacity(n);
                let mut cells = Vec::with_capacity(n);

                for idx in 0..n {
                    let (sx, sw, s_pad, ref cb) = extracted_slots[idx];
                    // Leading offset is carried by `tblInd` (`left_indent_pt`), never by the first
                    // cell's `tcMar.left` (`sx` is an absolute page coordinate).
                    let pad_left_pt = 0.0;
                    let pad_right_pt = if idx + 1 < n {
                        let (next_x, _, next_s_pad, _) = extracted_slots[idx + 1];
                        let raw_gap_pt = (next_x - (sx + sw)).max(0.0);
                        (raw_gap_pt - s_pad - next_s_pad).max(0.0)
                    } else {
                        0.0
                    };
                    let col_w_pt = sw + 2.0 * s_pad + pad_left_pt + pad_right_pt;
                    col_widths_pt.push(col_w_pt.max(8.0));
                    cells.push(TableCellIr {
                        x: idx,
                        y: 0,
                        colspan: 1,
                        rowspan: 1,
                        fill: None,
                        borders: BordersIr::default(),
                        inset: SidesPtIr {
                            top: 0.0,
                            bottom: 0.0,
                            left: pad_left_pt,
                            right: pad_right_pt,
                        },
                        halign: HAlignIr::Left,
                        valign: VAlignIr::Top,
                        is_header_cell: false,
                        blocks: cb.clone(),
                    });
                }

                let left_bound = self
                    .scope
                    .active_group_bounds_pt
                    .map(|(l, _)| l)
                    .unwrap_or(self.content_left_pt);
                let raw_left_off = stack_pos.x.to_pt() - left_bound;
                // Inline DrawingML shapes reserve `<wp:effectExtent l=stroke>` before the shape box,
                // so pull the table left by the first slot's stroke pad to keep the shape edge at
                // Typst's exact `x`.
                let first_stroke_pad_pt = extracted_slots.first().map_or(0.0, |s| s.2);
                let left_indent_pt = if self.scope.container_inset_pt <= 0.1
                    && list_level == 0
                    && inherited_align == HAlignIr::Left
                    && ((-144.0..=-1.5).contains(&raw_left_off)
                        || (2.0..=48.0).contains(&raw_left_off))
                {
                    raw_left_off - first_stroke_pad_pt
                } else if list_level > 0 && inherited_align == HAlignIr::Left {
                    15.0 * list_level as f64 - first_stroke_pad_pt
                } else if inherited_align == HAlignIr::Left {
                    -first_stroke_pad_pt
                } else {
                    0.0
                };

                let tbl_align = match inherited_align {
                    HAlignIr::Center => HAlignIr::Center,
                    HAlignIr::Right => HAlignIr::Right,
                    _ => HAlignIr::Left,
                };

                self.report.tables_count += 1;
                blocks.push(BlockNode::Table(TableIr {
                    id: stack_id,
                    kind: TableKindIr::LayoutGrid,
                    col_widths_pt,
                    row_heights_pt: vec![0.0],
                    header_rows: 0,
                    footer_rows: 0,
                    rows: vec![TableRowIr {
                        height_pt: None,
                        is_header: false,
                        is_footer: false,
                        cells,
                    }],
                    align: tbl_align,
                    left_indent_pt,
                    summary: None,
                }));
            }
        }
    }
}
