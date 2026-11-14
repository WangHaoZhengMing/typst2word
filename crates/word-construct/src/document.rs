//! Solved Word Document Representation (`WordDocument`), constructed from an immutable [`DocumentIr`].
//!
//! [`build_word_document`] takes `&DocumentIr` by shared reference (never mutating `DocumentIr`),
//! resolves Word inter-block and list-item vertical spacing across all sections/headers/footers,
//! and pre-computes document-level Word compatibility flags (`has_cjk`, `has_cetz_canvas`).

use std::collections::BTreeMap;

use ir::{
    BlockNode, ContainerIr, ContainerStyleIr, DocumentIr, DocumentSectionIr, FontFaceMetricsIr,
    InlineNode, MetadataIr, NodeId, PageSetupIr, ShapeIr, TableCellIr, TableIr, TableKindIr,
    TableRowIr, VAlignIr, merge_split_styled_containers, visit_block_list_mut,
    visit_page_setup_blocks_mut,
};

use crate::objects::paragraph::compute_line_spacing_dxa_with_metrics;
use crate::objects::shape::{SolvedShapeGeometry, solve_shape_geometry};
use crate::objects::text_run::run_has_cjk;
use crate::pipeline::flow_spacing::resolve_paragraph_style_spacing;

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
    /// Pre-solved `(original_line_spacing_pt, w_line_240)` per paragraph `NodeId`, incorporating
    /// both `font_metrics` and read-only `NodeReferenceLayout::paragraph_lines` while keeping
    /// `DocumentIr` 100% immutable and Word-independent.
    pub solved_line_dxa: BTreeMap<NodeId, (Option<f64>, i64)>,
    /// Pre-solved DrawingML `<wps:bodyPr>`, `<wp:effectExtent>`, and `<w:position>` geometry per shape `NodeId`.
    pub solved_shapes: BTreeMap<NodeId, SolvedShapeGeometry>,
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
    /// with `self.font_metrics` if `id` is unset or a container/table solver overrode `line_spacing_pt`.
    pub fn resolve_paragraph_line_dxa(
        &self,
        id: NodeId,
        content: &[InlineNode],
        line_spacing_pt: Option<f64>,
    ) -> i64 {
        if !id.is_unset()
            && let Some(&(orig_ls, solved_dxa)) = self.solved_line_dxa.get(&id)
            && orig_ls == line_spacing_pt
        {
            return solved_dxa;
        }
        compute_line_spacing_dxa_with_metrics(content, line_spacing_pt, &self.font_metrics, None)
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
    let mut solved_line_dxa: BTreeMap<NodeId, (Option<f64>, i64)> = BTreeMap::new();
    let mut solved_shapes: BTreeMap<NodeId, SolvedShapeGeometry> = BTreeMap::new();

    let mut solve_section_lists = |ps: &mut PageSetupIr, sec_blocks: &mut Vec<BlockNode>| {
        let fallback_w = (ps.width_pt - ps.margin_left_pt - ps.margin_right_pt).max(72.0);
        let mut resolve_blocks = |blk_list: &mut Vec<BlockNode>| {
            promote_standalone_partial_border_inline_boxes(blk_list, ir);
            lower_containers_in_blocks(blk_list, fallback_w);
            merge_split_styled_containers(blk_list);
            for block in blk_list.iter_mut() {
                match block {
                    BlockNode::Paragraph {
                        id, content, style, ..
                    } => {
                        let node_layout = ir.layout(*id);
                        resolve_paragraph_style_spacing(style, node_layout);
                        let para_lines = node_layout.and_then(|l| l.paragraph_lines.as_ref());
                        let w_line = compute_line_spacing_dxa_with_metrics(
                            content,
                            style.line_spacing_pt,
                            &ir.font_metrics,
                            para_lines,
                        );
                        if !id.is_unset() {
                            solved_line_dxa.insert(*id, (style.line_spacing_pt, w_line));
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
                    BlockNode::ShapeBlock(shape) => {
                        if shape.id.is_set() {
                            let geom =
                                solve_shape_geometry(shape, false, 240, None, &ir.font_metrics);
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
        solved_shapes,
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
                    let geom = solve_shape_geometry(
                        shape,
                        true,
                        parent_w_line,
                        ctx,
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
                || entries.iter().any(|e| inlines_have_cjk(&e.title))
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
