//! OPC package assembly: zips all parts and renders package-level parts
//! (`[Content_Types].xml`, `_rels/.rels`, `document.xml.rels`, `docProps/*`).

use std::fmt::Write as _;
use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::docx::builder::DocxBuilder;
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    pub(crate) fn build(&mut self) -> Result<Vec<u8>, String> {
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
        let font_table_xml = Self::render_font_table_xml();
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
        Self::zip_write_str(&mut zip, "word/fontTable.xml", &font_table_xml, options)?;

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

    pub(crate) fn zip_write_str(
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

    pub(crate) fn render_content_types_xml(&self) -> String {
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
  <Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/>
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

    pub(crate) fn render_root_rels_xml() -> String {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#
            .to_string()
    }

    pub(crate) fn render_document_rels_xml(&self) -> String {
        let mut out = String::new();
        out.push_str(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/>
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable" Target="fontTable.xml"/>"#,
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

    pub(crate) fn render_core_props_xml(&self) -> String {
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

    pub(crate) fn render_app_props_xml() -> String {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>typst2word</Application>
</Properties>"#
            .to_string()
    }

    pub(crate) fn render_font_table_xml() -> String {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:font w:name="DejaVu Sans Mono">
    <w:altName w:val="Consolas"/>
    <w:charset w:val="00"/>
    <w:family w:val="modern"/>
    <w:pitch w:val="fixed"/>
  </w:font>
  <w:font w:name="Libertinus Serif">
    <w:altName w:val="Times New Roman"/>
    <w:charset w:val="00"/>
    <w:family w:val="roman"/>
    <w:pitch w:val="variable"/>
  </w:font>
  <w:font w:name="New Computer Modern Math">
    <w:altName w:val="Cambria Math"/>
    <w:charset w:val="00"/>
    <w:family w:val="roman"/>
    <w:pitch w:val="variable"/>
  </w:font>
</w:fonts>"#
            .to_string()
    }
}
