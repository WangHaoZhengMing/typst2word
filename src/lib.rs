//! `typst2word` — Semantic + Visual Typst (`.typ`) to Microsoft Word (`.docx`) converter.
//!
//! Workspace Crate Pipeline:
//! 1. [`compiler`]: Self-contained Typst 0.15.1 compiler & query engine (`TypstWorld`,
//!    `CUSTOM_ROUTINES`, `CompiledTypstDocument`) preserving both pre-layout `Content`
//!    semantics (`Introspector`) and laid-out `PagedDocument` `Frame` geometry (`Tag::Start` / `Tag::End`).
//! 2. [`extract`]: Pure Typst semantic & PDF physical layout extractor producing structurally
//!    normalized [`DocumentIr`] (zero Word-specific logic).
//! 3. [`ir`]: Exhaustive enum-driven Semantic + PDF Metrics Intermediate Representation (`DocumentIr`,
//!    `ir::pdf`, `ir::normalize`, `ir::units`, `ir::anchor`).
//! 4. [`word_construct`]: Word layout & compensation engine (`build_word_document(&DocumentIr) -> WordDocument`):
//!    - `word_construct::pipeline`: Word recognition & spacing solvers (`FontRecognition`,
//!      `LanguageAndScriptRecognition`, `InterBlockFlowAndSpacing`).
//!    - `word_construct::objects`: Per-object Word Construct builders (`paragraph`, `heading`, `list`,
//!      `table`, `equation`, `figure`, `code`, `text_run`, `shape`, `cetz`).
//! 5. [`docx_writer`]: Pure OpenXML & ZIP package writer (`DocxWriter` consuming `&WordDocument`).

pub use compiler;
pub use docx_writer;
pub use extract;
pub use ir;
pub use word_construct;

use std::path::Path;

use compiler::{TypstWorld, compile_world};
#[cfg(not(target_arch = "wasm32"))]
use docx_writer::write_docx_to_path;
use docx_writer::{DocxWriteOptions, write_docx_to_bytes_with_options};
use extract::{extract_document_ir, extract_document_ir_with_embed_mode};
use ir::{DocumentIr, FontEmbedMode};
use word_construct::build_word_document;

/// Compile a Typst `.typ` file and extract its pure `DocumentIr` (never mutated by Word passes).
#[cfg(not(target_arch = "wasm32"))]
pub fn compile_file_to_ir(input_path: &Path) -> Result<DocumentIr, String> {
    compile_file_to_ir_with_inputs(input_path, std::iter::empty::<(&str, &str)>())
}

/// Compile a Typst `.typ` file with `sys.inputs` key-value pairs and extract its pure `DocumentIr`.
#[cfg(not(target_arch = "wasm32"))]
pub fn compile_file_to_ir_with_inputs<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
) -> Result<DocumentIr, String> {
    compile_file_to_ir_with_options(input_path, inputs, FontEmbedMode::default())
}

/// Compile a Typst `.typ` file with `sys.inputs` and explicit [`FontEmbedMode`].
#[cfg(not(target_arch = "wasm32"))]
pub fn compile_file_to_ir_with_options<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
    embed_mode: FontEmbedMode,
) -> Result<DocumentIr, String> {
    let world = TypstWorld::from_file(input_path)?.with_string_inputs(inputs);
    let compiled = compile_world(&world)?;
    Ok(extract_document_ir_with_embed_mode(&compiled, &world, embed_mode))
}

/// Compile an in-memory Typst source string and extract its pure `DocumentIr` (never mutated by Word passes).
pub fn compile_str_to_ir(source: &str, root_dir: Option<&Path>) -> Result<DocumentIr, String> {
    compile_str_to_ir_with_inputs(source, root_dir, std::iter::empty::<(&str, &str)>())
}

/// Compile an in-memory Typst source string with `sys.inputs` key-value pairs and extract its pure `DocumentIr`.
pub fn compile_str_to_ir_with_inputs<K: AsRef<str>, V: AsRef<str>>(
    source: &str,
    root_dir: Option<&Path>,
    inputs: impl IntoIterator<Item = (K, V)>,
) -> Result<DocumentIr, String> {
    let world =
        TypstWorld::from_str(source, root_dir.map(|p| p.to_path_buf())).with_string_inputs(inputs);
    let compiled = compile_world(&world)?;
    Ok(extract_document_ir(&compiled, &world))
}

