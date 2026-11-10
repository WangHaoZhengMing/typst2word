//! `footnote` -> `<w:footnoteReference>` (bodies go to `footnotes.xml`).

use std::fmt::Write as _;

use crate::writer::{DocxWriter, FootnoteRecord};
use word_construct::model::BlockNode;

impl<'a> DocxWriter<'a> {
    /// `InlineNode::Footnote` -> registers the footnote body and emits a `<w:footnoteReference>` run.
    pub(crate) fn render_footnote_ref(
        &mut self,
        id: usize,
        content: &[BlockNode],
        out: &mut String,
    ) {
        if !self.footnotes.iter().any(|f| f.id == id) {
            self.footnotes.push(FootnoteRecord {
                id: id,
                blocks: content.to_vec(),
            });
        }
        let _ = write!(
            out,
            r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/><w:vertAlign w:val="superscript"/></w:rPr><w:footnoteReference w:id="{id}"/></w:r>"#
        );
    }
}
