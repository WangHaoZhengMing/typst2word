//! Structural IR normalization and recursive tree traversal helpers (`ir::normalize`).
//!
//! Performs Word-independent structural cleanup on [`DocumentIr`]:
//! 1. Coalescing adjacent compatible [`InlineNode::Text`] runs split by Typst's layout engine.
//! 2. Identity-first merging of [`BlockNode::Container`] / [`BlockNode::CodeBlock`] nodes split across
//!    Typst columns/pages (preserving 1-to-N `LayoutFragmentIr` slices in `DocumentIr::reference_layout`).
//! 3. Cleaning leading, trailing, or duplicate consecutive [`BlockNode::PageBreak`] markers.

use std::collections::BTreeMap;

use crate::{
    BlockLayoutSpanIr, BlockNode, DocumentIr, InlineNode, NodeId, NodeOrigin, NodeReferenceLayout,
    PageSetupIr, TableKindIr, TableRowIr,
};

/// Normalize `DocumentIr` structure (coalesce adjacent text runs, merge split containers,
/// and clean redundant page breaks).
pub fn normalize_document_ir(doc: &mut DocumentIr) {
    visit_document_inline_lists_mut(doc, &mut coalesce_adjacent_text_runs);
    merge_split_containers(doc);
    visit_document_block_lists_mut(doc, &mut merge_split_styled_containers);
    clean_redundant_pagebreaks(&mut doc.blocks);
    for section in &mut doc.sections {
        clean_redundant_pagebreaks(&mut section.blocks);
    }
}

/// Merge adjacent `InlineNode::Text` runs that share identical formatting, font family,
/// tracking, baseline shift, text outline, and locale.
pub fn coalesce_adjacent_text_runs(inlines: &mut Vec<InlineNode>) {
    let mut i = 1;
    while i < inlines.len() {
        let can_merge = match (&inlines[i - 1], &inlines[i]) {
            (InlineNode::Text(a), InlineNode::Text(b)) => {
                a.font_family == b.font_family
                    && a.font_size_pt.map(|v| (v * 10.0).round() as i64)
                        == b.font_size_pt.map(|v| (v * 10.0).round() as i64)
                    && a.bold == b.bold
                    && a.italic == b.italic
                    && a.underline == b.underline
                    && a.strike == b.strike
                    && a.overline == b.overline
                    && a.color == b.color
                    && a.highlight == b.highlight
                    && a.border == b.border
                    && a.script == b.script
                    && a.smallcaps == b.smallcaps
                    && a.is_code == b.is_code
                    && a.tracking_pt.map(|v| (v * 20.0).round() as i64)
                        == b.tracking_pt.map(|v| (v * 20.0).round() as i64)
                    && a.baseline_shift_pt.map(|v| (v * 20.0).round() as i64)
                        == b.baseline_shift_pt.map(|v| (v * 20.0).round() as i64)
                    && a.text_outline == b.text_outline
                    && a.locale == b.locale
            }
            _ => false,
        };
        if can_merge {
            let InlineNode::Text(b) = inlines.remove(i) else {
                unreachable!()
            };
            if let InlineNode::Text(a) = &mut inlines[i - 1] {
                a.text.push_str(&b.text);
            }
        } else {
            i += 1;
        }
    }
}

/// Merge split `BlockNode::Container` and `BlockNode::CodeBlock` fragments across `doc` using
/// `NodeOrigin.typst_location` identity first (and preserving 1-to-N `ReferenceLayout` fragments),
/// falling back to border/fill continuity only when neither fragment carries a `typst_location`.
pub fn merge_split_containers(doc: &mut DocumentIr) {
    let mut origins = std::mem::take(&mut doc.origins);
    let mut layouts = std::mem::take(&mut doc.reference_layout);

    let mut merge_fn = |blocks: &mut Vec<BlockNode>| {
        merge_block_list_by_identity(blocks, &origins, &mut layouts);
    };
    visit_page_setup_blocks_mut(&mut doc.page_setup, &mut merge_fn);
    visit_block_list_mut(&mut doc.blocks, &mut merge_fn);
    for section in &mut doc.sections {
        visit_page_setup_blocks_mut(&mut section.page_setup, &mut merge_fn);
        visit_block_list_mut(&mut section.blocks, &mut merge_fn);
    }

    // Prune orphaned origin entries from merged secondary fragments if desired (keep primary IDs).
    origins.retain(|id, _| !id.is_unset());
    doc.origins = origins;
    doc.reference_layout = layouts;
}

