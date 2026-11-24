//! Inter-block vertical spacing solver (`flow_spacing`):
//! Resolves semantic `ParagraphStyleIr` fields and read-only `NodeReferenceLayout`
//! (`BlockLayoutSpanIr` + `NextBlockLayoutMeasureIr`) into concrete `ParagraphStyleIr::space_after_pt`
//! and [`crate::document::SolvedNodeLayout`] `before_dxa` / `after_dxa` overrides.

use std::collections::BTreeMap;

use ir::{
    BlockLayoutSpanIr, BlockNode, DocumentIr, NextBlockKindIr, NextBlockLayoutMeasureIr, NodeId,
    NodeReferenceLayout, PageSetupIr, ParagraphStyleIr, TableKindIr,
};

use crate::document::SolvedNodeLayout;
use crate::units::pt_to_dxa;
use crate::{InterBlockGapAction, NextBlockSpacingContext, WordParagraphConstruct};

/// How a lowered Word block participates in `compatibilityMode=15` vertical margin collapsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockMarginCarrier {
    /// Paragraph-backed (`<w:p>`): adjacent `w:after` / `w:before` margins collapse via `max(after, before)`.
    Paragraph,
    /// Table-backed (`<w:tbl>`): does NOT collapse margins across its boundary; trailing gaps transfer onto
    /// the following block's `w:before` (or a `40 dxa` separator `<w:p>` between consecutive tables).
    TableBacked,
}

/// Unified vertical box metrics for any Word block in inter-block flow spacing.
///
/// Encapsulates Word's intrinsic top and bottom empty vertical air (`ink_top_offset_pt`,
/// `ink_bottom_offset_pt`) relative to Typst's reference layout box plus its [`BlockMarginCarrier`],
/// allowing every adjacent block pair `(A, B)` to share one additive gap formula:
/// `needed_gap_pt(A, B, gap_pt) = (gap_pt - A.ink_bottom_offset_pt - B.ink_top_offset_pt).max(0.0)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WordBlockVerticalBox {
    pub carrier: BlockMarginCarrier,
    /// Intrinsic Word vertical air (`pt`) above the block's first visible ink/cap-height edge.
    pub ink_top_offset_pt: f64,
    /// Intrinsic Word vertical air (`pt`) below the block's last visible baseline/ink edge.
    pub ink_bottom_offset_pt: f64,
}

impl WordBlockVerticalBox {
    /// Derive [`WordBlockVerticalBox`] for a [`BlockNode`] given a surrounding `fallback_font_pt`.
    pub fn for_block(block: &BlockNode, fallback_font_pt: f64) -> Self {
        let fb = fallback_font_pt.max(6.0);
        match block {
            BlockNode::Container(_) => Self {
                carrier: BlockMarginCarrier::TableBacked,
                ink_top_offset_pt: 0.0,
                ink_bottom_offset_pt: 0.0,
            },
            BlockNode::Table(_) => Self {
                carrier: BlockMarginCarrier::TableBacked,
                ink_top_offset_pt: 0.0,
                ink_bottom_offset_pt: 0.0,
            },
            BlockNode::CodeBlock { fill, stroke, .. } if fill.is_some() || stroke.is_some() => {
                Self {
                    carrier: BlockMarginCarrier::TableBacked,
                    ink_top_offset_pt: 0.0,
                    ink_bottom_offset_pt: 0.0,
                }
            }
            BlockNode::HorizontalRule { .. } => Self {
                carrier: BlockMarginCarrier::Paragraph,
                ink_top_offset_pt: 0.0,
                ink_bottom_offset_pt: 0.0,
            },
            BlockNode::ShapeBlock(_) | BlockNode::CetzCanvas(_) | BlockNode::ImageBlock(_) => {
                Self {
                    carrier: BlockMarginCarrier::Paragraph,
                    ink_top_offset_pt: 0.02 * fb,
                    ink_bottom_offset_pt: 0.02 * fb,
                }
            }
            BlockNode::Figure { .. } => Self {
                carrier: BlockMarginCarrier::Paragraph,
                ink_top_offset_pt: 0.0,
                ink_bottom_offset_pt: 0.24 * fb,
            },
            BlockNode::EquationBlock(eq) => {
                let sz = eq.font_size_pt.unwrap_or(fb).max(6.0);
                let carrier = if eq.numbering_text.is_some() {
                    BlockMarginCarrier::TableBacked
                } else {
                    BlockMarginCarrier::Paragraph
                };
                Self {
                    carrier,
                    ink_top_offset_pt: 0.18 * sz,
                    ink_bottom_offset_pt: 0.18 * sz,
                }
            }
            BlockNode::Paragraph { style, .. } => {
                let sz = style.resolved_font_size_pt.unwrap_or(fb).max(6.0);
                Self {
                    carrier: BlockMarginCarrier::Paragraph,
                    ink_top_offset_pt: 0.20 * sz,
                    ink_bottom_offset_pt: 0.18 * sz,
                }
            }
            BlockNode::Heading { content, .. } | BlockNode::Title { content, .. } => {
                let sz = ir::dominant_inline_font_size_pt(content, fb).max(6.0);
                Self {
                    carrier: BlockMarginCarrier::Paragraph,
                    ink_top_offset_pt: 0.20 * sz,
                    ink_bottom_offset_pt: 0.18 * sz,
                }
            }
            _ => Self {
                carrier: BlockMarginCarrier::Paragraph,
                ink_top_offset_pt: 0.20 * fb,
                ink_bottom_offset_pt: 0.18 * fb,
            },
        }
    }

