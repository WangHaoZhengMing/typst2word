//! `bibliography` -> hanging-indent reference entries.

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use word_construct::model::{BibEntryIr, InlineNode};
use crate::xml::escape_xml;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_bibliography(
        &mut self,
        title: Option<&[InlineNode]>,
        entries: &[BibEntryIr],
        collapse_before: bool,
        out: &mut String,
    ) {
        if let Some(t) = title {
            let before = if collapse_before { 0 } else { 120 };
            let _ = write!(
                out,
                r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/><w:spacing w:before="{before}" w:after="60" w:line="240" w:lineRule="auto"/></w:pPr>"#,
            );
            self.render_inlines(t, out);
            out.push_str("</w:p>");
        }
        for (idx, entry) in entries.iter().enumerate() {
            let before = if idx == 0 && title.is_none() && collapse_before {
                0
            } else {
                40
            };
            let _ = write!(
                out,
                r#"<w:p><w:pPr><w:ind w:left="480" w:hanging="480"/><w:spacing w:before="{before}" w:after="80"/></w:pPr>"#,
            );
            if let Some(prefix) = &entry.prefix {
                let _ = write!(
                    out,
                    r#"<w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">{} </w:t></w:r>"#,
                    escape_xml(prefix)
                );
            }
            self.render_inlines(&entry.content, out);
            out.push_str("</w:p>");
        }
    }
}
