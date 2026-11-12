use std::io::Read;
use zip::ZipArchive;

use typst2word::convert_typst_str_to_docx_bytes;
use typst2word::ir::{BlockNode, ShapeKindIr};

fn read_zip_entry(docx_bytes: &[u8], entry_name: &str) -> String {
    let cursor = std::io::Cursor::new(docx_bytes);
    let mut archive = ZipArchive::new(cursor).expect("valid zip archive");
    let mut file = archive
        .by_name(entry_name)
        .unwrap_or_else(|_| panic!("missing zip entry: {entry_name}"));
    let mut content = String::new();
    file.read_to_string(&mut content).expect("utf-8 xml entry");
    content
}

fn assert_valid_docx_xml(docx_bytes: &[u8]) -> String {
    let doc_xml = read_zip_entry(docx_bytes, "word/document.xml");
    roxmltree::Document::parse(&doc_xml).expect("word/document.xml must be well-formed XML");

    let styles_xml = read_zip_entry(docx_bytes, "word/styles.xml");
    roxmltree::Document::parse(&styles_xml).expect("word/styles.xml must be well-formed XML");

    let footnotes_xml = read_zip_entry(docx_bytes, "word/footnotes.xml");
    roxmltree::Document::parse(&footnotes_xml).expect("word/footnotes.xml must be well-formed XML");

    let rels_xml = read_zip_entry(docx_bytes, "word/_rels/document.xml.rels");
    roxmltree::Document::parse(&rels_xml)
        .expect("word/_rels/document.xml.rels must be well-formed XML");

    doc_xml
}

#[test]
fn test_semantic_elements_headings_lists_tables_code_quotes_footnotes() {
    let typst_src = r##"
#set document(title: "Semantic Test Document", author: "Antigravity", keywords: ("typst", "word"))
#set page(paper: "a4", header: [Header Text], footer: [Footer Page])
#set heading(numbering: "1.1")

#title[Typst to Word Semantic Conversion]

#outline()

= Introduction <sec-intro>

This paragraph tests *bold*, _italic_, #underline[underlined], #strike[strikethrough],
#highlight(fill: yellow)[highlighted], H#sub[2]O, E=mc#super[2], #smallcaps[SmallCaps],
`inline_code()`, #text(fill: rgb("#D32F2F"))[colored text], a #link("https://typst.app")[Typst Link],
a reference to @sec-intro, and a footnote#footnote[This is a rich *footnote* body.].

== Lists and Terms

- First bullet item
  - Nested bullet item
- Second bullet item

+ First numbered item
+ Second numbered item

/ Semantic IR: Intermediate representation preserving both semantics and visuals.
/ OMML: Office Math Markup Language used by Microsoft Word.

== Blockquote and Code Block

#quote(block: true, attribution: [Donald Knuth])[
  Premature optimization is the root of all evil.
]

```rust
fn main() {
    let message = "Hello, Word!";
    println!("{message}");
}
```

== Table with Colspan, Rowspan, and Fills

#figure(
  table(
    columns: (100pt, 100pt, 100pt),
    fill: (x, y) => if y == 0 { rgb("#E2E8F0") } else { none },
    table.header([*Category*], [*Metric*], [*Value*]),
    table.cell(rowspan: 2)[Performance], [Latency], [12ms],
    [Throughput], [10k req/s],
    table.cell(colspan: 2)[*Total Score*], [*99.5%*],
  ),
  caption: [Benchmark results table],
) <fig-table>
"##;

    let (docx_bytes, ir) = convert_typst_str_to_docx_bytes(typst_src, None)
        .expect("compilation and conversion succeeded");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    assert_eq!(ir.metadata.title.as_deref(), Some("Semantic Test Document"));
    assert!(ir.report.headings_count >= 4);
    assert!(ir.report.paragraphs_count >= 1);
    assert!(ir.report.lists_count >= 3);
    assert!(ir.report.tables_count >= 1);
    assert!(ir.report.quotes_count >= 1);
    assert!(ir.report.code_blocks_count >= 1);
    assert!(ir.report.figures_count >= 1);
    assert!(ir.report.footnotes_count >= 1);
    assert!(ir.report.links_count >= 1);

    // Check table merges and styles in Word XML
    assert!(doc_xml.contains("<w:gridSpan w:val=\"2\"/>"));
    assert!(doc_xml.contains("<w:vMerge w:val=\"restart\"/>"));
    assert!(doc_xml.contains("<w:vMerge/>"));
    assert!(doc_xml.contains("<w:footnoteReference"));
}

