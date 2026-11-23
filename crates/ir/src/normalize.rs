//! Structural IR normalization and recursive tree traversal helpers (`ir::normalize`).
//!
//! Performs Word-independent structural cleanup on [`DocumentIr`]:
//! 1. Coalescing adjacent compatible [`InlineNode::Text`] runs split by Typst's layout engine.
//! 2. Identity-first merging of [`BlockNode::Container`] / [`BlockNode::CodeBlock`] nodes split across
//!    Typst columns/pages (preserving 1-to-N `LayoutFragmentIr` slices in `DocumentIr::reference_layout`).
//! 3. Cleaning leading, trailing, or duplicate consecutive [`BlockNode::PageBreak`] markers.
//! 4. Canonicalizing `_typst_loc_<hash>` bookmark anchors into deterministic sequential IDs.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::{
    BlockLayoutSpanIr, BlockNode, DocumentIr, FieldKindIr, InlineNode, LinkTargetIr, NodeId,
    NodeOrigin, NodeReferenceLayout, PageSetupIr, ParagraphStyleIr, SemanticRoleIr,
};

/// Normalize `DocumentIr` structure (coalesce adjacent text runs, collapse `#show heading: none`
/// zero-fragment headings, merge split containers, clean redundant page breaks, and canonicalize
/// intra-document bookmark anchors).
pub fn normalize_document_ir(doc: &mut DocumentIr) {
    visit_document_inline_lists_mut(doc, &mut coalesce_adjacent_text_runs);
    visit_document_inline_lists_mut(doc, &mut resolve_weak_horizontal_spaces);
    collapse_hidden_empty_headings(doc);
    merge_split_containers(doc);
    clean_redundant_pagebreaks(&mut doc.blocks);
    for section in &mut doc.sections {
        clean_redundant_pagebreaks(&mut section.blocks);
    }
    canonicalize_bookmark_anchors(doc);
}

/// Collapse Typst weak horizontal spaces (`#h(..., weak: true)`) using Typst's spacing algebra:
/// - Drop weak spaces at the start/end of an inline sequence or adjacent to `LineBreak` / `Tab`.
/// - Drop weak spaces adjacent to a strong `HorizontalSpace` or whitespace at the boundary of
///   an adjacent `Text` run.
/// - Merge consecutive weak spaces to `max(w1, w2)` and clear `weak = false` on surviving spaces.
pub fn resolve_weak_horizontal_spaces(inlines: &mut Vec<InlineNode>) {
    if !inlines
        .iter()
        .any(|n| matches!(n, InlineNode::HorizontalSpace { weak: true, .. }))
    {
        return;
    }

    let is_left_absorber = |node: &InlineNode| match node {
        InlineNode::LineBreak | InlineNode::Tab => true,
        InlineNode::HorizontalSpace { weak: false, .. } => true,
        InlineNode::Text(r) => r.text.chars().last().is_some_and(char::is_whitespace),
        _ => false,
    };
    let is_right_absorber = |node: &InlineNode| match node {
        InlineNode::LineBreak | InlineNode::Tab => true,
        InlineNode::HorizontalSpace { weak: false, .. } => true,
        InlineNode::Text(r) => r.text.chars().next().is_some_and(char::is_whitespace),
        _ => false,
    };

    // First coalesce consecutive weak spaces into a single weak space of maximum width.
    let mut merged: Vec<InlineNode> = Vec::with_capacity(inlines.len());
    for node in std::mem::take(inlines) {
        if let InlineNode::HorizontalSpace {
            width_pt: w_new,
            weak: true,
        } = node
        {
            if let Some(InlineNode::HorizontalSpace {
                width_pt: w_prev,
                weak: true,
            }) = merged.last_mut()
            {
                *w_prev = w_prev.max(w_new);
                continue;
            }
        }
        merged.push(node);
    }

    let len = merged.len();
    let mut out = Vec::with_capacity(len);
    for (i, node) in merged.iter().enumerate() {
        if let InlineNode::HorizontalSpace {
            width_pt,
            weak: true,
        } = *node
        {
            let prev = (i > 0).then(|| &merged[i - 1]);
            let next = merged.get(i + 1);
            let absorbed = prev.is_none_or(is_left_absorber) || next.is_none_or(is_right_absorber);
            if !absorbed && width_pt > 0.5 {
                out.push(InlineNode::HorizontalSpace {
                    width_pt,
                    weak: false,
                });
            }
        } else {
            out.push(node.clone());
        }
    }
    *inlines = out;
}

