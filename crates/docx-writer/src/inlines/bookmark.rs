//! `<w:bookmarkStart>` / `<w:bookmarkEnd>` anchors for cross references.

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use crate::xml::escape_xml;

impl<'a> DocxWriter<'a> {
    pub(crate) fn emit_bookmark_start(&mut self, name: &str, out: &mut String) -> u32 {
        let id = self.next_bookmark_id;
        self.next_bookmark_id += 1;
        let _ = write!(
            out,
            r#"<w:bookmarkStart w:id="{id}" w:name="{}"/>"#,
            escape_xml(name)
        );
        id
    }

    pub(crate) fn emit_bookmark_end(id: u32, out: &mut String) {
        let _ = write!(out, r#"<w:bookmarkEnd w:id="{id}"/>"#);
    }
}
