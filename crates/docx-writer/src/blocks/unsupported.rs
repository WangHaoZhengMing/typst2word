//! Unsupported block element -> dashed red placeholder box.

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use word_construct::model::UnsupportedIr;
use crate::xml::escape_xml;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_unsupported_block(&self, unsup: &UnsupportedIr, out: &mut String) {
        let badge = unsup.display_badge();
        let _ = write!(
            out,
            r#"<w:p><w:pPr><w:pBdr><w:top w:val="dashed" w:sz="6" w:space="4" w:color="D32F2F"/><w:left w:val="dashed" w:sz="6" w:space="6" w:color="D32F2F"/><w:bottom w:val="dashed" w:sz="6" w:space="4" w:color="D32F2F"/><w:right w:val="dashed" w:sz="6" w:space="6" w:color="D32F2F"/></w:pBdr><w:shd w:val="clear" w:color="auto" w:fill="FFF5F5"/><w:spacing w:before="80" w:after="80"/></w:pPr><w:r><w:rPr><w:b/><w:color w:val="C62828"/></w:rPr><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
            escape_xml(&badge)
        );
    }
}