fn merge_reference_layouts(
    layouts: &mut BTreeMap<NodeId, NodeReferenceLayout>,
    keep_id: NodeId,
    removed_id: NodeId,
) {
    if keep_id.is_unset() || removed_id.is_unset() || keep_id == removed_id {
        return;
    }
    let Some(removed) = layouts.remove(&removed_id) else {
        return;
    };
    let dst = layouts.entry(keep_id).or_default();
    dst.fragments.extend(removed.fragments);
    if !dst.fragments.is_empty() {
        dst.span = BlockLayoutSpanIr::from_fragments(&dst.fragments);
    } else {
        dst.span = dst.span.merge_with(removed.span);
    }
    if let Some(rem_flow) = removed.flow {
        let dst_flow = dst.flow.get_or_insert_with(Default::default);
        dst_flow.self_span = dst.span;
        if rem_flow.next.is_some() {
            dst_flow.next = rem_flow.next;
        }
        dst_flow
            .trailing_transitions
            .extend(rem_flow.trailing_transitions);
    }
    if dst.paragraph_lines.is_none() {
        dst.paragraph_lines = removed.paragraph_lines;
    }
}

fn same_nonzero_typst_location(
    origins: &BTreeMap<NodeId, NodeOrigin>,
    a: NodeId,
    b: NodeId,
) -> Option<bool> {
    let loc_a = origins.get(&a).and_then(|o| o.typst_location);
    let loc_b = origins.get(&b).and_then(|o| o.typst_location);
    match (loc_a, loc_b) {
        (Some(la), Some(lb)) => Some(la == lb),
        _ => None,
    }
}

fn merge_block_list_by_identity(
    blocks: &mut Vec<BlockNode>,
    origins: &BTreeMap<NodeId, NodeOrigin>,
    layouts: &mut BTreeMap<NodeId, NodeReferenceLayout>,
) {
    let mut i = 1;
    while i < blocks.len() {
        let should_merge_containers = match (&blocks[i - 1], &blocks[i]) {
            (BlockNode::Container(a), BlockNode::Container(b)) => {
                match same_nonzero_typst_location(origins, a.id, b.id) {
                    Some(same) => same,
                    None => {
                        a.style.fill == b.style.fill
                            && a.style.borders.left == b.style.borders.left
                            && a.style.borders.right == b.style.borders.right
                            && (a.style.borders == b.style.borders
                                || (a.style.borders.bottom.is_none()
                                    && b.style.borders.top.is_none()))
                    }
                }
            }
            _ => false,
        };

        if should_merge_containers {
            let BlockNode::Container(mut b) = blocks.remove(i) else {
                unreachable!()
            };
            if let BlockNode::Container(a) = &mut blocks[i - 1] {
                merge_reference_layouts(layouts, a.id, b.id);
                if a.style.borders.bottom.is_none() && b.style.borders.bottom.is_some() {
                    a.style.borders.bottom = b.style.borders.bottom.take();
                }
                if a.style.inset.bottom == 0.0 && b.style.inset.bottom > 0.0 {
                    a.style.inset.bottom = b.style.inset.bottom;
                }
                merge_continuation_child_blocks(&mut a.body, b.body, origins, layouts);
            }
            continue;
        }

        let should_merge_code = match (&blocks[i - 1], &blocks[i]) {
            (BlockNode::CodeBlock { id: id_a, .. }, BlockNode::CodeBlock { id: id_b, .. }) => {
                same_nonzero_typst_location(origins, *id_a, *id_b) == Some(true)
            }
            _ => false,
        };

        if should_merge_code {
            let BlockNode::CodeBlock {
                id: id_b,
                lines: b_lines,
                ..
            } = blocks.remove(i)
            else {
                unreachable!()
            };
            if let BlockNode::CodeBlock {
                id: id_a,
                lines: a_lines,
                ..
            } = &mut blocks[i - 1]
            {
                merge_reference_layouts(layouts, *id_a, id_b);
                a_lines.extend(b_lines);
                normalize_code_lines_font(a_lines);
            }
            continue;
        }

        i += 1;
    }
}

