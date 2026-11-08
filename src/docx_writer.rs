//! OpenXML `.docx` Writer (`DocumentIr` -> `.docx` ZIP package).
//!
//! Generates a complete, valid Microsoft Word `.docx` archive with:
//! - Native Word DrawingML shapes (`<wps:wsp>`) and CeTZ vector groups (`<wpg:wgp>`)
//! - Native Word Office Math (`<m:oMath>` & `<m:oMathPara>`)
//! - Semantic tables & grids (`<w:gridSpan>`, `<w:vMerge>`, `<w:tcBorders>`, `<w:shd>`, `<w:tblHeader>`)
//! - Headings, titles, outlines (TOC), bibliographies, blockquotes, figures, footnotes, links, refs
//! - Syntax-highlighted code blocks (`RawLine` token colors preserved)
//! - Page setup (dimensions, margins, columns, background fill, header & footer)
//! - Clear visual badges/boxes for unsupported Typst elements (`[不支持的元素: ...]`)

use std::fmt::Write as _;
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::ir::*;
use crate::shape_dml::{
    cetz_canvas_to_drawingml_run, pt_to_dxa, pt_to_emu, shape_to_drawingml_run,
};

/// Write `DocumentIr` to a `.docx` file on disk.
pub fn write_docx_to_path(ir: &DocumentIr, output_path: &Path) -> Result<(), String> {
    let bytes = write_docx_to_bytes(ir)?;
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create output directory: {e}"))?;
        }
    }
    let mut file =
        File::create(output_path).map_err(|e| format!("failed to create output file: {e}"))?;
    file.write_all(&bytes)
        .map_err(|e| format!("failed to write docx bytes: {e}"))?;
    Ok(())
}

/// Serialize `DocumentIr` into an in-memory `.docx` byte vector.
pub fn write_docx_to_bytes(ir: &DocumentIr) -> Result<Vec<u8>, String> {
    let mut builder = DocxBuilder::new(ir);
    builder.build()
}

struct MediaAsset {
    rel_id: String,
    filename: String,
    #[allow(dead_code)]
    format: ImageFormatIr,
    data: Vec<u8>,
}

struct HyperlinkRel {
    rel_id: String,
    url: String,
}

struct FootnoteRecord {
    id: usize,
    blocks: Vec<BlockNode>,
}

struct DocxBuilder<'a> {
    ir: &'a DocumentIr,
    next_rel_id: usize,
    next_drawing_id: u32,
    next_bookmark_id: u32,
    media_assets: Vec<MediaAsset>,
    hyperlinks: Vec<HyperlinkRel>,
    footnotes: Vec<FootnoteRecord>,
    header_rel_id: Option<String>,
    footer_rel_id: Option<String>,
}

impl<'a> DocxBuilder<'a> {
    fn new(ir: &'a DocumentIr) -> Self {
        Self {
            ir,
            // Reserve rId1..rId4 for styles, numbering, settings, footnotes
            next_rel_id: 5,
            next_drawing_id: 1,
            next_bookmark_id: 1,
            media_assets: Vec::new(),
            hyperlinks: Vec::new(),
            footnotes: Vec::new(),
            header_rel_id: None,
            footer_rel_id: None,
        }
    }

    fn alloc_rel_id(&mut self) -> String {
        let id = self.next_rel_id;
        self.next_rel_id += 1;
        format!("rId{id}")
    }

    fn register_hyperlink(&mut self, url: &str) -> String {
        if let Some(existing) = self.hyperlinks.iter().find(|h| h.url == url) {
            return existing.rel_id.clone();
        }
        let rel_id = self.alloc_rel_id();
        self.hyperlinks.push(HyperlinkRel {
            rel_id: rel_id.clone(),
            url: url.to_string(),
        });
        rel_id
    }

    fn register_image(&mut self, img: &ImageIr) -> (String, u32) {
        let rel_id = self.alloc_rel_id();
        let idx = self.media_assets.len() + 1;
        let filename = format!("image{}.{}", idx, img.format.extension());
        self.media_assets.push(MediaAsset {
            rel_id: rel_id.clone(),
            filename,
            format: img.format,
            data: img.data.clone(),
        });
        let drawing_id = self.next_drawing_id;
        self.next_drawing_id += 1;
        (rel_id, drawing_id)
    }

    fn build(&mut self) -> Result<Vec<u8>, String> {
        if !self.ir.page_setup.header.is_empty() {
            self.header_rel_id = Some(self.alloc_rel_id());
        }
        if !self.ir.page_setup.footer.is_empty() {
            self.footer_rel_id = Some(self.alloc_rel_id());
        }

        // Render main document body XML first so all media assets, hyperlinks, and footnotes are registered.
        let document_xml = self.render_document_xml();
        let header_xml = if self.header_rel_id.is_some() {
            Some(self.render_header_or_footer_xml(&self.ir.page_setup.header.clone(), "hdr"))
        } else {
            None
        };
        let footer_xml = if self.footer_rel_id.is_some() {
            Some(self.render_header_or_footer_xml(&self.ir.page_setup.footer.clone(), "ftr"))
        } else {
            None
        };
        let footnotes_xml = self.render_footnotes_xml();
        let styles_xml = self.render_styles_xml();
        let numbering_xml = self.render_numbering_xml();
        let settings_xml = self.render_settings_xml();
        let doc_rels_xml = self.render_document_rels_xml();
        let root_rels_xml = Self::render_root_rels_xml();
        let content_types_xml = self.render_content_types_xml();
        let core_props_xml = self.render_core_props_xml();
        let app_props_xml = Self::render_app_props_xml();

        let buf = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);

        Self::zip_write_str(&mut zip, "[Content_Types].xml", &content_types_xml, options)?;
        Self::zip_write_str(&mut zip, "_rels/.rels", &root_rels_xml, options)?;
        Self::zip_write_str(&mut zip, "docProps/core.xml", &core_props_xml, options)?;
        Self::zip_write_str(&mut zip, "docProps/app.xml", &app_props_xml, options)?;
        Self::zip_write_str(&mut zip, "word/document.xml", &document_xml, options)?;
        Self::zip_write_str(
            &mut zip,
            "word/_rels/document.xml.rels",
            &doc_rels_xml,
            options,
        )?;
        Self::zip_write_str(&mut zip, "word/styles.xml", &styles_xml, options)?;
        Self::zip_write_str(&mut zip, "word/numbering.xml", &numbering_xml, options)?;
        Self::zip_write_str(&mut zip, "word/settings.xml", &settings_xml, options)?;
        Self::zip_write_str(&mut zip, "word/footnotes.xml", &footnotes_xml, options)?;

