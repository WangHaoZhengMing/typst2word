//! `list` / `enum` / `terms` -> `ListSequenceSource` / `ListItemConstruct` -> numbered paragraphs (`<w:numPr>`).

mod item;

use std::fmt::Write as _;

use word_construct::list::{ListSequenceSource, solve_list_sequence_construct};
use crate::writer::DocxWriter;
use ir::{BlockNode, EnumItemIr, InlineNode, ListItemIr, TermItemIr, TextRunIr};

impl<'a> DocxWriter<'a> {
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
            items, tight, level, spacing_pt, indent_pt, hanging_pt, None, None, out,
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
        line_dxa_override: Option<i64>,
        out: &mut String,
    ) {
        let constructs = solve_list_sequence_construct(
            ListSequenceSource::Bullet { items },
            tight,
            level,
            spacing_pt,
            indent_pt,
            hanging_pt,
            trailing_after_dxa,
            line_dxa_override,
        );
        for item_construct in &constructs {
            self.render_list_or_enum_item_blocks(item_construct.blocks, item_construct, out);
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
            items, tight, level, spacing_pt, indent_pt, hanging_pt, None, None, out,
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
        line_dxa_override: Option<i64>,
        out: &mut String,
    ) {
        let ilvl = level.min(8);
        let start_val = items.first().and_then(|it| it.number).unwrap_or(1);
        let num_id = self.alloc_enum_num_id(ilvl, start_val);
        let constructs = solve_list_sequence_construct(
            ListSequenceSource::DecimalEnum {
                items,
                num_id,
                start_val,
            },
            tight,
            level,
            spacing_pt,
            indent_pt,
            hanging_pt,
            trailing_after_dxa,
            line_dxa_override,
        );
        for item_construct in &constructs {
            self.render_list_or_enum_item_blocks(item_construct.blocks, item_construct, out);
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
}
