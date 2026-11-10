//! Solved Word Document Representation (`WordDocument`), constructed from an immutable [`DocumentIr`].
//!
//! [`build_word_document`] takes `&DocumentIr` by shared reference (never mutating `DocumentIr`),
//! resolves Word inter-block and list-item vertical spacing across all sections/headers/footers,
//! and pre-computes document-level Word compatibility flags (`has_cjk`, `has_cetz_canvas`).

use ir::{
    BlockNode, DocumentIr, DocumentSectionIr, InlineNode, MetadataIr, PageSetupIr,
    visit_block_list_mut,
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

    let mut resolve_blocks = |blk_list: &mut Vec<BlockNode>| {
        for block in blk_list.iter_mut() {
            if let BlockNode::Paragraph { style, .. } = block {
                resolve_paragraph_style_spacing(style);
            }
        }
    };

    visit_block_list_mut(&mut page_setup.header, &mut resolve_blocks);
    visit_block_list_mut(&mut page_setup.footer, &mut resolve_blocks);
    visit_block_list_mut(&mut blocks, &mut resolve_blocks);
    for section in &mut sections {
        visit_block_list_mut(&mut section.page_setup.header, &mut resolve_blocks);
        visit_block_list_mut(&mut section.page_setup.footer, &mut resolve_blocks);
        visit_block_list_mut(&mut section.blocks, &mut resolve_blocks);
    }

    let has_cjk = blocks_have_cjk(&blocks)
        || blocks_have_cjk(&page_setup.header)
        || blocks_have_cjk(&page_setup.footer)
        || sections.iter().any(|s| {
            blocks_have_cjk(&s.blocks)
                || blocks_have_cjk(&s.page_setup.header)
                || blocks_have_cjk(&s.page_setup.footer)
        });

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
        BlockNode::Table(t) => t
            .rows
            .iter()
            .any(|r| r.cells.iter().any(|c| blocks_have_cjk(&c.blocks))),
        BlockNode::Quote { body, attribution } => {
            blocks_have_cjk(body) || attribution.as_deref().is_some_and(inlines_have_cjk)
        }
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
