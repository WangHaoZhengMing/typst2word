//! Multi-page Word section consolidation (`consolidate_page_sections`).
//!
//! Groups consecutive pages sharing identical page geometry and `#set page(...)` rules into
//! Word sections, detecting uniform furniture, distinct first-page furniture (`<w:titlePg/>`),
//! and alternating odd/even furniture (`<w:evenAndOddHeaders/>`).

use typst::foundations::Styles;

use ir::{BlockNode, FieldKindIr, InlineNode, NodeId, PageSetupIr, TextRunIr};

/// Compare two `PageSetupIr` headers/footers after stripping `NodeId`s so dynamic
/// page fields and per-page node IDs do not cause spurious section breaks.
fn canonical_furniture_value(blocks: &[BlockNode]) -> Option<serde_json::Value> {
    let mut cloned = blocks.to_vec();
    strip_node_ids_recursive(&mut cloned);
    serde_json::to_value(&cloned).ok()
}

fn furniture_eq(a: &[BlockNode], b: &[BlockNode]) -> bool {
    canonical_furniture_value(a) == canonical_furniture_value(b)
}

fn strip_node_ids_recursive(blocks: &mut [BlockNode]) {
    for b in blocks {
        b.set_node_id(NodeId::UNSET);
        match b {
            BlockNode::Paragraph { bookmark, .. } | BlockNode::Heading { bookmark, .. } => {
                *bookmark = None;
            }
            BlockNode::List { items, .. } => {
                for item in items {
                    strip_node_ids_recursive(&mut item.blocks);
                }
            }
            BlockNode::Enum { items, .. } => {
                for item in items {
                    strip_node_ids_recursive(&mut item.blocks);
                }
            }
            BlockNode::Terms { items, .. } => {
                for item in items {
                    strip_node_ids_recursive(&mut item.description);
                }
            }
            BlockNode::Container(c) => strip_node_ids_recursive(&mut c.body),
            BlockNode::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        strip_node_ids_recursive(&mut cell.blocks);
                    }
                }
            }
            BlockNode::Quote { body, .. }
            | BlockNode::Figure { body, .. }
            | BlockNode::Columns { body, .. } => strip_node_ids_recursive(body),
            BlockNode::ShapeBlock(s) => strip_node_ids_recursive(&mut s.body),
            _ => {}
        }
    }
}

fn setups_identical(a: &PageSetupIr, b: &PageSetupIr) -> bool {
    a.has_same_geometry(b)
        && a.page_number_format == b.page_number_format
        && furniture_eq(&a.header, &b.header)
        && furniture_eq(&a.footer, &b.footer)
}

/// Compare only `PageElem` `#set page(...)` rules so non-page liftable styles (such as
/// `#set enum(...)` or `#set par(...)`) do not split consecutive pages into separate Word sections.
fn page_elem_styles_eq(a: Option<&Styles>, b: Option<&Styles>) -> bool {
    use typst::foundations::NativeElement as _;
    match (a, b) {
        (Some(sa), Some(sb)) => sa
            .as_slice()
            .iter()
            .filter(|s| s.element() == Some(typst::layout::PageElem::ELEM))
            .eq(sb
                .as_slice()
                .iter()
                .filter(|s| s.element() == Some(typst::layout::PageElem::ELEM))),
        (None, None) => true,
        _ => false,
    }
}

/// Consolidate per-page `PageSetupIr`s into `(page_range, consolidated_setup)` sections.
pub(crate) fn consolidate_page_sections(
    page_setups: &[PageSetupIr],
    page_styles: &[Styles],
) -> Vec<(std::ops::Range<usize>, PageSetupIr)> {
    if page_setups.is_empty() {
        return vec![(0..0, PageSetupIr::default())];
    }

    let mut normalized_setups = page_setups.to_vec();
    normalize_dynamic_page_counters(&mut normalized_setups);
    let page_setups = &normalized_setups;

    let mut raw: Vec<(std::ops::Range<usize>, PageSetupIr)> = Vec::new();
    let n = page_setups.len();
    let mut i = 0;

    while i < n {
        // Try dynamic consolidation across maximal run `i..end` sharing the page box & `#set page` styles.
        // Identical `#set page` styles guarantee identical source header/footer distances, so the
        // measured `header_margin_pt` (which varies with furniture presence) is not compared here.
        let mut dyn_end = i + 1;
        while dyn_end < n
            && page_setups[dyn_end].has_same_page_box(&page_setups[i])
            && page_elem_styles_eq(page_styles.get(dyn_end), page_styles.get(i))
        {
            dyn_end += 1;
        }

        if dyn_end - i >= 2
            && let Some(consolidated) = try_consolidate_dynamic_run(&page_setups[i..dyn_end], i)
        {
            raw.push((i..dyn_end, consolidated));
            i = dyn_end;
            continue;
        }

        // Standard run: group pages whose resolved `PageSetupIr` (ignoring `NodeId`s) is equal.
        let mut std_end = i + 1;
        while std_end < n
            && page_setups[std_end].page_number_start.is_none()
            && setups_identical(&page_setups[std_end], &page_setups[i])
        {
            std_end += 1;
        }
        raw.push((i..std_end, page_setups[i].clone()));
        i = std_end;
    }

    // Coalesce adjacent sections whose default geometry and furniture are identical.
    // A preceding `different_first_page` section may absorb a following uniform section with
    // matching default header/footer (as long as the following section does not restart numbering).
    let mut out: Vec<(std::ops::Range<usize>, PageSetupIr)> = Vec::with_capacity(raw.len());
    for (range, setup) in raw {
        if let Some((prev_range, prev_setup)) = out.last_mut()
            && prev_range.end == range.start
            && !prev_setup.different_odd_even
            && !setup.different_first_page
            && !setup.different_odd_even
            && setup.page_number_start.is_none()
            && setups_identical(prev_setup, &setup)
        {
            prev_range.end = range.end;
        } else {
            out.push((range, setup));
        }
    }

    out
}

