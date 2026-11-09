//! `list` / `enum` / `terms` -> numbered paragraphs (`<w:numPr>`) or hanging-indent terms.

mod item;

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{BlockNode, EnumItemIr, InlineNode, ListItemIr, TermItemIr, TextRunIr};
use crate::units::pt_to_dxa;

/// Type-safe representation of a list/enum item's marker style in Word.
#[derive(Debug, Clone, Copy)]
pub(super) enum ListMarkerKind<'a> {
    /// Standard Word `<w:numPr>` at a specific `ilvl`.
    NumberedLevel(usize),
    /// Literal marker text prefix (e.g. custom bullet or `(1)` enum marker).
    CustomPrefix(&'a str),
}

/// Layout and numbering context for rendering a single list or enum item.
pub(super) struct ListItemRenderCtx<'a> {
    pub(super) num_id: usize,
    pub(super) ilvl: usize,
    pub(super) marker: ListMarkerKind<'a>,
    pub(super) is_single_digit_enum_item: bool,
    pub(super) tight: bool,
    pub(super) item_gap_dxa: i64,
    pub(super) item_end_after_dxa: i64,
    pub(super) before_dxa: i64,
    pub(super) indent_pt: f64,
    pub(super) hanging_pt: f64,
    pub(super) is_last_top: bool,
}