#[test]
fn test_native_shapes_conversion_to_word_drawingml() {
    let typst_src = r##"
= Native Shapes Test

#rect(
  width: 140pt,
  height: 45pt,
  fill: rgb("#E3F2FD"),
  stroke: 2pt + rgb("#1E88E5"),
  radius: 6pt,
)[*Rounded Rect with Text*]

#square(size: 40pt, fill: rgb("#FFF9C4"), stroke: 1pt + rgb("#FBC02D"))[Sq]

#circle(radius: 25pt, fill: rgb("#E8F5E9"), stroke: 1.5pt + rgb("#43A047"))[Circle]

#ellipse(width: 100pt, height: 40pt, fill: rgb("#FCE4EC"), stroke: 1pt + rgb("#D81B60"))[Ellipse]

#line(start: (0pt, 0pt), end: (80pt, 20pt), stroke: (paint: red, thickness: 2pt, dash: "dashed"))

#polygon(
  fill: rgb("#EDE7F6"),
  stroke: 1.5pt + rgb("#5E35B1"),
  (0pt, 40pt),
  (30pt, 0pt),
  (60pt, 40pt),
)

#curve(
  fill: rgb("#E0F7FA"),
  stroke: 1.5pt + rgb("#00838F"),
  curve.move((0pt, 20pt)),
  curve.line((40pt, 0pt)),
  curve.cubic((60pt, 10pt), (60pt, 30pt), (40pt, 40pt)),
  curve.close(),
)
"##;

    let (docx_bytes, ir) =
        convert_typst_str_to_docx_bytes(typst_src, None).expect("shape conversion succeeded");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    assert!(
        ir.report.shapes_count >= 7,
        "expected at least 7 shapes, got {}",
        ir.report.shapes_count
    );

    // Check that we extracted Rect, Ellipse, Line, Polygon, CustomPath shapes
    let mut saw_rect_with_body = false;
    let mut saw_ellipse = false;
    let mut saw_polygon = false;
    let mut saw_custom_path = false;
    for block in &ir.blocks {
        if let BlockNode::ShapeBlock(sh) = block {
            match &sh.kind {
                ShapeKindIr::Rect if !sh.body.is_empty() => saw_rect_with_body = true,
                ShapeKindIr::Ellipse => saw_ellipse = true,
                ShapeKindIr::Polygon { .. } => saw_polygon = true,
                ShapeKindIr::CustomPath { .. } => saw_custom_path = true,
                _ => {}
            }
        }
    }
    assert!(
        saw_rect_with_body,
        "Rect with text body should be extracted"
    );
    assert!(saw_ellipse, "Circle/Ellipse should be extracted");
    assert!(saw_polygon, "Polygon should be extracted");
    assert!(saw_custom_path, "Curve should be extracted");

    // Verify DrawingML XML elements in `word/document.xml`
    assert!(doc_xml.contains("<wps:wsp>"));
    assert!(doc_xml.contains("<a:prstGeom prst=\"roundRect\">"));
    assert!(doc_xml.contains("<a:prstGeom prst=\"ellipse\">"));
    assert!(doc_xml.contains("<a:custGeom>"));
    assert!(doc_xml.contains("<a:cubicBezTo>"));
    assert!(doc_xml.contains("<wps:txbx><w:txbxContent>"));
}