fn try_consolidate_dynamic_run(run: &[PageSetupIr], start_page_idx: usize) -> Option<PageSetupIr> {
    if run.len() < 2 {
        return None;
    }

    // 1. All pages already identical.
    if run.iter().skip(1).all(|p| {
        p.page_number_start.is_none()
            && furniture_eq(&p.header, &run[0].header)
            && furniture_eq(&p.footer, &run[0].footer)
    }) {
        return Some(run[0].clone());
    }

    // 2. First page differs (`different_first_page = true`), pages `1..` all share `(H_rest, F_rest)`.
    // If page `1` restarts page numbering (`page_number_start.is_some()`), it MUST start a separate
    // Word section (`<w:pgNumType w:start="..."/>` only applies at a section boundary).
    let rest_uniform = run[1].page_number_start.is_none()
        && run.iter().skip(2).all(|p| {
            p.page_number_start.is_none()
                && furniture_eq(&p.header, &run[1].header)
                && furniture_eq(&p.footer, &run[1].footer)
        });
    if rest_uniform {
        let mut merged = run[1].clone();
        merged.different_first_page = true;
        merged.first_page_header = Some(run[0].header.clone());
        merged.first_page_footer = Some(run[0].footer.clone());
        if merged.page_number_start.is_none() {
            merged.page_number_start = run[0].page_number_start;
        }
        if merged.header.is_empty() && !run[0].header.is_empty() {
            merged.header_margin_pt = run[0].header_margin_pt;
        }
        if merged.footer.is_empty() && !run[0].footer.is_empty() {
            merged.footer_margin_pt = run[0].footer_margin_pt;
        }
        return Some(merged);
    }

    // 3. Odd/even alternation (`different_odd_even = true`), optionally with distinct first page.
    // Require at least one repeated parity page so arbitrary 3-page runs never falsely enable
    // document-wide `<w:evenAndOddHeaders/>` (which breaks even pages in other sections):
    // - without distinct first page: `run.len() >= 3` (`A, B, A`)
    // - with distinct first page:    `run.len() >= 4` (`First, A, B, A`)
    if run.len() >= 3 {
        let odd_ref = (0..run.len())
            .skip(1)
            .find(|&off| (start_page_idx + off + 1) % 2 == 1)
            .or_else(|| (0..run.len()).find(|&off| (start_page_idx + off + 1) % 2 == 1));
        let even_ref = (0..run.len())
            .skip(1)
            .find(|&off| (start_page_idx + off + 1) % 2 == 0)
            .or_else(|| (0..run.len()).find(|&off| (start_page_idx + off + 1) % 2 == 0));

        if let (Some(oi), Some(ei)) = (odd_ref, even_ref) {
            let odd_even_distinct = !furniture_eq(&run[oi].header, &run[ei].header)
                || !furniture_eq(&run[oi].footer, &run[ei].footer);
            let tail_matches = odd_even_distinct
                && (1..run.len()).all(|off| {
                    let target = if (start_page_idx + off + 1) % 2 == 1 {
                        &run[oi]
                    } else {
                        &run[ei]
                    };
                    run[off].page_number_start.is_none()
                        && furniture_eq(&run[off].header, &target.header)
                        && furniture_eq(&run[off].footer, &target.footer)
                });
            if tail_matches {
                let first_target = if (start_page_idx + 1) % 2 == 1 {
                    &run[oi]
                } else {
                    &run[ei]
                };
                let first_matches = furniture_eq(&run[0].header, &first_target.header)
                    && furniture_eq(&run[0].footer, &first_target.footer);

                if first_matches || run.len() >= 4 {
                    let mut merged = run[oi].clone();
                    merged.different_odd_even = true;
                    merged.even_page_header = Some(run[ei].header.clone());
                    merged.even_page_footer = Some(run[ei].footer.clone());
                    if merged.page_number_start.is_none() {
                        merged.page_number_start = run[0].page_number_start;
                    }
                    if !first_matches {
                        merged.different_first_page = true;
                        merged.first_page_header = Some(run[0].header.clone());
                        merged.first_page_footer = Some(run[0].footer.clone());
                    }
                    return Some(merged);
                }
            }
        }
    }

    None
}

