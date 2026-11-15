//! Solved Word Document Representation (`WordDocument`), constructed from an immutable [`DocumentIr`].
//!
//! [`build_word_document`] takes `&DocumentIr` by shared reference (never mutating `DocumentIr`),
//! resolves Word inter-block and list-item vertical spacing across all sections/headers/footers,
//! and pre-computes document-level Word compatibility flags (`has_cjk`, `has_cetz_canvas`).

use std::collections::BTreeMap;

use ir::{
    BlockNode, ContainerIr, ContainerStyleIr, DocumentIr, DocumentSectionIr, FontFaceMetricsIr,
    HAlignIr, InlineNode, MetadataIr, NodeId, PageSetupIr, ShapeIr, ShapeKindIr, SidesPtIr,
    TableCellIr, TableIr, TableKindIr, TableRowIr, VAlignIr, merge_split_styled_containers,
    visit_block_list_mut, visit_page_setup_blocks_mut,
};

use crate::objects::paragraph::compute_line_spacing_dxa_with_metrics;
use crate::objects::shape::{
    ShapeLocalBaselinesPt, SolvedShapeGeometry, solve_shape_geometry,
    solve_shape_geometry_with_baselines,
};
use crate::objects::text_run::run_has_cjk;
use crate::pipeline::flow_spacing::{
    resolve_paragraph_style_spacing, solve_container_and_figure_flow_spacing,
};
use crate::units::pt_to_dxa;

/// Pre-solved `w:line` (`1/240` line units) for a `BlockNode::Paragraph`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SolvedParagraphLineDxa {
    pub orig_line_spacing_pt: Option<f64>,
    pub line_dxa: i64,
    /// Whether Typst laid out this paragraph across 2+ visual lines (`line_count >= 2`),
    /// in which case single-line table/grid cell-height trims must not compress its line step.
    pub is_multiline: bool,
}

/// Solved Word document ready for OpenXML (`.docx`) serialization by `docx-writer`.
#[derive(Debug, Clone, Default)]
pub struct WordDocument {
    pub sections: Vec<DocumentSectionIr>,
    pub metadata: MetadataIr,
    pub page_setup: PageSetupIr,
    pub blocks: Vec<BlockNode>,
    /// Extracted OpenType binary font metrics (`upem`, `hhea`, `OS/2`, CJK codepage, system status)
    /// carried from `DocumentIr` for `docx-writer` line-box queries.
    pub font_metrics: BTreeMap<String, FontFaceMetricsIr>,
    /// Pre-solved `w:line` (`1/240` line units) per paragraph `NodeId`, incorporating
    /// both `font_metrics` and read-only `NodeReferenceLayout::paragraph_lines` while keeping
    /// `DocumentIr` 100% immutable and Word-independent.
    pub solved_line_dxa: BTreeMap<NodeId, SolvedParagraphLineDxa>,
    /// Pre-solved `(w_line, is_exact)` per `CodeBlock` `NodeId`. When Typst measured a uniform
    /// baseline step across lines (`uniform_baseline_step_pt`), `is_exact = true` (`w:lineRule="exact"`
    /// in twips) keeps mixed ASCII + CJK comment lines on an identical fixed-pitch grid.
    pub solved_code_line_spacing: BTreeMap<NodeId, (i64, bool)>,
    /// Pre-solved DrawingML `<wps:bodyPr>`, `<wp:effectExtent>`, and `<w:position>` geometry per shape `NodeId`.
    pub solved_shapes: BTreeMap<NodeId, SolvedShapeGeometry>,
    /// Pre-solved `w:before` (`dxa`) overrides for blocks immediately following `<w:tbl>` containers/tables.
    pub solved_block_before_dxa: BTreeMap<NodeId, i64>,
    /// Pre-solved `w:after` (`dxa`) overrides for `ShapeBlock` / `Figure` wrappers and table-to-table separators.
    pub solved_block_after_dxa: BTreeMap<NodeId, i64>,
    /// Whether the document contains any East Asian (CJK) characters or CJK typeface runs
    /// (requires `<w:useFELayout/>` and `zh-CN` theme font language in `word/settings.xml`).
    pub has_cjk: bool,
    /// Whether the document contains any CeTZ canvases (`<wpg:wgp>` WordprocessingGroup),
    /// requiring `compatibilityMode=15` in `word/settings.xml`.
    pub has_cetz_canvas: bool,
}

