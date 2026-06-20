use std::collections::HashMap;
use std::io::{Seek, Write};

use docx_rs::XMLDocx;
use zip::write::FileOptions;

pub fn pack_with_svg<W>(
    mut xml: XMLDocx,
    writer: W,
    svg_media: &HashMap<String, Vec<u8>>,
    raw_xml_replacements: &HashMap<String, String>,
) -> zip::result::ZipResult<()>
where
    W: Write + Seek,
{
    if !svg_media.is_empty() {
        xml.content_type = add_svg_content_type(xml.content_type);
        xml.document_rels = rewrite_svg_relationships(xml.document_rels, svg_media);
    }

    if !raw_xml_replacements.is_empty() {
        xml.document = rewrite_raw_xml_placeholders(xml.document, raw_xml_replacements);
    }

    let mut zip = zip::ZipWriter::new(writer);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755);

    zip.add_directory("word/", Default::default())?;
    zip.add_directory("word/_rels", Default::default())?;
    zip.add_directory("_rels/", Default::default())?;
    zip.add_directory("docProps/", Default::default())?;

    write_file(&mut zip, "[Content_Types].xml", &xml.content_type, options)?;
    write_file(&mut zip, "_rels/.rels", &xml.rels, options)?;
    write_file(&mut zip, "docProps/app.xml", &xml.doc_props.app, options)?;
    write_file(&mut zip, "docProps/core.xml", &xml.doc_props.core, options)?;
    write_file(
        &mut zip,
        "docProps/custom.xml",
        &xml.doc_props.custom,
        options,
    )?;
    write_file(
        &mut zip,
        "word/_rels/document.xml.rels",
        &xml.document_rels,
        options,
    )?;
    write_file(&mut zip, "word/document.xml", &xml.document, options)?;
    write_file(&mut zip, "word/styles.xml", &xml.styles, options)?;
    write_file(&mut zip, "word/settings.xml", &xml.settings, options)?;
    write_file(&mut zip, "word/fontTable.xml", &xml.font_table, options)?;
    write_file(&mut zip, "word/comments.xml", &xml.comments, options)?;
    write_file(&mut zip, "word/numbering.xml", &xml.numberings, options)?;
    write_file(
        &mut zip,
        "word/commentsExtended.xml",
        &xml.comments_extended,
        options,
    )?;
    write_file(&mut zip, "word/footnotes.xml", &xml.footnotes, options)?;

    for (index, header) in xml.headers.iter().enumerate() {
        write_file(
            &mut zip,
            &format!("word/header{}.xml", index + 1),
            header,
            options,
        )?;

        if let Some(rels) = xml.header_rels.get(index) {
            write_file(
                &mut zip,
                &format!("word/_rels/header{}.xml.rels", index + 1),
                rels,
                options,
            )?;
        }
    }

    for (index, footer) in xml.footers.iter().enumerate() {
        write_file(
            &mut zip,
            &format!("word/footer{}.xml", index + 1),
            footer,
            options,
        )?;

        if let Some(rels) = xml.footer_rels.get(index) {
            write_file(
                &mut zip,
                &format!("word/_rels/footer{}.xml.rels", index + 1),
                rels,
                options,
            )?;
        }
    }

    if !xml.media.is_empty() {
        zip.add_directory("word/media/", Default::default())?;
        for (id, png_bytes) in xml.media {
            if let Some(svg_bytes) = svg_media.get(&id) {
                write_file(
                    &mut zip,
                    &format!("word/media/{id}.svg"),
                    svg_bytes,
                    options,
                )?;
            } else {
                write_file(
                    &mut zip,
                    &format!("word/media/{id}.png"),
                    &png_bytes,
                    options,
                )?;
            }
        }
    }

    if let Some(taskpanes) = xml.taskpanes {
        zip.add_directory("word/webextensions/", Default::default())?;
        write_file(
            &mut zip,
            "word/webextensions/taskpanes.xml",
            &taskpanes,
            options,
        )?;

        zip.add_directory("word/webextensions/_rels", Default::default())?;
        write_file(
            &mut zip,
            "word/webextensions/_rels/taskpanes.xml.rels",
            &xml.taskpanes_rels,
            options,
        )?;

        for (index, extension) in xml.web_extensions.iter().enumerate() {
            write_file(
                &mut zip,
                &format!("word/webextensions/webextension{}.xml", index + 1),
                extension,
                options,
            )?;
        }
    }

    if !xml.custom_items.is_empty() {
        zip.add_directory("customXml/_rels", Default::default())?;
    }

    for (index, item) in xml.custom_items.into_iter().enumerate() {
        let item_number = index + 1;
        write_file(
            &mut zip,
            &format!("customXml/_rels/item{item_number}.xml.rels"),
            &xml.custom_item_rels[index],
            options,
        )?;
        write_file(
            &mut zip,
            &format!("customXml/item{item_number}.xml"),
            &item,
            options,
        )?;
        write_file(
            &mut zip,
            &format!("customXml/itemProps{item_number}.xml"),
            &xml.custom_item_props[index],
            options,
        )?;
    }

    zip.finish()?;
    Ok(())
}

