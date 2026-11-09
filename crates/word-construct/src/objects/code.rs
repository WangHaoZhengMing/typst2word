//! Word CodeBlock Construct (`raw(block: true)`).
//!
//! Models the exhaustive [`CodeBlockConstruct`] enum (`UnboxedLines` vs `ShadedBoxTable`).

use ir::StrokeIr;

/// Exhaustive Word layout representation for `BlockNode::CodeBlock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeBlockConstruct<'a> {
    /// Standard unboxed Typst `raw(block: true)` rendered as syntax-colored `<w:p>` lines.
    UnboxedLines { left_indent_dxa: usize },
    /// Boxed/shaded code block rendered as a 1-cell full-width `<w:tbl>`.
    ShadedBoxTable {
        bg_hex: &'a str,
        border_color_hex: &'a str,
    },
}

impl<'a> CodeBlockConstruct<'a> {
    pub fn solve(
        fill: Option<&'a str>,
        stroke: Option<&'a StrokeIr>,
        left_indent_dxa: usize,
    ) -> Self {
        if fill.is_none() && stroke.is_none() {
            Self::UnboxedLines { left_indent_dxa }
        } else {
            Self::ShadedBoxTable {
                bg_hex: fill.unwrap_or("F6F8FA"),
                border_color_hex: stroke.map(|s| s.color.as_str()).unwrap_or("D0D7DE"),
            }
        }
    }
}