impl WordDocument {
    /// Construct a [`WordDocument`] from an immutable [`DocumentIr`].
    pub fn from_ir(ir: &DocumentIr) -> Self {
        build_word_document(ir)
    }

    /// Look up the pre-solved `w:line` (`1/240` line units) for paragraph `id`, or recompute
    /// with `self.font_metrics` if `id` is unset or a container/table solver overrode a single-line
    /// paragraph's `line_spacing_pt`. Multi-line paragraphs (`is_multiline == true`) always retain
    /// their calibrated Typst baseline step rather than single-line cell-height trims.
    pub fn resolve_paragraph_line_dxa(
        &self,
        id: NodeId,
        content: &[InlineNode],
        line_spacing_pt: Option<f64>,
    ) -> i64 {
        if !id.is_unset()
            && let Some(&solved) = self.solved_line_dxa.get(&id)
            && (solved.orig_line_spacing_pt == line_spacing_pt || solved.is_multiline)
        {
            return solved.line_dxa;
        }
        compute_line_spacing_dxa_with_metrics(content, line_spacing_pt, &self.font_metrics, None)
    }

    /// Look up the pre-solved `(w:line, is_exact)` for a `CodeBlock` `id`, or compute
    /// `w:lineRule="auto"` from its first text run and `self.font_metrics`.
    pub fn resolve_code_block_line_spacing(
        &self,
        id: NodeId,
        lines: &[Vec<ir::TextRunIr>],
    ) -> (i64, bool) {
        if !id.is_unset()
            && let Some(&solved) = self.solved_code_line_spacing.get(&id)
        {
            return solved;
        }
        let sample: Vec<InlineNode> = lines
            .iter()
            .flatten()
            .find(|r| !r.text.is_empty())
            .or_else(|| lines.iter().flatten().next())
            .cloned()
            .map(InlineNode::Text)
            .into_iter()
            .collect();
        (
            compute_line_spacing_dxa_with_metrics(&sample, None, &self.font_metrics, None),
            false,
        )
    }

    /// Look up the pre-solved `w:line` (`1/240` line units or exact `dxa`) for a `CodeBlock` `id`.
    pub fn resolve_code_block_line_dxa(
        &self,
        id: NodeId,
        lines: &[Vec<ir::TextRunIr>],
    ) -> i64 {
        self.resolve_code_block_line_spacing(id, lines).0
    }

    /// Look up pre-solved DrawingML shape geometry for `shape.id`, or compute on demand for synthetic shapes.
    pub fn resolve_shape_geometry(&self, shape: &ShapeIr, is_inline: bool) -> SolvedShapeGeometry {
        if shape.id.is_set()
            && let Some(&solved) = self.solved_shapes.get(&shape.id)
        {
            return solved;
        }
        solve_shape_geometry(shape, is_inline, 240, None, &self.font_metrics)
    }
}

impl From<&DocumentIr> for WordDocument {
    fn from(ir: &DocumentIr) -> Self {
        build_word_document(ir)
    }
}

impl From<DocumentIr> for WordDocument {
    fn from(ir: DocumentIr) -> Self {
        build_word_document(&ir)
    }
}