    /// Unified inter-block spacing formula subtracting both blocks' intrinsic Word vertical air.
    #[inline]
    pub fn needed_gap_pt(prev: Self, next: Self, measured_gap_pt: f64) -> f64 {
        (measured_gap_pt - prev.ink_bottom_offset_pt - next.ink_top_offset_pt).max(0.0)
    }
}

/// Active section's vertical content band (`pt`) derived from [`PageSetupIr`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageVerticalBand {
    pub content_top_pt: f64,
    pub content_bottom_pt: f64,
}

impl PageVerticalBand {
    pub fn from_page_setup(ps: &PageSetupIr) -> Self {
        let content_top_pt = ps.margin_top_pt.max(0.0);
        let content_bottom_pt = (ps.height_pt - ps.margin_bottom_pt).max(content_top_pt + 1.0);
        Self {
            content_top_pt,
            content_bottom_pt,
        }
    }

    #[inline]
    pub fn content_height_pt(self) -> f64 {
        (self.content_bottom_pt - self.content_top_pt).max(1.0)
    }
}

/// Resolve a single paragraph's `space_after_pt` from its semantic `StyleChain` properties,
/// optional read-only [`NodeReferenceLayout`], and the active section's [`PageVerticalBand`].
pub fn resolve_paragraph_style_spacing(
    style: &mut ParagraphStyleIr,
    node_layout: Option<&NodeReferenceLayout>,
    page_band: PageVerticalBand,
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
        apply_trailing_transition(&mut style.space_after_pt, span_a, measure, page_band);
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
    solved_nodes: &mut BTreeMap<NodeId, SolvedNodeLayout>,
) {
    for idx in 0..blocks.len().saturating_sub(1) {
        let id_a = blocks[idx].node_id();
        if id_a.is_unset() {
            continue;
        }
        let id_b = blocks[idx + 1].node_id();
        let flow_opt = ir.layout(id_a).and_then(|l| l.flow.as_ref());
        let involves_full_bbox_block = matches!(
            blocks[idx],
            BlockNode::Figure { .. } | BlockNode::EquationBlock(_)
        ) || matches!(blocks[idx + 1], BlockNode::EquationBlock(_));
        let (span_a, span_b, next_font_pt) = if involves_full_bbox_block
            && !id_b.is_unset()
            && let (Some(lay_a), Some(lay_b)) = (ir.layout(id_a), ir.layout(id_b))
        {
            let nf = flow_opt
                .and_then(|f| f.next.map(|n| n.next_font_pt))
                .unwrap_or(11.0)
                .max(6.0);
            (lay_a.span, lay_b.span, nf)
        } else if let Some(flow) = flow_opt
            && let Some(next) = flow.next
        {
            (flow.self_span, next.span, next.next_font_pt.max(6.0))
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
            if !id_b.is_unset() && matches!(blocks[idx + 1], BlockNode::EquationBlock(_)) {
                solved_nodes.entry(id_b).or_default().before_dxa = Some(0);
            }
            match &blocks[idx] {
                BlockNode::ShapeBlock(_) | BlockNode::Figure { .. } => {
                    solved_nodes.entry(id_a).or_default().after_dxa = Some(0);
                }
                BlockNode::EquationBlock(eq) => {
                    solved_nodes.entry(id_a).or_default().after_dxa = Some(0);
                    if eq.numbering_text.is_some() && pb == pa + 1 {
                        apply_table_backed_post_gap(
                            id_a,
                            &mut blocks[idx + 1],
                            next_font_pt,
                            solved_nodes,
                        );
                    }
                }
                BlockNode::Container(_) if pb == pa + 1 => {
                    // `<w:tbl>` has 0pt intrinsic bottom margin in Word, whereas Typst still applies
                    // `par.spacing` (~`1.0 * font_size` beyond internal leading) when testing whether
                    // the next block fits on page `pa`; Word suppresses `w:before` after soft breaks.
                    apply_table_backed_post_gap(id_a, &mut blocks[idx + 1], next_font_pt, solved_nodes);
                }
                BlockNode::CodeBlock { fill, stroke, .. }
                    if (fill.is_some() || stroke.is_some()) && pb == pa + 1 =>
                {
                    apply_table_backed_post_gap(id_a, &mut blocks[idx + 1], next_font_pt, solved_nodes);
                }
                BlockNode::Table(t)
                    if (t.kind != TableKindIr::LayoutGrid || is_graphic_object_row(t))
                        && pb == pa + 1 =>
                {
                    apply_table_backed_post_gap(id_a, &mut blocks[idx + 1], next_font_pt, solved_nodes);
                }
                _ => {}
            }
            continue;
        }

        let gap_pt = top_b - bottom_a;
        if !(-2.0..=500.0).contains(&gap_pt) {
            continue;
        }

        let box_a = WordBlockVerticalBox::for_block(&blocks[idx], next_font_pt);
        let box_b = WordBlockVerticalBox::for_block(&blocks[idx + 1], next_font_pt);
        let needed_gap_pt = WordBlockVerticalBox::needed_gap_pt(box_a, box_b, gap_pt);

        // 1. Solve pre-gap onto `blocks[idx + 1]` when `blocks[idx + 1]` is an `EquationBlock`
        //    or `HorizontalRule` (`#line(...)`).
        match &blocks[idx + 1] {
            BlockNode::EquationBlock(eq_b) => {
                let is_numbered_b = eq_b.numbering_text.is_some();
                // For consecutive numbered equation -> equation, table row boundary zeroes bottom air.
                let eff_box_a = if matches!(blocks[idx], BlockNode::EquationBlock(ref eq_a) if eq_a.numbering_text.is_some()) {
                    WordBlockVerticalBox {
                        ink_bottom_offset_pt: 0.0,
                        ..box_a
                    }
                } else {
                    box_a
                };
                let needed_before_pt =
                    WordBlockVerticalBox::needed_gap_pt(eff_box_a, box_b, gap_pt);
                if !is_numbered_b && !id_b.is_unset() {
                    solved_nodes.entry(id_b).or_default().before_dxa =
                        Some(pt_to_dxa(needed_before_pt));
                }
                if let BlockNode::Paragraph { style, .. } = &mut blocks[idx] {
                    if is_numbered_b {
                        style.space_after_pt = needed_before_pt;
                    } else if style.space_after_pt > needed_before_pt {
                        style.space_after_pt = needed_before_pt;
                    }
                }
            }
            BlockNode::HorizontalRule { .. } if !id_b.is_unset() => {
                solved_nodes.entry(id_b).or_default().before_dxa = Some(pt_to_dxa(needed_gap_pt));
            }
            _ => {}
        }

        // 2. Solve post-gap after `blocks[idx]` using its unified `WordBlockVerticalBox` & carrier.
        let is_non_layout_table = match &blocks[idx] {
            BlockNode::Table(t) => t.kind != TableKindIr::LayoutGrid || is_graphic_object_row(t),
            _ => true,
        };
        if !is_non_layout_table {
            continue;
        }

        match &blocks[idx] {
            BlockNode::HorizontalRule { .. } | BlockNode::Figure { .. } => {
                solved_nodes.entry(id_a).or_default().after_dxa = Some(pt_to_dxa(needed_gap_pt));
            }
            BlockNode::ShapeBlock(_) => {
                let needed_dxa = pt_to_dxa(needed_gap_pt);
                solved_nodes.entry(id_a).or_default().after_dxa = Some(needed_dxa);
                if matches!(blocks[idx + 1], BlockNode::ShapeBlock(_)) && !id_b.is_unset() {
                    solved_nodes.entry(id_b).or_default().before_dxa = Some(needed_dxa);
                }
            }
            BlockNode::EquationBlock(_)
            | BlockNode::Container(_)
            | BlockNode::Table(_) => {
                if box_a.carrier == BlockMarginCarrier::TableBacked {
                    apply_table_backed_post_gap(
                        id_a,
                        &mut blocks[idx + 1],
                        needed_gap_pt,
                        solved_nodes,
                    );
                } else {
                    solved_nodes.entry(id_a).or_default().after_dxa =
                        Some(pt_to_dxa(needed_gap_pt));
                }
            }
            BlockNode::CodeBlock { fill, stroke, .. } if fill.is_some() || stroke.is_some() => {
                apply_table_backed_post_gap(
                    id_a,
                    &mut blocks[idx + 1],
                    needed_gap_pt,
                    solved_nodes,
                );
            }
            _ => {}
        }
    }
}