/// Convert a Typst `.typ` file directly to a `.docx` file on disk, returning the `DocumentIr`.
#[cfg(not(target_arch = "wasm32"))]
pub fn convert_typst_file_to_docx(
    input_path: &Path,
    output_path: &Path,
) -> Result<DocumentIr, String> {
    convert_typst_file_to_docx_with_inputs(
        input_path,
        output_path,
        std::iter::empty::<(&str, &str)>(),
    )
}

/// Convert a Typst `.typ` file with `sys.inputs` directly to a `.docx` file on disk.
#[cfg(not(target_arch = "wasm32"))]
pub fn convert_typst_file_to_docx_with_inputs<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    output_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
) -> Result<DocumentIr, String> {
    convert_typst_file_to_docx_with_options(
        input_path,
        output_path,
        inputs,
        FontEmbedMode::default(),
    )
}

/// Convert a Typst `.typ` file with `sys.inputs` and explicit [`FontEmbedMode`] to a `.docx` file.
#[cfg(not(target_arch = "wasm32"))]
pub fn convert_typst_file_to_docx_with_options<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    output_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
    embed_mode: FontEmbedMode,
) -> Result<DocumentIr, String> {
    let ir = compile_file_to_ir_with_options(input_path, inputs, embed_mode)?;
    let word_doc = build_word_document(&ir);
    write_docx_to_path(&word_doc, output_path)?;
    Ok(ir)
}

/// Convert an in-memory Typst source string to `.docx` bytes, returning `(docx_bytes, DocumentIr)`.
pub fn convert_typst_str_to_docx_bytes(
    source: &str,
    root_dir: Option<&Path>,
) -> Result<(Vec<u8>, DocumentIr), String> {
    convert_typst_str_to_docx_bytes_with_options(source, root_dir, DocxWriteOptions::default())
}

