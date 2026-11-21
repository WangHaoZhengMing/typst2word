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
    Figure {
        starts_with_table: bool,
        ends_with_table: bool,
    },
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
    pub fn classify(block: &BlockNode) -> Self {
        match block {
            BlockNode::Title { .. } => Self::Title,
            BlockNode::Heading { .. } => Self::Heading,
            BlockNode::Paragraph { .. } => Self::Paragraph,
            BlockNode::List { .. } => Self::BulletList,
            BlockNode::Enum { .. } => Self::DecimalEnum,
            BlockNode::Terms { .. } => Self::TermsList,
            BlockNode::Container(_) | BlockNode::Table(_) => Self::Table,
            BlockNode::Quote { .. } => Self::Quote,
            BlockNode::CodeBlock { fill, stroke, .. } => Self::CodeBlock {
                is_table_backed: fill.is_some() || stroke.is_some(),
            },
            BlockNode::Figure {
                body,
                caption,
                caption_pos,
                ..
            } => {
                let has_top_caption =
                    caption.is_some() && *caption_pos == ir::CaptionPositionIr::Top;
                let has_bottom_caption =
                    caption.is_some() && *caption_pos == ir::CaptionPositionIr::Bottom;
                let starts_with_table = !has_top_caption
                    && body
                        .first()
                        .is_some_and(|b| Self::classify(b).starts_with_table());
                let ends_with_table = !has_bottom_caption
                    && body
                        .last()
                        .is_some_and(|b| Self::classify(b).ends_with_table());
                Self::Figure {
                    starts_with_table,
                    ends_with_table,
                }
            }
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

    /// Whether this Word block object starts with a `<w:tbl>` element.
    #[inline]
    pub const fn starts_with_table(self) -> bool {
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
                | Self::Figure {
                    starts_with_table: true,
                    ..
                }
        )
    }

    /// Whether this Word block object ends with a `<w:tbl>` element.
    #[inline]
    pub const fn ends_with_table(self) -> bool {
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
                | Self::Figure {
                    ends_with_table: true,
                    ..
                }
        )
    }

    /// Whether this Word block object is emitted as a top-level `<w:tbl>` (requiring a minimal
    /// separator `<w:p>` if immediately followed by another `<w:tbl>`-backed block so Word
    /// does not merge the two tables).
    #[inline]
    pub const fn is_table_backed(self) -> bool {
        self.starts_with_table() || self.ends_with_table()
    }
}

/// Pre-solved neighbor relationship context for `blocks[idx]` within a sequential block list,
/// centralizing inter-block Word rules so `docx-writer` remains a pure XML serializer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockSequenceNeighborContext {
    /// Suppress `w:before` at the start of a section/cell/container or immediately after an
    /// explicit `PageBreak` / `ColumnBreak`.
    pub collapse_before: bool,
    /// Use compact post-heading `w:after` when followed directly by a body `Paragraph`
    /// (whose own line-box internal top leading supplies the rest of Typst's gap).
    pub heading_compact_after: bool,
    /// Propagate `<w:keepNext/>` onto a `Figure` when followed by a `sticky` paragraph.
    pub figure_keep_with_next: bool,
    /// Optional `w:after` (`dxa`) for a `2pt` (`40 dxa`) pre-`Figure` spacer paragraph
    /// when following a `<w:tbl>` container/table.
    pub figure_pre_spacer_after_dxa: Option<i64>,
    /// Solved `w:after` (`dxa`) for a `Figure` wrapper/caption paragraph.
    pub figure_after_dxa: i64,
    /// Solved `w:before` (`dxa`) for a `BlockNode::EquationBlock`.
    pub equation_before_dxa: i64,
    /// Solved `w:after` (`dxa`) for a `BlockNode::EquationBlock`.
    pub equation_after_dxa: i64,
    /// Whether an `Enum` immediately follows a `<w:tbl>`-backed block (which has `0pt` bottom margin).
    pub enum_after_table_block: bool,
    /// Optional `w:after` (`dxa`) for a minimal `2pt` (`40 dxa`) separator paragraph emitted
    /// between two adjacent `<w:tbl>`-backed blocks so Word does not merge them into one table.
    pub table_to_table_separator_after_dxa: Option<i64>,
}

impl BlockSequenceNeighborContext {
    /// Solve neighbor rules for `blocks[idx]` against `doc.solved_nodes`.
    pub fn for_index(blocks: &[BlockNode], idx: usize, doc: &crate::WordDocument) -> Self {
        let block = &blocks[idx];
        let prev = idx.checked_sub(1).and_then(|p| blocks.get(p));
        let next = blocks.get(idx + 1);

        let collapse_before = prev.is_none_or(|b| {
            matches!(b, BlockNode::PageBreak | BlockNode::ColumnBreak)
        });
        let heading_compact_after = matches!(next, Some(BlockNode::Paragraph { .. }));
        let figure_keep_with_next = matches!(
            next,
            Some(BlockNode::Paragraph { style, .. }) if style.sticky
        );

        let id = block.node_id();
        let figure_pre_spacer_after_dxa = if !collapse_before
            && matches!(block, BlockNode::Figure { .. })
            && let Some(before_dxa) = doc.solved_before_dxa(id)
            && before_dxa > 20
        {
            Some((before_dxa - 40).max(0))
        } else {
            None
        };
        let figure_after_dxa = doc.solved_after_dxa(id).unwrap_or(60);

        let (equation_before_dxa, equation_after_dxa) =
            if let BlockNode::EquationBlock(eq) = block {
                let (def_before, def_after) =
                    equation::EquationBlockConstruct::default_flow_spacing_dxa(eq);
                let before = if collapse_before || eq.numbering_text.is_some() {
                    0
                } else {
                    doc.solved_before_dxa(id).unwrap_or(def_before)
                };
                let after = if next.is_none() || eq.numbering_text.is_some() {
                    0
                } else {
                    doc.solved_after_dxa(id).unwrap_or(def_after)
                };
                (before, after)
            } else {
                (0, 0)
            };

        let enum_after_table_block =
            prev.is_some_and(|b| WordBlockObjectKind::classify(b).ends_with_table());

        let cur_ends_with_table = WordBlockObjectKind::classify(block).ends_with_table();
        let next_starts_with_table =
            next.is_some_and(|b| WordBlockObjectKind::classify(b).starts_with_table());
        let table_to_table_separator_after_dxa = if cur_ends_with_table && next_starts_with_table {
            Some(doc.solved_after_dxa(id).unwrap_or(40))
        } else {
            None
        };

        Self {
            collapse_before,
            heading_compact_after,
            figure_keep_with_next,
            figure_pre_spacer_after_dxa,
            figure_after_dxa,
            equation_before_dxa,
            equation_after_dxa,
            enum_after_table_block,
            table_to_table_separator_after_dxa,
        }
    }
}