/// Build a solved [`WordDocument`] from an immutable [`DocumentIr`] without mutating `ir`.
pub fn build_word_document(ir: &DocumentIr) -> WordDocument {
    let mut page_setup = ir.page_setup.clone();
    let mut blocks = ir.blocks.clone();
    let mut sections = ir.sections.clone();
    let mut solved_line_dxa: BTreeMap<NodeId, SolvedParagraphLineDxa> = BTreeMap::new();
    let mut solved_code_line_spacing: BTreeMap<NodeId, (i64, bool)> = BTreeMap::new();
    let mut solved_shapes: BTreeMap<NodeId, SolvedShapeGeometry> = BTreeMap::new();
    let mut solved_block_before_dxa: BTreeMap<NodeId, i64> = BTreeMap::new();
    let mut solved_block_after_dxa: BTreeMap<NodeId, i64> = BTreeMap::new();

    let mut solve_section_lists = |ps: &mut PageSetupIr, sec_blocks: &mut Vec<BlockNode>| {
        let fallback_w = (ps.width_pt - ps.margin_left_pt - ps.margin_right_pt).max(72.0);
        let mut resolve_blocks = |blk_list: &mut Vec<BlockNode>| {
            promote_standalone_partial_border_inline_boxes(blk_list, ir);
            solve_container_and_figure_flow_spacing(
                blk_list,
                ir,
                &mut solved_block_before_dxa,
                &mut solved_block_after_dxa,
            );
            lower_containers_in_blocks(blk_list, fallback_w);
            merge_split_styled_containers(blk_list);
            refine_word_tables_in_blocks(blk_list, ir);
            for block in blk_list.iter_mut() {
                match block {
                    BlockNode::Paragraph {
                        id, content, style, ..
                    } => {
                        // In Typst, `K >= 1` trailing `linebreak()`s at the end of a `ParElem`
                        // produce `K - 1` blank lines because the paragraph boundary itself ends
                        // the final line; in Word, `</w:p>` already terminates a line, so every
                        // `<w:br/>` before `</w:p>` adds a full blank line. Strip one trailing
                        // `LineBreak` here while keeping `DocumentIr` untouched.
                        if matches!(content.last(), Some(InlineNode::LineBreak)) {
                            content.pop();
                        }
                        let node_layout = ir.layout(*id);
                        resolve_paragraph_style_spacing(style, node_layout);
                        let para_lines = node_layout.and_then(|l| l.paragraph_lines.as_ref());
                        let is_multiline = para_lines.is_some_and(|p| p.line_count >= 2);
                        let w_line = compute_line_spacing_dxa_with_metrics(
                            content,
                            style.line_spacing_pt,
                            &ir.font_metrics,
                            para_lines,
                        );
                        if !id.is_unset() {
                            solved_line_dxa.insert(
                                *id,
                                SolvedParagraphLineDxa {
                                    orig_line_spacing_pt: style.line_spacing_pt,
                                    line_dxa: w_line,
                                    is_multiline,
                                },
                            );
                        }
                        solve_inline_shapes_recursive(content, w_line, ir, &mut solved_shapes);
                    }
                    BlockNode::Heading { content, .. } | BlockNode::Title { content, .. } => {
                        let w_line = compute_line_spacing_dxa_with_metrics(
                            content,
                            None,
                            &ir.font_metrics,
                            None,
                        );
                        solve_inline_shapes_recursive(content, w_line, ir, &mut solved_shapes);
                    }
                    BlockNode::CodeBlock { id, lines, .. } => {
                        let para_lines =
                            ir.layout(*id).and_then(|l| l.paragraph_lines.as_ref());
                        if let Some(step_pt) = para_lines.and_then(|p| p.uniform_baseline_step_pt) {
                            // Multi-line `raw(block: true)` uses an exact fixed-pitch line step
                            // (`w:lineRule="exact"` in twips) so CJK comments or paragraph marks
                            // never stretch individual lines above Typst's uniform code grid.
                            let exact_dxa = pt_to_dxa(step_pt).max(160);
                            if !id.is_unset() {
                                solved_code_line_spacing.insert(*id, (exact_dxa, true));
                            }
                        } else {
                            let sample: Vec<InlineNode> = lines
                                .iter()
                                .flatten()
                                .find(|r| !r.text.is_empty())
                                .or_else(|| lines.iter().flatten().next())
                                .cloned()
                                .map(InlineNode::Text)
                                .into_iter()
                                .collect();
                            if !sample.is_empty() && !id.is_unset() {
                                let w_line = compute_line_spacing_dxa_with_metrics(
                                    &sample,
                                    None,
                                    &ir.font_metrics,
                                    para_lines,
                                );
                                solved_code_line_spacing.insert(*id, (w_line, false));
                            }
                        }
                    }
                    BlockNode::ShapeBlock(shape) => {
                        if shape.id.is_set() {
                            let local_baselines = shape_local_baselines_pt(shape, ir);
                            let geom = solve_shape_geometry_with_baselines(
                                shape,
                                false,
                                240,
                                None,
                                local_baselines,
                                &ir.font_metrics,
                            );
                            solved_shapes.insert(shape.id, geom);
                        }
                    }
                    _ => {}
                }
            }
        };
        visit_page_setup_blocks_mut(ps, &mut resolve_blocks);
        visit_block_list_mut(sec_blocks, &mut resolve_blocks);
    };

    solve_section_lists(&mut page_setup, &mut blocks);
    for section in &mut sections {
        solve_section_lists(&mut section.page_setup, &mut section.blocks);
    }

    let has_cjk = blocks_have_cjk(&blocks)
        || page_setup_has_cjk(&page_setup)
        || sections
            .iter()
            .any(|s| blocks_have_cjk(&s.blocks) || page_setup_has_cjk(&s.page_setup));

    let has_cetz_canvas = blocks
        .iter()
        .any(|b| matches!(b, BlockNode::CetzCanvas(_)))
        || sections
            .iter()
            .any(|s| s.blocks.iter().any(|b| matches!(b, BlockNode::CetzCanvas(_))));

    WordDocument {
        sections,
        metadata: ir.metadata.clone(),
        page_setup,
        blocks,
        font_metrics: ir.font_metrics.clone(),
        solved_line_dxa,
        solved_code_line_spacing,
        solved_shapes,
        solved_block_before_dxa,
        solved_block_after_dxa,
        has_cjk,
        has_cetz_canvas,
    }
}

