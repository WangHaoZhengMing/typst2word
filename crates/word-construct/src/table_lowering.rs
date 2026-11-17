//! Word-specific container lowering and table refinement passes run during [`crate::build_word_document`]
//! while keeping [`ir::DocumentIr`] strictly immutable.

use ir::{
    BlockNode, ContainerIr, ContainerStyleIr, DocumentIr, HAlignIr, InlineNode, NodeId, ShapeKindIr,
    SidesPtIr, TableCellIr, TableIr, TableKindIr, TableRowIr, VAlignIr,
};

/// Promote standalone rectangular partial-border `InlineNode::ShapeInline` boxes
/// (e.g. `#box(width: 100%, stroke: (left: 2pt + blue), fill: ...)[...]`) into flow
/// `BlockNode::Container`s (`breakable: false`) because DrawingML `<a:ln>` cannot stroke
/// individual sides of a rectangle whereas Word 1x1 `<w:tbl>` (`<w:tcBorders>`) can.
pub(crate) fn promote_standalone_partial_border_inline_boxes(
    blocks: &mut Vec<BlockNode>,
    ir: &DocumentIr,
) {
    let mut i = 0;
    while i < blocks.len() {
        let split_idx = match &blocks[i] {
            BlockNode::Paragraph { content, .. } => content.iter().position(|inl| {
                if let InlineNode::ShapeInline(sh) = inl {
                    let has_partial_border = sh.stroke.is_none() && sh.borders.has_any();
                    let standalone = ir
                        .layout(sh.id)
                        .and_then(|l| l.inline_box_context.as_ref())
                        .is_none_or(|c| !c.shares_line_with_text);
                    sh.corner_radius_pt.is_none()
                        && has_partial_border
                        && standalone
                        && !sh.body.is_empty()
                } else {
                    false
                }
            }),
            _ => None,
        };

        let Some(pos) = split_idx else {
            i += 1;
            continue;
        };

        let BlockNode::Paragraph {
            id: orig_id,
            mut content,
            style,
            bookmark,
        } = blocks.remove(i)
        else {
            unreachable!()
        };

        let after_inlines = trim_edge_breaks(content.split_off(pos + 1));
        let Some(InlineNode::ShapeInline(sh)) = content.pop() else {
            unreachable!()
        };
        let before_inlines = trim_edge_breaks(content);

        let mut replacement = Vec::with_capacity(3);
        let mut used_orig_id = false;
        if !before_inlines.is_empty() {
            let mut b_style = style.clone();
            b_style.space_after_pt = style.resolved_par_leading_pt.unwrap_or(5.0).min(8.0);
            replacement.push(BlockNode::Paragraph {
                id: orig_id,
                content: before_inlines,
                style: b_style,
                bookmark: bookmark.clone(),
            });
            used_orig_id = true;
        }

        replacement.push(BlockNode::Container(ContainerIr {
            id: sh.id,
            width_pt: Some(sh.width_pt),
            height_pt: None,
            align: style.align,
            left_indent_pt: style.left_indent_pt,
            style: ContainerStyleIr {
                fill: sh.fill,
                borders: sh.borders,
                inset: sh.inset,
                corner_radius_pt: None,
            },
            breakable: false,
            sticky: false,
            body: sh.body,
        }));

        if !after_inlines.is_empty() {
            let mut a_style = style.clone();
            a_style.first_line_indent_pt = 0.0;
            replacement.push(BlockNode::Paragraph {
                id: if used_orig_id { NodeId::UNSET } else { orig_id },
                content: after_inlines,
                style: a_style,
                bookmark: if used_orig_id { None } else { bookmark },
            });
        }

        let added = replacement.len();
        blocks.splice(i..i, replacement);
        i += added.min(1);
    }
}

fn trim_edge_breaks(mut inlines: Vec<InlineNode>) -> Vec<InlineNode> {
    let is_blank = |inl: &InlineNode| match inl {
        InlineNode::LineBreak => true,
        InlineNode::Text(r) => {
            r.text.trim().is_empty() && r.underline.is_none() && r.highlight.is_none()
        }
        InlineNode::HorizontalSpace { .. } => true,
        _ => false,
    };
    while inlines.first().is_some_and(is_blank) {
        inlines.remove(0);
    }
    while inlines.last().is_some_and(is_blank) {
        inlines.pop();
    }
    inlines
}

