//! Pass 3 (`WordPipelinePassKind::InterBlockFlowAndSpacing`):
//! Resolves semantic `ParagraphStyleIr` fields and read-only `NodeReferenceLayout`
//! (`BlockLayoutSpanIr` + `NextBlockLayoutMeasureIr`) into concrete `ParagraphStyleIr::space_after_pt`
//! using [`crate::WordParagraphConstruct`].

use std::collections::BTreeMap;

use ir::{
    BlockLayoutSpanIr, BlockNode, DocumentIr, NextBlockKindIr, NextBlockLayoutMeasureIr, NodeId,
    NodeReferenceLayout, ParagraphStyleIr, TableKindIr, visit_document_block_lists_mut,
};

use crate::units::pt_to_dxa;
use crate::{InterBlockGapAction, NextBlockSpacingContext, WordParagraphConstruct};

/// Run Pass 3 (`InterBlockFlowAndSpacing`) across all paragraphs in `doc`.
pub fn run_flow_spacing_pass(doc: &mut DocumentIr) {
    let ref_layout = doc.reference_layout.clone();
    visit_document_block_lists_mut(doc, &mut |blocks| {
        for block in blocks.iter_mut() {
            if let BlockNode::Paragraph { id, style, .. } = block {
                let node_layout = (!id.is_unset())
                    .then(|| ref_layout.get(id))
                    .flatten();
                resolve_paragraph_style_spacing(style, node_layout);
            }
        }
    });
}

/// Resolve a single paragraph's `space_after_pt` from its semantic `StyleChain` properties
/// and optional read-only [`NodeReferenceLayout`].
pub fn resolve_paragraph_style_spacing(
    style: &mut ParagraphStyleIr,
    node_layout: Option<&NodeReferenceLayout>,
) {
    let flow = node_layout.and_then(|l| l.flow.as_ref());

    // 0. Explicit-height empty spacer block (`#block(height: H)[ ]`):
    //    Resolve post-spacer gap from either direct `flow.next` or container/list-trailing
    //    `flow.trailing_transitions` so `w:after` covers the measured inter-block separation
    //    (including any adjacent heading `w:before` that Word collapses via `max(after, before)`).
    if style.line_spacing_pt.is_some() && style.resolved_font_size_pt.is_none() {
        let below_pt = style.resolved_par_spacing_pt.take().unwrap_or(0.0);
        let transition = flow.and_then(|f| {
            f.next
                .map(|m| (f.self_span, m))
                .or_else(|| f.trailing_transitions.last().copied())
        });
        if let Some((span_a, measure)) = transition {
            if let (Some((pa, bottom_a)), Some((pb, top_b))) = (
                span_a.end_page_and_bottom_y(),
                measure.span.start_page_and_top_y(),
            ) {
                if pa == pb && top_b >= bottom_a - 1.0 {
                    let post_gap_pt = (top_b - bottom_a - 0.06 * measure.next_font_pt).max(0.0);
                    style.space_after_pt += post_gap_pt;
                } else if pa != pb {
                    style.space_after_pt += below_pt;
                }
            } else {
                style.space_after_pt += below_pt;
            }
        }
        return;
    }

    // 1. Resolve initial paragraph space_after from Typst `par.spacing` & `par.leading`.
    if let (
        Some(base_font_pt),
        Some(default_font_pt),
        Some(par_spacing_pt),
        Some(par_leading_pt),
    ) = (
        style.resolved_font_size_pt,
        style.resolved_default_font_size_pt,
        style.resolved_par_spacing_pt,
        style.resolved_par_leading_pt,
    ) {
        let mut sa = WordParagraphConstruct::solve_initial_space_after_pt(
            style.ends_with_explicit_linebreak,
            base_font_pt,
            default_font_pt,
            par_spacing_pt,
            par_leading_pt,
        );
        let in_multi_block_group_tail = flow.is_some_and(|f| f.in_multi_block_group_tail);
        // Only restore `6.05pt` (`0.55em`) on a multi-block group's tail paragraph when Typst's
        // internal group scope collapsed `par.spacing` down to `par.leading` (`|spacing - leading| < 0.15`),
        // NOT when the user explicitly configured a tight `par(spacing: 0.6em, leading: 0.65em)`.
        if in_multi_block_group_tail && sa < 6.0 && (par_spacing_pt - par_leading_pt).abs() < 0.15 {
            sa = 6.05;
        }
        style.space_after_pt = sa;
    }

    let Some(flow) = flow else {
        if style.pad_bottom_pt > 0.1 {
            style.space_after_pt += style.pad_bottom_pt;
        }
        return;
    };

    // 2. Resolve direct block-to-block gap from `flow.self_span` and `flow.next`.
    if let Some(next) = flow.next {
        if let (Some(par_spacing_pt), Some(par_leading_pt)) =
            (style.resolved_par_spacing_pt, style.resolved_par_leading_pt)
            && (par_spacing_pt - par_leading_pt).abs() < 0.1
            && let (Some((pa, bottom_a)), Some((pb, top_b))) = (
                flow.self_span.end_page_and_bottom_y(),
                next.span.start_page_and_top_y(),
            )
            && pa == pb
            && top_b >= bottom_a
        {
            let gap_pt = top_b - bottom_a;
            if gap_pt > par_leading_pt + 1.5 && gap_pt <= 48.0 {
                style.space_after_pt = (gap_pt - par_leading_pt).max(0.0);
            }
        }
        // Honor negative `#v(...)` between same-page blocks (e.g. `•••` + `#v(-5pt)` + `raw`),
        // where the measured inter-block gap shrinks strictly below a normal `par.leading` gap.
        if let Some(par_leading_pt) = style.resolved_par_leading_pt
            && next.kind != NextBlockKindIr::TightList
            && let (Some((pa, bottom_a)), Some((pb, top_b))) = (
                flow.self_span.end_page_and_bottom_y(),
                next.span.start_page_and_top_y(),
            )
            && pa == pb
        {
            let gap_pt = top_b - bottom_a;
            if gap_pt < par_leading_pt - 1.0 && gap_pt >= -24.0 {
                style.space_after_pt =
                    (style.space_after_pt - (par_leading_pt - gap_pt)).max(0.0);
            }
        }
        apply_direct_paragraph_transition(&mut style.space_after_pt, flow.self_span, next);
    }

    // 3. Resolve any additional trailing transitions (e.g. list item sibling gaps or outer block gaps).
    for &(span_a, measure) in &flow.trailing_transitions {
        apply_trailing_transition(&mut style.space_after_pt, span_a, measure);
    }

    if style.pad_bottom_pt > 0.1 {
        style.space_after_pt += style.pad_bottom_pt;
    }
}