fn solve_inline_shapes_recursive(
    inlines: &[InlineNode],
    parent_w_line: i64,
    ir: &DocumentIr,
    out: &mut BTreeMap<NodeId, SolvedShapeGeometry>,
) {
    for inl in inlines {
        match inl {
            InlineNode::ShapeInline(shape) => {
                if shape.id.is_set() {
                    let ctx = ir
                        .layout(shape.id)
                        .and_then(|l| l.inline_box_context.as_ref());
                    let local_baselines = shape_local_baselines_pt(shape, ir);
                    let geom = solve_shape_geometry_with_baselines(
                        shape,
                        true,
                        parent_w_line,
                        ctx,
                        local_baselines,
                        &ir.font_metrics,
                    );
                    out.insert(shape.id, geom);
                }
            }
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. }
            | InlineNode::Field { content, .. } => {
                solve_inline_shapes_recursive(content, parent_w_line, ir, out);
            }
            _ => {}
        }
    }
}

/// Measure ground-truth local baselines (`b_0` and optional `b_1` + exact code step) inside a
/// `ShapeBlock` relative to its top edge when laid out on the same page.
fn shape_local_baselines_pt(shape: &ShapeIr, ir: &DocumentIr) -> Option<ShapeLocalBaselinesPt> {
    let shape_frag = ir.layout(shape.id)?.fragments.first()?;
    let first_block = shape.body.first()?;
    let first_block_id = first_block.node_id();
    if first_block_id.is_unset() {
        return None;
    }
    let child0_layout = ir.layout(first_block_id)?;
    let child0_frag = child0_layout.fragments.first()?;
    if child0_frag.page_idx != shape_frag.page_idx {
        return None;
    }
    let p0 = child0_layout.paragraph_lines.as_ref()?;
    let b_0 = p0.first_baseline_y_pt - shape_frag.top_y_pt;
    if !(b_0 > 0.5 && b_0 < shape.height_pt) {
        return None;
    }
    let first_is_single_line_para = match first_block {
        BlockNode::Paragraph { .. } => p0.line_count == 1,
        BlockNode::CodeBlock { lines, .. } => lines.len() == 1 && p0.line_count <= 1,
        _ => false,
    };
    let second_baseline_and_step_pt = shape.body.get(1).and_then(|second_block| {
        let second_id = second_block.node_id();
        if second_id.is_unset() {
            return None;
        }
        let child1_layout = ir.layout(second_id)?;
        let child1_frag = child1_layout.fragments.first()?;
        if child1_frag.page_idx != shape_frag.page_idx {
            return None;
        }
        let p1 = child1_layout.paragraph_lines.as_ref()?;
        let b_1 = p1.first_baseline_y_pt - shape_frag.top_y_pt;
        (b_1 > b_0 && b_1 < shape.height_pt).then_some((b_1, p1.uniform_baseline_step_pt))
    });
    Some(ShapeLocalBaselinesPt {
        first_baseline_pt: b_0,
        first_is_single_line_para,
        first_visual_line_count: p0.line_count.max(1),
        first_para_exact_step_pt: p0.uniform_baseline_step_pt,
        second_baseline_and_step_pt,
    })
}

