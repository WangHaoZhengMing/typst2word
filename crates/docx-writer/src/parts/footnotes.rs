//! `word/footnotes.xml`: bodies of all footnotes collected while rendering.

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use word_construct::model::BlockNode;
use word_construct::units::pt_to_dxa;

impl<'a> DocxWriter<'a> {
    // =========================================================================
    // Supporting OpenXML Package Parts (`footnotes.xml`, `styles.xml`,
    // `numbering.xml`, `settings.xml`, `.rels`, `[Content_Types].xml`, `core.xml`)
    // =========================================================================
    pub(crate) fn render_footnotes_xml(&mut self) -> String {
        let mut out = String::new();
        out.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
        let _ = write!(out, "<w:footnotes {}>", Self::xml_namespaces());
        let content_w_pt = (self.doc.page_setup.width_pt
            - self.doc.page_setup.margin_left_pt
            - self.doc.page_setup.margin_right_pt)
            .max(72.0);
        let sep_right_ind_dxa = pt_to_dxa(content_w_pt * 0.70);
        let _ = write!(
            out,
            r#"<w:footnote w:type="separator" w:id="-1"><w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="4" w:space="0" w:color="000000"/></w:pBdr><w:ind w:right="{sep_right_ind_dxa}"/><w:spacing w:before="0" w:after="40" w:line="40" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr></w:pPr></w:p></w:footnote>"#,
        );
        out.push_str(
            r#"<w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:pPr><w:spacing w:before="0" w:after="40" w:line="40" w:lineRule="exact"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>"#,
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
                    r#"<w:p><w:pPr><w:pStyle w:val="FootnoteText"/><w:spacing w:before="0" w:after="0" w:line="220" w:lineRule="auto"/></w:pPr><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/><w:vertAlign w:val="superscript"/></w:rPr><w:footnoteRef/></w:r></w:p>"#,
                );
            } else {
                for (b_idx, block) in blocks.iter().enumerate() {
                    if b_idx == 0 {
                        if let BlockNode::Paragraph { content, .. } = block {
                            out.push_str(
                                r#"<w:p><w:pPr><w:pStyle w:val="FootnoteText"/><w:ind w:left="187"/><w:spacing w:before="0" w:after="0" w:line="220" w:lineRule="auto"/></w:pPr><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/><w:spacing w:val="10"/><w:vertAlign w:val="superscript"/></w:rPr><w:footnoteRef/></w:r>"#,
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