/// Solve inter-item `w:before` spacing inside `BlockNode::Enum` / `BlockNode::List` whenever an
/// item ends with a `<w:tbl>` (e.g. a `#choice` option grid) and the following item starts with
/// a `Paragraph` on the same page.
///
/// Word `<w:tbl>` blocks carry zero trailing `w:after`, so both Typst's `enum.spacing` and any
/// explicit `#v(...)` placed after the trailing grid must transfer onto the next item's `w:before`.
/// Using the next paragraph's `first_baseline_y_pt` (rather than ink `top_y_pt`) keeps the gap
/// exact even when line 1 of the next item contains a tall inline fraction (`x^2/a^2`) that
/// protrudes upward into the inter-item whitespace in Typst.
pub fn solve_list_inter_item_spacing(
    blocks: &[BlockNode],
    ir: &DocumentIr,
    solved_nodes: &mut BTreeMap<NodeId, SolvedNodeLayout>,
) {
    let resolver = crate::font_metrics::FontMetricsResolver::new(&ir.font_metrics);
    let mut solve_pair = |prev_blocks: &[BlockNode], next_blocks: &[BlockNode]| {
        let Some(BlockNode::Table(tbl)) = prev_blocks.last() else {
            return;
        };
        let Some(BlockNode::Paragraph {
            id: id_b,
            content,
            style,
            ..
        }) = next_blocks.first()
        else {
            return;
        };
        if tbl.id.is_unset() || id_b.is_unset() {
            return;
        }
        let (Some(lay_a), Some(lay_b)) = (ir.layout(tbl.id), ir.layout(*id_b)) else {
            return;
        };
        let (Some((pa, bottom_a)), Some((pb, top_b))) = (
            lay_a.span.end_page_and_bottom_y(),
            lay_b.span.start_page_and_top_y(),
        ) else {
            return;
        };
        if pa != pb {
            return;
        }
        let sz_pt = style
            .resolved_font_size_pt
            .unwrap_or_else(|| ir::dominant_inline_font_size_pt(content, 11.0))
            .max(6.0);
        let has_cjk = crate::objects::paragraph::content_has_cjk_chars(content);
        let dom_font = content.iter().find_map(|inl| match inl {
            ir::InlineNode::Text(r) if !r.text.trim().is_empty() => r.font_family.as_deref(),
            _ => None,
        });
        let w_line_240 = crate::objects::paragraph::compute_line_spacing_dxa_with_metrics(
            content,
            style.line_spacing_pt,
            &ir.font_metrics,
            lay_b.paragraph_lines.as_ref(),
        );
        let (base_ascent_pt, _, _) =
            resolver.word_line_ascent_descent_total_pt(dom_font, sz_pt, has_cjk, w_line_240);

        let needed_before_pt = if let Some(pl) = lay_b.paragraph_lines.as_ref()
            && pl.first_baseline_y_pt > bottom_a
        {
            let has_line1_frac = content.iter().any(|inl| match inl {
                ir::InlineNode::EquationInline(eq) => {
                    crate::objects::equation::analyze_math_slice(&eq.body, eq.is_block)
                        .max_fraction_depth
                        > 0
                }
                _ => false,
            });
            let eff_ascent_pt = if has_line1_frac {
                base_ascent_pt + sz_pt * 0.22
            } else {
                base_ascent_pt
            };
            (pl.first_baseline_y_pt - bottom_a - eff_ascent_pt).clamp(0.0, 120.0)
        } else if top_b >= bottom_a - 2.0 {
            let box_a = WordBlockVerticalBox::for_block(&BlockNode::Table(tbl.clone()), sz_pt);
            let box_b = WordBlockVerticalBox::for_block(next_blocks.first().unwrap(), sz_pt);
            WordBlockVerticalBox::needed_gap_pt(box_a, box_b, top_b - bottom_a).clamp(0.0, 120.0)
        } else {
            return;
        };

        solved_nodes.entry(*id_b).or_default().before_dxa = Some(pt_to_dxa(needed_before_pt));
    };

    for block in blocks {
        match block {
            BlockNode::Enum { items, .. } => {
                for pair in items.windows(2) {
                    solve_pair(&pair[0].blocks, &pair[1].blocks);
                }
            }
            BlockNode::List { items, .. } => {
                for pair in items.windows(2) {
                    solve_pair(&pair[0].blocks, &pair[1].blocks);
                }
            }
            _ => {}
        }
    }
}