#[test]
fn test_math_formulas_conversion_to_word_omml() {
    let typst_src = r#"
#set math.equation(numbering: "(1)")

= Formula Conversion Test

Inline formula: $E = m c^2$ and $alpha_1 + beta^2 = frac(1, 2)$.

$ integral_0^infinity e^(-x^2) dif x = frac(sqrt(pi), 2) $ <eq-gauss>

$ sum_(k=1)^n k^2 = frac(n(n+1)(2n+1), 6), quad lim_(x -> 0) frac(sin(x), x) = 1 $

$ A = mat(1, 2, 3; 4, 5, 6; 7, 8, 9), quad f(x) = cases(1 &"if" x >= 0, -1 &"if" x < 0) $

$ hat(x) + vec(v) + overbrace(a + b + c, "sum") + cancel(y) + binom(n, k) + root(3, x^2 + 1) $

$ a &= b + c \
    &= d + e $
"#;

    let (docx_bytes, ir) =
        convert_typst_str_to_docx_bytes(typst_src, None).expect("math conversion succeeded");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    assert!(
        ir.report.equations_count >= 7,
        "expected at least 7 equations, got {}",
        ir.report.equations_count
    );

    // Verify OMML structures in `word/document.xml`
    assert!(doc_xml.contains("<m:oMathPara>"));
    assert!(doc_xml.contains("<m:oMath>"));
    assert!(doc_xml.contains("<m:f>"), "fraction <m:f>");
    assert!(doc_xml.contains("<m:rad>"), "radical <m:rad>");
    assert!(doc_xml.contains("<m:nary>"), "n-ary integral/sum <m:nary>");
    assert!(doc_xml.contains("<m:limLow>"), "limit <m:limLow>");
    assert!(doc_xml.contains("<m:m>"), "matrix/cases <m:m>");
    assert!(doc_xml.contains("<m:acc>"), "accent <m:acc>");
    assert!(doc_xml.contains("<m:groupChr>"), "overbrace <m:groupChr>");
    assert!(doc_xml.contains("<m:borderBox>"), "cancel <m:borderBox>");
    assert!(
        doc_xml.contains("<m:eqArr>"),
        "aligned equation array <m:eqArr>"
    );
}

#[test]
fn test_cetz_drawing_conversion_to_word_drawingml_group() {
    let typst_src = r##"
#import "@preview/cetz:0.3.4": canvas, draw

= CeTZ Vector Drawing Test

#canvas(length: 1cm, {
  import draw: *
  rect((0, 0), (3, 2), fill: rgb("#E3F2FD"), stroke: 1.5pt + blue)
  circle((1.5, 1), radius: 0.6, fill: rgb("#FFF9C4"), stroke: red)
  line((0, 0), (3, 2), stroke: (paint: green, thickness: 1pt, dash: "dashed"))
  bezier((0, 2), (3, 0), (1, 2.5), (2, -0.5), stroke: purple)
  content((1.5, 1), [*Center* $O$])
})
"##;

    let (docx_bytes, ir) =
        convert_typst_str_to_docx_bytes(typst_src, None).expect("CeTZ conversion succeeded");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    assert_eq!(
        ir.report.cetz_canvases_count, 1,
        "expected 1 CeTZ canvas extracted"
    );

    // Check CeTZ canvas paths and labels in IR
    let canvas = ir
        .blocks
        .iter()
        .find_map(|b| match b {
            BlockNode::CetzCanvas(c) => Some(c),
            _ => None,
        })
        .expect("BlockNode::CetzCanvas present");
    assert!(
        canvas.paths.len() >= 4,
        "expected at least 4 vector paths in CeTZ canvas, got {}",
        canvas.paths.len()
    );
    assert!(
        !canvas.labels.is_empty(),
        "expected CeTZ content label to be extracted"
    );

    // Check WordprocessingGroup (`<wpg:wgp>`) and custom geometry in `word/document.xml`
    assert!(doc_xml.contains("<wpg:wgp>"));
    assert!(doc_xml.contains("<a:custGeom>"));
    assert!(doc_xml.contains("<wps:txbx>"));
    assert!(doc_xml.contains("Center"));
}

#[test]
fn test_unsupported_elements_placeholder_conversion() {
    let typst_src = r##"
= Unsupported Elements Test

#html.elem("div")[Custom HTML Div Content]

#skew(ax: 20deg)[Skewed content]

#repeat[. ]

#let pat = tiling(size: (20pt, 20pt))[#line(start: (0%, 0%), end: (100%, 100%))]
#rect(width: 80pt, height: 30pt, fill: pat)
"##;

    let (docx_bytes, ir) = convert_typst_str_to_docx_bytes(typst_src, None)
        .expect("unsupported elements should produce placeholders without failing");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    assert!(
        ir.report.unsupported_elements.len() >= 4,
        "expected at least 4 unsupported elements recorded, got {:?}",
        ir.report.unsupported_elements
    );
    assert!(doc_xml.contains("不支持的元素:"));
    assert!(doc_xml.contains("html.elem"));
    assert!(doc_xml.contains("skew"));
    assert!(doc_xml.contains("repeat"));
    assert!(doc_xml.contains("tiling"));
}

