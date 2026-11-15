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
        border_sz_eighths_pt: u32,
        left_indent_dxa: usize,
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
            let border_sz_eighths_pt = stroke
                .map(|s| ((s.thickness_pt * 8.0).round() as u32).clamp(2, 96))
                .unwrap_or(if fill.is_some() { 0 } else { 6 });
            Self::ShadedBoxTable {
                bg_hex: fill.unwrap_or("F6F8FA"),
                border_color_hex: stroke.map(|s| s.color.as_str()).or(fill).unwrap_or("D0D7DE"),
                border_sz_eighths_pt,
                left_indent_dxa,
            }
        }
    }
}