/// Convert an in-memory Typst source string to `.docx` bytes with explicit [`DocxWriteOptions`].
pub fn convert_typst_str_to_docx_bytes_with_options(
    source: &str,
    root_dir: Option<&Path>,
    options: DocxWriteOptions,
) -> Result<(Vec<u8>, DocumentIr), String> {
    let ir = compile_str_to_ir(source, root_dir)?;
    let word_doc = build_word_document(&ir);
    let bytes = write_docx_to_bytes_with_options(&word_doc, options)?;
    Ok((bytes, ir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equation_text_and_number_follow_the_fonts_used_in_layout() {
        let source = r#"
#set text(font: "DejaVu Sans Mono")
#set math.equation(numbering: "(1)")
$ #text(font: "Libertinus Serif")[same] + #text(font: "DejaVu Sans Mono")[same]
  + #text(font: ((name: "DejaVu Sans Mono", covers: regex("A")), "Libertinus Serif"))[AB] $
"#;
        let (bytes, _) = convert_typst_str_to_docx_bytes(source, None).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut xml)
            .unwrap();
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let mut runs = Vec::new();
        for run in doc.descendants().filter(|n| n.tag_name().name() == "r") {
            let text: String = run
                .children()
                .filter(|n| n.tag_name().name() == "t")
                .filter_map(|n| n.text())
                .collect();
            if matches!(text.as_str(), "same" | "A" | "B" | "(1)") {
                let fonts = run
                    .descendants()
                    .find(|n| n.tag_name().name() == "rFonts")
                    .unwrap();
                let font = fonts
                    .attributes()
                    .find(|a| a.name() == "ascii")
                    .unwrap()
                    .value();
                runs.push((text, font));
            }
        }
        assert_eq!(
            runs.iter()
                .map(|(t, f)| (t.as_str(), *f))
                .collect::<Vec<_>>(),
            [
                ("same", "Libertinus Serif"),
                ("same", "DejaVu Sans Mono"),
                ("A", "DejaVu Sans Mono"),
                ("B", "Libertinus Serif"),
                ("(1)", "NewComputerModernMath"),
            ]
        );
    }

    #[test]
    fn equations_keep_the_realized_math_font_in_every_embedding_mode() {
        let world = TypstWorld::from_str("Inline $x$ and display:\n\n$ sqrt(y) $", None);
        let compiled = compile_world(&world).unwrap();
        for mode in [
            FontEmbedMode::Subset,
            FontEmbedMode::Full,
            FontEmbedMode::None,
        ] {
            let ir = extract_document_ir_with_embed_mode(&compiled, &world, mode);
            let family = ir
                .math_font_family
                .as_deref()
                .expect("realized math family");
            if mode == FontEmbedMode::None {
                assert!(ir.embedded_fonts.is_empty());
            } else {
                let face = ir.embedded_fonts.iter().find(|f| f.is_math).unwrap();
                assert_eq!(face.word_family, family);
                assert_eq!(face.typst_family, "New Computer Modern Math");
                assert!(!face.is_subset);
                let count = u16::from_be_bytes([face.sfnt_data[4], face.sfnt_data[5]]) as usize;
                assert!(
                    face.sfnt_data[12..]
                        .chunks_exact(16)
                        .take(count)
                        .any(|r| &r[..4] == b"MATH")
                );
            }

            let bytes = docx_writer::write_docx_to_bytes(&build_word_document(&ir)).unwrap();
            let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
            for (part, expected) in [
                (
                    "word/settings.xml",
                    format!(r#"<m:mathFont m:val="{family}"/>"#),
                ),
                ("word/document.xml", format!(r#"w:ascii="{family}""#)),
                (
                    "word/fontTable.xml",
                    format!(r#"<w:font w:name="{family}">"#),
                ),
            ] {
                let mut xml = String::new();
                std::io::Read::read_to_string(&mut zip.by_name(part).unwrap(), &mut xml).unwrap();
                assert!(
                    xml.contains(&expected),
                    "{mode:?}: {part} missing {expected}"
                );
            }
        }
    }

    #[test]
    fn centered_auto_table_preserves_alignment_bold_cells_and_single_line_width() {
        let source = r#"
#align(center)[
  #table(
    columns: (auto, auto),
    align: (right + horizon, center + horizon),
    stroke: none,
    inset: 5pt,
    [#text(weight: "bold")[姓名：]], [王浩然], table.hline(start: 1),
    [#text(weight: "bold")[班级：]], [计算机学科学与技术2301], table.hline(start: 1),
  )
]
"#;
        let world = TypstWorld::from_str(source, None);
        let compiled = compile_world(&world).expect("typst compile should succeed");
        let ir = extract_document_ir_with_embed_mode(&compiled, &world, FontEmbedMode::None);

        let table = ir
            .blocks
            .iter()
            .find_map(|b| match b {
                ir::BlockNode::Table(t) => Some(t),
                _ => None,
            })
            .expect("expected extracted table");
        assert_eq!(table.align, ir::HAlignIr::Center);
        assert_eq!(table.cols.len(), 2);
        assert!(
            table.cols.iter().all(|c| c.is_auto()),
            "columns: (auto, auto) must preserve TableColTrackIr::Auto, got {:?}",
            table.cols
        );
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0].cells[0].halign, ir::HAlignIr::Right);
        assert_eq!(table.rows[0].cells[1].halign, ir::HAlignIr::Center);

        let first_cell_bold = match &table.rows[0].cells[0].blocks[0] {
            ir::BlockNode::Paragraph { content, .. } => content.iter().all(|inl| match inl {
                ir::InlineNode::Text(r) => r.bold,
                _ => true,
            }),
            _ => false,
        };
        assert!(
            first_cell_bold,
            "inline-fragment #text(weight: \"bold\")[...] inside table cell must preserve bold"
        );

        let word_doc = build_word_document(&ir);
        let solved_table = word_doc
            .blocks
            .iter()
            .find_map(|b| match b {
                ir::BlockNode::Table(t) => Some(t),
                _ => None,
            })
            .expect("expected solved table");
        assert!(
            solved_table.col_width_pt(0) >= 45.5,
            "col 0 must be wide enough for bold 3-char CJK+colon in Word, got {}",
            solved_table.col_width_pt(0)
        );
        assert_eq!(
            word_construct::table::TableConstruct::solve(solved_table).layout_mode,
            word_construct::table::TableLayoutMode::Autofit
        );
        let docx_bytes = docx_writer::write_docx_to_bytes(&word_doc)
            .expect("docx serialization should succeed");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(docx_bytes))
            .expect("valid docx zip archive");
        let mut doc_xml = String::new();
        std::io::Read::read_to_string(
            &mut zip.by_name("word/document.xml").expect("word/document.xml"),
            &mut doc_xml,
        )
        .expect("read word/document.xml");
        assert!(
            doc_xml.contains(r#"<w:tblPr><w:jc w:val="center"/>"#),
            "centered table must emit <w:jc w:val=\"center\"/>"
        );
        assert!(
            doc_xml.contains(r#"<w:tblLayout w:type="autofit"/>"#),
            "all-auto semantic table must emit <w:tblLayout w:type=\"autofit\"/>"
        );
    }

    #[test]
    fn exam_layout_preserves_indents_math_funcs_dfrac_ctrlpr_primes_and_custdash() {
        let source = r#"
#set page(
  paper: "a4",
  footer: context align(center)[第#counter(page).display()页（共#counter(page).final().at(0)页）],
)
#set text(font: ("Times New Roman", "Songti SC"), size: 12pt, lang: "zh")
#set par(leading: 1.2em, justify: true)
#set heading(hanging-indent: 2em)
#show heading.where(level: 1): body => text(font: "Heiti SC", size: 12pt)[#body]
#show math.equation.where(block: false): it => h(0.25em, weak: true) + it + h(0.25em, weak: true)

#let dfrac(num, den) = math.display(math.frac(num, den))
#let problem(body) = block[
  #set par(first-line-indent: 0pt)
  #h(2em)#body
]

#text(font: "Heiti SC")[绝密★启用前]

= 一、选择题：本题共8小题，每小题5分，共40分。

1. 已知$alpha$，$beta$为角，集合$A=\{0,1,3\}$，$B=\{x | sqrt(x)=x\}$，且$A E perp C E$，内角为$60 degree$，且$3 sin 2 alpha cos alpha = dfrac(sqrt(3), 15)$。

  #grid(
    columns: (1fr, 1fr),
    [A. $y=plus.minus dfrac(sqrt(3), 6) x$],
    [B. $S_n > dfrac(2, 3) a_1$],
    [C. $M'$],
    [D. 当$k=2$时，面积的最小值为$dfrac(sqrt(3), 15)$],
  )

#problem[若$triangle A B C$的面积为$dfrac(sqrt(7), 4)$，求轨迹$M'$的方程。]

#line(length: 100pt, stroke: (thickness: 0.5pt, dash: (5pt, 3pt)))
#pagebreak()
第二页内容。
"#;
        let world = TypstWorld::from_str(source, None);
        let compiled = compile_world(&world).expect("typst compile should succeed");
        let ir = extract_document_ir_with_embed_mode(&compiled, &world, FontEmbedMode::None);
        assert!(ir.cjk_latin_spacing, "default cjk-latin-spacing should be auto");

        let word_doc = build_word_document(&ir);
        let docx_bytes = docx_writer::write_docx_to_bytes(&word_doc)
            .expect("docx serialization should succeed");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(docx_bytes))
            .expect("valid docx zip archive");

        let read_part = |zip: &mut zip::ZipArchive<_>, name: &str| {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut zip.by_name(name).unwrap(), &mut s).unwrap();
            s
        };
        let doc_xml = read_part(&mut zip, "word/document.xml");
        let styles_xml = read_part(&mut zip, "word/styles.xml");
        let footer_xml = read_part(&mut zip, "word/footer1.xml");

        assert!(
            styles_xml.contains(r#"<w:autoSpaceDE w:val="1"/>"#)
                && styles_xml.contains(r#"<w:autoSpaceDN w:val="1"/>"#),
            "cjk-latin-spacing: auto must enable autoSpaceDE/autoSpaceDN"
        );
        assert!(
            doc_xml.contains(r#"<w:ind w:left="480" w:hanging="480"/>"#),
            "heading hanging-indent: 2em must emit 480 dxa hanging indent"
        );
        assert!(
            doc_xml.contains("<m:func>") && doc_xml.contains("<m:fName>"),
            "OpElem (sin/cos) must serialize as OMML <m:func>"
        );
        assert!(
            doc_xml.contains("<m:fPr><m:type m:val=\"bar\"/><m:ctrlPr>"),
            "inline dfrac must emit <m:fPr><m:ctrlPr> with calibrated <w:sz>"
        );
        assert!(
            !doc_xml.contains("<m:sup><m:r>") || !doc_xml.contains("<m:t>′</m:t></m:r></m:sup>"),
            "PrimesElem (M') must not be double-raised inside <m:sSup><m:sup>"
        );
        assert!(
            !doc_xml.contains("<m:t xml:space=\"preserve\">\u{2005}</m:t>"),
            "$60 degree$ must not inject U+2005 space after numeric literal"
        );
        assert!(
            doc_xml.contains("<m:t>⊥</m:t>") && !doc_xml.contains("<m:t>⟂</m:t>"),
            "Typst perp (U+27C2) must normalize to OMML relation U+22A5 (⊥)"
        );
        assert!(
            doc_xml.contains("<m:t>∣</m:t>"),
            "bare set-builder vertical bar must normalize to OMML relation U+2223 (∣)"
        );
        assert!(
            doc_xml.contains("<m:t xml:space=\"preserve\">,\u{2006}</m:t>"),
            "math commas followed by digits/atoms must append U+2006 thin punctuation space"
        );
        assert!(
            doc_xml.contains("&#x2005;"),
            "narrow horizontal math spaces must use non-stretchable U+2005"
        );
        assert!(
            !doc_xml.contains("，</w:t></w:r><w:r><w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\" w:eastAsia=\"Times New Roman\" w:cs=\"Times New Roman\"/><w:sz w:val=\"24\"/><w:szCs w:val=\"24\"/></w:rPr><w:t xml:space=\"preserve\">&#x2005;</w:t></w:r>"),
            "weak horizontal space immediately after fullwidth comma `，` must be absorbed"
        );
        assert!(
            !doc_xml.contains(r#"w:left="480" w:right="0" w:firstLine="480""#),
            "#problem with tall dfrac must not double-indent with both w:left and w:firstLine"
        );
        assert!(
            !doc_xml.contains(r#"w:firstLine="1020""#),
            "choice cell with dfrac must not infer spurious firstLine indent"
        );
        assert!(
            !doc_xml.contains(r#"<w:sz w:val="27"/>"#)
                && !doc_xml.contains(r#"<w:sz w:val="37"/>"#)
                && !doc_xml.contains(r#"<w:w w:val=""#),
            "inline dfrac with sibling subscripts (S_n > 2/3 a_1) must keep 12pt base / 16.5pt fraction with zero <w:w> squishing"
        );
        for para in doc_xml.split("<w:p>").skip(1) {
            let para_body = para.split("</w:p>").next().unwrap_or("");
            if para_body.contains("<m:f>") || para_body.contains("<m:rad>") {
                if let Some(after_line) = para_body.split(r#"w:line=""#).nth(1)
                    && let Some(val_str) = after_line.split('"').next()
                    && let Ok(val) = val_str.parse::<i64>()
                {
                    assert!(
                        val >= 240,
                        "paragraph with inline vertical fraction or radical must use w:line >= 240, got {val} in: {para_body}"
                    );
                }
            }
        }
        assert!(
            footer_xml.contains("NUMPAGES"),
            "footer denominator must normalize to NUMPAGES"
        );
        assert!(
            footer_xml.contains(
                r#"<w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman" w:eastAsia="Times New Roman" w:cs="Times New Roman"/><w:sz w:val="24"/><w:szCs w:val="24"/><w:lang w:val="zh-CN" w:eastAsia="zh-CN"/></w:rPr><w:instrText xml:space="preserve"> NUMPAGES"#
            ),
            "NUMPAGES field must preserve the digit run's Times New Roman font"
        );
    }
}