fn write_file<W: Write + Seek>(
    zip: &mut zip::ZipWriter<W>,
    path: &str,
    bytes: &[u8],
    options: FileOptions,
) -> zip::result::ZipResult<()> {
    zip.start_file(path, options)?;
    zip.write_all(bytes)?;
    Ok(())
}

fn add_svg_content_type(bytes: Vec<u8>) -> Vec<u8> {
    let mut xml = match String::from_utf8(bytes) {
        Ok(xml) => xml,
        Err(error) => return error.into_bytes(),
    };

    if xml.contains(r#"Extension="svg""#) {
        return xml.into_bytes();
    }

    let default = r#"<Default Extension="svg" ContentType="image/svg+xml" />"#;
    if let Some(index) = xml.rfind("</Types>") {
        xml.insert_str(index, default);
    }

    xml.into_bytes()
}

fn rewrite_svg_relationships(bytes: Vec<u8>, svg_media: &HashMap<String, Vec<u8>>) -> Vec<u8> {
    let mut xml = match String::from_utf8(bytes) {
        Ok(xml) => xml,
        Err(error) => return error.into_bytes(),
    };

    for id in svg_media.keys() {
        xml = xml.replace(
            &format!(r#"Target="media/{id}.png""#),
            &format!(r#"Target="media/{id}.svg""#),
        );
    }

    xml.into_bytes()
}

fn rewrite_raw_xml_placeholders(bytes: Vec<u8>, replacements: &HashMap<String, String>) -> Vec<u8> {
    let mut xml = match String::from_utf8(bytes) {
        Ok(xml) => xml,
        Err(error) => return error.into_bytes(),
    };

    xml = ensure_math_namespace(xml);

    for (placeholder, replacement) in replacements {
        xml = replace_placeholder_run(xml, placeholder, replacement);
    }

    xml.into_bytes()
}

fn ensure_math_namespace(mut xml: String) -> String {
    if xml.contains("xmlns:m=") {
        return xml;
    }

    if let Some(index) = xml.find("<w:document") {
        if let Some(end) = xml[index..].find('>') {
            xml.insert_str(
                index + end,
                r#" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math""#,
            );
        }
    }

    xml
}

fn replace_placeholder_run(mut xml: String, placeholder: &str, replacement: &str) -> String {
    while let Some(text_index) = xml.find(placeholder) {
        let Some(run_start) = find_run_start(&xml, text_index) else {
            xml.replace_range(text_index..text_index + placeholder.len(), replacement);
            continue;
        };
        let Some(relative_run_end) = xml[text_index..].find("</w:r>") else {
            xml.replace_range(text_index..text_index + placeholder.len(), replacement);
            continue;
        };
        let run_end = text_index + relative_run_end + "</w:r>".len();
        xml.replace_range(run_start..run_end, replacement);
    }

    xml
}

fn find_run_start(xml: &str, before: usize) -> Option<usize> {
    let mut search_end = before;
    while let Some(relative_start) = xml[..search_end].rfind("<w:r") {
        let name_end = relative_start + "<w:r".len();
        if xml[name_end..]
            .chars()
            .next()
            .is_some_and(|value| value == '>' || value.is_whitespace())
        {
            return Some(relative_start);
        }

        search_end = relative_start;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::replace_placeholder_run;

    #[test]
    fn replaces_whole_run_without_matching_run_properties() {
        let xml = r#"<w:p><w:r><w:rPr /><w:t>TYPSTOMML1</w:t></w:r><w:r><w:t>,</w:t></w:r></w:p>"#;
        let replaced = replace_placeholder_run(
            xml.to_string(),
            "TYPSTOMML1",
            "<m:oMath><m:r><m:t>x</m:t></m:r></m:oMath>",
        );

        assert_eq!(
            replaced,
            r#"<w:p><m:oMath><m:r><m:t>x</m:t></m:r></m:oMath><w:r><w:t>,</w:t></w:r></w:p>"#
        );
    }
}
