//! `word/document.xml`, `word/header1.xml`, `word/footer1.xml` and `<w:sectPr>` (page setup).

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::BlockNode;
use crate::units::pt_to_dxa;

impl<'a> DocxBuilder<'a> {
    // =========================================================================
    // Document XML & Block Rendering
    // =========================================================================
    pub(crate) fn xml_namespaces() -> &'static str {
        concat!(
            r#"xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas" "#,
            r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" "#,
            r#"xmlns:o="urn:schemas-microsoft-com:office:office" "#,
            r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
            r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math" "#,
            r#"xmlns:v="urn:schemas-microsoft-com:vml" "#,
            r#"xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing" "#,
            r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
            r#"xmlns:w10="urn:schemas-microsoft-com:office:word" "#,
            r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
            r#"xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" "#,
            r#"xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" "#,
            r#"xmlns:wpi="http://schemas.microsoft.com/office/word/2010/wordprocessingInk" "#,
            r#"xmlns:wne="http://schemas.microsoft.com/office/word/2006/wordml" "#,
            r#"xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" "#,
            r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
            r#"xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" "#,
            r#"xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main""#
        )
    }

    pub(crate) fn render_document_xml(&mut self) -> String {
        let mut out = String::with_capacity(16384);
        out.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
        let _ = write!(out, "<w:document {}>", Self::xml_namespaces());

        if let Some(bg_hex) = &self.ir.page_setup.fill {
            let _ = write!(out, r#"<w:background w:color="{bg_hex}"/><w:body>"#);
        } else {
            out.push_str("<w:body>");
        }

        let blocks = self.ir.blocks.clone();
        self.render_blocks(&blocks, &mut out);

        // Emit `<w:sectPr>` for page setup.
        self.render_sect_pr(&mut out);

        out.push_str("</w:body></w:document>");
        out
    }

    pub(crate) fn render_header_or_footer_xml(
        &mut self,
        blocks: &[BlockNode],
        tag: &str,
    ) -> String {
        let mut out = String::new();
        out.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
        let _ = write!(out, "<w:{tag} {}>", Self::xml_namespaces());
        self.render_blocks(blocks, &mut out);
        let _ = write!(out, "</w:{tag}>");
        out
    }

    pub(crate) fn render_sect_pr(&self, out: &mut String) {
        let ps = &self.ir.page_setup;
        out.push_str("<w:sectPr>");
        if let Some(hid) = &self.header_rel_id {
            let _ = write!(out, r#"<w:headerReference w:type="default" r:id="{hid}"/>"#);
        }
        if let Some(fid) = &self.footer_rel_id {
            let _ = write!(out, r#"<w:footerReference w:type="default" r:id="{fid}"/>"#);
        }
        let w_dxa = pt_to_dxa(ps.width_pt).max(1440);
        let h_dxa = pt_to_dxa(ps.height_pt).max(1440);
        let orient = if ps.width_pt > ps.height_pt {
            r#" w:orient="landscape""#
        } else {
            ""
        };
        let _ = write!(out, r#"<w:pgSz w:w="{w_dxa}" w:h="{h_dxa}"{orient}/>"#);
        let _ = write!(
            out,
            r#"<w:pgMar w:top="{}" w:right="{}" w:bottom="{}" w:left="{}" w:header="720" w:footer="720" w:gutter="0"/>"#,
            pt_to_dxa(ps.margin_top_pt),
            pt_to_dxa(ps.margin_right_pt),
            pt_to_dxa(ps.margin_bottom_pt),
            pt_to_dxa(ps.margin_left_pt),
        );
        if ps.columns > 1 {
            let _ = write!(out, r#"<w:cols w:num="{}" w:space="720"/>"#, ps.columns);
        }
        out.push_str("</w:sectPr>");
    }
}
