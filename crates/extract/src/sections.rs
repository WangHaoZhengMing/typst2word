//! Multi-page Word section consolidation (`consolidate_page_sections`).
//!
//! Groups consecutive pages sharing identical page geometry and `#set page(...)` rules into
//! Word sections, detecting uniform furniture, distinct first-page furniture (`<w:titlePg/>`),
//! and alternating odd/even furniture (`<w:evenAndOddHeaders/>`).

use typst::foundations::Styles;

use ir::{BlockNode, NodeId, PageSetupIr};

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
        while std_end < n && setups_identical(&page_setups[std_end], &page_setups[i]) {
            std_end += 1;
        }
        raw.push((i..std_end, page_setups[i].clone()));
        i = std_end;
    }

    // Coalesce adjacent static sections whose resolved `PageSetupIr` is identical.
    let mut out: Vec<(std::ops::Range<usize>, PageSetupIr)> = Vec::with_capacity(raw.len());
    for (range, setup) in raw {
        if let Some((prev_range, prev_setup)) = out.last_mut()
            && prev_range.end == range.start
            && !prev_setup.different_first_page
            && !prev_setup.different_odd_even
            && !setup.different_first_page
            && !setup.different_odd_even
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
        furniture_eq(&p.header, &run[0].header) && furniture_eq(&p.footer, &run[0].footer)
    }) {
        return Some(run[0].clone());
    }

    // 2. First page differs (`different_first_page = true`), pages `1..` all share `(H_rest, F_rest)`.
    let rest_uniform = run.iter().skip(2).all(|p| {
        furniture_eq(&p.header, &run[1].header) && furniture_eq(&p.footer, &run[1].footer)
    });
    if rest_uniform {
        let mut merged = run[1].clone();
        merged.different_first_page = true;
        merged.first_page_header = Some(run[0].header.clone());
        merged.first_page_footer = Some(run[0].footer.clone());
        if merged.header.is_empty() && !run[0].header.is_empty() {
            merged.header_margin_pt = run[0].header_margin_pt;
        }
        if merged.footer.is_empty() && !run[0].footer.is_empty() {
            merged.footer_margin_pt = run[0].footer_margin_pt;
        }
        return Some(merged);
    }

    // 3. Odd/even alternation (`different_odd_even = true`), optionally with distinct first page.
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
            let tail_matches = (1..run.len()).all(|off| {
                let target = if (start_page_idx + off + 1) % 2 == 1 {
                    &run[oi]
                } else {
                    &run[ei]
                };
                furniture_eq(&run[off].header, &target.header)
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

                let mut merged = run[oi].clone();
                merged.different_odd_even = true;
                merged.even_page_header = Some(run[ei].header.clone());
                merged.even_page_footer = Some(run[ei].footer.clone());
                if !first_matches {
                    merged.different_first_page = true;
                    merged.first_page_header = Some(run[0].header.clone());
                    merged.first_page_footer = Some(run[0].footer.clone());
                }
                return Some(merged);
            }
        }
    }

    None
}
