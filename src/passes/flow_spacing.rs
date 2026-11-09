//! Pass 5 (`PipelinePassKind::InterBlockFlowAndSpacing`):
//! Resolves `ParagraphPdfMetricsIr`, `InterBlockFlowIr`, and `extra_trailing_space_pt`
//! into concrete `ParagraphStyleIr::space_after_pt` using `word_construct::WordParagraphConstruct`.

use word_construct::{InterBlockGapAction, NextBlockSpacingContext, WordParagraphConstruct};

use crate::ir::{
    BlockNode, DocumentIr, InterBlockFlowIr, ParagraphPdfMetricsIr, ParagraphStyleIr,
};
use crate::passes::visit_document_block_lists_mut;

/// Run Pass 5 (`InterBlockFlowAndSpacing`) across all paragraphs in `doc`.
pub fn run_flow_spacing_pass(doc: &mut DocumentIr) {
    visit_document_block_lists_mut(doc, &mut |blocks| {
        for block in blocks.iter_mut() {
            if let BlockNode::Paragraph { style, .. } = block {
                resolve_paragraph_style_spacing(style);
            }
        }
    });
}

/// Resolve a single paragraph's `space_after_pt` from its `pdf_metrics`, `inter_block_flow`,
/// and `extra_trailing_space_pt`.
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

    // 2. Apply same-page inter-block PDF vertical gap overrides (`InterBlockFlowIr`).
    match style.inter_block_flow {
        InterBlockFlowIr::Standard => {}
        InterBlockFlowIr::BeforeTightList { gap_pt } => {
            if let InterBlockGapAction::ZeroSpaceAfter =
                WordParagraphConstruct::solve_inter_block_gap_for_context(
                    gap_pt,
                    11.0,
                    11.0,
                    NextBlockSpacingContext::TightList,
                )
            {
                style.space_after_pt = 0.0;
            }
        }
        InterBlockFlowIr::SamePageParagraphGap {
            gap_pt,
            prev_font_pt,
            next_font_pt,
        } => {
            if let InterBlockGapAction::SetExactSpaceAfter(target_pt) =
                WordParagraphConstruct::solve_inter_block_gap_for_context(
                    gap_pt,
                    prev_font_pt,
                    next_font_pt,
                    NextBlockSpacingContext::Paragraph,
                )
            {
                style.space_after_pt = target_pt;
            }
        }
        InterBlockFlowIr::SamePageBlockGap {
            gap_pt,
            prev_font_pt,
            next_font_pt,
        } => {
            if let InterBlockGapAction::SetExactSpaceAfter(target_pt) =
                WordParagraphConstruct::solve_inter_block_gap_for_context(
                    gap_pt,
                    prev_font_pt,
                    next_font_pt,
                    NextBlockSpacingContext::Other,
                )
            {
                style.space_after_pt = target_pt;
            }
        }
    }

    // 3. Add any accumulated explicit vertical whitespace (`#v(...)` or cross-page gap).
    if style.extra_trailing_space_pt > 0.0 {
        style.space_after_pt += style.extra_trailing_space_pt;
        style.extra_trailing_space_pt = 0.0;
    }
}