// =============================================================================
// Dynamic Page-Counter Normalization in Headers & Footers
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DenominatorFieldKind {
    /// Total pages across the entire document -> Word `NUMPAGES`.
    NumPages,
    /// Total pages within the current section/run (`run_len < total_doc_pages`) -> Word `SECTIONPAGES`.
    SectionPages,
}

#[derive(Debug, Clone)]
struct DynamicCounterPattern {
    prefix: String,
    start_val: u64,
    /// Optional `(mid_sep, denom_val, field_kind, tail_suffix)` when the suffix carries a
    /// total-page count such as `" / 41"`, `" of 10"`, or `" 页 / 共 10 页"`.
    denominator: Option<(String, u64, DenominatorFieldKind, String)>,
    suffix: String,
}

/// Detect monotonically incrementing static page counters (e.g. Touying's `1 / 41`, `2 / 41`,
/// Polylux slide counters, or custom `context` footers) across consecutive pages sharing the same
/// page box and normalize them into dynamic Word `PAGE` / `NUMPAGES` fields so consecutive pages
/// consolidate into clean Word sections.
fn normalize_dynamic_page_counters(setups: &mut [PageSetupIr]) {
    let total_doc_pages = setups.len();
    let mut start = 0;
    while start < total_doc_pages {
        let mut end = start + 1;
        while end < total_doc_pages && setups[end].has_same_page_box(&setups[start]) {
            end += 1;
        }
        if end - start >= 2 {
            for is_header in [false, true] {
                if !try_normalize_slot_counter(
                    &mut setups[start..end],
                    start,
                    total_doc_pages,
                    is_header,
                ) && end - start >= 3
                {
                    // Also handle a distinct/blank first page followed by incrementing pages.
                    let _ = try_normalize_slot_counter(
                        &mut setups[start + 1..end],
                        start + 1,
                        total_doc_pages,
                        is_header,
                    );
                }
            }
        }
        start = end;
    }
}

fn slot_blocks(setup: &PageSetupIr, is_header: bool) -> &[BlockNode] {
    if is_header {
        &setup.header
    } else {
        &setup.footer
    }
}

fn slot_blocks_mut(setup: &mut PageSetupIr, is_header: bool) -> &mut [BlockNode] {
    if is_header {
        &mut setup.header
    } else {
        &mut setup.footer
    }
}

fn try_normalize_slot_counter(
    slice: &mut [PageSetupIr],
    start_page_idx: usize,
    total_doc_pages: usize,
    is_header: bool,
) -> bool {
    if slice.len() < 2 {
        return false;
    }
    if blocks_have_page_field(slot_blocks(&slice[0], is_header)) {
        return normalize_static_denominator_after_page_field(slice, total_doc_pages, is_header);
    }

    let per_page_texts: Vec<Vec<String>> = slice
        .iter()
        .map(|s| {
            let mut texts = Vec::new();
            collect_blocks_texts(slot_blocks(s, is_header), &mut texts);
            texts
        })
        .collect();

    let leaf_count = per_page_texts[0].len();
    if leaf_count == 0 || per_page_texts.iter().any(|t| t.len() != leaf_count) {
        return false;
    }

    let mut matched: Vec<(usize, DynamicCounterPattern)> = Vec::new();
    for leaf_idx in 0..leaf_count {
        let col: Vec<&str> = per_page_texts
            .iter()
            .map(|t| t[leaf_idx].as_str())
            .collect();
        if col.iter().all(|&s| s == col[0]) {
            continue;
        }
        if let Some(pat) =
            parse_dynamic_counter_sequence(&col, start_page_idx, total_doc_pages, slice.len())
        {
            matched.push((leaf_idx, pat));
        }
    }

    if matched.is_empty() {
        return false;
    }

    let start_val = matched[0].1.start_val;
    for setup in slice.iter_mut() {
        let mut cursor = 0usize;
        rewrite_blocks_counter_leaves(slot_blocks_mut(setup, is_header), &matched, &mut cursor);
    }

    // Also normalize any static total-pages denominator split across 1..N subsequent `Text` runs
    // immediately following the newly inserted `Field(Page)` (e.g. `"1"` + `" "` + `"/"` + `" "` + `"37"`).
    let _ = normalize_static_denominator_after_page_field(slice, total_doc_pages, is_header);

    reconcile_counter_table_col_widths(slice, is_header);

    if start_val != (start_page_idx + 1) as u64 && slice[0].page_number_start.is_none() {
        slice[0].page_number_start = Some(start_val);
    }

    true
}

