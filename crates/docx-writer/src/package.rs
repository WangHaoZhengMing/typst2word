//! OPC package assembly: zips all parts and renders package-level parts
//! (`[Content_Types].xml`, `_rels/.rels`, `document.xml.rels`, `docProps/*`).

use std::fmt::Write as _;
use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::writer::{DocxWriter, SectionFurnitureRels};
use crate::xml::escape_xml;

impl<'a> DocxWriter<'a> {
    pub(crate) fn build(&mut self) -> Result<Vec<u8>, String> {
        let setups: Vec<_> = std::iter::once(self.doc.page_setup.clone())
            .chain(self.doc.sections.iter().map(|s| s.page_setup.clone()))
            .collect();
        let mut seen_header = false;
        let mut seen_footer = false;
        for setup in &setups {
            // An explicit empty part prevents inheriting a previous section's non-empty furniture,
            // while avoiding phantom empty header/footer height when no section has used one yet.
            let has_header = !setup.header.is_empty();
            let has_footer = !setup.footer.is_empty();
            let default_hdr = if has_header || seen_header {
                Some(self.alloc_rel_id())
            } else {
                None
            };
            let default_ftr = if has_footer || seen_footer {
                Some(self.alloc_rel_id())
            } else {
                None
            };
            let first_hdr = (setup.different_first_page && setup.first_page_header.is_some())
                .then(|| self.alloc_rel_id());
            let first_ftr = (setup.different_first_page && setup.first_page_footer.is_some())
                .then(|| self.alloc_rel_id());
            let even_hdr = (setup.different_odd_even && setup.even_page_header.is_some())
                .then(|| self.alloc_rel_id());
            let even_ftr = (setup.different_odd_even && setup.even_page_footer.is_some())
                .then(|| self.alloc_rel_id());
            seen_header |= has_header || first_hdr.is_some() || even_hdr.is_some();
            seen_footer |= has_footer || first_ftr.is_some() || even_ftr.is_some();
            self.section_furniture.push(SectionFurnitureRels {
                default_hdr,
                default_ftr,
                first_hdr,
                first_ftr,
                even_hdr,
                even_ftr,
            });
        }
        let document_xml = self.render_document_xml();
        let mut furniture_xml = Vec::new();
        for (index, setup) in setups.iter().enumerate() {
            self.current_setup = setup.clone();
            let sec_num = index + 1;
            let rels = self.section_furniture[index].clone();
            let empty: &[word_construct::model::BlockNode] = &[];
            let slots: [(&str, String, &[word_construct::model::BlockNode], bool); 6] = [
                (
                    "hdr",
                    format!("header{sec_num}"),
                    &setup.header,
                    rels.default_hdr.is_some(),
                ),
                (
                    "ftr",
                    format!("footer{sec_num}"),
                    &setup.footer,
                    rels.default_ftr.is_some(),
                ),
                (
                    "hdr",
                    format!("header{sec_num}_first"),
                    setup.first_page_header.as_deref().unwrap_or(empty),
                    rels.first_hdr.is_some(),
                ),
                (
                    "ftr",
                    format!("footer{sec_num}_first"),
                    setup.first_page_footer.as_deref().unwrap_or(empty),
                    rels.first_ftr.is_some(),
                ),
                (
                    "hdr",
                    format!("header{sec_num}_even"),
                    setup.even_page_header.as_deref().unwrap_or(empty),
                    rels.even_hdr.is_some(),
                ),
                (
                    "ftr",
                    format!("footer{sec_num}_even"),
                    setup.even_page_footer.as_deref().unwrap_or(empty),
                    rels.even_ftr.is_some(),
                ),
            ];
            for (tag, part, blocks, present) in slots {
                if present {
                    self.active_part = part.clone();
                    furniture_xml.push((part, self.render_header_or_footer_xml(blocks, tag)));
                }
            }
        }
        self.active_part = "footnotes".into();
        let footnotes_xml = self.render_footnotes_xml();
        let font_table_xml = Self::render_font_table_xml();
        let styles_xml = self.render_styles_xml();
        let numbering_xml = self.render_numbering_xml();
        let settings_xml = self.render_settings_xml();
        let doc_rels_xml = self.render_part_rels_xml("document");
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

        for (part, xml) in &furniture_xml {
            Self::zip_write_str(&mut zip, &format!("word/{part}.xml"), xml, options)?;
        }
        for part in furniture_xml.iter().map(|(part, _)| part.as_str()).chain(std::iter::once("footnotes")) {
            let rels = self.render_part_rels_xml(part);
            Self::zip_write_str(&mut zip, &format!("word/_rels/{part}.xml.rels"), &rels, options)?;
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
        for (index, rels) in self.section_furniture.iter().enumerate() {
            let sec_num = index + 1;
            for (kind, part_name, present) in [
                ("header", format!("header{sec_num}"), rels.default_hdr.is_some()),
                ("footer", format!("footer{sec_num}"), rels.default_ftr.is_some()),
                ("header", format!("header{sec_num}_first"), rels.first_hdr.is_some()),
                ("footer", format!("footer{sec_num}_first"), rels.first_ftr.is_some()),
                ("header", format!("header{sec_num}_even"), rels.even_hdr.is_some()),
                ("footer", format!("footer{sec_num}_even"), rels.even_ftr.is_some()),
            ] {
                if present {
                    let _ = write!(
                        out,
                        r#"<Override PartName="/word/{part_name}.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.{kind}+xml"/>"#
                    );
                }
            }
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

    pub(crate) fn render_part_rels_xml(&self, part: &str) -> String {
        let mut out = String::new();
        out.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
        if part == "document" {
        out.push_str(
            r#"  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/>
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable" Target="fontTable.xml"/>"#,
        );

        for (index, rels) in self.section_furniture.iter().enumerate() {
            let sec_num = index + 1;
            for (kind, target_name, id_opt) in [
                ("header", format!("header{sec_num}.xml"), &rels.default_hdr),
                ("footer", format!("footer{sec_num}.xml"), &rels.default_ftr),
                ("header", format!("header{sec_num}_first.xml"), &rels.first_hdr),
                ("footer", format!("footer{sec_num}_first.xml"), &rels.first_ftr),
                ("header", format!("header{sec_num}_even.xml"), &rels.even_hdr),
                ("footer", format!("footer{sec_num}_even.xml"), &rels.even_ftr),
            ] {
                if let Some(id) = id_opt {
                    let _ = write!(
                        out,
                        r#"<Relationship Id="{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{kind}" Target="{target_name}"/>"#
                    );
                }
            }
        }
        }
        for hl in self.hyperlinks.iter().filter(|h| h.part == part) {
            let _ = write!(
                out,
                r#"<Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{}" TargetMode="External"/>"#,
                hl.rel_id,
                escape_xml(&hl.url)
            );
        }
        for asset in self.media_assets.iter().filter(|a| a.part == part) {
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
        let title = escape_xml(self.doc.metadata.title.as_deref().unwrap_or(""));
        let creator = escape_xml(&self.doc.metadata.authors.join("; "));
        let description = escape_xml(self.doc.metadata.description.as_deref().unwrap_or(""));
        let keywords = escape_xml(&self.doc.metadata.keywords.join(", "));
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
