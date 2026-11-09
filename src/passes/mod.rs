//! Sequential IR pre-processing pipeline (`src/passes/`).
//!
//! Runs a deterministic, single-responsibility sequence of passes over `DocumentIr`
//! before constructing Word layout objects:
//! 1. [`PipelinePassKind::FontRecognition`]: Font family normalization and [`FontFamilyClassIr`] classification.
//! 2. [`PipelinePassKind::InlineCoalescing`]: Coalescing adjacent compatible [`TextRunIr`] runs.
//! 3. [`PipelinePassKind::LanguageAndScriptRecognition`]: CJK / East Asian symbol / Latin script classification ([`TextScriptClassIr`]).
//! 4. [`PipelinePassKind::ContainerAndPageBreakMerge`]: Cross-page styled container merging and redundant `PageBreak` cleanup.
//! 5. [`PipelinePassKind::InterBlockFlowAndSpacing`]: Resolving `ParagraphPdfMetricsIr` and `InterBlockFlowIr` into paragraph spacing.

pub mod container_merge;
pub mod flow_spacing;
pub mod font_recognize;
pub mod inline_coalesce;
pub mod lang_script;

use crate::ir::{BlockNode, DocumentIr, InlineNode};

/// Exhaustive enum of sequential pre-processing passes executed over `DocumentIr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelinePassKind {
    /// Pass 1: Normalize font family names (`STSong` -> `Songti SC`, code `DejaVu Sans Mono` -> `Consolas`)
    /// and classify each run's `FontFamilyClassIr`.
    FontRecognition,
    /// Pass 2: Coalesce adjacent compatible `TextRunIr`s across all inline sequences.
    InlineCoalescing,
    /// Pass 3: Classify script/language characteristics (`TextScriptClassIr` and math `is_cjk`).
    LanguageAndScriptRecognition,
    /// Pass 4: Merge cross-page split styled containers (`TableKindIr::StyledContainer`) and clean redundant `PageBreak`s.
    ContainerAndPageBreakMerge,
    /// Pass 5: Resolve Typst + PDF physical vertical spacing (`ParagraphPdfMetricsIr` and `InterBlockFlowIr`).
    InterBlockFlowAndSpacing,
}

impl PipelinePassKind {
    /// Ordered pipeline pass schedule.
    pub const ALL: [Self; 5] = [
        Self::FontRecognition,
        Self::InlineCoalescing,
        Self::LanguageAndScriptRecognition,
        Self::ContainerAndPageBreakMerge,
        Self::InterBlockFlowAndSpacing,
    ];

    /// Execute this single pass on `doc`.
    pub fn run(self, doc: &mut DocumentIr) {
        match self {
            Self::FontRecognition => font_recognize::run_font_recognition_pass(doc),
            Self::InlineCoalescing => inline_coalesce::run_inline_coalescing_pass(doc),
            Self::LanguageAndScriptRecognition => lang_script::run_language_and_script_pass(doc),
            Self::ContainerAndPageBreakMerge => container_merge::run_container_merge_pass(doc),
            Self::InterBlockFlowAndSpacing => flow_spacing::run_flow_spacing_pass(doc),
        }
    }
}

/// Run all sequential pre-processing passes on `doc` in canonical order.
pub fn run_pipeline_passes(doc: &mut DocumentIr) {
    for pass in PipelinePassKind::ALL {
        pass.run(doc);
    }
}

/// Recursively visit all `Vec<BlockNode>` sequences in a `DocumentIr` (bottom-up: children first, then parent).
pub(crate) fn visit_document_block_lists_mut<F>(doc: &mut DocumentIr, f: &mut F)
where
    F: FnMut(&mut Vec<BlockNode>),
{
    visit_block_list_mut(&mut doc.page_setup.header, f);
    visit_block_list_mut(&mut doc.page_setup.footer, f);
    visit_block_list_mut(&mut doc.blocks, f);
}

pub(crate) fn visit_block_list_mut<F>(blocks: &mut Vec<BlockNode>, f: &mut F)
where
    F: FnMut(&mut Vec<BlockNode>),
{
    for block in blocks.iter_mut() {
        visit_block_children_lists_mut(block, f);
    }
    f(blocks);
}

