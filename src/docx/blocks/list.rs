//! `list` / `enum` / `terms` -> numbered paragraphs (`<w:numPr>`) or hanging-indent terms.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{BlockNode, EnumItemIr, InlineNode, ListItemIr, TermItemIr, TextRunIr};
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    /// `BlockNode::List` -> bullet-numbered paragraphs (`numId` 1).
    pub(crate) fn render_list(
        &mut self,
        items: &[ListItemIr],
        tight: bool,
        level: usize,
        out: &mut String,
    ) {
        let ilvl = (level).min(8);
        for item in items {
            self.render_list_or_enum_item_blocks(
                &item.blocks,
                1, // numId 1 = bullet list
                ilvl,
                None,
                tight,
                out,
            );
        }
    }

    /// `BlockNode::Enum` -> decimal-numbered paragraphs (`numId` 2).
    pub(crate) fn render_enum(
        &mut self,
        items: &[EnumItemIr],
        tight: bool,
        level: usize,
        out: &mut String,
    ) {
        let ilvl = (level).min(8);
        for item in items {
            let custom_prefix = if Self::is_standard_decimal_marker(&item.marker_text) {
                None
            } else {
                Some(item.marker_text.as_str())
            };
            self.render_list_or_enum_item_blocks(
                &item.blocks,
                2, // numId 2 = decimal ordered list
                ilvl,
                custom_prefix,
                tight,
                out,
            );
        }
    }

    /// `BlockNode::Terms` -> hanging-indent paragraphs with bold terms.
    pub(crate) fn render_terms(&mut self, items: &[TermItemIr], tight: bool, out: &mut String) {
        let after = if tight { 60 } else { 140 };
        for item in items {
            out.push_str("<w:p><w:pPr>");
            let _ = write!(
                out,
                r#"<w:spacing w:before="40" w:after="{after}"/><w:ind w:left="360" w:hanging="360"/>"#
            );
            out.push_str("</w:pPr>");
            // Render term runs in bold followed by Typst's default 0.6em space separator and description.
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

    pub(crate) fn render_list_or_enum_item_blocks(
        &mut self,
        blocks: &[BlockNode],
        num_id: usize,
        ilvl: usize,
        custom_prefix: Option<&str>,
        tight: bool,
        out: &mut String,
    ) {
        let after_dxa = if tight { 40 } else { 120 };
        let indent_left = 360 * (ilvl + 1);
        if blocks.is_empty() {
            let _ = write!(
                out,
                r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{ilvl}"/><w:numId w:val="{num_id}"/></w:numPr><w:spacing w:after="{after_dxa}"/></w:pPr></w:p>"#
            );
            return;
        }

        for (idx, b) in blocks.iter().enumerate() {
            match b {
                BlockNode::Paragraph {
                    content, bookmark, ..
                } if idx == 0 => {
                    out.push_str("<w:p><w:pPr>");
                    if let Some(prefix) = custom_prefix {
                        let _ = write!(
                            out,
                            r#"<w:spacing w:after="{after_dxa}"/><w:ind w:left="{indent_left}" w:hanging="280"/></w:pPr>"#
                        );
                        let bm_id = bookmark
                            .as_ref()
                            .map(|bm| self.emit_bookmark_start(bm, out));
                        let _ = write!(
                            out,
                            r#"<w:r><w:t xml:space="preserve">{} </w:t></w:r>"#,
                            escape_xml(prefix)
                        );
                        self.render_inlines(content, out);
                        if let Some(id) = bm_id {
                            Self::emit_bookmark_end(id, out);
                        }
                        out.push_str("</w:p>");
                    } else {
                        let _ = write!(
                            out,
                            r#"<w:numPr><w:ilvl w:val="{ilvl}"/><w:numId w:val="{num_id}"/></w:numPr><w:spacing w:after="{after_dxa}"/></w:pPr>"#
                        );
                        let bm_id = bookmark
                            .as_ref()
                            .map(|bm| self.emit_bookmark_start(bm, out));
                        self.render_inlines(content, out);
                        if let Some(id) = bm_id {
                            Self::emit_bookmark_end(id, out);
                        }
                        out.push_str("</w:p>");
                    }
                }
                other => {
                    self.render_block(other, out);
                }
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
