//! Pass 3 (`WordPipelinePassKind::InterBlockFlowAndSpacing`):
//! Resolves `ParagraphPdfMetricsIr` and raw `FlowMeasureIr` (`BlockPdfSpanIr` + `NextBlockMeasureIr`)
//! into concrete `ParagraphStyleIr::space_after_pt` using [`crate::WordParagraphConstruct`].

use ir::{
    BlockNode, BlockPdfSpanIr, DocumentIr, NextBlockKindIr, NextBlockMeasureIr,
    ParagraphPdfMetricsIr, ParagraphStyleIr, visit_document_block_lists_mut,
};

use crate::{InterBlockGapAction, NextBlockSpacingContext, WordParagraphConstruct};

/// Run Pass 3 (`InterBlockFlowAndSpacing`) across all paragraphs in `doc`.
pub fn run_flow_spacing_pass(doc: &mut DocumentIr) {
    visit_document_block_lists_mut(doc, &mut |blocks| {
        for block in blocks.iter_mut() {
            if let BlockNode::Paragraph { style, .. } = block {
                resolve_paragraph_style_spacing(style);
            }
        }
    });
}

/// Resolve a single paragraph's `space_after_pt` from its `pdf_metrics` and raw `flow_measure`.
pub fn resolve_paragraph_style_spacing(style: &mut ParagraphStyleIr) {
    // 1. Resolve initial paragraph space_after from Typst `par.spacing` & `par.leading`.
    if let ParagraphPdfMetricsIr::Measured {
        ends_with_explicit_linebreak,
        base_font_pt,
        default_font_pt,
        par_spacing_pt,
        par_leading_pt,
        in_multi_block_group_tail,
    } = style.pdf_metrics
    {
        let mut sa = WordParagraphConstruct::solve_initial_space_after_pt(
            ends_with_explicit_linebreak,
            base_font_pt,
            default_font_pt,
            par_spacing_pt,
            par_leading_pt,
        );
        if in_multi_block_group_tail && (1.5..6.0).contains(&sa) {
            sa = 6.05;
        }
        style.space_after_pt = sa;
    }

    // 2. Resolve direct block-to-block gap from raw `flow_measure.self_span` and `flow_measure.next`.
    if let Some(next) = style.flow_measure.next {
        apply_direct_paragraph_transition(
            &mut style.space_after_pt,
            style.flow_measure.self_span,
            next,
        );
    }

    // 3. Resolve any additional trailing transitions (e.g. list item sibling gaps or outer block gaps).
    let transitions = std::mem::take(&mut style.flow_measure.trailing_transitions);
    for (span_a, measure) in transitions {
        apply_trailing_transition(&mut style.space_after_pt, span_a, measure);
    }
}

fn apply_direct_paragraph_transition(
    space_after_pt: &mut f64,
    span_a: BlockPdfSpanIr,
    next: NextBlockMeasureIr,
) {
    let (Some((page_a, bottom_a)), Some((page_b, top_b))) = (
        span_a.end_page_and_bottom_y(),
        next.span.start_page_and_top_y(),
    ) else {
        return;
    };

    if page_b == page_a + 1
        && bottom_a > 100.0
        && bottom_a < 560.0
        && top_b < 110.0
        && matches!(
            next.kind,
            NextBlockKindIr::Paragraph | NextBlockKindIr::HeadingFollowedByParagraph
        )
    {
        let extra_pt = (755.0 - bottom_a).max(0.0);
        *space_after_pt += extra_pt;
        return;
    }

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
    span_a: BlockPdfSpanIr,
    measure: NextBlockMeasureIr,
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
