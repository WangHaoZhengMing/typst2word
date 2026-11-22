//! Solved Word Document Representation (`WordDocument`), constructed from an immutable [`DocumentIr`].
//!
//! [`build_word_document`] takes `&DocumentIr` by shared reference (never mutating `DocumentIr`),
//! resolves Word inter-block and list-item vertical spacing across all sections/headers/footers,
//! and pre-computes document-level Word compatibility flags (`has_cjk`, `has_cetz_canvas`).

use std::collections::BTreeMap;

use ir::{
    BlockNode, DocumentIr, DocumentSectionIr, EmbeddedFontFaceIr, FontFaceMetricsIr, InlineNode,
    MetadataIr, NodeId, PageSetupIr, ShapeIr, visit_block_list_mut, visit_page_setup_blocks_mut,
};

use crate::objects::paragraph::compute_line_spacing_dxa_with_metrics;
use crate::objects::shape::{
    ShapeLocalBaselinesPt, SolvedShapeGeometry, solve_shape_geometry,
    solve_shape_geometry_with_baselines,
};
use crate::objects::text_run::run_has_cjk;
use crate::pipeline::flow_spacing::{
    PageVerticalBand, resolve_adjacent_paragraph_spacing, resolve_paragraph_style_spacing,
    solve_container_and_figure_flow_spacing,
};
use crate::table_lowering::{
    lower_containers_in_blocks, merge_split_styled_containers,
    promote_standalone_partial_border_inline_boxes, refine_word_tables_in_blocks,
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

/// Unified pre-solved Word layout decisions for a single semantic node (`NodeId`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SolvedNodeLayout {
    /// Pre-solved `w:line` (`1/240` line units) for a `Paragraph` node.
    pub paragraph_line: Option<SolvedParagraphLineDxa>,
    /// Pre-solved `(w_line, is_exact)` for a `CodeBlock` node.
    pub code_line_spacing: Option<(i64, bool)>,
    /// Pre-solved DrawingML `<wps:bodyPr>`, `<wp:effectExtent>`, and `<w:position>` geometry for a `Shape` node.
    pub shape: Option<SolvedShapeGeometry>,
    /// Pre-solved `w:before` (`dxa`) override for blocks following `<w:tbl>` containers/tables.
    pub before_dxa: Option<i64>,
    /// Pre-solved `w:after` (`dxa`) override for `ShapeBlock` / `Figure` wrappers and table-to-table separators.
    pub after_dxa: Option<i64>,
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
    /// Resolved Word OpenType `MATH` font family name (`nameID 1`, e.g. `"NewComputerModernMath"`)
    /// from equation layout, independent of embedding mode.
    pub math_font_family: Option<String>,
    /// Standalone SFNT font faces carried from `DocumentIr` for OOXML `.odttf` packaging.
    pub embedded_fonts: Vec<EmbeddedFontFaceIr>,
    /// Unified per-`NodeId` solved Word layout side-table (`paragraph_line`, `code_line_spacing`,
    /// `shape`, `before_dxa`, `after_dxa`), keeping `DocumentIr` 100% immutable.
    pub solved_nodes: BTreeMap<NodeId, SolvedNodeLayout>,
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

    /// Look up the [`SolvedNodeLayout`] entry for `id` (`None` when `id` is unset or unindexed).
    #[inline]
    pub fn node(&self, id: NodeId) -> Option<&SolvedNodeLayout> {
        (!id.is_unset()).then(|| self.solved_nodes.get(&id)).flatten()
    }

    /// Check whether `id` has a pre-solved paragraph `w:line` entry.
    #[inline]
    pub fn has_solved_paragraph_line(&self, id: NodeId) -> bool {
        self.node(id).is_some_and(|n| n.paragraph_line.is_some())
    }

    /// Look up any pre-solved `w:before` (`dxa`) override for `id`.
    #[inline]
    pub fn solved_before_dxa(&self, id: NodeId) -> Option<i64> {
        self.node(id).and_then(|n| n.before_dxa)
    }

    /// Look up any pre-solved `w:after` (`dxa`) override for `id`.
    #[inline]
    pub fn solved_after_dxa(&self, id: NodeId) -> Option<i64> {
        self.node(id).and_then(|n| n.after_dxa)
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
        if let Some(solved) = self.node(id).and_then(|n| n.paragraph_line)
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
        if let Some(solved) = self.node(id).and_then(|n| n.code_line_spacing) {
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
        if let Some(solved) = self.node(shape.id).and_then(|n| n.shape) {
            return solved;
        }
        solve_shape_geometry(shape, is_inline, 240, None, &self.font_metrics)
    }

    /// Merge multiple single-page [`WordDocument`]s into a multi-section batch document,
    /// shifting `NodeId`s per section so unified `solved_nodes` entries never collide.
    pub fn assemble_batch_sections(docs: &[&WordDocument]) -> Result<Self, String> {
        let first_doc = docs
            .first()
            .copied()
            .ok_or_else(|| "cannot assemble empty WordDocument batch".to_string())?;

        let mut merged_font_metrics = first_doc.font_metrics.clone();
        let mut merged_solved_nodes = first_doc.solved_nodes.clone();
        let mut first_blocks = first_doc.blocks.clone();

        let mut max_first_id = merged_solved_nodes.keys().map(|k| k.0).max().unwrap_or(0);
        visit_block_list_mut(&mut first_blocks, &mut |blocks| {
            for block in blocks.iter() {
                max_first_id = max_first_id.max(block.node_id().0);
            }
        });
        let mut next_id_offset = max_first_id.saturating_add(1000);

        let mut sections = Vec::with_capacity(docs.len().saturating_sub(1));
        for doc in &docs[1..] {
            for (family, metrics) in &doc.font_metrics {
                merged_font_metrics
                    .entry(family.clone())
                    .or_insert_with(|| metrics.clone());
            }

            let mut sec_blocks = doc.blocks.clone();
            let mut doc_max_id = doc.solved_nodes.keys().map(|k| k.0).max().unwrap_or(0);
            visit_block_list_mut(&mut sec_blocks, &mut |blocks| {
                for block in blocks.iter_mut() {
                    let old_id = block.node_id();
                    if old_id.is_set() {
                        doc_max_id = doc_max_id.max(old_id.0);
                        let new_id = NodeId(old_id.0.saturating_add(next_id_offset));
                        block.set_node_id(new_id);
                        if let Some(&solved) = doc.solved_nodes.get(&old_id) {
                            merged_solved_nodes.insert(new_id, solved);
                        }
                    }
                    match block {
                        BlockNode::Paragraph { content, .. }
                        | BlockNode::Heading { content, .. }
                        | BlockNode::Title { content, .. } => {
                            shift_inline_shape_ids(
                                content,
                                next_id_offset,
                                &mut doc_max_id,
                                &doc.solved_nodes,
                                &mut merged_solved_nodes,
                            );
                        }
                        _ => {}
                    }
                }
            });
            next_id_offset = next_id_offset.saturating_add(doc_max_id).saturating_add(1000);

            sections.push(DocumentSectionIr {
                page_setup: doc.page_setup.clone(),
                blocks: sec_blocks,
            });
        }

        let has_cetz_canvas = docs.iter().any(|d| d.has_cetz_canvas);
        let math_font_family = docs.iter().find_map(|d| d.math_font_family.clone());

        Ok(Self {
            sections,
            metadata: MetadataIr::default(),
            page_setup: first_doc.page_setup.clone(),
            blocks: first_blocks,
            font_metrics: merged_font_metrics,
            math_font_family,
            embedded_fonts: first_doc.embedded_fonts.clone(),
            solved_nodes: merged_solved_nodes,
            has_cjk: first_doc.has_cjk,
            has_cetz_canvas,
        })
    }
}

fn shift_inline_shape_ids(
    inlines: &mut [InlineNode],
    offset: u32,
    doc_max_id: &mut u32,
    src_nodes: &BTreeMap<NodeId, SolvedNodeLayout>,
    dst_nodes: &mut BTreeMap<NodeId, SolvedNodeLayout>,
) {
    for inl in inlines {
        match inl {
            InlineNode::ShapeInline(sh) => {
                if sh.id.is_set() {
                    let old_id = sh.id;
                    *doc_max_id = (*doc_max_id).max(old_id.0);
                    let new_id = NodeId(old_id.0.saturating_add(offset));
                    sh.id = new_id;
                    if let Some(&solved) = src_nodes.get(&old_id) {
                        dst_nodes.insert(new_id, solved);
                    }
                }
            }
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. }
            | InlineNode::Field { content, .. } => {
                shift_inline_shape_ids(content, offset, doc_max_id, src_nodes, dst_nodes);
            }
            _ => {}
        }
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
    let mut solved_nodes: BTreeMap<NodeId, SolvedNodeLayout> = BTreeMap::new();

    let mut solve_section_lists = |ps: &mut PageSetupIr, sec_blocks: &mut Vec<BlockNode>| {
        let fallback_w = (ps.width_pt - ps.margin_left_pt - ps.margin_right_pt).max(72.0);
        let page_band = PageVerticalBand::from_page_setup(ps);
        let mut resolve_blocks = |blk_list: &mut Vec<BlockNode>| {
            promote_standalone_partial_border_inline_boxes(blk_list, ir);
            solve_container_and_figure_flow_spacing(blk_list, ir, &mut solved_nodes);
            lower_containers_in_blocks(blk_list, fallback_w);
            merge_split_styled_containers(blk_list);
            refine_word_tables_in_blocks(blk_list, ir, fallback_w);
            for block in blk_list.iter_mut() {
                match block {
                    BlockNode::Paragraph {
                        id, content, style, ..
                    } => {
                        // In Typst, `K >= 1` trailing `linebreak()`s at the end of a `ParElem`
                        // produce `K - 1` blank lines because the paragraph boundary itself ends
                        // the final line; in Word, `</w:p>` already terminates a line, so every
                        // `<w:br/>` before `</w:p>` adds a full blank line.
                        if matches!(content.last(), Some(InlineNode::LineBreak)) {
                            content.pop();
                        }
                        let node_layout = ir.layout(*id);
                        resolve_paragraph_style_spacing(style, node_layout, page_band);
                        let para_lines = node_layout.and_then(|l| l.paragraph_lines.as_ref());
                        let is_multiline = para_lines.is_some_and(|p| p.line_count >= 2);
                        let w_line = compute_line_spacing_dxa_with_metrics(
                            content,
                            style.line_spacing_pt,
                            &ir.font_metrics,
                            para_lines,
                        );
                        if !id.is_unset() {
                            solved_nodes.entry(*id).or_default().paragraph_line =
                                Some(SolvedParagraphLineDxa {
                                    orig_line_spacing_pt: style.line_spacing_pt,
                                    line_dxa: w_line,
                                    is_multiline,
                                });
                        }
                        solve_inline_shapes_recursive(content, w_line, ir, &mut solved_nodes);
                    }
                    BlockNode::Heading { content, .. } | BlockNode::Title { content, .. } => {
                        let w_line = compute_line_spacing_dxa_with_metrics(
                            content,
                            None,
                            &ir.font_metrics,
                            None,
                        );
                        solve_inline_shapes_recursive(content, w_line, ir, &mut solved_nodes);
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
                                solved_nodes.entry(*id).or_default().code_line_spacing =
                                    Some((exact_dxa, true));
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
                                solved_nodes.entry(*id).or_default().code_line_spacing =
                                    Some((w_line, false));
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
                            solved_nodes.entry(shape.id).or_default().shape = Some(geom);
                        }
                    }
                    _ => {}
                }
            }
            resolve_adjacent_paragraph_spacing(blk_list);
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
        math_font_family: ir.math_font_family.clone(),
        embedded_fonts: ir.embedded_fonts.clone(),
        solved_nodes,
        has_cjk,
        has_cetz_canvas,
    }
}

fn solve_inline_shapes_recursive(
    inlines: &[InlineNode],
    parent_w_line: i64,
    ir: &DocumentIr,
    out: &mut BTreeMap<NodeId, SolvedNodeLayout>,
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
                    out.entry(shape.id).or_default().shape = Some(geom);
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