fn parse_dynamic_counter_sequence(
    texts: &[&str],
    start_page_idx: usize,
    total_doc_pages: usize,
    run_len: usize,
) -> Option<DynamicCounterPattern> {
    if texts.len() < 2 {
        return None;
    }

    // 1. Longest common prefix (never splitting UTF-8 codepoints or ASCII digits).
    let first = texts[0];
    let mut prefix_len = first.len();
    for &t in &texts[1..] {
        let common = first
            .bytes()
            .zip(t.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        prefix_len = prefix_len.min(common);
    }
    while prefix_len > 0 && !first.is_char_boundary(prefix_len) {
        prefix_len -= 1;
    }
    while prefix_len > 0 && first.as_bytes()[prefix_len - 1].is_ascii_digit() {
        prefix_len -= 1;
    }

    // 2. Longest common suffix on the remainder after `prefix_len` (never splitting codepoints or digits).
    let rem0 = &first[prefix_len..];
    let mut suffix_len = rem0.len();
    for &t in &texts[1..] {
        let rem = &t[prefix_len..];
        let common = rem0
            .bytes()
            .rev()
            .zip(rem.bytes().rev())
            .take_while(|(a, b)| a == b)
            .count();
        suffix_len = suffix_len.min(common);
    }
    let mut suffix_start = rem0.len() - suffix_len;
    while suffix_start < rem0.len() && !rem0.is_char_boundary(suffix_start) {
        suffix_start += 1;
    }
    while suffix_start < rem0.len() && rem0.as_bytes()[suffix_start].is_ascii_digit() {
        suffix_start += 1;
    }
    suffix_len = rem0.len() - suffix_start;

    // 3. Verify that every middle slice is an integer incrementing by +1 per page.
    let mut start_val: Option<u64> = None;
    for (idx, &t) in texts.iter().enumerate() {
        if t.len() < prefix_len + suffix_len {
            return None;
        }
        let mid = &t[prefix_len..t.len() - suffix_len];
        if mid.is_empty() || !mid.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let val: u64 = mid.parse().ok()?;
        match start_val {
            None => {
                // Guard against unrelated incrementing numbers (e.g. years like 2024, 2025).
                if val == 0 || val > (start_page_idx as u64).saturating_add(2) {
                    return None;
                }
                start_val = Some(val);
            }
            Some(v0) => {
                if val != v0.checked_add(idx as u64)? {
                    return None;
                }
            }
        }
    }

    let start_val = start_val?;
    let prefix = first[..prefix_len].to_string();
    let suffix = rem0[suffix_start..].to_string();
    let denominator = parse_denominator_in_suffix(&suffix, total_doc_pages, run_len);

    Some(DynamicCounterPattern {
        prefix,
        start_val,
        denominator,
        suffix,
    })
}

/// Detect a total-pages denominator inside a counter suffix (e.g. `" / 41"`, `"/41"`, `" of 10"`,
/// `" 页 / 共 10 页"`), distinguishing document-wide `NUMPAGES` from section-scoped `SECTIONPAGES`.
fn parse_denominator_in_suffix(
    suffix: &str,
    total_doc_pages: usize,
    run_len: usize,
) -> Option<(String, u64, DenominatorFieldKind, String)> {
    let bytes = suffix.as_bytes();
    let digit_start = bytes.iter().position(|b| b.is_ascii_digit())?;
    let mut digit_end = digit_start;
    while digit_end < bytes.len() && bytes[digit_end].is_ascii_digit() {
        digit_end += 1;
    }
    // Must have a separator before the denominator and no additional digit groups after.
    if digit_start == 0 || bytes[digit_end..].iter().any(|b| b.is_ascii_digit()) {
        return None;
    }
    let denom: u64 = suffix[digit_start..digit_end].parse().ok()?;
    if denom < 2 {
        return None;
    }
    let field_kind = if denom == total_doc_pages as u64 {
        DenominatorFieldKind::NumPages
    } else if denom == run_len as u64 {
        DenominatorFieldKind::SectionPages
    } else {
        return None;
    };
    Some((
        suffix[..digit_start].to_string(),
        denom,
        field_kind,
        suffix[digit_end..].to_string(),
    ))
}

fn blocks_have_total_pages_field(blocks: &[BlockNode]) -> bool {
    blocks.iter().any(|b| match b {
        BlockNode::Paragraph { content, .. }
        | BlockNode::Heading { content, .. }
        | BlockNode::Title { content, .. } => inlines_have_total_pages_field(content),
        BlockNode::Table(t) => t
            .rows
            .iter()
            .any(|r| r.cells.iter().any(|c| blocks_have_total_pages_field(&c.blocks))),
        BlockNode::Container(c) => blocks_have_total_pages_field(&c.body),
        BlockNode::ShapeBlock(s) => blocks_have_total_pages_field(&s.body),
        BlockNode::Quote { body, .. }
        | BlockNode::Figure { body, .. }
        | BlockNode::Columns { body, .. } => blocks_have_total_pages_field(body),
        _ => false,
    })
}

fn inlines_have_total_pages_field(inlines: &[InlineNode]) -> bool {
    inlines.iter().any(|i| match i {
        InlineNode::Field { kind, .. } => {
            matches!(kind, FieldKindIr::NumPages | FieldKindIr::SectionPages)
        }
        InlineNode::Link { content, .. }
        | InlineNode::Ref { content, .. }
        | InlineNode::Citation { content, .. } => inlines_have_total_pages_field(content),
        _ => false,
    })
}

/// Convert a static total-pages denominator immediately following an existing `FieldKindIr::Page`
/// (e.g. Touying `dewdrop` emitting `Field(Page)` + `Text(" / 37")` via `counter(page).final()`)
/// into a dynamic `NUMPAGES` / `SECTIONPAGES` field.
fn normalize_static_denominator_after_page_field(
    slice: &mut [PageSetupIr],
    total_doc_pages: usize,
    is_header: bool,
) -> bool {
    if blocks_have_total_pages_field(slot_blocks(&slice[0], is_header)) {
        return false;
    }
    let run_len = slice.len();
    let mut changed_any = false;
    for setup in slice.iter_mut() {
        if rewrite_blocks_denominator_after_page_field(
            slot_blocks_mut(setup, is_header),
            total_doc_pages,
            run_len,
        ) {
            changed_any = true;
        }
    }
    if changed_any {
        reconcile_counter_table_col_widths(slice, is_header);
    }
    changed_any
}

fn rewrite_blocks_denominator_after_page_field(
    blocks: &mut [BlockNode],
    total_doc_pages: usize,
    run_len: usize,
) -> bool {
    let mut changed = false;
    for b in blocks {
        match b {
            BlockNode::Paragraph { content, .. }
            | BlockNode::Heading { content, .. }
            | BlockNode::Title { content, .. } => {
                if rewrite_inlines_denominator_after_page_field(content, total_doc_pages, run_len) {
                    changed = true;
                }
            }
            BlockNode::Table(t) => {
                for r in &mut t.rows {
                    for c in &mut r.cells {
                        if rewrite_blocks_denominator_after_page_field(
                            &mut c.blocks,
                            total_doc_pages,
                            run_len,
                        ) {
                            changed = true;
                        }
                    }
                }
            }
            BlockNode::Container(c) => {
                if rewrite_blocks_denominator_after_page_field(
                    &mut c.body,
                    total_doc_pages,
                    run_len,
                ) {
                    changed = true;
                }
            }
            BlockNode::ShapeBlock(s) => {
                if rewrite_blocks_denominator_after_page_field(
                    &mut s.body,
                    total_doc_pages,
                    run_len,
                ) {
                    changed = true;
                }
            }
            BlockNode::Quote { body, .. }
            | BlockNode::Figure { body, .. }
            | BlockNode::Columns { body, .. } => {
                if rewrite_blocks_denominator_after_page_field(body, total_doc_pages, run_len) {
                    changed = true;
                }
            }
            _ => {}
        }
    }
    changed
}

fn rewrite_inlines_denominator_after_page_field(
    inlines: &mut Vec<InlineNode>,
    total_doc_pages: usize,
    run_len: usize,
) -> bool {
    let mut changed = false;
    let orig = std::mem::take(inlines);
    let mut out = Vec::with_capacity(orig.len() + 2);
    let mut i = 0;
    while i < orig.len() {
        let is_page_field = matches!(
            &orig[i],
            InlineNode::Field {
                kind: FieldKindIr::Page,
                ..
            }
        );
        out.push(orig[i].clone());
        if is_page_field {
            // Collect all consecutive `InlineNode::Text` runs immediately after `Field(Page)`
            // with their byte ranges so prefix, denominator (`NUMPAGES`), and suffix each keep
            // their own original font/run styling (e.g. `Songti SC` CJK text vs `Times New Roman`
            // digits in `"页（共"` + `"4"` + `"页）"`).
            let mut j = i + 1;
            let mut combined = String::new();
            let mut run_spans: Vec<(usize, usize, &TextRunIr)> = Vec::new();
            while let Some(InlineNode::Text(r)) = orig.get(j) {
                let start = combined.len();
                combined.push_str(&r.text);
                let end = combined.len();
                run_spans.push((start, end, r));
                j += 1;
            }
            if !run_spans.is_empty()
                && let Some((prefix, denom, field_kind, _suffix)) =
                    parse_denominator_in_suffix(&combined, total_doc_pages, run_len)
            {
                let digit_start = prefix.len();
                let digit_end = digit_start + denom.to_string().len();
                let denom_tpl = run_spans
                    .iter()
                    .find(|(s, e, _)| digit_start >= *s && digit_start < *e)
                    .map(|(_, _, r)| *r)
                    .unwrap_or(run_spans[0].2);
                push_sliced_text_runs(&run_spans, 0, digit_start, &mut out);
                push_denominator_nodes(denom_tpl, "", denom, field_kind, "", &mut out);
                push_sliced_text_runs(&run_spans, digit_end, combined.len(), &mut out);
                changed = true;
                i = j;
                continue;
            }
        }
        i += 1;
    }
    *inlines = out;
    changed
}

/// Emit the portion of `run_spans` intersecting byte range `[range_start, range_end)`,
/// preserving each source `TextRunIr`'s exact font and styling.
fn push_sliced_text_runs(
    run_spans: &[(usize, usize, &TextRunIr)],
    range_start: usize,
    range_end: usize,
    out: &mut Vec<InlineNode>,
) {
    if range_start >= range_end {
        return;
    }
    for &(s, e, run) in run_spans {
        let lo = range_start.max(s);
        let hi = range_end.min(e);
        if lo < hi {
            let mut sliced = run.clone();
            sliced.text = run.text[lo - s..hi - s].to_string();
            out.push(InlineNode::Text(sliced));
        }
    }
}

fn blocks_have_page_field(blocks: &[BlockNode]) -> bool {
    blocks.iter().any(|b| match b {
        BlockNode::Paragraph { content, .. }
        | BlockNode::Heading { content, .. }
        | BlockNode::Title { content, .. } => inlines_have_page_field(content),
        BlockNode::Table(t) => t
            .rows
            .iter()
            .any(|r| r.cells.iter().any(|c| blocks_have_page_field(&c.blocks))),
        BlockNode::Container(c) => blocks_have_page_field(&c.body),
        BlockNode::ShapeBlock(s) => blocks_have_page_field(&s.body),
        BlockNode::Quote { body, .. }
        | BlockNode::Figure { body, .. }
        | BlockNode::Columns { body, .. } => blocks_have_page_field(body),
        _ => false,
    })
}

fn inlines_have_page_field(inlines: &[InlineNode]) -> bool {
    inlines.iter().any(|inl| match inl {
        InlineNode::Field {
            kind: FieldKindIr::Page | FieldKindIr::NumPages | FieldKindIr::SectionPages,
            ..
        } => true,
        InlineNode::Link { content, .. }
        | InlineNode::Ref { content, .. }
        | InlineNode::Citation { content, .. }
        | InlineNode::Field { content, .. } => inlines_have_page_field(content),
        _ => false,
    })
}

fn collect_blocks_texts(blocks: &[BlockNode], out: &mut Vec<String>) {
    for b in blocks {
        match b {
            BlockNode::Paragraph { content, .. }
            | BlockNode::Heading { content, .. }
            | BlockNode::Title { content, .. } => collect_inlines_texts(content, out),
            BlockNode::Table(t) => {
                for r in &t.rows {
                    for c in &r.cells {
                        collect_blocks_texts(&c.blocks, out);
                    }
                }
            }
            BlockNode::Container(c) => collect_blocks_texts(&c.body, out),
            BlockNode::ShapeBlock(s) => collect_blocks_texts(&s.body, out),
            BlockNode::Quote { body, .. }
            | BlockNode::Figure { body, .. }
            | BlockNode::Columns { body, .. } => collect_blocks_texts(body, out),
            _ => {}
        }
    }
}

fn collect_inlines_texts(inlines: &[InlineNode], out: &mut Vec<String>) {
    for inl in inlines {
        match inl {
            InlineNode::Text(r) => out.push(r.text.clone()),
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. }
            | InlineNode::Field { content, .. } => collect_inlines_texts(content, out),
            _ => {}
        }
    }
}

