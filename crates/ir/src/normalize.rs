//! Structural IR normalization and recursive tree traversal helpers (`ir::normalize`).
//!
//! Performs Word-independent structural cleanup on [`DocumentIr`]:
//! 1. Coalescing adjacent compatible [`InlineNode::Text`] runs split by Typst's layout engine.
//! 2. Merging adjacent 1-cell [`TableKindIr::StyledContainer`] blocks split across Typst columns/pages.
//! 3. Cleaning leading, trailing, or duplicate consecutive [`BlockNode::PageBreak`] markers.

use crate::{BlockNode, DocumentIr, InlineNode, TableKindIr, TableRowIr};

/// Normalize `DocumentIr` structure (coalesce adjacent text runs, merge split styled containers,
/// and clean redundant page breaks).
pub fn normalize_document_ir(doc: &mut DocumentIr) {
    visit_document_inline_lists_mut(doc, &mut coalesce_adjacent_text_runs);
    visit_document_block_lists_mut(doc, &mut merge_split_styled_containers);
    clean_redundant_pagebreaks(&mut doc.blocks);
    for section in &mut doc.sections { clean_redundant_pagebreaks(&mut section.blocks); }
}

/// Merge adjacent `InlineNode::Text` runs that share identical formatting and font family.
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
                    && a.underline.is_some() == b.underline.is_some()
                    && a.strike.is_some() == b.strike.is_some()
                    && a.overline.is_some() == b.overline.is_some()
                    && a.color == b.color
                    && a.highlight == b.highlight
                    && a.border.is_some() == b.border.is_some()
                    && a.script == b.script
                    && a.smallcaps == b.smallcaps
                    && a.is_code == b.is_code
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

/// Recursively visit all `Vec<BlockNode>` sequences in a `DocumentIr` (bottom-up: children first, then parent).
pub fn visit_document_block_lists_mut<F>(doc: &mut DocumentIr, f: &mut F)
where
    F: FnMut(&mut Vec<BlockNode>),
{
    visit_block_list_mut(&mut doc.page_setup.header, f);
    visit_block_list_mut(&mut doc.page_setup.footer, f);
    visit_block_list_mut(&mut doc.blocks, f);
    for section in &mut doc.sections {
        visit_block_list_mut(&mut section.page_setup.header, f);
        visit_block_list_mut(&mut section.page_setup.footer, f);
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
        BlockNode::Table(table) => {
            for row in &mut table.rows {
                for cell in &mut row.cells {
                    visit_block_list_mut(&mut cell.blocks, f);
                }
            }
        }
        BlockNode::Quote { body, attribution } => {
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