/// Lower Word-specific `<w:tbl>` layout edge cases while keeping `DocumentIr` immutable:
/// 1. Prune degenerate `0%`-width (`<= 0.5pt`) empty columns (e.g. `17 / 17` progress bars).
/// 2. Convert ultra-thin (`0.5pt..=6.0pt`) unbordered solid-bar rows into cell-shaded rows
///    so Word `<wp:inline>` paragraph line boxes do not inflate `2pt` progress/divider bars.
/// 3. In 1-row `LayoutGrid`s, let tight `auto`-width single-line cells borrow slack width from
///    adjacent empty `1fr` spacer cells (preserving total width & outer edges) so counters like
///    `"17 / 17"` never wrap onto 2 lines due to minor Typst-vs-Word glyph advance differences.
pub(crate) fn refine_word_tables_in_blocks(blocks: &mut [BlockNode], ir: &DocumentIr) {
    for block in blocks.iter_mut() {
        let BlockNode::Table(tbl) = block else {
            continue;
        };

        // 1. Convert ultra-thin unbordered solid-bar rows (`0.5pt..=6.0pt`) to shaded empty cells.
        for row in &mut tbl.rows {
            let is_thin_row = row.height_pt.is_some_and(|h| (0.5..=6.0).contains(&h))
                && !row.cells.is_empty()
                && row.cells.iter().all(|c| {
                    !c.borders.has_any()
                        && ((c.blocks.is_empty() && c.fill.is_some())
                            || extract_solid_bar_fill(&c.blocks).is_some())
                });
            if is_thin_row {
                for cell in &mut row.cells {
                    if cell.fill.is_none() {
                        cell.fill = extract_solid_bar_fill(&cell.blocks);
                    }
                    cell.inset = SidesPtIr::zero();
                    cell.blocks.clear();
                }
            }
        }

        // 2. Prune degenerate `<= 0.5pt` empty columns when no cell uses `colspan > 1`.
        let has_colspan = tbl
            .rows
            .iter()
            .any(|r| r.cells.iter().any(|c| c.colspan > 1));
        if !has_colspan && tbl.col_widths_pt.len() > 1 {
            let mut col = tbl.col_widths_pt.len();
            while col > 0 {
                col -= 1;
                if tbl.col_widths_pt.len() <= 1 || tbl.col_widths_pt[col] > 0.5 {
                    continue;
                }
                let all_empty = tbl.rows.iter().all(|r| {
                    r.cells
                        .iter()
                        .filter(|c| c.x == col)
                        .all(|c| c.blocks.is_empty())
                });
                if all_empty {
                    tbl.col_widths_pt.remove(col);
                    for r in &mut tbl.rows {
                        r.cells.retain(|c| c.x != col);
                        for c in &mut r.cells {
                            if c.x > col {
                                c.x -= 1;
                            }
                        }
                    }
                }
            }
        }

        // 3. In 1-row `LayoutGrid`s, transfer slack from adjacent empty spacer cells to tight
        //    single-line `auto` cells while preserving total width and outer left/right edges.
        if tbl.kind == TableKindIr::LayoutGrid && tbl.rows.len() == 1 {
            let n = tbl.col_widths_pt.len();
            let row = &mut tbl.rows[0];
            let simple_grid = n >= 2
                && row.cells.len() == n
                && row
                    .cells
                    .iter()
                    .enumerate()
                    .all(|(i, c)| c.x == i && c.colspan == 1);
            if simple_grid {
                let is_spacer = |c: &TableCellIr| {
                    c.blocks.is_empty() && c.fill.is_none() && !c.borders.has_any()
                };
                for idx in 0..n {
                    let cur_w = tbl.col_widths_pt[idx];
                    if !is_single_line_cell(&row.cells[idx], ir) {
                        continue;
                    }
                    let want = (cur_w * 0.12).clamp(18.0, 48.0);
                    if idx + 1 < n
                        && is_spacer(&row.cells[idx + 1])
                        && tbl.col_widths_pt[idx + 1] > 48.0
                    {
                        let borrow =
                            ((tbl.col_widths_pt[idx + 1] - 24.0) * 0.5).clamp(0.0, want);
                        if borrow > 1.0 {
                            tbl.col_widths_pt[idx] += borrow;
                            tbl.col_widths_pt[idx + 1] -= borrow;
                        }
                    } else if idx > 0
                        && is_spacer(&row.cells[idx - 1])
                        && tbl.col_widths_pt[idx - 1] > 48.0
                    {
                        let borrow =
                            ((tbl.col_widths_pt[idx - 1] - 24.0) * 0.5).clamp(0.0, want);
                        if borrow > 1.0 {
                            tbl.col_widths_pt[idx - 1] -= borrow;
                            tbl.col_widths_pt[idx] += borrow;
                            row.cells[idx].halign = HAlignIr::Right;
                            if let Some(BlockNode::Paragraph { style, .. }) =
                                row.cells[idx].blocks.first_mut()
                                && matches!(style.align, HAlignIr::Left | HAlignIr::Right)
                            {
                                style.align = HAlignIr::Right;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn extract_solid_bar_fill(blocks: &[BlockNode]) -> Option<String> {
    if blocks.len() != 1 {
        return None;
    }
    let sh = match &blocks[0] {
        BlockNode::ShapeBlock(sh) => sh,
        BlockNode::Paragraph { content, .. } if content.len() == 1 => match &content[0] {
            InlineNode::ShapeInline(sh) => sh,
            _ => return None,
        },
        _ => return None,
    };
    (matches!(sh.kind, ShapeKindIr::Rect)
        && sh.body.is_empty()
        && sh.stroke.is_none()
        && !sh.borders.has_any()
        && sh.height_pt <= 6.5)
        .then(|| sh.fill.clone())
        .flatten()
}

fn is_single_line_cell(cell: &TableCellIr, ir: &DocumentIr) -> bool {
    if cell.blocks.len() != 1 {
        return false;
    }
    match &cell.blocks[0] {
        BlockNode::Paragraph { id, content, .. } => {
            if content.iter().any(|inl| matches!(inl, InlineNode::LineBreak)) {
                return false;
            }
            if !id.is_unset()
                && let Some(pl) = ir.layout(*id).and_then(|l| l.paragraph_lines.as_ref())
            {
                return pl.line_count == 1;
            }
            is_single_short_line_cell(cell)
        }
        BlockNode::Heading { id, content, .. } => {
            if content.iter().any(|inl| matches!(inl, InlineNode::LineBreak)) {
                return false;
            }
            !id.is_unset()
                && ir
                    .layout(*id)
                    .and_then(|l| l.paragraph_lines.as_ref())
                    .is_some_and(|pl| pl.line_count == 1)
        }
        _ => false,
    }
}

fn is_single_short_line_cell(cell: &TableCellIr) -> bool {
    if cell.blocks.len() != 1 {
        return false;
    }
    let BlockNode::Paragraph { content, .. } = &cell.blocks[0] else {
        return false;
    };
    let mut chars = 0usize;
    for inl in content {
        match inl {
            InlineNode::LineBreak => return false,
            InlineNode::Text(r) => chars += r.text.chars().count(),
            InlineNode::HorizontalSpace { .. } => chars += 1,
            _ => return false,
        }
    }
    (1..=24).contains(&chars)
}

/// Lower semantic [`BlockNode::Container`] nodes into Word 1x1 [`TableKindIr::StyledContainer`] tables.
pub fn lower_containers_in_blocks(blocks: &mut [BlockNode], fallback_width_pt: f64) {
    for block in blocks.iter_mut() {
        if let BlockNode::Container(_) = block {
            let BlockNode::Container(c) = std::mem::replace(block, BlockNode::PageBreak) else {
                unreachable!()
            };
            *block = BlockNode::Table(lower_container_to_table(c, fallback_width_pt));
        }
    }
}

fn lower_container_to_table(mut c: ContainerIr, fallback_width_pt: f64) -> TableIr {
    if !c.breakable {
        let len = c.body.len();
        for (idx, child) in c.body.iter_mut().enumerate() {
            if let BlockNode::Paragraph { style, .. } = child {
                style.breakable = false;
                if idx + 1 < len {
                    style.sticky = true;
                }
            }
        }
    }
    if c.sticky
        && let Some(BlockNode::Paragraph { style, .. }) = c.body.last_mut()
    {
        style.sticky = true;
    }
    let width_pt = c.width_pt.unwrap_or(fallback_width_pt).max(36.0);
    TableIr {
        id: c.id,
        kind: TableKindIr::StyledContainer,
        col_widths_pt: vec![width_pt],
        row_heights_pt: vec![0.0],
        header_rows: 0,
        footer_rows: 0,
        rows: vec![TableRowIr {
            height_pt: None,
            is_header: false,
            is_footer: false,
            cells: vec![TableCellIr {
                x: 0,
                y: 0,
                colspan: 1,
                rowspan: 1,
                fill: c.style.fill,
                borders: c.style.borders,
                inset: c.style.inset,
                halign: c.align,
                valign: VAlignIr::Top,
                is_header_cell: false,
                blocks: c.body,
            }],
        }],
        align: c.align,
        left_indent_pt: c.left_indent_pt,
        summary: None,
    }
}
