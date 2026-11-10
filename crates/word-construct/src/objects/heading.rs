//! Word Heading Construct (`Title` and `Heading1`..`Heading6`).
//!
//! Solves heading style ID, `w:before`, `w:after`, `w:line`, alignment, and optional
//! bold numbering prefix run.

use crate::objects::paragraph::compute_line_spacing_dxa;
use crate::units::pt_to_dxa;
use ir::{HAlignIr, InlineNode, TextRunIr};

/// Solved Word construct for a `Heading` (`Heading1`..`Heading6`) paragraph.
#[derive(Debug, Clone)]
pub struct HeadingConstruct {
    pub style_id: String,
    pub before_dxa: i64,
    pub after_dxa: i64,
    pub line_dxa: i64,
    pub align: HAlignIr,
    pub prefix_run: Option<TextRunIr>,
}

impl HeadingConstruct {
    pub fn solve(
        level: usize,
        numbering: Option<&str>,
        content: &[InlineNode],
        align: HAlignIr,
        collapse_before: bool,
        compact_after: bool,
    ) -> Self {
        let style_id = format!("Heading{}", level.clamp(1, 6));
        let default_sz = match level {
            1 => 15.4,
            2 => 13.2,
            _ => 11.0,
        };
        let sz_pt = content
            .iter()
            .find_map(|inl| {
                if let InlineNode::Text(r) = inl {
                    r.font_size_pt
                } else {
                    None
                }
            })
            .unwrap_or(default_sz);

        let (before_dxa, after_dxa, line_dxa) = if level >= 2 && sz_pt <= 13.0 {
            let line_dxa = compute_line_spacing_dxa(content, Some(0.65 * sz_pt));
            let before_dxa = if collapse_before {
                0
            } else {
                pt_to_dxa(sz_pt * 0.567)
            };
            let after_dxa = if compact_after {
                20
            } else {
                pt_to_dxa(sz_pt * 0.483)
            };
            (before_dxa, after_dxa, line_dxa)
        } else {
            let line_dxa = compute_line_spacing_dxa(content, None);
            let before_dxa = if collapse_before {
                0
            } else if level == 1 {
                if line_dxa < 200 { 224 } else { 240 }
            } else {
                160
            };
            let after_dxa = if compact_after {
                if line_dxa < 200 { 0 } else { 16 }
            } else {
                80
            };
            (before_dxa, after_dxa, line_dxa)
        };

        let prefix_run = numbering.map(|num_prefix| {
            let sep = if num_prefix.ends_with('、')
                || num_prefix.ends_with('．')
                || num_prefix.ends_with('）')
            {
                ""
            } else {
                " "
            };
            let text = format!("{num_prefix}{sep}");
            let font_family = content
                .iter()
                .find_map(|inl| {
                    if let InlineNode::Text(r) = inl {
                        r.font_family.clone()
                    } else {
                        None
                    }
                })
                .or_else(|| Some("Libertinus Serif".to_string()));
            TextRunIr {
                text,
                font_family,
                font_size_pt: Some(sz_pt),
                bold: true,
                ..Default::default()
            }
        });

        Self {
            style_id,
            before_dxa,
            after_dxa,
            line_dxa,
            align,
            prefix_run,
        }
    }
}