#[test]
fn test_transformed_shapes_columns_breaks_and_cetz_math_labels() {
    let typst_src = r##"
#import "@preview/cetz:0.3.4": canvas, draw
#set page(columns: 2)

= Transformed Shapes & Advanced CeTZ

#rotate(15deg)[
  #rect(width: 90pt, height: 35pt, fill: rgb("#FFF3E0"), stroke: 1.5pt + orange)[Rotated Box]
]

#scale(x: 120%, y: 80%)[
  #circle(radius: 20pt, fill: rgb("#E8EAF6"), stroke: 1pt + blue)[Scaled]
]

#colbreak()

#canvas(length: 1cm, {
  import draw: *
  grid((0, 0), (2, 2), step: 1, stroke: 0.4pt + gray)
  line((0, 0), (2, 2), stroke: 1.5pt + red)
  content((1, 1.5), [$ f(x) = integral_0^x t^2 dif t $])
})

#pagebreak()

Final page paragraph.
"##;

    let (docx_bytes, ir) =
        convert_typst_str_to_docx_bytes(typst_src, None).expect("advanced conversion succeeded");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    assert_eq!(ir.page_setup.columns, 2);
    assert!(ir.report.shapes_count >= 2);
    assert_eq!(ir.report.cetz_canvases_count, 1);

    // Verify rotation attribute `rot="900000"` (15 deg * 60000) in DrawingML
    assert!(doc_xml.contains("rot=\"900000\""));
    // Verify column break and page break in Word XML
    assert!(doc_xml.contains("<w:br w:type=\"column\"/>"));
    assert!(doc_xml.contains("<w:br w:type=\"page\"/>"));
    // Verify math inside CeTZ label textbox
    assert!(doc_xml.contains("<m:nary>"));
}

#[test]
fn test_read_only_pdf_anchors_and_word_construct_policies() {
    let typst_src = r##"
#set par(justify: true, linebreaks: "optimized")

= 排版算法与锚点校验测试

这是一段采用优化断行策略的中文段落，包含避头尾标点符号（如括号、逗号、句号）。

#block(sticky: true)[
  *粘性标题块：必须与下一段同页*
]

#block(breakable: false)[
  不可拆分块第一行。

  不可拆分块第二行。
]
"##;

    let (docx_bytes, ir) =
        convert_typst_str_to_docx_bytes(typst_src, None).expect("conversion with anchors succeeded");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    // 1. Read-only PDF line anchors should be populated on `DocumentIr` without mutating blocks.
    assert!(
        !ir.anchors.is_empty(),
        "expected read-only AnchorLineIr entries extracted from laid-out Typst frames"
    );
    let anchor_report = word_construct::compare_line_anchors(&ir.anchors, &ir.anchors, 0.5, 0.5);
    assert_eq!(anchor_report.token_parity_rate, 1.0);
    assert_eq!(
        anchor_report.exact_token_matches,
        anchor_report.total_golden_lines
    );

    // 2. Verify `LinebreaksIr::Optimized` + `justify` maps to `LineBreakPolicy::KnuthPlass`.
    let has_knuth_plass = ir.blocks.iter().any(|b| {
        matches!(
            b,
            typst2word::ir::BlockNode::Paragraph { style, .. }
                if word_construct::LineBreakPolicy::from_typst_linebreaks(
                    style.linebreaks,
                    style.justify,
                ) == word_construct::LineBreakPolicy::typst_knuth_plass()
        )
    });
    assert!(has_knuth_plass, "expected KnuthPlass policy on justified optimized paragraph");

    // 3. Verify `sticky: true` and `breakable: false` emit `<w:keepNext/>` and `<w:keepLines/>`.
    assert!(doc_xml.contains("<w:keepNext/>"));
    assert!(doc_xml.contains("<w:keepLines/>"));
}



