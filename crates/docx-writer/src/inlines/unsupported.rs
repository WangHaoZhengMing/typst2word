//! Unsupported inline element -> red badge run.

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use word_construct::model::UnsupportedIr;
use crate::xml::escape_xml;

impl<'a> DocxWriter<'a> {
    /// `InlineNode::UnsupportedInline` -> red badge run.
    pub(crate) fn render_unsupported_inline(&mut self, unsup: &UnsupportedIr, out: &mut String) {
        let badge = unsup.display_badge();
        let _ = write!(
            out,
            r#"<w:r><w:rPr><w:b/><w:color w:val="C62828"/><w:shd w:val="clear" w:color="auto" w:fill="FFEBEE"/></w:rPr><w:t xml:space="preserve">{}</w:t></w:r>"#,
            escape_xml(&badge)
        );
    }
}
