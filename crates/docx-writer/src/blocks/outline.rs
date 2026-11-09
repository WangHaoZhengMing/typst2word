//! `outline` -> TOC paragraphs with dot leaders and internal hyperlinks.

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use ir::{InlineNode, OutlineEntryIr, TextRunIr};
use ir::units::pt_to_dxa;
use crate::xml::escape_xml;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_outline(
        &mut self,
        title: Option<&[InlineNode]>,
        entries: &[OutlineEntryIr],
        out: &mut String,
    ) {
        if let Some(t) = title {
            out.push_str(
                r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/><w:spacing w:before="120" w:after="60" w:line="240" w:lineRule="auto"/></w:pPr>"#,
            );
            self.render_inlines(t, out);
            out.push_str("</w:p>");
        }
        let content_w_pt = (self.current_setup.width_pt
            - self.current_setup.margin_left_pt
            - self.current_setup.margin_right_pt)
            .max(360.0);
        let tab_pos = pt_to_dxa(content_w_pt);
        let num_entries = entries.len();
        for (idx, entry) in entries.iter().enumerate() {
            let indent = (entry.level.saturating_sub(1)) * 250;
            let after = if idx + 1 == num_entries { 80 } else { 22 };
            let _ = write!(
                out,
                r#"<w:p><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="{tab_pos}"/></w:tabs><w:ind w:left="{indent}"/><w:spacing w:before="22" w:after="{after}" w:line="250" w:lineRule="auto"/></w:pPr>"#
            );
            let page_run = TextRunIr {
                text: entry.page.clone(),
                font_family: Some("Libertinus Serif".to_string()),
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
                self.render_inline(&InlineNode::Field {
                    kind: ir::FieldKindIr::PageRef { bookmark: bm.clone() },
                    format: None,
                    content: vec![InlineNode::Text(page_run.clone())],
                }, out);
                out.push_str("</w:hyperlink>");
            } else {
                self.render_inlines(&entry.title, out);
                out.push_str(r#"<w:r><w:tab/></w:r>"#);
                self.render_text_run(&page_run, out);
            }
            out.push_str("</w:p>");
        }
    }
}
