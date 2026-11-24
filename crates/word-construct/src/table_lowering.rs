//! Word-specific container lowering and table refinement passes run during [`crate::build_word_document`]
//! while keeping [`ir::DocumentIr`] strictly immutable.

use ir::{
    BlockNode, ContainerIr, ContainerStyleIr, DocumentIr, HAlignIr, InlineNode, NodeId, ShapeKindIr,
    SidesPtIr, TableCellIr, TableColTrackIr, TableIr, TableKindIr, TableRowIr, VAlignIr,
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
pub(crate) fn refine_word_tables_in_blocks(
    blocks: &mut [BlockNode],
    ir: &DocumentIr,
    avail_content_w_pt: f64,
) {
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

        // 1b. Preserve Typst's measured minimum row height (`<w:trHeight w:hRule="atLeast"/>`)
        // on single-row `LayoutGrid`s containing tall inline math (e.g. `#choice` option grids
        // with `dfrac`), where Word's inline OMML `<m:f>` renders shorter than Typst's display
        // fraction and would otherwise compress vertical rhythm across the page.
        if tbl.kind == TableKindIr::LayoutGrid
            && tbl.rows.len() == 1
            && tbl.rows[0].height_pt.is_none()
            && !tbl.id.is_unset()
            && let Some(layout) = ir.layout(tbl.id)
            && let (Some((pa, top_y)), Some((pb, bottom_y))) = (
                layout.span.start_page_and_top_y(),
                layout.span.end_page_and_bottom_y(),
            )
            && pa == pb
        {
            let typst_h_pt = bottom_y - top_y;
            let has_tall_math = tbl.rows[0].cells.iter().any(|cell| {
                cell.blocks.iter().any(|b| match b {
                    BlockNode::Paragraph { content, .. } => {
                        crate::objects::equation::inlines_require_full_line_box(content)
                    }
                    _ => false,
                })
            });
            if has_tall_math && (16.0..=64.0).contains(&typst_h_pt) {
                tbl.rows[0].height_pt = Some(typst_h_pt);
            }
        }

        // 2. Prune degenerate `<= 0.5pt` empty columns when no cell uses `colspan > 1`.
        let has_colspan = tbl
            .rows
            .iter()
            .any(|r| r.cells.iter().any(|c| c.colspan > 1));
        if !has_colspan && tbl.cols.len() > 1 {
            let mut col = tbl.cols.len();
            while col > 0 {
                col -= 1;
                if tbl.cols.len() <= 1 || tbl.cols[col].width_pt() > 0.5 {
                    continue;
                }
                let all_empty = tbl.rows.iter().all(|r| {
                    r.cells
                        .iter()
                        .filter(|c| c.x == col)
                        .all(|c| c.blocks.is_empty())
                });
                if all_empty {
                    tbl.cols.remove(col);
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
            let n = tbl.cols.len();
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
                    let cur_w = tbl.cols[idx].width_pt();
                    if !is_single_line_cell(&row.cells[idx], ir) {
                        continue;
                    }
                    let want = (cur_w * 0.12).clamp(18.0, 48.0);
                    if idx + 1 < n
                        && is_spacer(&row.cells[idx + 1])
                        && tbl.cols[idx + 1].width_pt() > 48.0
                    {
                        let borrow =
                            ((tbl.cols[idx + 1].width_pt() - 24.0) * 0.5).clamp(0.0, want);
                        if borrow > 1.0 {
                            *tbl.cols[idx].width_pt_mut() += borrow;
                            *tbl.cols[idx + 1].width_pt_mut() -= borrow;
                        }
                    } else if idx > 0
                        && is_spacer(&row.cells[idx - 1])
                        && tbl.cols[idx - 1].width_pt() > 48.0
                    {
                        let borrow =
                            ((tbl.cols[idx - 1].width_pt() - 24.0) * 0.5).clamp(0.0, want);
                        if borrow > 1.0 {
                            *tbl.cols[idx - 1].width_pt_mut() -= borrow;
                            *tbl.cols[idx].width_pt_mut() += borrow;
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

        // 4. In `StyledContainer` cells (e.g. `#showybox` body rows), deduplicate horizontal
        //    double-indentation where both `cell.inset.left`/`right` (measured from the inner
        //    content bbox) and every direct indented child block carry the same horizontal inset.
        if tbl.kind == TableKindIr::StyledContainer {
            for row in &mut tbl.rows {
                for cell in &mut row.cells {
                    deduplicate_styled_container_cell_indents(cell);
                }
            }
        }

        // 5. Enforce Word single-line width floor for fixed-layout tables/grids (`<w:tblLayout w:type="fixed"/>`):
        //    Typst compresses fullwidth CJK punctuation (`：`, `，`, `。`, ...) to `0.5em` and uses
        //    its own font shaping, whereas Word measures fullwidth punctuation at `1.0em` and applies
        //    `<w:tcMar>` strictly inside `<w:tcW>`. Expand narrow single-line columns (into available
        //    page slack, or by borrowing from wide/spacer columns) so 1-line Typst cells never wrap
        //    onto 2 lines in Word.
        ensure_single_line_cell_word_widths(tbl, ir, avail_content_w_pt);
    }
}

fn ensure_single_line_cell_word_widths(
    tbl: &mut TableIr,
    ir: &DocumentIr,
    avail_content_w_pt: f64,
) {
    if tbl.kind == TableKindIr::StyledContainer || tbl.cols.is_empty() {
        return;
    }
    let n = tbl.cols.len();
    let num_rows = tbl.rows.len();
    let metrics = crate::WordFontMetricsProvider::new();

    let mut required_w = vec![0.0_f64; n];
    let mut has_multiline = vec![false; n];
    let mut is_empty_spacer = vec![true; n];

    for (row_idx, row) in tbl.rows.iter().enumerate() {
        for cell in &row.cells {
            if cell.x >= n {
                continue;
            }
            if !cell.blocks.is_empty() || cell.fill.is_some() || cell.borders.has_any() {
                for slot in is_empty_spacer
                    .iter_mut()
                    .take((cell.x + cell.colspan.max(1)).min(n))
                    .skip(cell.x)
                {
                    *slot = false;
                }
            }
            if cell.colspan != 1 {
                continue;
            }
            if !is_single_line_cell(cell, ir) {
                if !cell.blocks.is_empty() {
                    has_multiline[cell.x] = true;
                }
                continue;
            }
            let Some(text_w_pt) = estimate_single_line_cell_word_width_pt(cell, ir, &metrics)
            else {
                continue;
            };
            let cell_role = crate::table::resolve_table_cell_role(tbl.kind, &cell.blocks);
            let margins = crate::WordCellMarginsDxa::solve_for_row(
                cell.inset.top,
                cell.inset.bottom,
                cell.inset.left,
                cell.inset.right,
                cell_role,
                false,
                row_idx + 1 == num_rows,
            );
            let horiz_mar_pt = (margins.left_dxa + margins.right_dxa) as f64 / 20.0;
            let cushion_pt = (text_w_pt * 0.02).clamp(2.0, 3.5);
            required_w[cell.x] = required_w[cell.x].max(text_w_pt + horiz_mar_pt + cushion_pt);
        }
    }

    let max_table_w = (avail_content_w_pt - tbl.left_indent_pt.max(0.0)).max(72.0);
    let mut total_w: f64 = tbl.total_width_pt();

    for col in 0..n {
        let (cur_w, allow_page_slack_growth) = match tbl.cols[col] {
            // `Auto` columns are content-sized by definition: expand into page slack or borrow
            // from flexible siblings so Word never wraps single-line Typst cells.
            TableColTrackIr::Auto { width_pt } => (width_pt, true),
            // `Fixed` columns reflect explicit author lengths (`100pt`, `30%`, ...): preserve
            // author-specified widths rather than distorting fixed tracks.
            TableColTrackIr::Fixed { .. } => continue,
            // `Fraction` columns (`1fr`, `2fr`) already consume remaining width: only rebalance
            // against empty spacers or wider flexible siblings without expanding total table width.
            TableColTrackIr::Fraction { width_pt, .. } => (width_pt, false),
        };
        let req_w = required_w[col];
        if req_w <= cur_w + 0.5 {
            continue;
        }
        let mut need = (req_w - cur_w).min((cur_w * 0.45).max(16.0));

        // Phase A: Narrow table with `Auto` track -> grow directly into available page slack.
        if allow_page_slack_growth {
            let page_slack = (max_table_w - total_w).max(0.0);
            if page_slack > 0.5 {
                let grow = need.min(page_slack);
                *tbl.cols[col].width_pt_mut() += grow;
                total_w += grow;
                need -= grow;
            }
        }

        // Phase B: Borrow from empty spacers or flexible (`Fraction` / wide `Auto`) siblings,
        // never stealing width from author-`Fixed` column tracks.
        if need > 0.5 && n > 1 {
            let mut donors: Vec<(usize, u8, f64)> = (0..n)
                .filter(|&j| j != col)
                .filter_map(|j| {
                    let (wj, priority) = match tbl.cols[j] {
                        TableColTrackIr::Fraction { width_pt, .. } if is_empty_spacer[j] => {
                            (width_pt, 4)
                        }
                        TableColTrackIr::Auto { width_pt } if is_empty_spacer[j] => (width_pt, 3),
                        TableColTrackIr::Fraction { width_pt, .. } => (width_pt, 2),
                        TableColTrackIr::Auto { width_pt } => (width_pt, 1),
                        TableColTrackIr::Fixed { .. } => return None,
                    };
                    let floor_j = if is_empty_spacer[j] {
                        24.0
                    } else if has_multiline[j] {
                        (wj * 0.75).max(48.0)
                    } else {
                        required_w[j].max(24.0) + 12.0
                    };
                    let spare = (wj - floor_j).max(0.0);
                    (spare > 2.0).then_some((j, priority, spare))
                })
                .collect();
            donors.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
            });
            for (j, _, spare) in donors {
                if need <= 0.5 {
                    break;
                }
                let take = need.min(spare);
                *tbl.cols[col].width_pt_mut() += take;
                *tbl.cols[j].width_pt_mut() -= take;
                need -= take;
            }
        }
    }
}

fn estimate_single_line_cell_word_width_pt(
    cell: &TableCellIr,
    ir: &DocumentIr,
    metrics: &crate::WordFontMetricsProvider,
) -> Option<f64> {
    if cell.blocks.len() != 1 {
        return None;
    }
    let (id, content, default_size_pt) = match &cell.blocks[0] {
        BlockNode::Paragraph {
            id, content, style, ..
        } => (
            *id,
            content.as_slice(),
            style.resolved_font_size_pt.unwrap_or(11.0),
        ),
        BlockNode::Heading { id, content, .. } => (*id, content.as_slice(), 11.0),
        _ => return None,
    };
    if content.is_empty() {
        return None;
    }
    let word_w = estimate_inlines_word_advance_pt(content, default_size_pt, ir, metrics)?;
    let typst_w = (!id.is_unset())
        .then(|| {
            ir.layout(id)
                .and_then(|l| l.fragments.first())
                .map(|f| f.width_pt)
        })
        .flatten()
        .unwrap_or(0.0);
    Some(word_w.max(typst_w))
}

fn estimate_inlines_word_advance_pt(
    inlines: &[InlineNode],
    default_size_pt: f64,
    ir: &DocumentIr,
    metrics: &crate::WordFontMetricsProvider,
) -> Option<f64> {
    let mut width_pt = 0.0;
    for inl in inlines {
        match inl {
            InlineNode::Text(r) => {
                let sz = r.font_size_pt.unwrap_or(default_size_pt);
                let face = r
                    .font_family
                    .as_deref()
                    .and_then(|fam| ir.font_metrics.get(fam));
                width_pt +=
                    metrics.text_advance_pt(r.font_family.as_deref(), face, sz, r.bold, &r.text);
                if let Some(tr) = r.tracking_pt {
                    let chars = r.text.chars().count();
                    if chars > 1 {
                        width_pt += tr * (chars - 1) as f64;
                    }
                }
            }
            InlineNode::HorizontalSpace { width_pt: w, .. } => width_pt += *w,
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. }
            | InlineNode::Field { content, .. } => {
                width_pt +=
                    estimate_inlines_word_advance_pt(content, default_size_pt, ir, metrics)?;
            }
            InlineNode::LineBreak | InlineNode::Tab => return None,
            _ => return None,
        }
    }
    (width_pt > 0.5).then_some(width_pt)
}

fn deduplicate_styled_container_cell_indents(cell: &mut TableCellIr) {
    if cell.inset.left >= 1.0 {
        let mut min_left: Option<f64> = None;
        for block in &cell.blocks {
            let block_left = match block {
                BlockNode::Paragraph { style, .. } => Some(style.left_indent_pt),
                BlockNode::Heading { left_indent_pt, .. } => Some(*left_indent_pt),
                BlockNode::List { indent_pt, .. } | BlockNode::Enum { indent_pt, .. } => {
                    Some(*indent_pt)
                }
                BlockNode::Table(t) => Some(t.left_indent_pt),
                BlockNode::Container(c) => Some(c.left_indent_pt),
                _ => None,
            };
            if let Some(l) = block_left {
                min_left = Some(min_left.map_or(l, |cur| cur.min(l)));
            }
        }
        if let Some(shared_left) = min_left
            && shared_left >= 1.0
            && shared_left >= cell.inset.left - 1.5
        {
            let deduct = cell.inset.left.min(shared_left);
            for block in &mut cell.blocks {
                match block {
                    BlockNode::Paragraph { style, .. } => {
                        style.left_indent_pt = (style.left_indent_pt - deduct).max(0.0);
                    }
                    BlockNode::Heading { left_indent_pt, .. } => {
                        *left_indent_pt = (*left_indent_pt - deduct).max(0.0);
                    }
                    BlockNode::List { indent_pt, .. } | BlockNode::Enum { indent_pt, .. } => {
                        *indent_pt = (*indent_pt - deduct).max(0.0);
                    }
                    BlockNode::Table(t) => {
                        t.left_indent_pt = (t.left_indent_pt - deduct).max(0.0);
                    }
                    BlockNode::Container(c) => {
                        c.left_indent_pt = (c.left_indent_pt - deduct).max(0.0);
                    }
                    _ => {}
                }
            }
        }
    }

    if cell.inset.right >= 1.0 {
        let mut min_right: Option<f64> = None;
        for block in &cell.blocks {
            if let BlockNode::Paragraph { style, .. } = block {
                min_right = Some(min_right.map_or(style.right_indent_pt, |cur| {
                    cur.min(style.right_indent_pt)
                }));
            }
        }
        if let Some(shared_right) = min_right
            && shared_right >= 1.0
            && shared_right >= cell.inset.right - 1.5
        {
            let deduct = cell.inset.right.min(shared_right);
            for block in &mut cell.blocks {
                if let BlockNode::Paragraph { style, .. } = block {
                    style.right_indent_pt = (style.right_indent_pt - deduct).max(0.0);
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

/// Merge adjacent lowered 1-column [`TableKindIr::StyledContainer`] tables (e.g. a header bar
/// followed by a matching body box or cross-page split styled container fragments) into a single
/// Word `<w:tbl>`.
pub(crate) fn merge_split_styled_containers(blocks: &mut Vec<BlockNode>) {
    let mut i = 1;
    while i < blocks.len() {
        let merge_mode = match (&blocks[i - 1], &blocks[i]) {
            (BlockNode::Table(a), BlockNode::Table(b))
                if a.kind == TableKindIr::StyledContainer
                    && b.kind == TableKindIr::StyledContainer
                    && (1..=2).contains(&a.rows.len())
                    && b.rows.len() == 1
                    && a.rows.iter().all(|r| r.cells.len() == 1)
                    && b.rows[0].cells.len() == 1 =>
            {
                let a_last = &a.rows.last().unwrap().cells[0];
                let b_cell = &b.rows[0].cells[0];
                if a_last.fill == b_cell.fill && a_last.borders == b_cell.borders {
                    Some(true)
                } else if a.rows.len() == 1
                    && a_last.borders.left == b_cell.borders.left
                    && a_last.borders.right == b_cell.borders.right
                    && a_last.blocks.len() == 1
                    && matches!(b_cell.blocks.first(), Some(BlockNode::CodeBlock { .. }))
                {
                    Some(false)
                } else {
                    None
                }
            }
            _ => None,
        };
        let Some(append_into_last_row) = merge_mode else {
            i += 1;
            continue;
        };

        let BlockNode::Table(mut b) = blocks.remove(i) else {
            unreachable!()
        };
        if let BlockNode::Table(a) = &mut blocks[i - 1] {
            if !append_into_last_row {
                let mut body_row = b.rows.remove(0);
                body_row.cells[0].y = 1;
                a.rows.push(body_row);
                a.row_heights_pt.push(0.0);
                continue;
            }
            let b_blocks = std::mem::take(&mut b.rows[0].cells[0].blocks);
            let a_blocks = &mut a.rows.last_mut().unwrap().cells[0].blocks;
            merge_styled_container_child_blocks(a_blocks, b_blocks);
        }
    }
}

fn merge_styled_container_child_blocks(
    a_blocks: &mut Vec<BlockNode>,
    mut b_blocks: Vec<BlockNode>,
) {
    if !b_blocks.is_empty() {
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
        } else if let (
            Some(BlockNode::CodeBlock {
                lines: a_lines,
                fill: a_fill,
                stroke: a_stroke,
                ..
            }),
            Some(BlockNode::CodeBlock {
                fill: b_fill,
                stroke: b_stroke,
                ..
            }),
        ) = (a_blocks.last_mut(), b_blocks.first())
            && *a_fill == *b_fill
            && *a_stroke == *b_stroke
        {
            if let BlockNode::CodeBlock { lines: b_lines, .. } = b_blocks.remove(0) {
                a_lines.extend(b_lines);
            }
        }
    }
    a_blocks.extend(b_blocks);
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
        cols: vec![TableColTrackIr::Fixed { width_pt }],
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
