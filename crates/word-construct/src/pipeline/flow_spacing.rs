//! Pass 3 (`WordPipelinePassKind::InterBlockFlowAndSpacing`):
//! Resolves semantic `ParagraphStyleIr` fields and read-only `NodeReferenceLayout`
//! (`BlockLayoutSpanIr` + `NextBlockLayoutMeasureIr`) into concrete `ParagraphStyleIr::space_after_pt`
//! using [`crate::WordParagraphConstruct`].

use ir::{
    BlockLayoutSpanIr, BlockNode, DocumentIr, NextBlockKindIr, NextBlockLayoutMeasureIr,
    NodeReferenceLayout, ParagraphStyleIr, visit_document_block_lists_mut,
};

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
        apply_direct_paragraph_transition(&mut style.space_after_pt, flow.self_span, next);
    }

    // 3. Resolve any additional trailing transitions (e.g. list item sibling gaps or outer block gaps).
    for &(span_a, measure) in &flow.trailing_transitions {
        apply_trailing_transition(&mut style.space_after_pt, span_a, measure);
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