/// Solve inter-block vertical transitions after non-paragraph block containers (`ShapeBlock`,
/// `Container`, `Table`, `Figure`) before containers are lowered to Word `<w:tbl>`s.
///
/// Under Word's `compatibilityMode=15`, adjacent paragraph vertical margins collapse via
/// `max(w:after, w:before)`:
/// - `ShapeBlock` and `Figure` end with a `<w:p>` wrapper, so their full post-gap (including
///   `#v(1em)` and adjacent heading `above` spacing) maps directly to `solved_block_after_dxa`,
///   and is zeroed at page boundaries (`pa != pb`).
/// - `Container` and non-`LayoutGrid` `Table` serialize as `<w:tbl>` (which has no `w:after`),
///   so their post-gap transfers onto the following block's `space_before_pt` / `before_dxa`.
pub fn solve_container_and_figure_flow_spacing(
    blocks: &mut [BlockNode],
    ir: &DocumentIr,
    solved_before_dxa: &mut BTreeMap<NodeId, i64>,
    solved_after_dxa: &mut BTreeMap<NodeId, i64>,
) {
    for idx in 0..blocks.len().saturating_sub(1) {
        let id_a = blocks[idx].node_id();
        if id_a.is_unset() {
            continue;
        }
        let id_b = blocks[idx + 1].node_id();
        let flow_opt = ir.layout(id_a).and_then(|l| l.flow.as_ref());
        let (span_a, span_b, next_font_pt) = if matches!(blocks[idx], BlockNode::Figure { .. })
            && !id_b.is_unset()
            && let (Some(lay_a), Some(lay_b)) = (ir.layout(id_a), ir.layout(id_b))
        {
            let nf = flow_opt
                .and_then(|f| f.next.map(|n| n.next_font_pt))
                .unwrap_or(11.0)
                .max(8.0);
            (lay_a.span, lay_b.span, nf)
        } else if let Some(flow) = flow_opt
            && let Some(next) = flow.next
        {
            (flow.self_span, next.span, next.next_font_pt.max(8.0))
        } else if !id_b.is_unset()
            && let (Some(lay_a), Some(lay_b)) = (ir.layout(id_a), ir.layout(id_b))
        {
            (lay_a.span, lay_b.span, 11.0)
        } else {
            continue;
        };

        let (Some((pa, bottom_a)), Some((pb, top_b))) = (
            span_a.end_page_and_bottom_y(),
            span_b.start_page_and_top_y(),
        ) else {
            continue;
        };

        if pa != pb {
            match &blocks[idx] {
                BlockNode::ShapeBlock(_) | BlockNode::Figure { .. } => {
                    solved_after_dxa.insert(id_a, 0);
                }
                BlockNode::Container(_) if pb == pa + 1 => {
                    // `<w:tbl>` has 0pt intrinsic bottom margin in Word, whereas Typst still applies
                    // `par.spacing` (~`1.0 * font_size` beyond internal leading) when testing whether
                    // the next block fits on page `pa`; Word suppresses `w:before` after soft breaks.
                    apply_table_backed_post_gap(
                        id_a,
                        &mut blocks[idx + 1],
                        next_font_pt,
                        solved_before_dxa,
                        solved_after_dxa,
                    );
                }
                BlockNode::CodeBlock { fill, stroke, .. }
                    if (fill.is_some() || stroke.is_some()) && pb == pa + 1 =>
                {
                    apply_table_backed_post_gap(
                        id_a,
                        &mut blocks[idx + 1],
                        next_font_pt,
                        solved_before_dxa,
                        solved_after_dxa,
                    );
                }
                BlockNode::Table(t) if t.kind != TableKindIr::LayoutGrid && pb == pa + 1 => {
                    apply_table_backed_post_gap(
                        id_a,
                        &mut blocks[idx + 1],
                        next_font_pt,
                        solved_before_dxa,
                        solved_after_dxa,
                    );
                }
                _ => {}
            }
            continue;
        }

        let gap_pt = top_b - bottom_a;
        if !(-2.0..=500.0).contains(&gap_pt) {
            continue;
        }

        match &blocks[idx] {
            BlockNode::ShapeBlock(_) => {
                // Wrapper `<w:p>` uses a 1pt paragraph mark (`~0.02 * font` descent), leaving only
                // the following block's `~0.20 * next_font` internal top leading to subtract.
                let needed_after_pt = (gap_pt - 0.22 * next_font_pt).max(0.0);
                solved_after_dxa.insert(id_a, pt_to_dxa(needed_after_pt));
            }
            BlockNode::Figure { .. } => {
                let needed_after_pt = (gap_pt - 0.44 * next_font_pt).max(0.0);
                solved_after_dxa.insert(id_a, pt_to_dxa(needed_after_pt));
            }
            BlockNode::Container(_) => {
                let needed_before_pt = (gap_pt - 0.20 * next_font_pt).max(0.0);
                apply_table_backed_post_gap(
                    id_a,
                    &mut blocks[idx + 1],
                    needed_before_pt,
                    solved_before_dxa,
                    solved_after_dxa,
                );
            }
            BlockNode::CodeBlock { fill, stroke, .. } if fill.is_some() || stroke.is_some() => {
                let needed_before_pt = (gap_pt - 0.20 * next_font_pt).max(0.0);
                apply_table_backed_post_gap(
                    id_a,
                    &mut blocks[idx + 1],
                    needed_before_pt,
                    solved_before_dxa,
                    solved_after_dxa,
                );
            }
            BlockNode::Table(t) if t.kind != TableKindIr::LayoutGrid => {
                let needed_before_pt = (gap_pt - 0.20 * next_font_pt).max(0.0);
                apply_table_backed_post_gap(
                    id_a,
                    &mut blocks[idx + 1],
                    needed_before_pt,
                    solved_before_dxa,
                    solved_after_dxa,
                );
            }
            _ => {}
        }
    }
}