        if let Some(hxml) = &header_xml {
            Self::zip_write_str(&mut zip, "word/header1.xml", hxml, options)?;
        }
        if let Some(fxml) = &footer_xml {
            Self::zip_write_str(&mut zip, "word/footer1.xml", fxml, options)?;
        }

        for asset in &self.media_assets {
            let path = format!("word/media/{}", asset.filename);
            zip.start_file(path, options)
                .map_err(|e| format!("zip start_file error: {e}"))?;
            zip.write_all(&asset.data)
                .map_err(|e| format!("zip write_all error: {e}"))?;
        }

        let cursor = zip
            .finish()
            .map_err(|e| format!("failed to finalize docx zip: {e}"))?;
        Ok(cursor.into_inner())
    }

    fn zip_write_str(
        zip: &mut ZipWriter<Cursor<Vec<u8>>>,
        path: &str,
        content: &str,
        options: SimpleFileOptions,
    ) -> Result<(), String> {
        zip.start_file(path, options)
            .map_err(|e| format!("zip start_file({path}) error: {e}"))?;
        zip.write_all(content.as_bytes())
            .map_err(|e| format!("zip write_all({path}) error: {e}"))?;
        Ok(())
    }

    // =========================================================================
    // Document XML & Block Rendering
    // =========================================================================

    fn xml_namespaces() -> &'static str {
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

    fn render_document_xml(&mut self) -> String {
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

    fn render_header_or_footer_xml(&mut self, blocks: &[BlockNode], tag: &str) -> String {
        let mut out = String::new();
        out.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
        let _ = write!(out, "<w:{tag} {}>", Self::xml_namespaces());
        self.render_blocks(blocks, &mut out);
        let _ = write!(out, "</w:{tag}>");
        out
    }

    fn render_sect_pr(&self, out: &mut String) {
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

    fn render_blocks(&mut self, blocks: &[BlockNode], out: &mut String) {
        for block in blocks {
            self.render_block(block, out);
        }
    }

    fn render_block(&mut self, block: &BlockNode, out: &mut String) {
        match block {
            BlockNode::Title { content, align } => {
                out.push_str("<w:p><w:pPr><w:pStyle w:val=\"Title\"/>");
                Self::render_jc(*align, out);
                out.push_str("</w:pPr>");
                self.render_inlines(content, out);
                out.push_str("</w:p>");
            }
            BlockNode::Heading {
                level,
                numbering,
                content,
                bookmark,
                align,
            } => {
                let style_id = format!("Heading{}", (*level).clamp(1, 6));
                out.push_str("<w:p><w:pPr>");
                let _ = write!(out, r#"<w:pStyle w:val="{style_id}"/>"#);
                if *align != HAlignIr::Left {
                    Self::render_jc(*align, out);
                }
                out.push_str("</w:pPr>");

                let bm_id = bookmark
                    .as_ref()
                    .map(|bm| self.emit_bookmark_start(bm, out));
                if let Some(num_prefix) = numbering {
                    let default_sz = match *level {
                        1 => 15.4,
                        2 => 13.2,
                        _ => 11.0,
                    };
                    let sz_pt = content
                        .iter()
                        .find_map(|inl| {
                            if let InlineNode::Text(r) = inl {
                                r.font_size_pt
                            } else {
                                None
                            }
                        })
                        .unwrap_or(default_sz);
                    let prefix_run = TextRunIr {
                        text: format!("{num_prefix} "),
                        font_family: Some("Libertinus Serif".to_string()),
                        font_size_pt: Some(sz_pt),
                        bold: true,
                        ..Default::default()
                    };
                    self.render_text_run(&prefix_run, out);
                }
                self.render_inlines(content, out);
                if let Some(id) = bm_id {
                    Self::emit_bookmark_end(id, out);
                }
                out.push_str("</w:p>");
            }
            BlockNode::Paragraph {
                content,
                style,
                bookmark,
            } => {
                out.push_str("<w:p><w:pPr>");
                Self::render_jc(style.align, out);
                let before_dxa = pt_to_dxa(style.space_before_pt).max(0);
                let after_dxa = pt_to_dxa(style.space_after_pt).max(0);
                if let Some(ls_pt) = style.line_spacing_pt {
                    let line_dxa = (pt_to_dxa(ls_pt) + 240).max(240);
                    let _ = write!(
                        out,
                        r#"<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/>"#
                    );
                } else if before_dxa > 0 || after_dxa > 0 {
                    let _ = write!(
                        out,
                        r#"<w:spacing w:before="{before_dxa}" w:after="{after_dxa}"/>"#
                    );
                }
                let left_dxa = pt_to_dxa(style.left_indent_pt).max(0);
                let right_dxa = pt_to_dxa(style.right_indent_pt).max(0);
                let first_dxa = pt_to_dxa(style.first_line_indent_pt).max(0);
                let hang_dxa = pt_to_dxa(style.hanging_indent_pt).max(0);
                if left_dxa > 0 || right_dxa > 0 || first_dxa > 0 || hang_dxa > 0 {
                    let mut ind = format!(r#"<w:ind w:left="{left_dxa}" w:right="{right_dxa}""#);
                    if hang_dxa > 0 {
                        let _ = write!(ind, r#" w:hanging="{hang_dxa}""#);
                    } else if first_dxa > 0 {
                        let _ = write!(ind, r#" w:firstLine="{first_dxa}""#);
                    }
                    ind.push_str("/>");
                    out.push_str(&ind);
                }
                out.push_str("</w:pPr>");
                let bm_id = bookmark
                    .as_ref()
                    .map(|bm| self.emit_bookmark_start(bm, out));
                self.render_inlines(content, out);
                if let Some(id) = bm_id {
                    Self::emit_bookmark_end(id, out);
                }
                out.push_str("</w:p>");
            }
            BlockNode::List {
                items,
                tight,
                level,
            } => {
                let ilvl = (*level).min(8);
                for item in items {
                    self.render_list_or_enum_item_blocks(
                        &item.blocks,
                        1, // numId 1 = bullet list
                        ilvl,
                        None,
                        *tight,
                        out,
                    );
                }
            }
            BlockNode::Enum {
                items,
                tight,
                level,
            } => {
                let ilvl = (*level).min(8);
                for item in items {
                    let custom_prefix = if Self::is_standard_decimal_marker(&item.marker_text) {
                        None
                    } else {
                        Some(item.marker_text.as_str())
                    };
                    self.render_list_or_enum_item_blocks(
                        &item.blocks,
                        2, // numId 2 = decimal ordered list
                        ilvl,
                        custom_prefix,
                        *tight,
                        out,
                    );
                }
            }
            BlockNode::Terms { items, tight } => {
                let after = if *tight { 60 } else { 140 };
                for item in items {
                    out.push_str("<w:p><w:pPr>");
                    let _ = write!(
                        out,
                        r#"<w:spacing w:before="40" w:after="{after}"/><w:ind w:left="360" w:hanging="360"/>"#
                    );
                    out.push_str("</w:pPr>");
                    // Render term runs in bold followed by Typst's default 0.6em space separator and description.
                    for inline in &item.term {
                        if let InlineNode::Text(r) = inline {
                            let mut bold_run = r.clone();
                            bold_run.bold = true;
                            self.render_text_run(&bold_run, out);
                        } else {
                            self.render_inline(inline, out);
                        }
                    }
                    let sep_run = TextRunIr {
                        text: "  ".to_string(),
                        font_family: Some("Libertinus Serif".to_string()),
                        font_size_pt: Some(11.0),
                        ..Default::default()
                    };
                    self.render_text_run(&sep_run, out);
                    if let Some(BlockNode::Paragraph { content, .. }) = item.description.first() {
                        self.render_inlines(content, out);
                        out.push_str("</w:p>");
                        for rest in item.description.iter().skip(1) {
                            self.render_block(rest, out);
                        }
                    } else {
                        out.push_str("</w:p>");
                        self.render_blocks(&item.description, out);
                    }
                }
            }
            BlockNode::Table(table_ir) => {
                self.render_table(table_ir, out);
            }
            BlockNode::Quote { body, attribution } => {
                self.render_quote(body, attribution.as_deref(), out);
            }
            BlockNode::CodeBlock {
                lang,
                lines,
                fill,
                stroke,
            } => {
                self.render_code_block(
                    lang.as_deref(),
                    lines,
                    fill.as_deref(),
                    stroke.as_ref(),
                    out,
                );
            }
            BlockNode::Figure {
                body,
                caption,
                caption_top,
                kind: _,
                bookmark,
            } => {
                if *caption_top {
                    if let Some(cap) = caption {
                        self.render_caption_paragraph(cap, bookmark.as_deref(), out);
                    }
                    self.render_blocks(body, out);
                } else {
                    self.render_blocks(body, out);
                    if let Some(cap) = caption {
                        self.render_caption_paragraph(cap, bookmark.as_deref(), out);
                    }
                }
            }
            BlockNode::EquationBlock(eq) => {
                self.render_equation_block(eq, out);
            }
            BlockNode::ShapeBlock(shape) => {
                self.render_shape_block(shape, out);
            }
            BlockNode::CetzCanvas(canvas) => {
                self.render_cetz_block(canvas, out);
            }
            BlockNode::ImageBlock(img) => {
                out.push_str("<w:p><w:pPr>");
                Self::render_jc(img.align, out);
                out.push_str("<w:spacing w:before=\"120\" w:after=\"120\"/></w:pPr>");
                self.render_image_run(img, out);
                out.push_str("</w:p>");
            }
            BlockNode::Outline { title, entries } => {
                self.render_outline(title.as_deref(), entries, out);
            }
            BlockNode::Bibliography { title, entries } => {
                self.render_bibliography(title.as_deref(), entries, out);
            }
            BlockNode::HorizontalRule {
                length_pt: _,
                stroke,
                align: _,
            } => {
                let sz = ((stroke.thickness_pt * 8.0).round() as u32).clamp(2, 96);
                let val = Self::dash_to_w_border_val(&stroke.dash);
                let _ = write!(
                    out,
                    r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="{val}" w:sz="{sz}" w:space="1" w:color="{}"/></w:pBdr><w:spacing w:before="120" w:after="120"/></w:pPr></w:p>"#,
                    stroke.color
                );
            }
            BlockNode::PageBreak => {
                out.push_str(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#);
            }
            BlockNode::ColumnBreak => {
                out.push_str(r#"<w:p><w:r><w:br w:type="column"/></w:r></w:p>"#);
            }
            BlockNode::UnsupportedBlock(unsup) => {
                self.render_unsupported_block(unsup, out);
            }
        }
    }

    fn render_list_or_enum_item_blocks(
        &mut self,
        blocks: &[BlockNode],
        num_id: usize,
        ilvl: usize,
        custom_prefix: Option<&str>,
        tight: bool,
        out: &mut String,
    ) {
        let after_dxa = if tight { 40 } else { 120 };
        let indent_left = 360 * (ilvl + 1);
        if blocks.is_empty() {
            let _ = write!(
                out,
                r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{ilvl}"/><w:numId w:val="{num_id}"/></w:numPr><w:spacing w:after="{after_dxa}"/></w:pPr></w:p>"#
            );
            return;
        }

        for (idx, b) in blocks.iter().enumerate() {
            match b {
                BlockNode::Paragraph {
                    content, bookmark, ..
                } if idx == 0 => {
                    out.push_str("<w:p><w:pPr>");
                    if let Some(prefix) = custom_prefix {
                        let _ = write!(
                            out,
                            r#"<w:spacing w:after="{after_dxa}"/><w:ind w:left="{indent_left}" w:hanging="280"/></w:pPr>"#
                        );
                        let bm_id = bookmark
                            .as_ref()
                            .map(|bm| self.emit_bookmark_start(bm, out));
                        let _ = write!(
                            out,
                            r#"<w:r><w:t xml:space="preserve">{} </w:t></w:r>"#,
                            escape_xml(prefix)
                        );
                        self.render_inlines(content, out);
                        if let Some(id) = bm_id {
                            Self::emit_bookmark_end(id, out);
                        }
                        out.push_str("</w:p>");
                    } else {
                        let _ = write!(
                            out,
                            r#"<w:numPr><w:ilvl w:val="{ilvl}"/><w:numId w:val="{num_id}"/></w:numPr><w:spacing w:after="{after_dxa}"/></w:pPr>"#
                        );
                        let bm_id = bookmark
                            .as_ref()
                            .map(|bm| self.emit_bookmark_start(bm, out));
                        self.render_inlines(content, out);
                        if let Some(id) = bm_id {
                            Self::emit_bookmark_end(id, out);
                        }
                        out.push_str("</w:p>");
                    }
                }
                other => {
                    self.render_block(other, out);
                }
            }
        }
    }

    fn is_standard_decimal_marker(marker: &str) -> bool {
        let trimmed = marker.trim();
        trimmed.ends_with('.')
            && trimmed[..trimmed.len() - 1]
                .chars()
                .all(|c| c.is_ascii_digit())
    }

    // =========================================================================
    // Table & Grid Rendering
    // =========================================================================

    fn render_table(&mut self, table: &TableIr, out: &mut String) {
        out.push_str("<w:tbl><w:tblPr>");
        Self::render_table_jc(table.align, out);
        let total_w_dxa: i64 = table.col_widths_pt.iter().map(|&w| pt_to_dxa(w)).sum();
        if total_w_dxa > 0 {
            let _ = write!(out, r#"<w:tblW w:w="{total_w_dxa}" w:type="dxa"/>"#);
        } else {
            out.push_str(r#"<w:tblW w:w="5000" w:type="pct"/>"#);
        }
        if let Some(summary) = &table.summary {
            let _ = write!(
                out,
                r#"<w:tblCaption w:val="{}"/><w:tblDescription w:val="{}"/>"#,
                escape_xml(summary),
                escape_xml(summary)
            );
        }
        out.push_str(r#"<w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="0" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/>"#);
        out.push_str("</w:tblPr>");

        // `<w:tblGrid>`
        out.push_str("<w:tblGrid>");
        for &col_w in &table.col_widths_pt {
            let dxa = pt_to_dxa(col_w).max(144);
            let _ = write!(out, r#"<w:gridCol w:w="{dxa}"/>"#);
        }
        out.push_str("</w:tblGrid>");

        // Rows
        for row in &table.rows {
            out.push_str("<w:tr><w:trPr>");
            if row.is_header {
                out.push_str("<w:tblHeader/>");
            }
            if let Some(rh) = row.height_pt {
                let h_dxa = pt_to_dxa(rh);
                if h_dxa > 40 {
                    let _ = write!(out, r#"<w:trHeight w:val="{h_dxa}" w:hRule="atLeast"/>"#);
                }
            }
            out.push_str("</w:trPr>");

            for cell in &row.cells {
                if cell.hmerged_covered {
                    continue;
                }
                out.push_str("<w:tc><w:tcPr>");
                let span_cols = cell.colspan.max(1);
                let cell_w_pt: f64 = table
                    .col_widths_pt
                    .iter()
                    .skip(cell.x)
                    .take(span_cols)
                    .sum();
                let cell_w_dxa = pt_to_dxa(cell_w_pt).max(144);
                let _ = write!(out, r#"<w:tcW w:w="{cell_w_dxa}" w:type="dxa"/>"#);

                if cell.colspan > 1 {
                    let _ = write!(out, r#"<w:gridSpan w:val="{}"/>"#, cell.colspan);
                }
                match cell.vmerge {
                    Some(VMergeState::Restart) => {
                        out.push_str(r#"<w:vMerge w:val="restart"/>"#);
                    }
                    Some(VMergeState::Continue) => {
                        out.push_str("<w:vMerge/>");
                    }
                    None => {}
                }

                // Cell borders
                out.push_str("<w:tcBorders>");
                Self::render_tc_border("top", cell.borders.top.as_ref(), out);
                Self::render_tc_border("left", cell.borders.left.as_ref(), out);
                Self::render_tc_border("bottom", cell.borders.bottom.as_ref(), out);
                Self::render_tc_border("right", cell.borders.right.as_ref(), out);
                out.push_str("</w:tcBorders>");

                // Cell shading / background fill
                if let Some(fill_hex) = &cell.fill {
                    let _ = write!(
                        out,
                        r#"<w:shd w:val="clear" w:color="auto" w:fill="{fill_hex}"/>"#
                    );
                }

                // Cell padding / margins
                let _ = write!(
                    out,
                    r#"<w:tcMar><w:top w:w="{}" w:type="dxa"/><w:left w:w="{}" w:type="dxa"/><w:bottom w:w="{}" w:type="dxa"/><w:right w:w="{}" w:type="dxa"/></w:tcMar>"#,
                    pt_to_dxa(cell.inset.top),
                    pt_to_dxa(cell.inset.left),
                    pt_to_dxa(cell.inset.bottom),
                    pt_to_dxa(cell.inset.right),
                );

                // Vertical alignment
                let valign_str = match cell.valign {
                    VAlignIr::Top => "top",
                    VAlignIr::Center => "center",
                    VAlignIr::Bottom => "bottom",
                };
                let _ = write!(out, r#"<w:vAlign w:val="{valign_str}"/>"#);
                out.push_str("</w:tcPr>");

                // Cell body blocks (Word requires at least one `<w:p>` inside every `<w:tc>`!)
                if cell.blocks.is_empty() || cell.vmerge == Some(VMergeState::Continue) {
                    out.push_str("<w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr></w:p>");
                } else {
                    let mut cell_xml = String::new();
                    self.render_blocks(&cell.blocks, &mut cell_xml);
                    if !cell_xml.contains("<w:p") {
                        cell_xml.push_str("<w:p/>");
                    }
                    out.push_str(&cell_xml);
                }

                out.push_str("</w:tc>");
            }

            out.push_str("</w:tr>");
        }

        out.push_str("</w:tbl>");
        // Add a small spacing paragraph after a table so consecutive tables don't merge in Word.
        out.push_str("<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"60\"/></w:pPr></w:p>");
    }

    fn render_tc_border(side: &str, stroke: Option<&StrokeIr>, out: &mut String) {
        if let Some(s) = stroke {
            let sz = ((s.thickness_pt * 8.0).round() as u32).clamp(2, 96);
            let val = Self::dash_to_w_border_val(&s.dash);
            let _ = write!(
                out,
                r#"<w:{side} w:val="{val}" w:sz="{sz}" w:space="0" w:color="{}"/>"#,
                s.color
            );
        } else {
            let _ = write!(
                out,
                r#"<w:{side} w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#
            );
        }
    }

    fn dash_to_w_border_val(dash: &DashPatternIr) -> &'static str {
        match dash {
            DashPatternIr::Solid => "single",
            DashPatternIr::Dotted => "dotted",
            DashPatternIr::Dashed | DashPatternIr::Custom => "dashed",
            DashPatternIr::DashDot => "dotDash",
            DashPatternIr::DashDotDot => "dotDotDash",
        }
    }

    // =========================================================================
    // Blockquotes, Code Blocks, Figures, Equations, Shapes, CeTZ
    // =========================================================================

    fn render_quote(
        &mut self,
        body: &[BlockNode],
        attribution: Option<&[InlineNode]>,
        out: &mut String,
    ) {
        let after_body = if attribution.is_some() { 40 } else { 100 };
        for block in body {
            if let BlockNode::Paragraph { content, .. } = block {
                let _ = write!(
                    out,
                    r#"<w:p><w:pPr><w:pStyle w:val="Quote"/><w:ind w:left="440" w:right="440"/><w:spacing w:before="80" w:after="{after_body}"/></w:pPr>"#,
                );
                self.render_inlines(content, out);
                out.push_str("</w:p>");
            } else {
                self.render_block(block, out);
            }
        }
        if let Some(attr) = attribution {
            out.push_str(
                r#"<w:p><w:pPr><w:pStyle w:val="Quote"/><w:ind w:left="440" w:right="440"/><w:jc w:val="right"/><w:spacing w:before="20" w:after="120"/></w:pPr>"#,
            );
            let plain_attr = Self::plain_text_of_inlines(attr);
            let trimmed_attr = plain_attr.trim_start();
            if !trimmed_attr.starts_with('—') && !trimmed_attr.starts_with('-') {
                let dash_run = TextRunIr {
                    text: "— ".to_string(),
                    font_family: Some("Libertinus Serif".to_string()),
                    font_size_pt: Some(11.0),
                    ..Default::default()
                };
                self.render_text_run(&dash_run, out);
            }
            self.render_inlines(attr, out);
            out.push_str("</w:p>");
        }
    }

    fn render_code_block(
        &self,
        _lang: Option<&str>,
        lines: &[Vec<TextRunIr>],
        fill: Option<&str>,
        stroke: Option<&StrokeIr>,
        out: &mut String,
    ) {
        if fill.is_none() && stroke.is_none() {
            // Standard unboxed Typst `raw(block: true)`: render lines directly with exact syntax colors.
            let num_lines = lines.len();
            for (idx, line_runs) in lines.iter().enumerate() {
                let before = if idx == 0 { 80 } else { 0 };
                let after = if idx + 1 == num_lines { 100 } else { 15 };
                let _ = write!(
                    out,
                    r#"<w:p><w:pPr><w:spacing w:before="{before}" w:after="{after}" w:line="240" w:lineRule="auto"/></w:pPr>"#,
                );
                for run in line_runs {
                    let mut code_run = run.clone();
                    code_run.is_code = true;
                    if code_run.font_family.is_none() {
                        code_run.font_family = Some("Consolas".to_string());
                    }
                    self.render_text_run(&code_run, out);
                }
                out.push_str("</w:p>");
            }
            return;
        }

        let bg = fill.unwrap_or("F6F8FA");
        let bdr_color = stroke.map(|s| s.color.as_str()).unwrap_or("D0D7DE");
        // Render as a 1-cell full-width shaded code box when fill or stroke is set.
        let _ = write!(
            out,
            r#"<w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/><w:tblBorders><w:top w:val="single" w:sz="6" w:space="0" w:color="{bdr_color}"/><w:left w:val="single" w:sz="6" w:space="0" w:color="{bdr_color}"/><w:bottom w:val="single" w:sz="6" w:space="0" w:color="{bdr_color}"/><w:right w:val="single" w:sz="6" w:space="0" w:color="{bdr_color}"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="9000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="5000" w:type="pct"/><w:shd w:val="clear" w:color="auto" w:fill="{bg}"/><w:tcMar><w:top w:w="120" w:type="dxa"/><w:left w:w="160" w:type="dxa"/><w:bottom w:w="120" w:type="dxa"/><w:right w:w="160" w:type="dxa"/></w:tcMar></w:tcPr>"#
        );

        if lines.is_empty() {
            out.push_str("<w:p/>");
        } else {
            for line_runs in lines {
                out.push_str(
                    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="20" w:line="240" w:lineRule="auto"/></w:pPr>"#,
                );
                for run in line_runs {
                    let mut code_run = run.clone();
                    code_run.is_code = true;
                    if code_run.font_family.is_none() {
                        code_run.font_family = Some("Consolas".to_string());
                    }
                    self.render_text_run(&code_run, out);
                }
                out.push_str("</w:p>");
            }
        }

        out.push_str("</w:tc></w:tr></w:tbl>");
        out.push_str("<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"60\"/></w:pPr></w:p>");
    }

    fn render_caption_paragraph(
        &mut self,
        caption: &[InlineNode],
        bookmark: Option<&str>,
        out: &mut String,
    ) {
        out.push_str(
            r#"<w:p><w:pPr><w:pStyle w:val="Caption"/><w:jc w:val="center"/><w:spacing w:before="60" w:after="140"/></w:pPr>"#,
        );
        let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
        self.render_inlines(caption, out);
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }

    fn render_equation_block(&mut self, eq: &EquationIr, out: &mut String) {
        out.push_str(
            r#"<w:p><w:pPr><w:jc w:val="center"/><w:spacing w:before="120" w:after="120"/></w:pPr>"#,
        );
        let bm_id = eq
            .bookmark
            .as_ref()
            .map(|bm| self.emit_bookmark_start(bm, out));
        out.push_str("<m:oMathPara><m:oMathParaPr><m:jc m:val=\"center\"/></m:oMathParaPr>");
        out.push_str(&eq.omml_xml);
        out.push_str("</m:oMathPara>");
        if let Some(num) = &eq.numbering_text {
            let _ = write!(
                out,
                r#"<w:r><w:t xml:space="preserve">    {}</w:t></w:r>"#,
                escape_xml(num)
            );
        }
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }

    fn render_shape_block(&mut self, shape: &ShapeIr, out: &mut String) {
        let run_xml = shape_to_drawingml_run(shape);
        out.push_str("<w:p><w:pPr>");
        Self::render_jc(shape.align, out);
        out.push_str("<w:spacing w:before=\"80\" w:after=\"80\"/></w:pPr>");
        out.push_str(&run_xml);
        out.push_str("</w:p>");
    }

    fn render_cetz_block(&mut self, canvas: &CetzCanvasIr, out: &mut String) {
        let run_xml = cetz_canvas_to_drawingml_run(canvas);
        out.push_str("<w:p><w:pPr>");
        Self::render_jc(canvas.align, out);
        out.push_str("<w:spacing w:before=\"120\" w:after=\"120\"/></w:pPr>");
        out.push_str(&run_xml);
        out.push_str("</w:p>");
    }

    fn render_outline(
        &mut self,
        title: Option<&[InlineNode]>,
        entries: &[OutlineEntryIr],
        out: &mut String,
    ) {
        if let Some(t) = title {
            out.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr>"#);
            self.render_inlines(t, out);
            out.push_str("</w:p>");
        }
        let content_w_pt = (self.ir.page_setup.width_pt
            - self.ir.page_setup.margin_left_pt
            - self.ir.page_setup.margin_right_pt)
            .max(360.0);
        let tab_pos = pt_to_dxa(content_w_pt);
        for entry in entries {
            let indent = (entry.level.saturating_sub(1)) * 280;
            let _ = write!(
                out,
                r#"<w:p><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="{tab_pos}"/></w:tabs><w:ind w:left="{indent}"/><w:spacing w:before="30" w:after="30"/></w:pPr>"#
            );
            let page_run = TextRunIr {
                text: entry.page.clone(),
                font_family: Some("Libertinus Serif".to_string()),
                font_size_pt: Some(11.0),
                ..Default::default()
            };
            if let Some(bm) = &entry.target_bookmark {
                let _ = write!(
                    out,
                    r#"<w:hyperlink w:anchor="{}" w:history="1">"#,
                    escape_xml(bm)
                );
                self.render_inlines(&entry.title, out);
                out.push_str(r#"<w:r><w:tab/></w:r>"#);
                self.render_text_run(&page_run, out);
                out.push_str("</w:hyperlink>");
            } else {
                self.render_inlines(&entry.title, out);
                out.push_str(r#"<w:r><w:tab/></w:r>"#);
                self.render_text_run(&page_run, out);
            }
            out.push_str("</w:p>");
        }
    }

    fn render_bibliography(
        &mut self,
        title: Option<&[InlineNode]>,
        entries: &[BibEntryIr],
        out: &mut String,
    ) {
        if let Some(t) = title {
            out.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr>"#);
            self.render_inlines(t, out);
            out.push_str("</w:p>");
        }
        for entry in entries {
            out.push_str(
                r#"<w:p><w:pPr><w:ind w:left="480" w:hanging="480"/><w:spacing w:before="40" w:after="80"/></w:pPr>"#,
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

    fn render_unsupported_block(&self, unsup: &UnsupportedIr, out: &mut String) {
        let badge = unsup.display_badge();
        let _ = write!(
            out,
            r#"<w:p><w:pPr><w:pBdr><w:top w:val="dashed" w:sz="6" w:space="4" w:color="D32F2F"/><w:left w:val="dashed" w:sz="6" w:space="6" w:color="D32F2F"/><w:bottom w:val="dashed" w:sz="6" w:space="4" w:color="D32F2F"/><w:right w:val="dashed" w:sz="6" w:space="6" w:color="D32F2F"/></w:pBdr><w:shd w:val="clear" w:color="auto" w:fill="FFF5F5"/><w:spacing w:before="80" w:after="80"/></w:pPr><w:r><w:rPr><w:b/><w:color w:val="C62828"/></w:rPr><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
            escape_xml(&badge)
        );
    }

    // =========================================================================
    // Inline Rendering
    // =========================================================================

    fn render_inlines(&mut self, inlines: &[InlineNode], out: &mut String) {
        for inline in inlines {
            self.render_inline(inline, out);
        }
    }

    fn render_inline(&mut self, inline: &InlineNode, out: &mut String) {
        match inline {
            InlineNode::Text(run) => {
                self.render_text_run(run, out);
            }
            InlineNode::EquationInline(eq) => {
                out.push_str(&eq.omml_xml);
            }
            InlineNode::ShapeInline(shape) => {
                let run_xml = shape_to_drawingml_run(shape);
                out.push_str(&run_xml);
            }
            InlineNode::CetzInline(canvas) => {
                let run_xml = cetz_canvas_to_drawingml_run(canvas);
                out.push_str(&run_xml);
            }
            InlineNode::ImageInline(img) => {
                self.render_image_run(img, out);
            }
            InlineNode::Link {
                url,
                anchor,
                content,
            } => {
                if let Some(u) = url {
                    let r_id = self.register_hyperlink(u);
                    let _ = write!(out, r#"<w:hyperlink r:id="{r_id}" w:history="1">"#);
                    self.render_link_styled_inlines(content, out);
                    out.push_str("</w:hyperlink>");
                } else if let Some(a) = anchor {
                    let _ = write!(
                        out,
                        r#"<w:hyperlink w:anchor="{}" w:history="1">"#,
                        escape_xml(a)
                    );
                    self.render_link_styled_inlines(content, out);
                    out.push_str("</w:hyperlink>");
                } else {
                    self.render_inlines(content, out);
                }
            }
            InlineNode::Ref { target, content } => {
                let _ = write!(
                    out,
                    r#"<w:hyperlink w:anchor="{}" w:history="1">"#,
                    escape_xml(target)
                );
                self.render_inlines(content, out);
                out.push_str("</w:hyperlink>");
            }
            InlineNode::Citation { keys: _, content } => {
                self.render_inlines(content, out);
            }
            InlineNode::Footnote {
                id,
                number_text: _,
                content,
            } => {
                if !self.footnotes.iter().any(|f| f.id == *id) {
                    self.footnotes.push(FootnoteRecord {
                        id: *id,
                        blocks: content.clone(),
                    });
                }
                let _ = write!(
                    out,
                    r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/><w:vertAlign w:val="superscript"/></w:rPr><w:footnoteReference w:id="{id}"/></w:r>"#
                );
            }
            InlineNode::LineBreak => {
                out.push_str("<w:r><w:br/></w:r>");
            }
            InlineNode::Tab => {
                out.push_str("<w:r><w:tab/></w:r>");
            }
            InlineNode::HorizontalSpace { width_pt } => {
                let spaces = ((width_pt / 3.5).round() as usize).clamp(1, 8);
                let s = " ".repeat(spaces);
                let _ = write!(out, r#"<w:r><w:t xml:space="preserve">{s}</w:t></w:r>"#);
            }
            InlineNode::UnsupportedInline(unsup) => {
                let badge = unsup.display_badge();
                let _ = write!(
                    out,
                    r#"<w:r><w:rPr><w:b/><w:color w:val="C62828"/><w:shd w:val="clear" w:color="auto" w:fill="FFEBEE"/></w:rPr><w:t xml:space="preserve">{}</w:t></w:r>"#,
                    escape_xml(&badge)
                );
            }
        }
    }

    fn render_link_styled_inlines(&mut self, inlines: &[InlineNode], out: &mut String) {
        for inline in inlines {
            if let InlineNode::Text(run) = inline {
                let mut styled = run.clone();
                if styled.color.is_none() {
                    styled.color = Some("0563C1".to_string());
                }
                if styled.underline.is_none() {
                    styled.underline = Some(StrokeIr {
                        color: "0563C1".to_string(),
                        thickness_pt: 0.75,
                        ..Default::default()
                    });
                }
                self.render_text_run(&styled, out);
            } else {
                self.render_inline(inline, out);
            }
        }
    }

    fn render_text_run(&self, run: &TextRunIr, out: &mut String) {
        if run.text.is_empty() {
            return;
        }
        out.push_str("<w:r><w:rPr>");
        if let Some(fam) = &run.font_family {
            let esc = escape_xml(fam);
            let ea_esc = if fam == "Libertinus Serif" || fam == "New Computer Modern" {
                "Heiti SC".to_string()
            } else {
                esc.clone()
            };
            let _ = write!(
                out,
                r#"<w:rFonts w:ascii="{esc}" w:hAnsi="{esc}" w:eastAsia="{ea_esc}" w:cs="{esc}"/>"#
            );
        }
        if run.bold {
            out.push_str("<w:b/><w:bCs/>");
        }
        if run.italic {
            out.push_str("<w:i/><w:iCs/>");
        }
        if run.smallcaps {
            out.push_str("<w:smallCaps/>");
        }
        if let Some(ul) = &run.underline {
            let u_val = match ul.dash {
                DashPatternIr::Dotted => "dotted",
                DashPatternIr::Dashed | DashPatternIr::Custom => "dash",
                DashPatternIr::DashDot => "dotDash",
                DashPatternIr::DashDotDot => "dotDotDash",
                DashPatternIr::Solid => "single",
            };
            let _ = write!(out, r#"<w:u w:val="{u_val}" w:color="{}"/>"#, ul.color);
        }
        if run.strike.is_some() {
            out.push_str("<w:strike/>");
        }
        if let Some(color) = &run.color {
            let _ = write!(out, r#"<w:color w:val="{color}"/>"#);
        }
        if let Some(hl) = &run.highlight {
            let _ = write!(
                out,
                r#"<w:shd w:val="clear" w:color="auto" w:fill="{hl}"/>"#
            );
        }
        if let Some(sz_pt) = run.font_size_pt {
            let half_pts = ((sz_pt * 2.0).round() as u32).clamp(4, 288);
            let _ = write!(
                out,
                r#"<w:sz w:val="{half_pts}"/><w:szCs w:val="{half_pts}"/>"#
            );
        }
        match run.script {
            ScriptKindIr::Sub => out.push_str(r#"<w:vertAlign w:val="subscript"/>"#),
            ScriptKindIr::Super => out.push_str(r#"<w:vertAlign w:val="superscript"/>"#),
            ScriptKindIr::Normal => {}
        }
        out.push_str("</w:rPr><w:t xml:space=\"preserve\">");
        out.push_str(&escape_xml(&run.text));
        out.push_str("</w:t></w:r>");
    }

    fn render_image_run(&mut self, img: &ImageIr, out: &mut String) {
        let (rel_id, drawing_id) = self.register_image(img);
        let cx = pt_to_emu(img.width_pt).max(9525);
        let cy = pt_to_emu(img.height_pt).max(9525);
        let descr = escape_xml(img.alt.as_deref().unwrap_or("Image"));

        let blip_xml = if img.format == ImageFormatIr::Svg {
            format!(
                r#"<a:blip><a:extLst><a:ext uri="{{96DAC541-7B7A-43D3-8B79-37D633B846F1}}"><asvg:svgBlip r:embed="{rel_id}"/></a:ext></a:extLst></a:blip>"#
            )
        } else {
            format!(r#"<a:blip r:embed="{rel_id}"/>"#)
        };

        let _ = write!(
            out,
            concat!(
                r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">"#,
                r#"<wp:extent cx="{cx}" cy="{cy}"/>"#,
                r#"<wp:effectExtent l="0" t="0" r="0" b="0"/>"#,
                r#"<wp:docPr id="{drawing_id}" name="Picture {drawing_id}" descr="{descr}"/>"#,
                r#"<wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr>"#,
                r#"<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
                r#"<pic:pic>"#,
                r#"<pic:nvPicPr><pic:cNvPr id="{drawing_id}" name="Picture {drawing_id}" descr="{descr}"/><pic:cNvPicPr/></pic:nvPicPr>"#,
                r#"<pic:blipFill>{blip_xml}<a:stretch><a:fillRect/></a:stretch></pic:blipFill>"#,
                r#"<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr>"#,
                r#"</pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
            ),
            cx = cx,
            cy = cy,
            drawing_id = drawing_id,
            descr = descr,
            blip_xml = blip_xml,
        );
    }

    fn emit_bookmark_start(&mut self, name: &str, out: &mut String) -> u32 {
        let id = self.next_bookmark_id;
        self.next_bookmark_id += 1;
        let _ = write!(
            out,
            r#"<w:bookmarkStart w:id="{id}" w:name="{}"/>"#,
            escape_xml(name)
        );
        id
    }

    fn emit_bookmark_end(id: u32, out: &mut String) {
        let _ = write!(out, r#"<w:bookmarkEnd w:id="{id}"/>"#);
    }

    fn render_jc(align: HAlignIr, out: &mut String) {
        let val = match align {
            HAlignIr::Left => "left",
            HAlignIr::Center => "center",
            HAlignIr::Right => "right",
            HAlignIr::Justify => "both",
        };
        let _ = write!(out, r#"<w:jc w:val="{val}"/>"#);
    }

    fn render_table_jc(align: HAlignIr, out: &mut String) {
        let val = match align {
            HAlignIr::Left | HAlignIr::Justify => "left",
            HAlignIr::Center => "center",
            HAlignIr::Right => "right",
        };
        let _ = write!(out, r#"<w:jc w:val="{val}"/>"#);
    }

    fn plain_text_of_inlines(inlines: &[InlineNode]) -> String {
        let mut s = String::new();
        for item in inlines {
            if let InlineNode::Text(r) = item {
                s.push_str(&r.text);
            }
        }
        s
    }

    // =========================================================================
    // Supporting OpenXML Package Parts (`footnotes.xml`, `styles.xml`,
    // `numbering.xml`, `settings.xml`, `.rels`, `[Content_Types].xml`, `core.xml`)
    // =========================================================================

    fn render_footnotes_xml(&mut self) -> String {
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

    fn render_styles_xml(&self) -> String {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:ascii="Libertinus Serif" w:hAnsi="Libertinus Serif" w:eastAsia="Heiti SC" w:cs="Libertinus Serif"/>
        <w:sz w:val="22"/>
        <w:szCs w:val="22"/>
      </w:rPr>
    </w:rPrDefault>
    <w:pPrDefault>
      <w:pPr>
        <w:spacing w:after="120" w:line="276" w:lineRule="auto"/>
      </w:pPr>
    </w:pPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:qFormat/>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Title">
    <w:name w:val="Title"/>
    <w:basedOn w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:spacing w:before="120" w:after="200"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="44"/><w:szCs w:val="44"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:basedOn w:val="Normal"/>
    <w:next w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:keepNext/><w:spacing w:before="280" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="32"/><w:szCs w:val="32"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading2">
    <w:name w:val="heading 2"/>
    <w:basedOn w:val="Normal"/>
    <w:next w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:keepNext/><w:spacing w:before="240" w:after="100"/><w:outlineLvl w:val="1"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading3">
    <w:name w:val="heading 3"/>
    <w:basedOn w:val="Normal"/>
    <w:next w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:keepNext/><w:spacing w:before="200" w:after="80"/><w:outlineLvl w:val="2"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="24"/><w:szCs w:val="24"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading4">
    <w:name w:val="heading 4"/>
    <w:basedOn w:val="Normal"/>
    <w:next w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:keepNext/><w:spacing w:before="160" w:after="60"/><w:outlineLvl w:val="3"/></w:pPr>
    <w:rPr><w:b/><w:i/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading5">
    <w:name w:val="heading 5"/>
    <w:basedOn w:val="Normal"/>
    <w:next w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:keepNext/><w:spacing w:before="140" w:after="60"/><w:outlineLvl w:val="4"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading6">
    <w:name w:val="heading 6"/>
    <w:basedOn w:val="Normal"/>
    <w:next w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:keepNext/><w:spacing w:before="120" w:after="60"/><w:outlineLvl w:val="5"/></w:pPr>
    <w:rPr><w:i/><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Quote">
    <w:name w:val="Quote"/>
    <w:basedOn w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:ind w:left="440" w:right="440"/><w:spacing w:before="80" w:after="80"/></w:pPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Caption">
    <w:name w:val="caption"/>
    <w:basedOn w:val="Normal"/>
    <w:qFormat/>
    <w:rPr><w:sz w:val="20"/><w:szCs w:val="20"/><w:color w:val="475569"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="FootnoteText">
    <w:name w:val="footnote text"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:spacing w:after="40" w:line="240" w:lineRule="auto"/></w:pPr>
    <w:rPr><w:sz w:val="18"/><w:szCs w:val="18"/></w:rPr>
  </w:style>
  <w:style w:type="character" w:styleId="FootnoteReference">
    <w:name w:val="footnote reference"/>
    <w:rPr><w:vertAlign w:val="superscript"/></w:rPr>
  </w:style>
</w:styles>"#
            .to_string()
    }

    fn render_numbering_xml(&self) -> String {
        let mut out = String::new();
        out.push_str(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">"#,
        );
        let bullets = ["•", "◦", "▪", "•", "◦", "▪", "•", "◦", "▪"];
        for (lvl, b) in bullets.iter().enumerate() {
            let left = (lvl + 1) * 360;
            let _ = write!(
                out,
                r#"<w:lvl w:ilvl="{lvl}"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="{b}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{left}" w:hanging="240"/></w:pPr></w:lvl>"#
            );
        }
        out.push_str("</w:abstractNum><w:abstractNum w:abstractNumId=\"1\">");
        for lvl in 0..9 {
            let left = (lvl + 1) * 360;
            let fmt = match lvl % 3 {
                0 => "decimal",
                1 => "lowerLetter",
                _ => "lowerRoman",
            };
            let lvl_txt = format!("%{}.", lvl + 1);
            let _ = write!(
                out,
                r#"<w:lvl w:ilvl="{lvl}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/><w:lvlText w:val="{lvl_txt}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{left}" w:hanging="280"/></w:pPr></w:lvl>"#
            );
        }
        out.push_str(
            r#"</w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
  <w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#,
        );
        out
    }

    fn render_settings_xml(&self) -> String {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
  <w:zoom w:percent="100"/>
  <w:defaultTabStop w:val="720"/>
  <m:mathPr>
    <m:mathFont m:val="Cambria Math"/>
    <m:brkBin m:val="before"/>
    <m:brkBinSub m:val="--"/>
    <m:smallFrac m:val="0"/>
    <m:dispDef/>
    <m:lMargin m:val="0"/>
    <m:rMargin m:val="0"/>
    <m:defJc m:val="centerGroup"/>
    <m:wrapIndent m:val="1440"/>
    <m:intLim m:val="subSup"/>
    <m:naryLim m:val="undOvr"/>
  </m:mathPr>
</w:settings>"#
            .to_string()
    }

    fn render_document_rels_xml(&self) -> String {
        let mut out = String::new();
        out.push_str(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/>"#,
        );

        if let Some(hid) = &self.header_rel_id {
            let _ = write!(
                out,
                r#"<Relationship Id="{hid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#
            );
        }
        if let Some(fid) = &self.footer_rel_id {
            let _ = write!(
                out,
                r#"<Relationship Id="{fid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>"#
            );
        }
        for hl in &self.hyperlinks {
            let _ = write!(
                out,
                r#"<Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{}" TargetMode="External"/>"#,
                hl.rel_id,
                escape_xml(&hl.url)
            );
        }
        for asset in &self.media_assets {
            let _ = write!(
                out,
                r#"<Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/{}"/>"#,
                asset.rel_id, asset.filename
            );
        }
        out.push_str("</Relationships>");
        out
    }

    fn render_root_rels_xml() -> String {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#
            .to_string()
    }

    fn render_content_types_xml(&self) -> String {
        let mut out = String::new();
        out.push_str(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
  <Default Extension="gif" ContentType="image/gif"/>
  <Default Extension="svg" ContentType="image/svg+xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>
  <Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>
  <Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#,
        );
        if self.header_rel_id.is_some() {
            out.push_str(
                r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>"#,
            );
        }
        if self.footer_rel_id.is_some() {
            out.push_str(
                r#"<Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>"#,
            );
        }
        out.push_str("</Types>");
        out
    }

    fn render_core_props_xml(&self) -> String {
        let title = escape_xml(self.ir.metadata.title.as_deref().unwrap_or(""));
        let creator = escape_xml(&self.ir.metadata.authors.join("; "));
        let description = escape_xml(self.ir.metadata.description.as_deref().unwrap_or(""));
        let keywords = escape_xml(&self.ir.metadata.keywords.join(", "));
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:title>{title}</dc:title>
  <dc:creator>{creator}</dc:creator>
  <dc:description>{description}</dc:description>
  <cp:keywords>{keywords}</cp:keywords>
</cp:coreProperties>"#
        )
    }

    fn render_app_props_xml() -> String {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>typst2word</Application>
</Properties>"#
            .to_string()
    }
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && c != '\n' && c != '\r' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}
