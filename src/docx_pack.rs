use std::collections::HashMap;
use std::io::{Seek, Write};

use docx_rs::XMLDocx;
use zip::write::FileOptions;

pub fn pack_with_svg<W>(
    mut xml: XMLDocx,
    writer: W,
    svg_media: &HashMap<String, Vec<u8>>,
) -> zip::result::ZipResult<()>
where
    W: Write + Seek,
{
    if !svg_media.is_empty() {
        xml.content_type = add_svg_content_type(xml.content_type);
        xml.document_rels = rewrite_svg_relationships(xml.document_rels, svg_media);
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
