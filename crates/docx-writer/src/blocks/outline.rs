//! `outline` -> TOC paragraphs with dot leaders and internal hyperlinks.

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use word_construct::model::{InlineNode, OutlineEntryIr, TextRunIr};
use word_construct::units::pt_to_dxa;
use crate::xml::escape_xml;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_outline(
        &mut self,
        title: Option<&[InlineNode]>,
        entries: &[OutlineEntryIr],
        collapse_before: bool,
        out: &mut String,
    ) {
        if let Some(t) = title {
            let before = if collapse_before { 0 } else { 120 };
            let title_line_dxa = self.doc.resolve_paragraph_line_dxa(
                word_construct::model::NodeId::UNSET,
                t,
                None,
            );
            let _ = write!(
                out,
                r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/><w:spacing w:before="{before}" w:after="0" w:line="{title_line_dxa}" w:lineRule="auto"/></w:pPr>"#,
            );
            self.render_inlines(t, out);
            out.push_str("</w:p>");
        }
        let content_w_pt = (self.current_setup.width_pt
            - self.current_setup.margin_left_pt
            - self.current_setup.margin_right_pt)
            .max(120.0);
        let tab_pos = pt_to_dxa(content_w_pt);
        let num_entries = entries.len();
        for (idx, entry) in entries.iter().enumerate() {
            let indent = (entry.level.saturating_sub(1)) * 250;
            // Calibrate `w:line` from the entry's font metrics (`WordLineBox`) with zero artificial
            // inter-entry `before`/`after` padding so CJK and Western TOC rows match Typst's `par.leading`.
            let line_dxa = self.doc.resolve_paragraph_line_dxa(
                word_construct::model::NodeId::UNSET,
                &entry.title,
                None,
            );
            let after = if idx + 1 == num_entries { 80 } else { 0 };
            let _ = write!(
                out,
                r#"<w:p><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="{tab_pos}"/></w:tabs><w:ind w:left="{indent}"/><w:spacing w:before="0" w:after="{after}" w:line="{line_dxa}" w:lineRule="auto"/></w:pPr>"#
            );
            let page_run = TextRunIr {
                text: entry.page.clone(),
                font_size_pt: Some(11.0),
                ..Default::default()
            };
            if let Some(bm) = &entry.target_bookmark {
                let _ = write!(
                    out,
                    r#"<w:hyperlink w:anchor="{}" w:history="1">"#,
                    escape_xml(bm)
                );
                self.render_inlines(&entry.title, out);
                out.push_str(r#"<w:r><w:tab/></w:r>"#);
                if !entry.page_inlines.is_empty() {
                    self.render_inlines(&entry.page_inlines, out);
                } else {
                    self.render_inline(
                        &InlineNode::Field {
                            kind: word_construct::model::FieldKindIr::PageRef {
                                bookmark: bm.clone(),
                            },
                            format: None,
                            content: vec![InlineNode::Text(page_run.clone())],
                        },
                        out,
                    );
                }
                out.push_str("</w:hyperlink>");
            } else {
                self.render_inlines(&entry.title, out);
                out.push_str(r#"<w:r><w:tab/></w:r>"#);
                if !entry.page_inlines.is_empty() {
                    self.render_inlines(&entry.page_inlines, out);
                } else {
                    self.render_text_run(&page_run, out);
                }
            }
            out.push_str("</w:p>");
        }
    }
}