impl<'a> DocxBuilder<'a> {
    /// `BlockNode::List` -> bullet-numbered paragraphs (`numId` 1).
    pub(crate) fn render_list(
        &mut self,
        items: &[ListItemIr],
        tight: bool,
        level: usize,
        spacing_pt: Option<f64>,
        indent_pt: f64,
        hanging_pt: f64,
        out: &mut String,
    ) {
        self.render_list_with_trailing_gap(
            items, tight, level, spacing_pt, indent_pt, hanging_pt, None, out,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_list_with_trailing_gap(
        &mut self,
        items: &[ListItemIr],
        tight: bool,
        level: usize,
        spacing_pt: Option<f64>,
        indent_pt: f64,
        hanging_pt: f64,
        trailing_after_dxa: Option<i64>,
        out: &mut String,
    ) {
        let ilvl = level.min(8);
        let num_items = items.len();
        let item_gap_dxa = Self::compute_list_item_gap_dxa(tight, spacing_pt);
        for (idx, item) in items.iter().enumerate() {
            let is_last_in_list = idx + 1 == num_items;
            let is_last_top = level == 0 && is_last_in_list;
            let item_end_after_dxa = if is_last_in_list {
                trailing_after_dxa.unwrap_or(if is_last_top {
                    item_gap_dxa.max(40)
                } else {
                    item_gap_dxa
                })
            } else {
                item_gap_dxa
            };
            let prev_ended_with_table =
                idx > 0 && matches!(items[idx - 1].blocks.last(), Some(BlockNode::Table(_)));
            let before_dxa = if prev_ended_with_table && item_gap_dxa > 0 {
                item_gap_dxa + 64
            } else {
                0
            };
            let marker = match item.marker.as_str() {
                "•" => ListMarkerKind::NumberedLevel(0),
                "‣" => ListMarkerKind::NumberedLevel(1),
                "–" => ListMarkerKind::NumberedLevel(2),
                _ => ListMarkerKind::NumberedLevel(ilvl),
            };
            let ctx = ListItemRenderCtx {
                num_id: 1,
                ilvl,
                marker,
                is_single_digit_enum_item: false,
                tight,
                item_gap_dxa,
                item_end_after_dxa,
                before_dxa,
                indent_pt,
                hanging_pt,
                is_last_top,
            };
            self.render_list_or_enum_item_blocks(&item.blocks, &ctx, out);
        }
    }

    /// `BlockNode::Enum` -> decimal-numbered paragraphs (`numId` allocated per list).
    pub(crate) fn render_enum(
        &mut self,
        items: &[EnumItemIr],
        tight: bool,
        level: usize,
        spacing_pt: Option<f64>,
        indent_pt: f64,
        hanging_pt: f64,
        out: &mut String,
    ) {
        self.render_enum_with_trailing_gap(
            items, tight, level, spacing_pt, indent_pt, hanging_pt, None, out,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_enum_with_trailing_gap(
        &mut self,
        items: &[EnumItemIr],
        tight: bool,
        level: usize,
        spacing_pt: Option<f64>,
        indent_pt: f64,
        hanging_pt: f64,
        trailing_after_dxa: Option<i64>,
        out: &mut String,
    ) {
        let ilvl = level.min(8);
        let start_val = items.first().and_then(|it| it.number).unwrap_or(1);
        let num_id = self.alloc_enum_num_id(ilvl, start_val);
        let num_items = items.len();
        let item_gap_dxa = Self::compute_list_item_gap_dxa(tight, spacing_pt);
        for (idx, item) in items.iter().enumerate() {
            let is_last_in_list = idx + 1 == num_items;
            let is_last_top = level == 0 && is_last_in_list;
            let item_end_after_dxa = if is_last_in_list {
                trailing_after_dxa.unwrap_or(if is_last_top {
                    item_gap_dxa.max(40)
                } else {
                    item_gap_dxa
                })
            } else {
                item_gap_dxa
            };
            let prev_ended_with_table =
                idx > 0 && matches!(items[idx - 1].blocks.last(), Some(BlockNode::Table(_)));
            let before_dxa = if prev_ended_with_table && item_gap_dxa > 0 {
                item_gap_dxa + 64
            } else if idx == 0 && start_val > 1 {
                item_gap_dxa.max(180)
            } else {
                0
            };
            let marker = if Self::is_standard_decimal_marker(&item.marker_text) {
                ListMarkerKind::NumberedLevel(ilvl)
            } else {
                ListMarkerKind::CustomPrefix(item.marker_text.as_str())
            };
            let is_single_digit_enum_item = item.marker_text.trim().len() == 2
                && Self::is_standard_decimal_marker(&item.marker_text);
            let ctx = ListItemRenderCtx {
                num_id,
                ilvl,
                marker,
                is_single_digit_enum_item,
                tight,
                item_gap_dxa,
                item_end_after_dxa,
                before_dxa,
                indent_pt,
                hanging_pt,
                is_last_top,
            };
            self.render_list_or_enum_item_blocks(&item.blocks, &ctx, out);
        }
    }

    fn compute_list_item_gap_dxa(tight: bool, spacing_pt: Option<f64>) -> i64 {
        if let Some(sp_pt) = spacing_pt {
            pt_to_dxa(sp_pt).max(0)
        } else if tight {
            0
        } else {
            90
        }
    }

    /// `BlockNode::Terms` -> hanging-indent paragraphs with bold terms.
    pub(crate) fn render_terms(&mut self, items: &[TermItemIr], tight: bool, out: &mut String) {
        let num_items = items.len();
        for (idx, item) in items.iter().enumerate() {
            let is_last = idx + 1 == num_items;
            let after = if is_last {
                110
            } else if tight {
                15
            } else {
                110
            };
            let _ = write!(
                out,
                r#"<w:p><w:pPr><w:spacing w:before="10" w:after="{after}" w:line="246" w:lineRule="auto"/><w:ind w:left="440" w:hanging="440"/></w:pPr>"#
            );
            for inline in &item.term {
                if let InlineNode::Text(r) = inline {
                    let mut bold_run = r.clone();
                    bold_run.bold = true;
                    self.render_text_run(&bold_run, out);
                } else {
                    self.render_inline(inline, out);
                }
            }
            let sep_run = TextRunIr {
                text: "  ".to_string(),
                font_family: Some("Libertinus Serif".to_string()),
                font_size_pt: Some(11.0),
                ..Default::default()
            };
            self.render_text_run(&sep_run, out);
            if let Some(BlockNode::Paragraph { content, .. }) = item.description.first() {
                self.render_inlines(content, out);
                out.push_str("</w:p>");
                for rest in item.description.iter().skip(1) {
                    self.render_block(rest, out);
                }
            } else {
                out.push_str("</w:p>");
                self.render_blocks(&item.description, out);
            }
        }
    }

    pub(crate) fn is_standard_decimal_marker(marker: &str) -> bool {
        let trimmed = marker.trim();
        trimmed.ends_with('.')
            && trimmed[..trimmed.len() - 1]
                .chars()
                .all(|c| c.is_ascii_digit())
    }
}