fn merge_continuation_child_blocks(
    a_blocks: &mut Vec<BlockNode>,
    mut b_blocks: Vec<BlockNode>,
    origins: &BTreeMap<NodeId, NodeOrigin>,
    layouts: &mut BTreeMap<NodeId, NodeReferenceLayout>,
) {
    if !b_blocks.is_empty() {
        if let (
            Some(BlockNode::Enum {
                id: a_id,
                items: a_items,
                level: a_lvl,
                ..
            }),
            Some(BlockNode::Enum { level: b_lvl, .. }),
        ) = (a_blocks.last_mut(), b_blocks.first())
            && *a_lvl == *b_lvl
        {
            if let BlockNode::Enum {
                id: b_id,
                items: b_items,
                ..
            } = b_blocks.remove(0)
            {
                merge_reference_layouts(layouts, *a_id, b_id);
                a_items.extend(b_items);
            }
        } else if let (
            Some(BlockNode::List {
                id: a_id,
                items: a_items,
                level: a_lvl,
                ..
            }),
            Some(BlockNode::List { level: b_lvl, .. }),
        ) = (a_blocks.last_mut(), b_blocks.first())
            && *a_lvl == *b_lvl
        {
            if let BlockNode::List {
                id: b_id,
                items: b_items,
                ..
            } = b_blocks.remove(0)
            {
                merge_reference_layouts(layouts, *a_id, b_id);
                a_items.extend(b_items);
            }
        } else if let (
            Some(BlockNode::CodeBlock {
                id: a_id,
                lines: a_lines,
                fill: a_fill,
                stroke: a_stroke,
                ..
            }),
            Some(BlockNode::CodeBlock {
                id: b_id,
                fill: b_fill,
                stroke: b_stroke,
                ..
            }),
        ) = (a_blocks.last_mut(), b_blocks.first())
            && (same_nonzero_typst_location(origins, *a_id, *b_id) == Some(true)
                || (*a_fill == *b_fill && *a_stroke == *b_stroke))
        {
            if let BlockNode::CodeBlock {
                id: b_id,
                lines: b_lines,
                ..
            } = b_blocks.remove(0)
            {
                merge_reference_layouts(layouts, *a_id, b_id);
                a_lines.extend(b_lines);
                normalize_code_lines_font(a_lines);
            }
        }
    }
    a_blocks.extend(b_blocks);
}

fn normalize_code_lines_font(lines: &mut [Vec<crate::TextRunIr>]) {
    let base_font = lines
        .iter()
        .flatten()
        .find(|r| r.font_size_pt.is_some())
        .map(|r| (r.font_family.clone(), r.font_size_pt));
    if let Some((fam, sz)) = base_font {
        for line in lines.iter_mut() {
            for run in line.iter_mut() {
                if run.font_size_pt.is_none() {
                    run.font_size_pt = sz;
                }
                if run.font_family.is_none() {
                    run.font_family = fam.clone();
                }
            }
        }
    }
}

