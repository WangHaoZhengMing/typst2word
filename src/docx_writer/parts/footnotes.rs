//! `word/footnotes.xml`: bodies of all footnotes collected while rendering.

use std::fmt::Write as _;

use crate::docx_writer::writer::DocxWriter;
use crate::ir::BlockNode;

impl<'a> DocxWriter<'a> {
    // =========================================================================
    // Supporting OpenXML Package Parts (`footnotes.xml`, `styles.xml`,
    // `numbering.xml`, `settings.xml`, `.rels`, `[Content_Types].xml`, `core.xml`)
    // =========================================================================
    pub(crate) fn render_footnotes_xml(&mut self) -> String {
        let mut out = String::new();
        out.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
        let _ = write!(out, "<w:footnotes {}>", Self::xml_namespaces());
        out.push_str(
            r#"<w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>"#,
        );
        out.push_str(
            r#"<w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>"#,
        );

        let records: Vec<(usize, Vec<BlockNode>)> = self
            .footnotes
            .iter()
            .map(|f| (f.id, f.blocks.clone()))
            .collect();

        for (id, blocks) in records {
            let _ = write!(out, r#"<w:footnote w:id="{id}">"#);
            if blocks.is_empty() {
                out.push_str(
                    r#"<w:p><w:pPr><w:pStyle w:val="FootnoteText"/></w:pPr><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/><w:vertAlign w:val="superscript"/></w:rPr><w:footnoteRef/></w:r></w:p>"#,
                );
            } else {
                for (b_idx, block) in blocks.iter().enumerate() {
                    if b_idx == 0 {
                        if let BlockNode::Paragraph { content, .. } = block {
                            out.push_str(
                                r#"<w:p><w:pPr><w:pStyle w:val="FootnoteText"/></w:pPr><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/><w:vertAlign w:val="superscript"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> </w:t></w:r>"#,
                            );
                            self.render_inlines(content, &mut out);
                            out.push_str("</w:p>");
                            continue;
                        }
                    }
                    self.render_block(block, &mut out);
                }
            }
            out.push_str("</w:footnote>");
        }

        out.push_str("</w:footnotes>");
        out
    }
}