fn visit_block_children_lists_mut<F>(block: &mut BlockNode, f: &mut F)
where
    F: FnMut(&mut Vec<BlockNode>),
{
    match block {
        BlockNode::Title { content, .. }
        | BlockNode::Heading { content, .. }
        | BlockNode::Paragraph { content, .. } => {
            visit_inlines_block_lists_mut(content, f);
        }
        BlockNode::List { items, .. } => {
            for item in items {
                visit_block_list_mut(&mut item.blocks, f);
            }
        }
        BlockNode::Enum { items, .. } => {
            for item in items {
                visit_block_list_mut(&mut item.blocks, f);
            }
        }
        BlockNode::Terms { items, .. } => {
            for item in items {
                visit_inlines_block_lists_mut(&mut item.term, f);
                visit_block_list_mut(&mut item.description, f);
            }
        }
        BlockNode::Table(table) => {
            for row in &mut table.rows {
                for cell in &mut row.cells {
                    visit_block_list_mut(&mut cell.blocks, f);
                }
            }
        }
        BlockNode::Quote { body, attribution } => {
            visit_block_list_mut(body, f);
            if let Some(attr) = attribution {
                visit_inlines_block_lists_mut(attr, f);
            }
        }
        BlockNode::Figure { body, caption, .. } => {
            visit_block_list_mut(body, f);
            if let Some(cap) = caption {
                visit_inlines_block_lists_mut(cap, f);
            }
        }
        BlockNode::ShapeBlock(shape) => {
            visit_block_list_mut(&mut shape.body, f);
        }
        BlockNode::CetzCanvas(canvas) => {
            for lbl in &mut canvas.labels {
                visit_inlines_block_lists_mut(&mut lbl.content, f);
            }
        }
        BlockNode::Outline { title, entries } => {
            if let Some(t) = title {
                visit_inlines_block_lists_mut(t, f);
            }
            for entry in entries {
                visit_inlines_block_lists_mut(&mut entry.title, f);
            }
        }
        BlockNode::Bibliography { title, entries } => {
            if let Some(t) = title {
                visit_inlines_block_lists_mut(t, f);
            }
            for entry in entries {
                visit_inlines_block_lists_mut(&mut entry.content, f);
            }
        }
        BlockNode::Columns { body, .. } => {
            visit_block_list_mut(body, f);
        }
        BlockNode::CodeBlock { .. }
        | BlockNode::EquationBlock(_)
        | BlockNode::ImageBlock(_)
        | BlockNode::HorizontalRule { .. }
        | BlockNode::PageBreak
        | BlockNode::ColumnBreak
        | BlockNode::UnsupportedBlock(_) => {}
    }
}

fn visit_inlines_block_lists_mut<F>(inlines: &mut [InlineNode], f: &mut F)
where
    F: FnMut(&mut Vec<BlockNode>),
{
    for inline in inlines {
        match inline {
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. } => {
                visit_inlines_block_lists_mut(content, f);
            }
            InlineNode::Footnote { content, .. } => {
                visit_block_list_mut(content, f);
            }
            InlineNode::ShapeInline(shape) => {
                visit_block_list_mut(&mut shape.body, f);
            }
            InlineNode::CetzInline(canvas) => {
                for lbl in &mut canvas.labels {
                    visit_inlines_block_lists_mut(&mut lbl.content, f);
                }
            }
            InlineNode::Text(_)
            | InlineNode::EquationInline(_)
            | InlineNode::ImageInline(_)
            | InlineNode::LineBreak
            | InlineNode::Tab
            | InlineNode::HorizontalSpace { .. }
            | InlineNode::UnsupportedInline(_) => {}
        }
    }
}

/// Recursively visit all `Vec<InlineNode>` sequences in a `DocumentIr`.
pub(crate) fn visit_document_inline_lists_mut<F>(doc: &mut DocumentIr, f: &mut F)
where
    F: FnMut(&mut Vec<InlineNode>),
{
    visit_document_block_lists_mut(doc, &mut |blocks| {
        for block in blocks.iter_mut() {
            match block {
                BlockNode::Title { content, .. }
                | BlockNode::Heading { content, .. }
                | BlockNode::Paragraph { content, .. } => {
                    visit_inline_list_recursive_mut(content, f);
                }
                BlockNode::Terms { items, .. } => {
                    for item in items {
                        visit_inline_list_recursive_mut(&mut item.term, f);
                    }
                }
                BlockNode::Quote { attribution, .. } => {
                    if let Some(attr) = attribution {
                        visit_inline_list_recursive_mut(attr, f);
                    }
                }
                BlockNode::Figure { caption, .. } => {
                    if let Some(cap) = caption {
                        visit_inline_list_recursive_mut(cap, f);
                    }
                }
                BlockNode::Outline { title, entries } => {
                    if let Some(t) = title {
                        visit_inline_list_recursive_mut(t, f);
                    }
                    for entry in entries {
                        visit_inline_list_recursive_mut(&mut entry.title, f);
                    }
                }
                BlockNode::Bibliography { title, entries } => {
                    if let Some(t) = title {
                        visit_inline_list_recursive_mut(t, f);
                    }
                    for entry in entries {
                        visit_inline_list_recursive_mut(&mut entry.content, f);
                    }
                }
                BlockNode::CetzCanvas(canvas) => {
                    for lbl in &mut canvas.labels {
                        visit_inline_list_recursive_mut(&mut lbl.content, f);
                    }
                }
                _ => {}
            }
        }
    });
}

fn visit_inline_list_recursive_mut<F>(inlines: &mut Vec<InlineNode>, f: &mut F)
where
    F: FnMut(&mut Vec<InlineNode>),
{
    for inline in inlines.iter_mut() {
        match inline {
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. } => {
                visit_inline_list_recursive_mut(content, f);
            }
            InlineNode::CetzInline(canvas) => {
                for lbl in &mut canvas.labels {
                    visit_inline_list_recursive_mut(&mut lbl.content, f);
                }
            }
            _ => {}
        }
    }
    f(inlines);
}