/// Canonicalize Typst location-hash bookmark names (`_typst_loc_<hash>`) into deterministic
/// sequential document-order anchors (`_typst_loc_0001`, `_typst_loc_0002`, ...) so generated
/// OOXML is byte-reproducible regardless of source file path or `Span` hashes.
pub fn canonicalize_bookmark_anchors(doc: &mut DocumentIr) {
    struct AnchorTable {
        map: HashMap<String, String>,
        next_idx: usize,
    }

    impl AnchorTable {
        fn new() -> Self {
            Self {
                map: HashMap::new(),
                next_idx: 0,
            }
        }

        fn intern(&mut self, anchor: &str) -> String {
            if !anchor.starts_with("_typst_loc_") {
                return anchor.to_string();
            }
            if let Some(existing) = self.map.get(anchor) {
                return existing.clone();
            }
            self.next_idx += 1;
            let canonical = format!("_typst_loc_{:04}", self.next_idx);
            self.map.insert(anchor.to_string(), canonical.clone());
            canonical
        }

        fn rewrite_opt(&mut self, slot: &mut Option<String>) {
            if let Some(val) = slot.as_deref() {
                let mapped = self.intern(val);
                *slot = Some(mapped);
            }
        }
    }

    fn collect_defs_in_inlines(inlines: &[InlineNode], table: &mut AnchorTable) {
        for inline in inlines {
            match inline {
                InlineNode::EquationInline(eq) => {
                    if let Some(bm) = &eq.bookmark {
                        table.intern(bm);
                    }
                }
                InlineNode::Field { content, .. }
                | InlineNode::Link { content, .. }
                | InlineNode::Ref { content, .. }
                | InlineNode::Citation { content, .. } => collect_defs_in_inlines(content, table),
                InlineNode::Footnote { content, .. } => collect_defs_in_blocks(content, table),
                InlineNode::ShapeInline(shape) => collect_defs_in_blocks(&shape.body, table),
                InlineNode::CetzInline(canvas) => {
                    for lbl in &canvas.labels {
                        collect_defs_in_inlines(&lbl.content, table);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_defs_in_blocks(blocks: &[BlockNode], table: &mut AnchorTable) {
        for block in blocks {
            match block {
                BlockNode::Heading {
                    bookmark, content, ..
                }
                | BlockNode::Paragraph {
                    bookmark, content, ..
                } => {
                    if let Some(bm) = bookmark {
                        table.intern(bm);
                    }
                    collect_defs_in_inlines(content, table);
                }
                BlockNode::Title { content, .. } => collect_defs_in_inlines(content, table),
                BlockNode::Figure {
                    bookmark,
                    body,
                    caption,
                    ..
                } => {
                    if let Some(bm) = bookmark {
                        table.intern(bm);
                    }
                    collect_defs_in_blocks(body, table);
                    if let Some(cap) = caption {
                        collect_defs_in_inlines(cap, table);
                    }
                }
                BlockNode::EquationBlock(eq) => {
                    if let Some(bm) = &eq.bookmark {
                        table.intern(bm);
                    }
                }
                BlockNode::Bibliography { title, entries } => {
                    if let Some(t) = title {
                        collect_defs_in_inlines(t, table);
                    }
                    for entry in entries {
                        if let Some(bm) = &entry.bookmark {
                            table.intern(bm);
                        }
                        collect_defs_in_inlines(&entry.content, table);
                    }
                }
                BlockNode::List { items, .. } => {
                    for item in items {
                        collect_defs_in_blocks(&item.blocks, table);
                    }
                }
                BlockNode::Enum { items, .. } => {
                    for item in items {
                        collect_defs_in_blocks(&item.blocks, table);
                    }
                }
                BlockNode::Terms { items, .. } => {
                    for item in items {
                        collect_defs_in_inlines(&item.term, table);
                        collect_defs_in_blocks(&item.description, table);
                    }
                }
                BlockNode::Container(c) => collect_defs_in_blocks(&c.body, table),
                BlockNode::Table(t) => {
                    for row in &t.rows {
                        for cell in &row.cells {
                            collect_defs_in_blocks(&cell.blocks, table);
                        }
                    }
                }
                BlockNode::Quote {
                    body, attribution, ..
                } => {
                    collect_defs_in_blocks(body, table);
                    if let Some(attr) = attribution {
                        collect_defs_in_inlines(attr, table);
                    }
                }
                BlockNode::ShapeBlock(shape) => collect_defs_in_blocks(&shape.body, table),
                BlockNode::CetzCanvas(canvas) => {
                    for lbl in &canvas.labels {
                        collect_defs_in_inlines(&lbl.content, table);
                    }
                }
                BlockNode::Outline { title, entries } => {
                    if let Some(t) = title {
                        collect_defs_in_inlines(t, table);
                    }
                    for entry in entries {
                        collect_defs_in_inlines(&entry.title, table);
                        collect_defs_in_inlines(&entry.page_inlines, table);
                    }
                }
                BlockNode::Columns { body, .. } => collect_defs_in_blocks(body, table),
                _ => {}
            }
        }
    }

    fn collect_defs_in_setup(setup: &PageSetupIr, table: &mut AnchorTable) {
        collect_defs_in_blocks(&setup.header, table);
        collect_defs_in_blocks(&setup.footer, table);
        if let Some(fh) = &setup.first_page_header {
            collect_defs_in_blocks(fh, table);
        }
        if let Some(ff) = &setup.first_page_footer {
            collect_defs_in_blocks(ff, table);
        }
        if let Some(eh) = &setup.even_page_header {
            collect_defs_in_blocks(eh, table);
        }
        if let Some(ef) = &setup.even_page_footer {
            collect_defs_in_blocks(ef, table);
        }
    }

    let mut table = AnchorTable::new();
    // 1. Intern all bookmark definitions in strict pre-order across sections.
    collect_defs_in_setup(&doc.page_setup, &mut table);
    collect_defs_in_blocks(&doc.blocks, &mut table);
    for section in &doc.sections {
        collect_defs_in_setup(&section.page_setup, &mut table);
        collect_defs_in_blocks(&section.blocks, &mut table);
    }

    // 2. Rewrite all block-level bookmark definitions and outline/bibliography entries.
    visit_document_block_lists_mut(doc, &mut |blocks| {
        for block in blocks.iter_mut() {
            match block {
                BlockNode::Heading { bookmark, .. }
                | BlockNode::Paragraph { bookmark, .. }
                | BlockNode::Figure { bookmark, .. } => table.rewrite_opt(bookmark),
                BlockNode::EquationBlock(eq) => table.rewrite_opt(&mut eq.bookmark),
                BlockNode::Outline { entries, .. } => {
                    for entry in entries {
                        table.rewrite_opt(&mut entry.target_bookmark);
                    }
                }
                BlockNode::Bibliography { entries, .. } => {
                    for entry in entries {
                        table.rewrite_opt(&mut entry.bookmark);
                    }
                }
                _ => {}
            }
        }
    });

    // 3. Rewrite all inline bookmark targets (`Link`, `Ref`, `PageRef`, inline `Equation`).
    visit_document_inline_lists_mut(doc, &mut |inlines| {
        for inline in inlines.iter_mut() {
            match inline {
                InlineNode::Field {
                    kind: FieldKindIr::PageRef { bookmark },
                    ..
                } => {
                    *bookmark = table.intern(bookmark);
                }
                InlineNode::EquationInline(eq) => table.rewrite_opt(&mut eq.bookmark),
                InlineNode::Link {
                    target: Some(LinkTargetIr::Anchor(anchor)),
                    ..
                } => {
                    *anchor = table.intern(anchor);
                }
                InlineNode::Ref { target, .. } => {
                    *target = table.intern(target);
                }
                _ => {}
            }
        }
    });

    // 4. Keep `NodeOrigin.role` bookmark metadata synchronized.
    for origin in doc.origins.values_mut() {
        match &mut origin.role {
            Some(SemanticRoleIr::Heading { bookmark, .. })
            | Some(SemanticRoleIr::Figure { bookmark, .. }) => table.rewrite_opt(bookmark),
            _ => {}
        }
    }
}

/// Remove invisible zero-fragment `Heading` blocks created by `#show heading: none` (e.g. slide or
/// running-header templates that introspect headings without rendering them in flow), forwarding
/// any intra-document referenced bookmark onto the following block so links/outlines keep working.
fn collapse_hidden_empty_headings(doc: &mut DocumentIr) {
    let mut referenced_anchors: HashSet<String> = HashSet::new();
    visit_document_inline_lists_mut(doc, &mut |inlines| {
        for inline in inlines.iter() {
            match inline {
                InlineNode::Link {
                    target: Some(LinkTargetIr::Anchor(anchor)),
                    ..
                }
                | InlineNode::Ref { target: anchor, .. } => {
                    referenced_anchors.insert(anchor.clone());
                }
                _ => {}
            }
        }
    });
    visit_document_block_lists_mut(doc, &mut |blocks| {
        for block in blocks.iter() {
            if let BlockNode::Outline { entries, .. } = block {
                for entry in entries {
                    if let Some(bm) = &entry.target_bookmark {
                        referenced_anchors.insert(bm.clone());
                    }
                }
            }
        }
    });

    let layouts = std::mem::take(&mut doc.reference_layout);
    let mut removed_count = 0usize;
    let mut prune_fn = |blocks: &mut Vec<BlockNode>| {
        let mut i = 0;
        while i < blocks.len() {
            let is_hidden_heading = match &blocks[i] {
                BlockNode::Heading {
                    id,
                    numbering,
                    content,
                    box_style,
                    ..
                } => {
                    content.is_empty()
                        && numbering.is_none()
                        && box_style.is_none()
                        && layouts.get(id).is_none_or(|l| l.fragments.is_empty())
                }
                _ => false,
            };
            if !is_hidden_heading {
                i += 1;
                continue;
            }
            let BlockNode::Heading { bookmark, .. } = blocks.remove(i) else {
                unreachable!()
            };
            removed_count += 1;
            if let Some(bm) = bookmark
                && referenced_anchors.contains(&bm)
            {
                let attached = blocks
                    .get_mut(i)
                    .is_some_and(|next| attach_bookmark_to_first_carrier(next, &bm));
                if !attached {
                    blocks.insert(
                        i,
                        BlockNode::Paragraph {
                            id: NodeId::UNSET,
                            content: Vec::new(),
                            style: ParagraphStyleIr {
                                space_before_pt: 0.0,
                                space_after_pt: 0.0,
                                line_spacing_pt: Some(1.0),
                                ..Default::default()
                            },
                            bookmark: Some(bm),
                        },
                    );
                    i += 1;
                }
            }
        }
    };
    visit_document_block_lists_mut(doc, &mut prune_fn);
    doc.reference_layout = layouts;
    doc.report.headings_count = doc.report.headings_count.saturating_sub(removed_count);
}

fn attach_bookmark_to_first_carrier(block: &mut BlockNode, bm: &str) -> bool {
    match block {
        BlockNode::Paragraph { bookmark, .. }
        | BlockNode::Heading { bookmark, .. }
        | BlockNode::Figure { bookmark, .. } => {
            if bookmark.is_none() {
                *bookmark = Some(bm.to_string());
                true
            } else {
                false
            }
        }
        BlockNode::List { items, .. } => items
            .first_mut()
            .and_then(|it| it.blocks.first_mut())
            .is_some_and(|b| attach_bookmark_to_first_carrier(b, bm)),
        BlockNode::Enum { items, .. } => items
            .first_mut()
            .and_then(|it| it.blocks.first_mut())
            .is_some_and(|b| attach_bookmark_to_first_carrier(b, bm)),
        BlockNode::Container(c) => c
            .body
            .first_mut()
            .is_some_and(|b| attach_bookmark_to_first_carrier(b, bm)),
        _ => false,
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
            (
                BlockNode::CodeBlock {
                    id: id_a,
                    fill: fill_a,
                    stroke: stroke_a,
                    ..
                },
                BlockNode::CodeBlock {
                    id: id_b,
                    lang: lang_b,
                    fill: fill_b,
                    stroke: stroke_b,
                    ..
                },
            ) => {
                if same_nonzero_typst_location(origins, *id_a, *id_b) == Some(true) {
                    true
                } else {
                    // Typst only emits `Tag::Start(RawElem)` on the opening page; cross-page
                    // continuation fragments arrive without `typst_location` or `lang`.
                    let b_has_no_loc = origins
                        .get(id_b)
                        .and_then(|o| o.typst_location.as_ref())
                        .is_none();
                    let next_page_continuation = layouts
                        .get(id_a)
                        .and_then(|la| la.span.end_page_and_bottom_y())
                        .zip(
                            layouts
                                .get(id_b)
                                .and_then(|lb| lb.span.start_page_and_top_y()),
                        )
                        .is_some_and(|((end_p, _), (start_p, _))| start_p == end_p + 1);
                    b_has_no_loc
                        && lang_b.is_none()
                        && next_page_continuation
                        && fill_a == fill_b
                        && stroke_a == stroke_b
                }
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
                visit_inlines_block_lists_mut(&mut entry.page_inlines, f);
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
                        visit_inline_list_recursive_mut(&mut entry.page_inlines, f);
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