/// Promote standalone rectangular partial-border `InlineNode::ShapeInline` boxes
/// (e.g. `#box(width: 100%, stroke: (left: 2pt + blue), fill: ...)[...]`) into flow
/// `BlockNode::Container`s (`breakable: false`) because DrawingML `<a:ln>` cannot stroke
/// individual sides of a rectangle whereas Word 1x1 `<w:tbl>` (`<w:tcBorders>`) can.
fn promote_standalone_partial_border_inline_boxes(blocks: &mut Vec<BlockNode>, ir: &DocumentIr) {
    let mut i = 0;
    while i < blocks.len() {
        let split_idx = match &blocks[i] {
            BlockNode::Paragraph { content, .. } => content.iter().position(|inl| {
                if let InlineNode::ShapeInline(sh) = inl {
                    let has_partial_border = sh.stroke.is_none() && sh.borders.has_any();
                    let standalone = ir
                        .layout(sh.id)
                        .and_then(|l| l.inline_box_context.as_ref())
                        .is_none_or(|c| !c.shares_line_with_text);
                    sh.corner_radius_pt.is_none() && has_partial_border && standalone && !sh.body.is_empty()
                } else {
                    false
                }
            }),
            _ => None,
        };

        let Some(pos) = split_idx else {
            i += 1;
            continue;
        };

        let BlockNode::Paragraph {
            id: orig_id,
            mut content,
            style,
            bookmark,
        } = blocks.remove(i)
        else {
            unreachable!()
        };

        let after_inlines = trim_edge_breaks(content.split_off(pos + 1));
        let Some(InlineNode::ShapeInline(sh)) = content.pop() else {
            unreachable!()
        };
        let before_inlines = trim_edge_breaks(content);

        let mut replacement = Vec::with_capacity(3);
        let mut used_orig_id = false;
        if !before_inlines.is_empty() {
            let mut b_style = style.clone();
            b_style.space_after_pt = style.resolved_par_leading_pt.unwrap_or(5.0).min(8.0);
            replacement.push(BlockNode::Paragraph {
                id: orig_id,
                content: before_inlines,
                style: b_style,
                bookmark: bookmark.clone(),
            });
            used_orig_id = true;
        }

        replacement.push(BlockNode::Container(ContainerIr {
            id: sh.id,
            width_pt: Some(sh.width_pt),
            height_pt: None,
            align: style.align,
            left_indent_pt: style.left_indent_pt,
            style: ContainerStyleIr {
                fill: sh.fill,
                borders: sh.borders,
                inset: sh.inset,
                corner_radius_pt: None,
            },
            breakable: false,
            sticky: false,
            body: sh.body,
        }));

        if !after_inlines.is_empty() {
            let mut a_style = style.clone();
            a_style.first_line_indent_pt = 0.0;
            replacement.push(BlockNode::Paragraph {
                id: if used_orig_id { NodeId::UNSET } else { orig_id },
                content: after_inlines,
                style: a_style,
                bookmark: if used_orig_id { None } else { bookmark },
            });
        }

        let added = replacement.len();
        blocks.splice(i..i, replacement);
        i += added.min(1);
    }
}

fn trim_edge_breaks(mut inlines: Vec<InlineNode>) -> Vec<InlineNode> {
    let is_blank = |inl: &InlineNode| match inl {
        InlineNode::LineBreak => true,
        InlineNode::Text(r) => r.text.trim().is_empty() && r.underline.is_none() && r.highlight.is_none(),
        InlineNode::HorizontalSpace { .. } => true,
        _ => false,
    };
    while inlines.first().is_some_and(is_blank) {
        inlines.remove(0);
    }
    while inlines.last().is_some_and(is_blank) {
        inlines.pop();
    }
    inlines
}