fn apply_table_backed_post_gap(
    prev_table_id: NodeId,
    next_block: &mut BlockNode,
    needed_before_pt: f64,
    solved_before_dxa: &mut BTreeMap<NodeId, i64>,
    solved_after_dxa: &mut BTreeMap<NodeId, i64>,
) {
    let needed_dxa = pt_to_dxa(needed_before_pt);
    match next_block {
        BlockNode::Paragraph { style, .. } => {
            style.space_before_pt = style.space_before_pt.max(needed_before_pt);
        }
        BlockNode::Container(_) | BlockNode::Table(_) => {
            // Between two `<w:tbl>` blocks, `docx-writer` emits a `2pt` (`40 dxa`) exact separator `<w:p>`;
            // store `(needed_dxa - 40)` as its `w:after`.
            solved_after_dxa.insert(prev_table_id, (needed_dxa - 40).max(0));
        }
        BlockNode::CodeBlock { fill, stroke, .. } if fill.is_some() || stroke.is_some() => {
            solved_after_dxa.insert(prev_table_id, (needed_dxa - 40).max(0));
        }
        other => {
            let next_id = other.node_id();
            if !next_id.is_unset() {
                let entry = solved_before_dxa.entry(next_id).or_insert(0);
                *entry = (*entry).max(needed_dxa);
            }
        }
    }
}