#[test]
fn explicit_breaks_use_element_identity_not_repeated_text() {
    use typst2word::ir::{InlineNode, ParagraphPdfMetricsIr};
    let ir = typst2word::compile_str_to_ir("Same ending\\\n\nSame ending", None).unwrap();
    let paragraphs: Vec<_> = ir.blocks.iter().filter_map(|b| {
        if let BlockNode::Paragraph { content, style, .. } = b { Some((content, style)) } else { None }
    }).collect();
    assert_eq!(paragraphs.len(), 2);
    assert!(matches!(paragraphs[0].0.last(), Some(InlineNode::LineBreak)));
    assert!(!paragraphs[1].0.iter().any(|i| matches!(i, InlineNode::LineBreak)));
    assert!(matches!(paragraphs[1].1.pdf_metrics, ParagraphPdfMetricsIr::Measured { ends_with_explicit_linebreak: false, .. }));
    let ir = typst2word::compile_str_to_ir("#for i in range(2) [Same#linebreak()next#linebreak()#linebreak()last\n\n]", None).unwrap();
    for b in &ir.blocks {
        if let BlockNode::Paragraph { content, .. } = b {
            assert_eq!(content.iter().filter(|i| matches!(i, InlineNode::LineBreak)).count(), 3);
        }
    }
}

#[test]
fn test_rounded_box_header_card_with_nested_grid() {
    let typst_src = r##"
#set text(font: ("New Computer Modern", "PingFang SC"), size: 11pt)
#box(
  radius: 6pt,
  stroke: 2pt,
  inset: 1em,
)[
  #align(center)[
    #text(17pt, weight: "bold")[Homework 2]
    #grid(
      columns: (1fr, auto),
      gutter: 1em,
      align: left,
      [*Course*: Algorithm Design and Analysis], [*Due Date*: 2025/3/23],
      [*Instructor*: Lei Li], [*Semester*: Spring 2025],
    )
  ]
]

Status #box(fill: rgb("#E0F2FE"), inset: (x: 4pt, y: 1pt))[ACTIVE] verified.
"##;

    let (docx_bytes, ir) =
        convert_typst_str_to_docx_bytes(typst_src, None).expect("rounded box header card converts");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    // 1. Standalone `#box(radius: 6pt, stroke: 2pt, inset: 1em)[...]` becomes a rounded `ShapeBlock`
    //    containing a centered title `Paragraph` and a 2x2 `Table` (`LayoutGrid`).
    let Some(BlockNode::ShapeBlock(card)) = ir.blocks.first() else {
        panic!("expected first block to be BlockNode::ShapeBlock, got {:?}", ir.blocks.first());
    };
    assert!(matches!(card.kind, ShapeKindIr::Rect));
    assert!(
        card.corner_radius_pt.is_some_and(|r| (4.5..=6.5).contains(&r)),
        "expected ~5-6pt centerline corner radius, got {:?}",
        card.corner_radius_pt
    );
    assert_eq!(card.body.len(), 2);
    assert!(matches!(
        &card.body[0],
        BlockNode::Paragraph { style, .. } if style.align == typst2word::ir::HAlignIr::Center
    ));
    assert!(matches!(&card.body[1], BlockNode::Table(tbl) if tbl.rows.len() == 2));

    // 2. Generated OOXML uses `<a:prstGeom prst="roundRect">`, `<a:spAutoFit/>`, nested `<w:tbl>`
    //    inside `<w:txbxContent>`, trailing `</w:p>`, and zero `<w:bdr` character borders.
    assert!(doc_xml.contains(r#"<a:prstGeom prst="roundRect">"#));
    assert!(doc_xml.contains("<a:spAutoFit/>"));
    assert!(doc_xml.contains("</w:tbl><w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"20\" w:lineRule=\"exact\"/>"));
    assert!(!doc_xml.contains("<w:bdr "));

    // 3. True inline `#box(...)` inside text remains an inline run in a regular `Paragraph`.
    assert!(matches!(ir.blocks.get(1), Some(BlockNode::Paragraph { .. })));
    assert!(doc_xml.contains("E0F2FE"));
}