fn rewrite_blocks_counter_leaves(
    blocks: &mut [BlockNode],
    matched: &[(usize, DynamicCounterPattern)],
    cursor: &mut usize,
) {
    for b in blocks {
        match b {
            BlockNode::Paragraph { content, .. }
            | BlockNode::Heading { content, .. }
            | BlockNode::Title { content, .. } => {
                rewrite_inlines_counter_leaves(content, matched, cursor);
            }
            BlockNode::Table(t) => {
                for r in &mut t.rows {
                    for c in &mut r.cells {
                        rewrite_blocks_counter_leaves(&mut c.blocks, matched, cursor);
                    }
                }
            }
            BlockNode::Container(c) => {
                rewrite_blocks_counter_leaves(&mut c.body, matched, cursor);
            }
            BlockNode::ShapeBlock(s) => {
                rewrite_blocks_counter_leaves(&mut s.body, matched, cursor);
            }
            BlockNode::Quote { body, .. }
            | BlockNode::Figure { body, .. }
            | BlockNode::Columns { body, .. } => {
                rewrite_blocks_counter_leaves(body, matched, cursor);
            }
            _ => {}
        }
    }
}

fn rewrite_inlines_counter_leaves(
    inlines: &mut Vec<InlineNode>,
    matched: &[(usize, DynamicCounterPattern)],
    cursor: &mut usize,
) {
    let mut new_inlines = Vec::with_capacity(inlines.len());
    for inl in std::mem::take(inlines) {
        match inl {
            InlineNode::Text(run) => {
                let idx = *cursor;
                *cursor += 1;
                if let Some((_, pat)) = matched.iter().find(|(i, _)| *i == idx) {
                    push_dynamic_counter_nodes(&run, pat, &mut new_inlines);
                } else {
                    new_inlines.push(InlineNode::Text(run));
                }
            }
            InlineNode::Link { target, mut content } => {
                rewrite_inlines_counter_leaves(&mut content, matched, cursor);
                new_inlines.push(InlineNode::Link { target, content });
            }
            InlineNode::Ref { target, mut content } => {
                rewrite_inlines_counter_leaves(&mut content, matched, cursor);
                new_inlines.push(InlineNode::Ref { target, content });
            }
            InlineNode::Citation { keys, mut content } => {
                rewrite_inlines_counter_leaves(&mut content, matched, cursor);
                new_inlines.push(InlineNode::Citation { keys, content });
            }
            InlineNode::Field {
                kind,
                format,
                mut content,
            } => {
                rewrite_inlines_counter_leaves(&mut content, matched, cursor);
                new_inlines.push(InlineNode::Field {
                    kind,
                    format,
                    content,
                });
            }
            other => new_inlines.push(other),
        }
    }
    *inlines = new_inlines;
}

