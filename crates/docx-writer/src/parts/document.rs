//! `word/document.xml`, `word/header1.xml`, `word/footer1.xml` and `<w:sectPr>` (page setup).

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use word_construct::model::{BlockNode, InlineNode, PageNumberFormatIr};
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
            let rels = &self.section_furniture[index];
            self.header_rel_id = rels.default_hdr.clone();
            self.footer_rel_id = rels.default_ftr.clone();
            self.first_header_rel_id = rels.first_hdr.clone();
            self.first_footer_rel_id = rels.first_ftr.clone();
            self.even_header_rel_id = rels.even_hdr.clone();
            self.even_footer_rel_id = rels.even_ftr.clone();
            self.active_columns = (setup.columns, 18.0);
            self.has_scoped_columns = false;
            self.render_blocks(blocks, &mut out);
            if index + 1 < sections.len() {
                out.push_str(
                    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr>"#,
                );
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
        if blocks.is_empty() {
            out.push_str("<w:p/>");
        }
        let _ = write!(out, "</w:{tag}>");
        out
    }

    pub(crate) fn render_sect_pr(&self, out: &mut String) {
        let ps = &self.current_setup;
        self.render_sect_pr_inner(ps.columns, 36.0, self.has_scoped_columns, out);
    }

    pub(crate) fn first_block_first_line_metrics(&self) -> (Option<&str>, Option<f64>, bool) {
        let (inlines, default_sz) = match self.current_blocks.first() {
            Some(BlockNode::Heading { level, content, .. }) => {
                let dsz = match level {
                    1 => 15.4,
                    2 => 13.2,
                    _ => 11.0,
                };
                (Some(content.as_slice()), Some(dsz))
            }
            Some(BlockNode::Paragraph { content, .. }) => (Some(content.as_slice()), Some(11.0)),
            Some(BlockNode::Title { content, .. }) => (Some(content.as_slice()), Some(18.0)),
            _ => (None, None),
        };
        let Some(inlines) = inlines else {
            return (None, None, false);
        };
        let is_cjk_family = |fam: &str| {
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
                || fl.contains("微软雅黑")
        };
        let cjk_font = inlines.iter().find_map(|inl| match inl {
            InlineNode::Text(r) => r.font_family.as_deref().filter(|f| is_cjk_family(f)),
            _ => None,
        });
        let first_font = cjk_font.or_else(|| {
            inlines.iter().find_map(|inl| match inl {
                InlineNode::Text(r) => r.font_family.as_deref(),
                _ => None,
            })
        });
        let font_size_pt = inlines
            .iter()
            .find_map(|inl| match inl {
                InlineNode::Text(r) => r.font_size_pt,
                _ => None,
            })
            .or(default_sz);
        let is_cjk = cjk_font.is_some() || word_construct::paragraph::content_has_cjk(inlines);
        (first_font, font_size_pt, is_cjk)
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
        if !is_continuous {
            if let Some(fhid) = &self.first_header_rel_id {
                let _ = write!(out, r#"<w:headerReference w:type="first" r:id="{fhid}"/>"#);
            }
            if let Some(ehid) = &self.even_header_rel_id {
                let _ = write!(out, r#"<w:headerReference w:type="even" r:id="{ehid}"/>"#);
            }
        }
        if let Some(fid) = &self.footer_rel_id {
            let _ = write!(out, r#"<w:footerReference w:type="default" r:id="{fid}"/>"#);
        }
        if !is_continuous {
            if let Some(ffid) = &self.first_footer_rel_id {
                let _ = write!(out, r#"<w:footerReference w:type="first" r:id="{ffid}"/>"#);
            }
            if let Some(efid) = &self.even_footer_rel_id {
                let _ = write!(out, r#"<w:footerReference w:type="even" r:id="{efid}"/>"#);
            }
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
        let (first_font, first_sz_pt, first_is_cjk) = self.first_block_first_line_metrics();
        let (top_pt, bottom_pt) = self
            .options
            .page_margin_calibration
            .margins_for_first_line(
                ps.margin_top_pt,
                ps.margin_bottom_pt,
                first_font,
                first_sz_pt,
                first_is_cjk,
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
        if !is_continuous
            && (ps
                .page_number_format
                .is_some_and(|f| f != PageNumberFormatIr::Arabic)
                || ps.page_number_start.is_some())
        {
            let fmt_val = match ps.page_number_format.unwrap_or(PageNumberFormatIr::Arabic) {
                PageNumberFormatIr::Arabic => "decimal",
                PageNumberFormatIr::LowerRoman => "lowerRoman",
                PageNumberFormatIr::UpperRoman => "upperRoman",
                PageNumberFormatIr::LowerAlpha => "lowerLetter",
                PageNumberFormatIr::UpperAlpha => "upperLetter",
            };
            if let Some(start) = ps.page_number_start {
                let _ = write!(
                    out,
                    r#"<w:pgNumType w:fmt="{fmt_val}" w:start="{start}"/>"#
                );
            } else {
                let _ = write!(out, r#"<w:pgNumType w:fmt="{fmt_val}"/>"#);
            }
        }
        if cols > 1 {
            let space_dxa = pt_to_dxa(gutter_pt).max(120);
            let _ = write!(out, r#"<w:cols w:num="{cols}" w:space="{space_dxa}"/>"#);
        } else if self.has_scoped_columns {
            out.push_str(r#"<w:cols w:space="720"/>"#);
        }
        if ps.different_first_page && !is_continuous {
            out.push_str("<w:titlePg/>");
        }
        out.push_str("</w:sectPr>");
    }
}
