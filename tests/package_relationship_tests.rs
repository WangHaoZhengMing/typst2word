use std::io::Read;

#[test]
fn every_part_resolves_its_own_links_and_images() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("logo.svg"), r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#).unwrap();
    let source = r#"
#set page(header: [#link("https://example.com")[Header] #image("logo.svg")],
          footer: [#link("https://example.com")[Footer] #image("logo.svg")])
#link("https://example.com")[Body] #image("logo.svg")
Note#footnote[#link("https://example.com")[Note link] #image("logo.svg")]
"#;
    let (bytes, _) = typst2word::convert_typst_str_to_docx_bytes(source, Some(dir.path())).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut files = std::collections::HashMap::new();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        files.insert(name, bytes);
    }
    for part in ["document", "header1", "footer1", "footnotes"] {
        let xml = std::str::from_utf8(&files[&format!("word/{part}.xml")]).unwrap();
        let rels = std::str::from_utf8(&files[&format!("word/_rels/{part}.xml.rels")]).unwrap();
        let doc = roxmltree::Document::parse(xml).unwrap();
        let rels = roxmltree::Document::parse(rels).unwrap();
        let mut links = 0;
        let mut images = 0;
        for attr in doc.descendants().flat_map(|n| n.attributes()) {
            if attr.namespace() != Some("http://schemas.openxmlformats.org/officeDocument/2006/relationships") { continue; }
            let rel = rels.descendants().find(|n| n.attribute("Id") == Some(attr.value())).expect("reference resolves in its owning part");
            let target = rel.attribute("Target").unwrap();
            if rel.attribute("TargetMode") == Some("External") {
                assert_eq!(target, "https://example.com");
                links += 1;
            } else {
                assert!(files.contains_key(&format!("word/{target}")), "missing {target}");
                if target.starts_with("media/") { images += 1; }
            }
        }
        assert!(links > 0 && images > 0, "{part} must exercise both resource types");
    }
}