fn push_dynamic_counter_nodes(
    template: &TextRunIr,
    pat: &DynamicCounterPattern,
    out: &mut Vec<InlineNode>,
) {
    if !pat.prefix.is_empty() {
        let mut r = template.clone();
        r.text = pat.prefix.clone();
        out.push(InlineNode::Text(r));
    }
    let mut page_run = template.clone();
    page_run.text = pat.start_val.to_string();
    out.push(InlineNode::Field {
        kind: FieldKindIr::Page,
        format: Some("Arabic".into()),
        content: vec![InlineNode::Text(page_run)],
    });
    if let Some((mid_sep, denom, field_kind, tail_suffix)) = &pat.denominator {
        push_denominator_nodes(template, mid_sep, *denom, *field_kind, tail_suffix, out);
    } else if !pat.suffix.is_empty() {
        let mut r = template.clone();
        r.text = pat.suffix.clone();
        out.push(InlineNode::Text(r));
    }
}

fn push_denominator_nodes(
    template: &TextRunIr,
    prefix: &str,
    denom: u64,
    field_kind: DenominatorFieldKind,
    suffix: &str,
    out: &mut Vec<InlineNode>,
) {
    if !prefix.is_empty() {
        let mut r = template.clone();
        r.text = prefix.to_string();
        out.push(InlineNode::Text(r));
    }
    let mut total_run = template.clone();
    total_run.text = denom.to_string();
    out.push(InlineNode::Field {
        kind: match field_kind {
            DenominatorFieldKind::NumPages => FieldKindIr::NumPages,
            DenominatorFieldKind::SectionPages => FieldKindIr::SectionPages,
        },
        format: Some("Arabic".into()),
        content: vec![InlineNode::Text(total_run)],
    });
    if !suffix.is_empty() {
        let mut r = template.clone();
        r.text = suffix.to_string();
        out.push(InlineNode::Text(r));
    }
}

