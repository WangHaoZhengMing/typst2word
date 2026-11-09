//! Per-object Word Construct Builders (`src/construct/`).
//!
//! Each Word layout object family lives in its own module and transforms pipeline-annotated
//! [`crate::ir`] nodes into solved Word constructs (using `dxa`, `w:line`, `half_pt`, `emu`,
//! and exhaustive domain `enum`s), encapsulating all Microsoft Word layout math before
//! serialization by the DOCX writer.

pub mod cetz;
pub mod code;
pub mod equation;
pub mod figure;
pub mod heading;
pub mod list;
pub mod paragraph;
pub mod shape;
pub mod table;
pub mod text_run;

use ir::BlockNode;

/// Exhaustive enumeration of Word block-level object families constructed from [`BlockNode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordBlockObjectKind {
    Title,
    Heading,
    Paragraph,
    BulletList,
    DecimalEnum,
    TermsList,
    Table,
    Quote,
    CodeBlock { is_table_backed: bool },
    Figure,
    EquationBlock { is_table_backed: bool },
    ShapeBlock,
    CetzCanvas,
    ImageBlock,
    Outline,
    Bibliography,
    HorizontalRule,
    PageBreak,
    ColumnBreak,
    ScopedColumns,
    UnsupportedBlock,
}

impl WordBlockObjectKind {
    /// Classify a [`BlockNode`] into its exhaustive [`WordBlockObjectKind`].
    #[inline]
    pub const fn classify(block: &BlockNode) -> Self {
        match block {
            BlockNode::Title { .. } => Self::Title,
            BlockNode::Heading { .. } => Self::Heading,
            BlockNode::Paragraph { .. } => Self::Paragraph,
            BlockNode::List { .. } => Self::BulletList,
            BlockNode::Enum { .. } => Self::DecimalEnum,
            BlockNode::Terms { .. } => Self::TermsList,
            BlockNode::Table(_) => Self::Table,
            BlockNode::Quote { .. } => Self::Quote,
            BlockNode::CodeBlock { fill, stroke, .. } => Self::CodeBlock {
                is_table_backed: fill.is_some() || stroke.is_some(),
            },
            BlockNode::Figure { .. } => Self::Figure,
            BlockNode::EquationBlock(eq) => Self::EquationBlock {
                is_table_backed: eq.numbering_text.is_some(),
            },
            BlockNode::ShapeBlock(_) => Self::ShapeBlock,
            BlockNode::CetzCanvas(_) => Self::CetzCanvas,
            BlockNode::ImageBlock(_) => Self::ImageBlock,
            BlockNode::Outline { .. } => Self::Outline,
            BlockNode::Bibliography { .. } => Self::Bibliography,
            BlockNode::HorizontalRule { .. } => Self::HorizontalRule,
            BlockNode::PageBreak => Self::PageBreak,
            BlockNode::ColumnBreak => Self::ColumnBreak,
            BlockNode::Columns { .. } => Self::ScopedColumns,
            BlockNode::UnsupportedBlock(_) => Self::UnsupportedBlock,
        }
    }

    /// Whether this Word block object is emitted as a top-level `<w:tbl>` (requiring a minimal
    /// separator `<w:p>` if immediately followed by another `<w:tbl>`-backed block so Word
    /// does not merge the two tables).
    #[inline]
    pub const fn is_table_backed(self) -> bool {
        matches!(
            self,
            Self::Table
                | Self::UnsupportedBlock
                | Self::EquationBlock {
                    is_table_backed: true
                }
                | Self::CodeBlock {
                    is_table_backed: true
                }
        )
    }
}
