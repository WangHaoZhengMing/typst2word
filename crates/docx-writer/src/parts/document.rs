//! `word/document.xml`, `word/header1.xml`, `word/footer1.xml` and `<w:sectPr>` (page setup).

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use word_construct::model::{BlockNode, InlineNode};
use word_construct::units::pt_to_dxa;

impl<'a> DocxWriter<'a> {
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

        if let Some(bg_hex) = &self.current_setup.fill {
            let _ = write!(out, r#"<w:background w:color="{bg_hex}"/><w:body>"#);
        } else {
            out.push_str("<w:body>");
        }

        let sections: Vec<_> = std::iter::once((self.doc.page_setup.clone(), self.doc.blocks.clone()))
            .chain(self.doc.sections.iter().map(|s| (s.page_setup.clone(), s.blocks.clone()))).collect();
        for (index, (setup, blocks)) in sections.iter().enumerate() {
            self.current_setup = setup.clone();
            self.current_blocks = blocks.clone();
            self.header_rel_id = self.section_furniture[index].0.clone();
            self.footer_rel_id = self.section_furniture[index].1.clone();
            self.active_columns = (setup.columns, 18.0);
            self.has_scoped_columns = false;
            self.render_blocks(blocks, &mut out);
            if index + 1 < sections.len() {
                out.push_str("<w:p><w:pPr>");
                self.render_sect_pr(&mut out);
                out.push_str("</w:pPr></w:p>");
            } else {
                self.render_sect_pr(&mut out);
            }
        }

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
        if blocks.is_empty() { out.push_str("<w:p/>"); }
        let _ = write!(out, "</w:{tag}>");
        out
    }

    pub(crate) fn render_sect_pr(&self, out: &mut String) {
        let ps = &self.current_setup;
        self.render_sect_pr_inner(ps.columns, 36.0, self.has_scoped_columns, out);
    }

    pub(crate) fn first_block_uses_cjk_font(&self) -> bool {
        let inlines = match self.current_blocks.first() {
            Some(BlockNode::Heading { content, .. }) => Some(content.as_slice()),
            Some(BlockNode::Paragraph { content, .. }) => Some(content.as_slice()),
            Some(BlockNode::Title { content, .. }) => Some(content.as_slice()),
            _ => None,
        };
        let Some(inlines) = inlines else {
            return false;
        };
        inlines.iter().any(|inl| {
            if let InlineNode::Text(r) = inl
                && let Some(fam) = &r.font_family
            {
                let fl = fam.to_ascii_lowercase();
                fl.contains("song")
                    || fl.contains("hei")
                    || fl.contains("kai")
                    || fl.contains("fang")
                    || fl.contains("pingfang")
                    || fl.contains("sim")
                    || fl.contains("yahei")
                    || fl.contains("宋体")
                    || fl.contains("黑体")
                    || fl.contains("楷体")
            } else {
                false
            }
        })
    }

    pub(crate) fn render_sect_pr_inner(
        &self,
        cols: usize,
        gutter_pt: f64,
        is_continuous: bool,
        out: &mut String,
    ) {
        let ps = &self.current_setup;
        out.push_str("<w:sectPr>");
        if let Some(hid) = &self.header_rel_id {
            let _ = write!(out, r#"<w:headerReference w:type="default" r:id="{hid}"/>"#);
        }
        if let Some(fid) = &self.footer_rel_id {
            let _ = write!(out, r#"<w:footerReference w:type="default" r:id="{fid}"/>"#);
        }
        if is_continuous {
            out.push_str(r#"<w:type w:val="continuous"/>"#);
        }
        let w_dxa = pt_to_dxa(ps.width_pt).max(1440);
        let h_dxa = pt_to_dxa(ps.height_pt).max(1440);
        let orient = if ps.width_pt > ps.height_pt {
            r#" w:orient="landscape""#
        } else {
            ""
        };
        let _ = write!(out, r#"<w:pgSz w:w="{w_dxa}" w:h="{h_dxa}"{orient}/>"#);
        let (top_pt, bottom_pt) = self.options.page_margin_calibration.margins(
            ps.margin_top_pt, ps.margin_bottom_pt, self.first_block_uses_cjk_font(),
        );
        let top_dxa = pt_to_dxa(top_pt);
        let bottom_dxa = pt_to_dxa(bottom_pt);
        let _ = write!(
            out,
            r#"<w:pgMar w:top="{top_dxa}" w:right="{}" w:bottom="{bottom_dxa}" w:left="{}" w:header="{}" w:footer="{}" w:gutter="0"/>"#,
            pt_to_dxa(ps.margin_right_pt),
            pt_to_dxa(ps.margin_left_pt),
            pt_to_dxa(ps.header_margin_pt),
            pt_to_dxa(ps.footer_margin_pt),
        );
        if cols > 1 {
            let space_dxa = pt_to_dxa(gutter_pt).max(120);
            let _ = write!(out, r#"<w:cols w:num="{cols}" w:space="{space_dxa}"/>"#);
        } else if self.has_scoped_columns {
            out.push_str(r#"<w:cols w:space="720"/>"#);
        }
        out.push_str("</w:sectPr>");
    }
}