/// Merge adjacent lowered 1-cell `TableKindIr::StyledContainer` tables if present.
pub fn merge_split_styled_containers(blocks: &mut Vec<BlockNode>) {
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
            let b_cell = &mut b.rows[0].cells[0];
            let mut b_blocks = std::mem::take(&mut b_cell.blocks);
            let starts_with_single_list_item = a.rows.len() == 1
                && matches!(
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
                    is_footer: false,
                    cells: vec![row1_cell],
                });
                a.rows.push(TableRowIr {
                    height_pt: None,
                    is_header: false,
                    is_footer: false,
                    cells: vec![row2_cell],
                });
            } else {
                let empty_origins = BTreeMap::new();
                let mut empty_layouts = BTreeMap::new();
                let a_blocks = &mut a.rows.last_mut().unwrap().cells[0].blocks;
                merge_continuation_child_blocks(
                    a_blocks,
                    b_blocks,
                    &empty_origins,
                    &mut empty_layouts,
                );
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

/// Visit all `Vec<BlockNode>` sequences inside a [`PageSetupIr`] (default, first-page, and even-page furniture).
pub fn visit_page_setup_blocks_mut<F>(setup: &mut PageSetupIr, f: &mut F)
where
    F: FnMut(&mut Vec<BlockNode>),
{
    visit_block_list_mut(&mut setup.header, f);
    visit_block_list_mut(&mut setup.footer, f);
    if let Some(fh) = &mut setup.first_page_header {
        visit_block_list_mut(fh, f);
    }
    if let Some(ff) = &mut setup.first_page_footer {
        visit_block_list_mut(ff, f);
    }
    if let Some(eh) = &mut setup.even_page_header {
        visit_block_list_mut(eh, f);
    }
    if let Some(ef) = &mut setup.even_page_footer {
        visit_block_list_mut(ef, f);
    }
}

/// Recursively visit all `Vec<BlockNode>` sequences in a `DocumentIr` (bottom-up: children first, then parent).
pub fn visit_document_block_lists_mut<F>(doc: &mut DocumentIr, f: &mut F)
where
    F: FnMut(&mut Vec<BlockNode>),
{
    visit_page_setup_blocks_mut(&mut doc.page_setup, f);
    visit_block_list_mut(&mut doc.blocks, f);
    for section in &mut doc.sections {
        visit_page_setup_blocks_mut(&mut section.page_setup, f);
        visit_block_list_mut(&mut section.blocks, f);
    }
}

/// Recursively visit a single `Vec<BlockNode>` and all nested `Vec<BlockNode>` lists inside it.
pub fn visit_block_list_mut<F>(blocks: &mut Vec<BlockNode>, f: &mut F)
where
    F: FnMut(&mut Vec<BlockNode>),
{
    for block in blocks.iter_mut() {
        visit_block_children_lists_mut(block, f);
    }
    f(blocks);
}

fn visit_block_children_lists_mut<F>(block: &mut BlockNode, f: &mut F)
where
    F: FnMut(&mut Vec<BlockNode>),
{
    match block {
        BlockNode::Title { content, .. }
        | BlockNode::Heading { content, .. }
        | BlockNode::Paragraph { content, .. } => {
            visit_inlines_block_lists_mut(content, f);
        }
        BlockNode::List { items, .. } => {
            for item in items {
                visit_block_list_mut(&mut item.blocks, f);
            }
        }
        BlockNode::Enum { items, .. } => {
            for item in items {
                visit_block_list_mut(&mut item.blocks, f);
            }
        }
        BlockNode::Terms { items, .. } => {
            for item in items {
                visit_inlines_block_lists_mut(&mut item.term, f);
                visit_block_list_mut(&mut item.description, f);
            }
        }
        BlockNode::Container(container) => {
            visit_block_list_mut(&mut container.body, f);
        }
        BlockNode::Table(table) => {
            for row in &mut table.rows {
                for cell in &mut row.cells {
                    visit_block_list_mut(&mut cell.blocks, f);
                }
            }
        }
        BlockNode::Quote {
            body, attribution, ..
        } => {
            visit_block_list_mut(body, f);
            if let Some(attr) = attribution {
                visit_inlines_block_lists_mut(attr, f);
            }
        }
        BlockNode::Figure { body, caption, .. } => {
            visit_block_list_mut(body, f);
            if let Some(cap) = caption {
                visit_inlines_block_lists_mut(cap, f);
            }
        }
        BlockNode::ShapeBlock(shape) => {
            visit_block_list_mut(&mut shape.body, f);
        }
        BlockNode::CetzCanvas(canvas) => {
            for lbl in &mut canvas.labels {
                visit_inlines_block_lists_mut(&mut lbl.content, f);
            }
        }
        BlockNode::Outline { title, entries } => {
            if let Some(t) = title {
                visit_inlines_block_lists_mut(t, f);
            }
            for entry in entries {
                visit_inlines_block_lists_mut(&mut entry.title, f);
            }
        }
        BlockNode::Bibliography { title, entries } => {
            if let Some(t) = title {
                visit_inlines_block_lists_mut(t, f);
            }
            for entry in entries {
                visit_inlines_block_lists_mut(&mut entry.content, f);
            }
        }
        BlockNode::Columns { body, .. } => {
            visit_block_list_mut(body, f);
        }
        BlockNode::CodeBlock { .. }
        | BlockNode::EquationBlock(_)
        | BlockNode::ImageBlock(_)
        | BlockNode::HorizontalRule { .. }
        | BlockNode::PageBreak
        | BlockNode::ColumnBreak
        | BlockNode::UnsupportedBlock(_) => {}
    }
}

fn visit_inlines_block_lists_mut<F>(inlines: &mut [InlineNode], f: &mut F)
where
    F: FnMut(&mut Vec<BlockNode>),
{
    for inline in inlines {
        match inline {
            InlineNode::Field { content, .. }
            | InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. } => {
                visit_inlines_block_lists_mut(content, f);
            }
            InlineNode::Footnote { content, .. } => {
                visit_block_list_mut(content, f);
            }
            InlineNode::ShapeInline(shape) => {
                visit_block_list_mut(&mut shape.body, f);
            }
            InlineNode::CetzInline(canvas) => {
                for lbl in &mut canvas.labels {
                    visit_inlines_block_lists_mut(&mut lbl.content, f);
                }
            }
            InlineNode::Text(_)
            | InlineNode::EquationInline(_)
            | InlineNode::ImageInline(_)
            | InlineNode::LineBreak
            | InlineNode::Tab
            | InlineNode::HorizontalSpace { .. }
            | InlineNode::UnsupportedInline(_) => {}
        }
    }
}

