use std::io::Read;
fn xml(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    zip.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}
#[test]
fn page_numbering_and_outline_are_dynamic_fields() {
    for (pattern, expected) in [("1", "PAGE \\* Arabic"), ("i", "PAGE \\* roman"), ("1 / 1", "NUMPAGES \\* Arabic")] {
        let source = format!("#set page(numbering: \"{pattern}\")\n#outline()\n= First\n#pagebreak()\n= Second");
        let (bytes, _) = typst2word::convert_typst_str_to_docx_bytes(&source, None).unwrap();
        let footer = xml(&bytes, "word/footer1.xml");
        assert!(footer.contains(expected), "{footer}");
        assert!(footer.contains("w:dirty=\"true\""));
        let body = xml(&bytes, "word/document.xml");
        assert!(body.contains("PAGEREF "));
        roxmltree::Document::parse(&body).unwrap();
        roxmltree::Document::parse(&footer).unwrap();
    }
}
