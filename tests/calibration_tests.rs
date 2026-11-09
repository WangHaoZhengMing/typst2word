use std::io::Read;
#[test]
fn margin_policy_can_preserve_explicit_geometry_without_legacy_clamping() {
    use typst2word::docx_writer::{DocxWriteOptions, write_docx_to_bytes_with_options};
    use typst2word::word_construct::page_calibration::PageMarginCalibration;
    let mut ir = typst2word::ir::DocumentIr::default();
    ir.page_setup.margin_top_pt = 10.0;
    ir.page_setup.margin_bottom_pt = 12.0;
    for (policy, top, bottom) in [
        (PageMarginCalibration::Preserve, "200", "240"),
        (PageMarginCalibration::LegacyWord, "360", "360"),
    ] {
        let bytes = write_docx_to_bytes_with_options(
            &ir,
            DocxWriteOptions {
                page_margin_calibration: policy,
            },
        )
        .unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut xml = String::new();
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let margins = doc
            .descendants()
            .find(|n| n.tag_name().name() == "pgMar")
            .unwrap();
        let ns = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
        assert_eq!(margins.attribute((ns, "top")), Some(top));
        assert_eq!(margins.attribute((ns, "bottom")), Some(bottom));
    }
}
