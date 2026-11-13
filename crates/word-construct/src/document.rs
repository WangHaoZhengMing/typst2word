//! Solved Word Document Representation (`WordDocument`), constructed from an immutable [`DocumentIr`].
//!
//! [`build_word_document`] takes `&DocumentIr` by shared reference (never mutating `DocumentIr`),
//! resolves Word inter-block and list-item vertical spacing across all sections/headers/footers,
//! and pre-computes document-level Word compatibility flags (`has_cjk`, `has_cetz_canvas`).

use ir::{
    BlockNode, ContainerIr, DocumentIr, DocumentSectionIr, InlineNode, MetadataIr, PageSetupIr,
    TableCellIr, TableIr, TableKindIr, TableRowIr, VAlignIr, merge_split_styled_containers,
    visit_block_list_mut, visit_page_setup_blocks_mut,
};

use crate::objects::text_run::run_has_cjk;
use crate::pipeline::flow_spacing::resolve_paragraph_style_spacing;

/// Solved Word document ready for OpenXML (`.docx`) serialization by `docx-writer`.
#[derive(Debug, Clone, Default)]
pub struct WordDocument {
    pub sections: Vec<DocumentSectionIr>,
    pub metadata: MetadataIr,
    pub page_setup: PageSetupIr,
    pub blocks: Vec<BlockNode>,
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

    let solve_section_lists = |ps: &mut PageSetupIr, sec_blocks: &mut Vec<BlockNode>| {
        let fallback_w = (ps.width_pt - ps.margin_left_pt - ps.margin_right_pt).max(72.0);
        let mut resolve_blocks = |blk_list: &mut Vec<BlockNode>| {
            lower_containers_in_blocks(blk_list, fallback_w);
            merge_split_styled_containers(blk_list);
            for block in blk_list.iter_mut() {
                if let BlockNode::Paragraph { id, style, .. } = block {
                    resolve_paragraph_style_spacing(style, ir.layout(*id));
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
        has_cjk,
        has_cetz_canvas,
    }
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