fn inlines_have_cjk(inlines: &[InlineNode]) -> bool {
    inlines.iter().any(|inl| match inl {
        InlineNode::Text(r) => run_has_cjk(r),
        InlineNode::Field { content, .. }
        | InlineNode::Link { content, .. }
        | InlineNode::Ref { content, .. }
        | InlineNode::Citation { content, .. } => inlines_have_cjk(content),
        InlineNode::Footnote { content, .. } => blocks_have_cjk(content),
        InlineNode::ShapeInline(s) => blocks_have_cjk(&s.body),
        InlineNode::CetzInline(c) => c.labels.iter().any(|l| inlines_have_cjk(&l.content)),
        _ => false,
    })
}

fn blocks_have_cjk(blocks: &[BlockNode]) -> bool {
    blocks.iter().any(|b| match b {
        BlockNode::Title { content, .. }
        | BlockNode::Heading { content, .. }
        | BlockNode::Paragraph { content, .. } => inlines_have_cjk(content),
        BlockNode::List { items, .. } => items.iter().any(|i| blocks_have_cjk(&i.blocks)),
        BlockNode::Enum { items, .. } => items.iter().any(|i| blocks_have_cjk(&i.blocks)),
        BlockNode::Terms { items, .. } => items
            .iter()
            .any(|i| inlines_have_cjk(&i.term) || blocks_have_cjk(&i.description)),
        BlockNode::Container(c) => blocks_have_cjk(&c.body),
        BlockNode::Table(t) => t
            .rows
            .iter()
            .any(|r| r.cells.iter().any(|c| blocks_have_cjk(&c.blocks))),
        BlockNode::Quote {
            body, attribution, ..
        } => blocks_have_cjk(body) || attribution.as_deref().is_some_and(inlines_have_cjk),
        BlockNode::CodeBlock { lines, .. } => {
            lines.iter().any(|line| line.iter().any(run_has_cjk))
        }
        BlockNode::Figure { body, caption, .. } => {
            blocks_have_cjk(body) || caption.as_deref().is_some_and(inlines_have_cjk)
        }
        BlockNode::ShapeBlock(s) => blocks_have_cjk(&s.body),
        BlockNode::CetzCanvas(c) => c.labels.iter().any(|l| inlines_have_cjk(&l.content)),
        BlockNode::Outline { title, entries } => {
            title.as_deref().is_some_and(inlines_have_cjk)
                || entries
                    .iter()
                    .any(|e| inlines_have_cjk(&e.title) || inlines_have_cjk(&e.page_inlines))
        }
        BlockNode::Bibliography { title, entries } => {
            title.as_deref().is_some_and(inlines_have_cjk)
                || entries.iter().any(|e| inlines_have_cjk(&e.content))
        }
        BlockNode::Columns { body, .. } => blocks_have_cjk(body),
        _ => false,
    })
}

fn page_setup_has_cjk(ps: &PageSetupIr) -> bool {
    blocks_have_cjk(&ps.header)
        || blocks_have_cjk(&ps.footer)
        || ps.first_page_header.as_deref().is_some_and(blocks_have_cjk)
        || ps.first_page_footer.as_deref().is_some_and(blocks_have_cjk)
        || ps.even_page_header.as_deref().is_some_and(blocks_have_cjk)
        || ps.even_page_footer.as_deref().is_some_and(blocks_have_cjk)
}

