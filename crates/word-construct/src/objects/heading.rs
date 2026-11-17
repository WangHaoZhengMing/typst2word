//! Word Heading Construct (`Title` and `Heading1`..`Heading6`).
//!
//! Solves heading style ID, `w:before`, `w:after`, `w:line`, alignment, and optional
//! bold numbering prefix run.

use std::collections::BTreeMap;

use crate::font_metrics::estimate_text_advance_pt;
use crate::objects::paragraph::compute_line_spacing_dxa_with_metrics;
use crate::units::pt_to_dxa;
use ir::{FontFaceMetricsIr, HAlignIr, InlineNode, TextRunIr};

/// Solved Word construct for a `Heading` (`Heading1`..`Heading6`) paragraph.
#[derive(Debug, Clone)]
pub struct HeadingConstruct {
    pub style_id: String,
    pub before_dxa: i64,
    pub after_dxa: i64,
    pub line_dxa: i64,
    pub align: HAlignIr,
    /// Hanging indent (`dxa`) for left-aligned numbered headings so wrapped lines align under
    /// the heading title rather than under the numbering prefix (`left - hanging == 0` on line 1).
    pub hanging_dxa: i64,
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
        Self::solve_with_metrics(
            level,
            numbering,
            content,
            align,
            collapse_before,
            compact_after,
            &BTreeMap::new(),
        )
    }

    pub fn solve_with_metrics(
        level: usize,
        numbering: Option<&str>,
        content: &[InlineNode],
        align: HAlignIr,
        collapse_before: bool,
        compact_after: bool,
        font_metrics: &BTreeMap<String, FontFaceMetricsIr>,
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
            let line_dxa = compute_line_spacing_dxa_with_metrics(
                content,
                Some(0.65 * sz_pt),
                font_metrics,
                None,
            );
            let before_dxa = if collapse_before {
                0
            } else {
                pt_to_dxa(sz_pt * 0.567)
            };
            let after_dxa = if compact_after {
                20
            } else {
                // Solid blocks (`Table`, `ShapeBlock`, `Figure`, `List`) have zero internal top
                // leading, so `0.55em` Typst gap minus `~0.23em` heading descent leaves `0.32em`
                // (while `Heading -> Heading` still collapses to the next heading's `0.567em` `w:before`).
                pt_to_dxa(sz_pt * 0.32)
            };
            (before_dxa, after_dxa, line_dxa)
        } else {
            let line_dxa =
                compute_line_spacing_dxa_with_metrics(content, None, font_metrics, None);
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

        let hanging_dxa = match (&prefix_run, align) {
            (Some(pref), HAlignIr::Left) => {
                let face = pref
                    .font_family
                    .as_deref()
                    .and_then(|fam| font_metrics.get(fam));
                let adv_pt = estimate_text_advance_pt(
                    &pref.text,
                    pref.font_family.as_deref(),
                    face,
                    sz_pt,
                    pref.bold,
                );
                pt_to_dxa(adv_pt).max(0)
            }
            _ => 0,
        };

        Self {
            style_id,
            before_dxa,
            after_dxa,
            line_dxa,
            align,
            hanging_dxa,
            prefix_run,
        }
    }
}