/// When a header/footer layout grid auto-sizes the column containing a dynamic page counter
/// (e.g. `"1 / 41"` -> 41.5pt vs `"20 / 41"` -> 59.9pt), unify `cols` across the run
/// using the widest counter column so `furniture_eq` matches and multi-digit page numbers never wrap.
fn reconcile_counter_table_col_widths(slice: &mut [PageSetupIr], is_header: bool) {
    let block_count = slot_blocks(&slice[0], is_header).len();
    if slice
        .iter()
        .any(|s| slot_blocks(s, is_header).len() != block_count)
    {
        return;
    }

    for bi in 0..block_count {
        let BlockNode::Table(ref t0) = slot_blocks(&slice[0], is_header)[bi] else {
            continue;
        };
        let ncols = t0.cols.len();
        if ncols == 0 {
            continue;
        }
        let total_w0: f64 = t0.total_width_pt();

        let counter_cols: Vec<usize> = (0..ncols)
            .filter(|&col_idx| {
                t0.rows.iter().any(|r| {
                    r.cells
                        .iter()
                        .any(|c| c.x == col_idx && blocks_have_page_field(&c.blocks))
                })
            })
            .collect();
        if counter_cols.is_empty() {
            continue;
        }

        let all_compatible = slice.iter().all(|s| {
            matches!(
                &slot_blocks(s, is_header)[bi],
                BlockNode::Table(t)
                    if t.cols.len() == ncols
                        && (t.total_width_pt() - total_w0).abs() <= 2.0
            )
        });
        if !all_compatible {
            continue;
        }

        let best_cols = slice
            .iter()
            .filter_map(|s| match &slot_blocks(s, is_header)[bi] {
                BlockNode::Table(t) => Some(&t.cols),
                _ => None,
            })
            .max_by(|a, b| {
                let wa: f64 = counter_cols.iter().map(|&c| a[c].width_pt()).sum();
                let wb: f64 = counter_cols.iter().map(|&c| b[c].width_pt()).sum();
                wa.total_cmp(&wb)
            })
            .cloned();

        if let Some(cols) = best_cols {
            for s in slice.iter_mut() {
                if let BlockNode::Table(t) = &mut slot_blocks_mut(s, is_header)[bi] {
                    t.cols = cols.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::ParagraphStyleIr;

    fn page_with_footer(text: Option<&str>) -> PageSetupIr {
        let mut ps = PageSetupIr::default();
        if let Some(t) = text {
            ps.footer = vec![BlockNode::Paragraph {
                id: NodeId::UNSET,
                content: vec![InlineNode::Text(TextRunIr {
                    text: t.to_string(),
                    ..Default::default()
                })],
                style: ParagraphStyleIr::default(),
                bookmark: None,
            }];
        }
        ps
    }

    fn footer_field_kinds(setup: &PageSetupIr) -> Vec<FieldKindIr> {
        let mut out = Vec::new();
        for blk in &setup.footer {
            if let BlockNode::Paragraph { content, .. } = blk {
                for inl in content {
                    if let InlineNode::Field { kind, .. } = inl {
                        out.push(kind.clone());
                    }
                }
            }
        }
        out
    }

    #[test]
    fn cover_plus_restarted_body_uses_section_pages_in_separate_section() {
        let setups = vec![
            page_with_footer(None),
            page_with_footer(Some("1 / 4")),
            page_with_footer(Some("2 / 4")),
            page_with_footer(Some("3 / 4")),
            page_with_footer(Some("4 / 4")),
        ];
        let styles = vec![Styles::new(); setups.len()];
        let sections = consolidate_page_sections(&setups, &styles);
        assert_eq!(sections.len(), 2, "cover + restarted body must split into 2 sections");
        assert_eq!(sections[0].0, 0..1);
        assert_eq!(sections[1].0, 1..5);
        assert_eq!(sections[1].1.page_number_start, Some(1));
        assert_eq!(
            footer_field_kinds(&sections[1].1),
            vec![FieldKindIr::Page, FieldKindIr::SectionPages]
        );
    }

    #[test]
    fn uniform_document_counter_uses_numpages_in_single_section() {
        let setups = vec![
            page_with_footer(Some("1 / 5")),
            page_with_footer(Some("2 / 5")),
            page_with_footer(Some("3 / 5")),
            page_with_footer(Some("4 / 5")),
            page_with_footer(Some("5 / 5")),
        ];
        let styles = vec![Styles::new(); setups.len()];
        let sections = consolidate_page_sections(&setups, &styles);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, 0..5);
        assert_eq!(
            footer_field_kinds(&sections[0].1),
            vec![FieldKindIr::Page, FieldKindIr::NumPages]
        );
    }
}