/// Recursively visit all `Vec<InlineNode>` sequences in a `DocumentIr`.
pub fn visit_document_inline_lists_mut<F>(doc: &mut DocumentIr, f: &mut F)
where
    F: FnMut(&mut Vec<InlineNode>),
{
    visit_document_block_lists_mut(doc, &mut |blocks| {
        for block in blocks.iter_mut() {
            match block {
                BlockNode::Title { content, .. }
                | BlockNode::Heading { content, .. }
                | BlockNode::Paragraph { content, .. } => {
                    visit_inline_list_recursive_mut(content, f);
                }
                BlockNode::Terms { items, .. } => {
                    for item in items {
                        visit_inline_list_recursive_mut(&mut item.term, f);
                    }
                }
                BlockNode::Quote { attribution, .. } => {
                    if let Some(attr) = attribution {
                        visit_inline_list_recursive_mut(attr, f);
                    }
                }
                BlockNode::Figure { caption, .. } => {
                    if let Some(cap) = caption {
                        visit_inline_list_recursive_mut(cap, f);
                    }
                }
                BlockNode::Outline { title, entries } => {
                    if let Some(t) = title {
                        visit_inline_list_recursive_mut(t, f);
                    }
                    for entry in entries {
                        visit_inline_list_recursive_mut(&mut entry.title, f);
                    }
                }
                BlockNode::Bibliography { title, entries } => {
                    if let Some(t) = title {
                        visit_inline_list_recursive_mut(t, f);
                    }
                    for entry in entries {
                        visit_inline_list_recursive_mut(&mut entry.content, f);
                    }
                }
                BlockNode::CetzCanvas(canvas) => {
                    for lbl in &mut canvas.labels {
                        visit_inline_list_recursive_mut(&mut lbl.content, f);
                    }
                }
                _ => {}
            }
        }
    });
}

fn visit_inline_list_recursive_mut<F>(inlines: &mut Vec<InlineNode>, f: &mut F)
where
    F: FnMut(&mut Vec<InlineNode>),
{
    for inline in inlines.iter_mut() {
        match inline {
            InlineNode::Field { content, .. }
            | InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. } => {
                visit_inline_list_recursive_mut(content, f);
            }
            InlineNode::CetzInline(canvas) => {
                for lbl in &mut canvas.labels {
                    visit_inline_list_recursive_mut(&mut lbl.content, f);
                }
            }
            _ => {}
        }
    }
    f(inlines);
}