fn apply_direct_paragraph_transition(
    space_after_pt: &mut f64,
    span_a: BlockLayoutSpanIr,
    next: NextBlockLayoutMeasureIr,
) {
    let (Some((page_a, bottom_a)), Some((page_b, top_b))) = (
        span_a.end_page_and_bottom_y(),
        next.span.start_page_and_top_y(),
    ) else {
        return;
    };

    if page_a != page_b || top_b < bottom_a - 2.0 {
        return;
    }

    let gap_pt = top_b - bottom_a;
    if next.kind == NextBlockKindIr::TightList && gap_pt <= 10.0 {
        if let InterBlockGapAction::ZeroSpaceAfter =
            WordParagraphConstruct::solve_inter_block_gap(
                gap_pt,
                11.0,
                11.0,
                NextBlockSpacingContext::TightList,
            )
        {
            *space_after_pt = 0.0;
        }
    } else if next.kind == NextBlockKindIr::Paragraph
        && (next.prev_font_pt >= 15.0 || next.next_font_pt >= 15.0)
        && gap_pt > 10.0
        && gap_pt < 60.0
    {
        if let InterBlockGapAction::SetExactSpaceAfter(target_pt) =
            WordParagraphConstruct::solve_inter_block_gap(
                gap_pt,
                next.prev_font_pt,
                next.next_font_pt,
                NextBlockSpacingContext::Paragraph,
            )
        {
            *space_after_pt = target_pt;
        }
    } else if gap_pt > 48.0 && gap_pt < 500.0 {
        *space_after_pt += (gap_pt - 12.0).max(0.0);
    }
}

fn apply_trailing_transition(
    space_after_pt: &mut f64,
    span_a: BlockLayoutSpanIr,
    measure: NextBlockLayoutMeasureIr,
) {
    let (Some((pa, bottom_a)), Some((pb, top_b))) = (
        span_a.end_page_and_bottom_y(),
        measure.span.start_page_and_top_y(),
    ) else {
        return;
    };

    if let NextBlockKindIr::ListItemSibling {
        list_level,
        active_columns,
        prev_item_block_count,
        next_item_starts_with_paragraph,
    } = measure.kind
    {
        if pa == pb && top_b >= bottom_a - 2.0 {
            let gap_pt = top_b - bottom_a;
            if gap_pt > 48.0 && gap_pt < 500.0 {
                *space_after_pt += (gap_pt - 10.0).max(0.0);
            }
        } else if pb == pa + 1
            && list_level == 0
            && active_columns <= 1
            && prev_item_block_count <= 2
            && bottom_a > 100.0
            && bottom_a < 660.0
            && next_item_starts_with_paragraph
        {
            let gap_pt = (760.0 - bottom_a) + (top_b - 71.0).max(0.0);
            if gap_pt > 60.0 && gap_pt < 500.0 {
                *space_after_pt += (gap_pt - 15.0).max(0.0);
            }
        }
        return;
    }

    if pa == pb && top_b >= bottom_a - 2.0 {
        let gap_pt = top_b - bottom_a;
        let is_tight_list = measure.kind == NextBlockKindIr::TightList && gap_pt <= 10.0;
        let is_display_par = measure.kind == NextBlockKindIr::Paragraph
            && (measure.prev_font_pt >= 15.0 || measure.next_font_pt >= 15.0)
            && gap_pt > 10.0
            && gap_pt < 60.0;
        if !is_tight_list && !is_display_par && gap_pt > 48.0 && gap_pt < 500.0 {
            *space_after_pt += (gap_pt - 12.0).max(0.0);
        }
    }
}
