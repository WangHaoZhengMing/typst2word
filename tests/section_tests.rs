use std::io::Read;
#[test]
fn page_changes_become_sections_with_independent_furniture() {
    let source = r#"
#set page(width: 300pt, height: 400pt, margin: 20pt, header: [First header], numbering: "1")
First body
#set page(width: 500pt, height: 300pt, margin: 40pt, header: [Second header], numbering: "i")
Second body
#set page(width: 500pt, height: 300pt, margin: 30pt, header: none, footer: none, columns: 2)
Third body
"#;
    let (bytes, ir) = typst2word::convert_typst_str_to_docx_bytes(source, None).unwrap();
    assert_eq!(ir.sections.len(), 2);
    assert_eq!(ir.page_setup.margin_left_pt, 20.0);
    assert_eq!(ir.sections[0].page_setup.margin_left_pt, 40.0);
    assert_eq!(ir.sections[1].page_setup.margin_left_pt, 30.0);
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let sizes: Vec<_> = doc.descendants().filter(|n| n.tag_name().name() == "pgSz").map(|n| {
        let ns = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
        (n.attribute((ns, "w")).unwrap(), n.attribute((ns, "h")).unwrap())
    }).collect();
    assert_eq!(sizes, [("6000", "8000"), ("10000", "6000"), ("10000", "6000")]);
    for (part, expected) in [("header1", "First header"), ("header2", "Second header"), ("footer2", "PAGE \\* roman"), ("header3", "<w:p/>")] {
        let mut xml = String::new();
        zip.by_name(&format!("word/{part}.xml")).unwrap().read_to_string(&mut xml).unwrap();
        assert!(xml.contains(expected), "{part}: {xml}");
    }
}
#[test]
fn natural_page_wraps_remain_in_one_section() {
    let (_, ir) = typst2word::convert_typst_str_to_docx_bytes("#set page(height: 160pt, margin: 20pt, numbering: \"1\")\n#lorem(300)", None).unwrap();
    assert!(ir.report.pages_count > 1);
    assert!(ir.sections.is_empty(), "natural page wrap must preserve continuous flow");
}