/// Lower Word-specific `<w:tbl>` layout edge cases while keeping `DocumentIr` immutable:
/// 1. Prune degenerate `0%`-width (`<= 0.5pt`) empty columns (e.g. `17 / 17` progress bars).
/// 2. Convert ultra-thin (`0.5pt..=6.0pt`) unbordered solid-bar rows into cell-shaded rows
///    so Word `<wp:inline>` paragraph line boxes do not inflate `2pt` progress/divider bars.
/// 3. In 1-row `LayoutGrid`s, let tight `auto`-width single-line cells borrow slack width from
///    adjacent empty `1fr` spacer cells (preserving total width & outer edges) so counters like
///    `"17 / 17"` never wrap onto 2 lines due to minor Typst-vs-Word glyph advance differences.
fn refine_word_tables_in_blocks(blocks: &mut [BlockNode], ir: &DocumentIr) {
    for block in blocks.iter_mut() {
        let BlockNode::Table(tbl) = block else {
            continue;
        };

        // 1. Convert ultra-thin unbordered solid-bar rows (`0.5pt..=6.0pt`) to shaded empty cells.
        for row in &mut tbl.rows {
            let is_thin_row = row.height_pt.is_some_and(|h| (0.5..=6.0).contains(&h))
                && !row.cells.is_empty()
                && row.cells.iter().all(|c| {
                    !c.borders.has_any()
                        && ((c.blocks.is_empty() && c.fill.is_some())
                            || extract_solid_bar_fill(&c.blocks).is_some())
                });
            if is_thin_row {
                for cell in &mut row.cells {
                    if cell.fill.is_none() {
                        cell.fill = extract_solid_bar_fill(&cell.blocks);
                    }
                    cell.inset = SidesPtIr::zero();
                    cell.blocks.clear();
                }
            }
        }

        // 2. Prune degenerate `<= 0.5pt` empty columns when no cell uses `colspan > 1`.
        let has_colspan = tbl
            .rows
            .iter()
            .any(|r| r.cells.iter().any(|c| c.colspan > 1));
        if !has_colspan && tbl.col_widths_pt.len() > 1 {
            let mut col = tbl.col_widths_pt.len();
            while col > 0 {
                col -= 1;
                if tbl.col_widths_pt.len() <= 1 || tbl.col_widths_pt[col] > 0.5 {
                    continue;
                }
                let all_empty = tbl.rows.iter().all(|r| {
                    r.cells
                        .iter()
                        .filter(|c| c.x == col)
                        .all(|c| c.blocks.is_empty())
                });
                if all_empty {
                    tbl.col_widths_pt.remove(col);
                    for r in &mut tbl.rows {
                        r.cells.retain(|c| c.x != col);
                        for c in &mut r.cells {
                            if c.x > col {
                                c.x -= 1;
                            }
                        }
                    }
                }
            }
        }

        // 3. In 1-row `LayoutGrid`s, transfer slack from adjacent empty spacer cells to tight
        //    single-line `auto` cells while preserving total width and outer left/right edges.
        if tbl.kind == TableKindIr::LayoutGrid && tbl.rows.len() == 1 {
            let n = tbl.col_widths_pt.len();
            let row = &mut tbl.rows[0];
            let simple_grid = n >= 2
                && row.cells.len() == n
                && row
                    .cells
                    .iter()
                    .enumerate()
                    .all(|(i, c)| c.x == i && c.colspan == 1);
            if simple_grid {
                let is_spacer = |c: &TableCellIr| {
                    c.blocks.is_empty() && c.fill.is_none() && !c.borders.has_any()
                };
                for idx in 0..n {
                    let cur_w = tbl.col_widths_pt[idx];
                    if !is_single_line_cell(&row.cells[idx], ir) {
                        continue;
                    }
                    let want = (cur_w * 0.12).clamp(18.0, 48.0);
                    if idx + 1 < n
                        && is_spacer(&row.cells[idx + 1])
                        && tbl.col_widths_pt[idx + 1] > 48.0
                    {
                        let borrow = ((tbl.col_widths_pt[idx + 1] - 24.0) * 0.5)
                            .clamp(0.0, want);
                        if borrow > 1.0 {
                            tbl.col_widths_pt[idx] += borrow;
                            tbl.col_widths_pt[idx + 1] -= borrow;
                        }
                    } else if idx > 0
                        && is_spacer(&row.cells[idx - 1])
                        && tbl.col_widths_pt[idx - 1] > 48.0
                    {
                        let borrow = ((tbl.col_widths_pt[idx - 1] - 24.0) * 0.5)
                            .clamp(0.0, want);
                        if borrow > 1.0 {
                            tbl.col_widths_pt[idx - 1] -= borrow;
                            tbl.col_widths_pt[idx] += borrow;
                            row.cells[idx].halign = HAlignIr::Right;
                            if let Some(BlockNode::Paragraph { style, .. }) =
                                row.cells[idx].blocks.first_mut()
                                && matches!(style.align, HAlignIr::Left | HAlignIr::Right)
                            {
                                style.align = HAlignIr::Right;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn extract_solid_bar_fill(blocks: &[BlockNode]) -> Option<String> {
    if blocks.len() != 1 {
        return None;
    }
    let sh = match &blocks[0] {
        BlockNode::ShapeBlock(sh) => sh,
        BlockNode::Paragraph { content, .. } if content.len() == 1 => match &content[0] {
            InlineNode::ShapeInline(sh) => sh,
            _ => return None,
        },
        _ => return None,
    };
    (matches!(sh.kind, ShapeKindIr::Rect)
        && sh.body.is_empty()
        && sh.stroke.is_none()
        && !sh.borders.has_any()
        && sh.height_pt <= 6.5)
        .then(|| sh.fill.clone())
        .flatten()
}

fn is_single_line_cell(cell: &TableCellIr, ir: &DocumentIr) -> bool {
    if cell.blocks.len() != 1 {
        return false;
    }
    match &cell.blocks[0] {
        BlockNode::Paragraph { id, content, .. } => {
            if content.iter().any(|inl| matches!(inl, InlineNode::LineBreak)) {
                return false;
            }
            if !id.is_unset()
                && let Some(pl) = ir.layout(*id).and_then(|l| l.paragraph_lines.as_ref())
            {
                return pl.line_count == 1;
            }
            is_single_short_line_cell(cell)
        }
        BlockNode::Heading { id, content, .. } => {
            if content.iter().any(|inl| matches!(inl, InlineNode::LineBreak)) {
                return false;
            }
            !id.is_unset()
                && ir
                    .layout(*id)
                    .and_then(|l| l.paragraph_lines.as_ref())
                    .is_some_and(|pl| pl.line_count == 1)
        }
        _ => false,
    }
}

fn is_single_short_line_cell(cell: &TableCellIr) -> bool {
    if cell.blocks.len() != 1 {
        return false;
    }
    let BlockNode::Paragraph { content, .. } = &cell.blocks[0] else {
        return false;
    };
    let mut chars = 0usize;
    for inl in content {
        match inl {
            InlineNode::LineBreak => return false,
            InlineNode::Text(r) => chars += r.text.chars().count(),
            InlineNode::HorizontalSpace { .. } => chars += 1,
            _ => return false,
        }
    }
    (1..=24).contains(&chars)
}

/// Lower semantic [`BlockNode::Container`] nodes into Word 1x1 [`TableKindIr::StyledContainer`] tables.
pub fn lower_containers_in_blocks(blocks: &mut [BlockNode], fallback_width_pt: f64) {
    for block in blocks.iter_mut() {
        if let BlockNode::Container(_) = block {
            let BlockNode::Container(c) = std::mem::replace(block, BlockNode::PageBreak) else {
                unreachable!()
            };
            *block = BlockNode::Table(lower_container_to_table(c, fallback_width_pt));
        }
    }
}

fn lower_container_to_table(mut c: ContainerIr, fallback_width_pt: f64) -> TableIr {
    if !c.breakable {
        let len = c.body.len();
        for (idx, child) in c.body.iter_mut().enumerate() {
            if let BlockNode::Paragraph { style, .. } = child {
                style.breakable = false;
                if idx + 1 < len {
                    style.sticky = true;
                }
            }
        }
    }
    if c.sticky
        && let Some(BlockNode::Paragraph { style, .. }) = c.body.last_mut()
    {
        style.sticky = true;
    }
    let width_pt = c.width_pt.unwrap_or(fallback_width_pt).max(36.0);
    TableIr {
        id: c.id,
        kind: TableKindIr::StyledContainer,
        col_widths_pt: vec![width_pt],
        row_heights_pt: vec![0.0],
        header_rows: 0,
        footer_rows: 0,
        rows: vec![TableRowIr {
            height_pt: None,
            is_header: false,
            is_footer: false,
            cells: vec![TableCellIr {
                x: 0,
                y: 0,
                colspan: 1,
                rowspan: 1,
                fill: c.style.fill,
                borders: c.style.borders,
                inset: c.style.inset,
                halign: c.align,
                valign: VAlignIr::Top,
                is_header_cell: false,
                blocks: c.body,
            }],
        }],
        align: c.align,
        left_indent_pt: c.left_indent_pt,
        summary: None,
    }
}