/// Whether a block lowers to a Word paragraph wrapping a single inline drawing (no text line box).
fn is_graphic_block(block: &BlockNode) -> bool {
    matches!(
        block,
        BlockNode::ShapeBlock(_) | BlockNode::CetzCanvas(_) | BlockNode::ImageBlock(_)
    )
}

/// Whether a `LayoutGrid` table is a single horizontal row of graphic objects (e.g. lowered
/// `#stack(dir: ltr, rect, ellipse, polygon)`), whose trailing gap must be solved like any other
/// `<w:tbl>`-backed block because no cell paragraph carries a Typst `par.spacing`.
fn is_graphic_object_row(t: &ir::TableIr) -> bool {
    t.kind == TableKindIr::LayoutGrid
        && t.rows.len() == 1
        && t.rows[0]
            .cells
            .iter()
            .all(|c| !c.blocks.is_empty() && c.blocks.iter().all(is_graphic_block))
}

fn apply_table_backed_post_gap(
    prev_table_id: NodeId,
    next_block: &mut BlockNode,
    needed_before_pt: f64,
    solved_nodes: &mut BTreeMap<NodeId, SolvedNodeLayout>,
) {
    let needed_dxa = pt_to_dxa(needed_before_pt);
    match next_block {
        BlockNode::Paragraph { style, .. } => {
            style.space_before_pt = style.space_before_pt.max(needed_before_pt);
        }
        BlockNode::Container(_) | BlockNode::Table(_) => {
            // Between two `<w:tbl>` blocks, `docx-writer` emits a `2pt` (`40 dxa`) exact separator `<w:p>`;
            // store `(needed_dxa - 40)` as its `w:after`.
            solved_nodes.entry(prev_table_id).or_default().after_dxa = Some((needed_dxa - 40).max(0));
        }
        BlockNode::CodeBlock { fill, stroke, .. } if fill.is_some() || stroke.is_some() => {
            solved_nodes.entry(prev_table_id).or_default().after_dxa = Some((needed_dxa - 40).max(0));
        }
        other => {
            let next_id = other.node_id();
            if !next_id.is_unset() {
                let slot = &mut solved_nodes.entry(next_id).or_default().before_dxa;
                *slot = Some(slot.unwrap_or(0).max(needed_dxa));
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
    let ctx = match next.kind {
        NextBlockKindIr::TightList => NextBlockSpacingContext::TightList,
        NextBlockKindIr::Paragraph => NextBlockSpacingContext::Paragraph,
        _ => NextBlockSpacingContext::Other,
    };
    match WordParagraphConstruct::solve_inter_block_gap_with_bounds(
        gap_pt,
        next.prev_font_pt,
        next.next_font_pt,
        ctx,
        48.0,
        500.0,
    ) {
        InterBlockGapAction::ZeroSpaceAfter => *space_after_pt = 0.0,
        InterBlockGapAction::SetExactSpaceAfter(target_pt) => *space_after_pt = target_pt,
        InterBlockGapAction::AddExtraSpaceAfter(extra_pt) => *space_after_pt += extra_pt,
        InterBlockGapAction::KeepDefault => {}
    }
}

/// Resolve adjacent-paragraph font-size transition spacing inside `word-construct` so `docx-writer`
/// never mutates paragraph `space_after_pt` during OpenXML serialization.
pub fn resolve_adjacent_paragraph_spacing(blocks: &mut [BlockNode]) {
    for idx in 0..blocks.len().saturating_sub(1) {
        let (left, right) = blocks.split_at_mut(idx + 1);
        if let BlockNode::Paragraph { content, style, .. } = &mut left[idx] {
            *style = crate::objects::paragraph::ParagraphConstruct::solve_adjacent_paragraph_style(
                content,
                style,
                right.first(),
            );
        }
    }
}

fn apply_trailing_transition(
    space_after_pt: &mut f64,
    span_a: BlockLayoutSpanIr,
    measure: NextBlockLayoutMeasureIr,
    page_band: PageVerticalBand,
) {
    let (Some((pa, bottom_a)), Some((pb, top_b))) = (
        span_a.end_page_and_bottom_y(),
        measure.span.start_page_and_top_y(),
    ) else {
        return;
    };

    let content_top = page_band.content_top_pt;
    let content_bottom = page_band.content_bottom_pt;
    let content_h = page_band.content_height_pt();
    let max_gap_pt = (content_h * 0.75).max(120.0);

    if let NextBlockKindIr::ListItemSibling {
        list_level,
        active_columns,
        prev_item_block_count,
        next_item_starts_with_paragraph,
    } = measure.kind
    {
        if pa == pb && top_b >= bottom_a - 2.0 {
            let gap_pt = top_b - bottom_a;
            if gap_pt > 48.0 && gap_pt < max_gap_pt {
                *space_after_pt += (gap_pt - 10.0).max(0.0);
            }
        } else if pb == pa + 1
            && list_level == 0
            && active_columns <= 1
            && prev_item_block_count <= 2
            && next_item_starts_with_paragraph
        {
            let effective_bottom =
                (content_bottom - measure.prev_font_pt.max(11.0)).max(content_top);
            if bottom_a > content_top + 24.0 && bottom_a < effective_bottom - 100.0 {
                let gap_pt = (effective_bottom - bottom_a) + (top_b - content_top).max(0.0);
                if gap_pt > 60.0 && gap_pt < max_gap_pt {
                    *space_after_pt += (gap_pt - 15.0).max(0.0);
                }
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
        if !is_tight_list && !is_display_par && gap_pt > 48.0 && gap_pt < max_gap_pt {
            *space_after_pt += (gap_pt - 12.0).max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::{FlowMeasureIr, NodeReferenceLayout};

    #[test]
    fn cross_page_list_gap_uses_actual_page_setup_dimensions() {
        let small_page = PageSetupIr {
            width_pt: 419.53,
            height_pt: 595.28, // A5
            margin_top_pt: 40.0,
            margin_bottom_pt: 40.0,
            ..Default::default()
        };
        let band = PageVerticalBand::from_page_setup(&small_page);
        let span_a = BlockLayoutSpanIr::SinglePage {
            page_idx: 0,
            top_y_pt: 250.0,
            bottom_y_pt: 300.0,
            x_pt: 40.0,
        };
        let span_b = BlockLayoutSpanIr::SinglePage {
            page_idx: 1,
            top_y_pt: 45.0,
            bottom_y_pt: 60.0,
            x_pt: 40.0,
        };
        let layout = NodeReferenceLayout {
            flow: Some(FlowMeasureIr {
                self_span: span_a,
                next: None,
                trailing_transitions: vec![(
                    span_a,
                    NextBlockLayoutMeasureIr {
                        span: span_b,
                        kind: NextBlockKindIr::ListItemSibling {
                            list_level: 0,
                            active_columns: 1,
                            prev_item_block_count: 1,
                            next_item_starts_with_paragraph: true,
                        },
                        prev_font_pt: 11.0,
                        next_font_pt: 11.0,
                    },
                )],
                in_multi_block_group_tail: false,
            }),
            ..Default::default()
        };

        let mut style = ParagraphStyleIr {
            space_after_pt: 0.0,
            ..Default::default()
        };
        resolve_paragraph_style_spacing(&mut style, Some(&layout), band);
        // effective_bottom = (595.28 - 40.0) - 11.0 = 544.28
        // gap_pt = (544.28 - 300.0) + (45.0 - 40.0) = 249.28 => + (249.28 - 15.0) = 234.28
        assert!(
            (style.space_after_pt - 234.28).abs() < 0.05,
            "expected ~234.28pt on A5 page, got {}",
            style.space_after_pt
        );
    }
}
