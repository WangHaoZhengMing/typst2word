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
    use typst2word::ir::InlineNode;
    let ir = typst2word::compile_str_to_ir("Same ending\\\n\nSame ending", None).unwrap();
    let paragraphs: Vec<_> = ir
        .blocks
        .iter()
        .filter_map(|b| {
            if let BlockNode::Paragraph {
                id, content, style, ..
            } = b
            {
                Some((*id, content, style))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(paragraphs.len(), 2);
    assert_ne!(paragraphs[0].0, paragraphs[1].0);
    assert!(matches!(paragraphs[0].1.last(), Some(InlineNode::LineBreak)));
    assert!(paragraphs[0].2.ends_with_explicit_linebreak);
    assert!(!paragraphs[1].1.iter().any(|i| matches!(i, InlineNode::LineBreak)));
    assert!(!paragraphs[1].2.ends_with_explicit_linebreak);
    assert!(ir.layout(paragraphs[1].0).is_some());
    let ir = typst2word::compile_str_to_ir(
        "#for i in range(2) [Same#linebreak()next#linebreak()#linebreak()last\n\n]",
        None,
    )
    .unwrap();
    for b in &ir.blocks {
        if let BlockNode::Paragraph { content, .. } = b {
            assert_eq!(
                content
                    .iter()
                    .filter(|i| matches!(i, InlineNode::LineBreak))
                    .count(),
                3
            );
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

#[test]
fn test_pingfang_calibrated_line_spacing_and_spatial_inline_box_in_enum() {
    let typst_src = r##"
#set text(font: "PingFang SC", size: 11pt, lang: "zh")
+ 第一条多行列表测试内容，用于验证苹方字体行距校准与基线步进提取是否准确匹配双引擎排版宽度与高度。

+ 洛克在《教育话语》中强调要与孩子讲道理，把孩子当成理性的人对待。\
  #box(inset: 12pt, stroke: 1pt + gray, radius: 6pt, [
    中文翻译对洛克原文的“pride”理解有误，将之译为“自负”，失去了原文中对儿童主体性的尊重
  ])\
  中国社会流行的“长大了就懂了”这种说法，则完全拒绝了与孩子讲道理。
"##;

    let (docx_bytes, ir) = convert_typst_str_to_docx_bytes(typst_src, None)
        .expect("enum with spatial inline #box converts cleanly");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    // 1. `DocumentIr` remains pure and records binary OpenType metrics when `PingFang SC` is available.
    if let Some(pf) = ir.font_metrics.get("PingFang SC") {
        assert!(pf.has_cjk_codepage);
        assert!(pf.hhea_span_ratio() > 1.35);
    }

    // 2. Spatial `#box(inset: 12pt, stroke: 1pt + gray, radius: 6pt)[...]` inside enum item 2
    //    becomes `InlineNode::ShapeInline` (`roundRect` DrawingML textbox) without poisoning
    //    the parent paragraph's `hanging_indent_pt` or emitting `<w:bdr>` character borders.
    let Some(BlockNode::Enum { items, .. }) = ir.blocks.first() else {
        panic!("expected BlockNode::Enum, got {:?}", ir.blocks.first());
    };
    assert_eq!(items.len(), 2);
    let Some(BlockNode::Paragraph { content, style, .. }) = items[1].blocks.first() else {
        panic!("expected Paragraph in enum item 2");
    };
    assert!(
        style.hanging_indent_pt < 0.5,
        "padded inline #box must not trigger false hanging_indent_pt, got {}",
        style.hanging_indent_pt
    );
    assert!(content.iter().any(|inl| matches!(
        inl,
        typst2word::ir::InlineNode::ShapeInline(s)
            if s.corner_radius_pt.is_some_and(|r| (5.5..=6.5).contains(&r))
                && (s.inset.left - 12.0).abs() < 1.0
    )));
    assert!(doc_xml.contains(r#"<a:prstGeom prst="roundRect">"#));
    assert!(!doc_xml.contains("<w:bdr "));
    assert!(doc_xml.contains(r#"w:line="199" w:lineRule="auto""#));
}

#[test]
fn test_box_elem_exact_semantics_baseline_and_partial_border_lowering() {
    let typst_src = r##"
#set text(font: "PingFang SC", size: 11pt, lang: "zh")
Step #box(baseline: 30%, circle(radius: 8pt, fill: rgb("#DBEAFE"), stroke: 0.8pt + rgb("#1D4ED8"))[1]) begins.

+ 结尾盒子上方有段内行距但下方无同段行。\
  #box(inset: 12pt, stroke: 1pt + gray, radius: 6pt)[柏拉图奴隶男孩的回忆论示例]

#box(width: 100%, fill: rgb("#F8FAFC"), stroke: (left: 2.5pt + rgb("#2563EB")), inset: 10pt)[
  单侧左边框直角行内盒子应由 word-construct 自动升级为原生单格表格边框
]
"##;

    let (docx_bytes, ir) = convert_typst_str_to_docx_bytes(typst_src, None)
        .expect("BoxElem exact semantics and lowering convert cleanly");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    // 1. `#box(baseline: 30%, circle(...))` propagates `baseline_shift_pt` -> `<w:position w:val="..."/>`.
    assert!(
        doc_xml.contains("<w:position w:val=\"-"),
        "expected negative half-point w:position for downward #box(baseline: 30%)"
    );

    // 2. Enum trailing `#box(inset: 12pt, radius: 6pt)` extracts exact `12.0pt` inset and `6.0pt` radius
    //    directly from `BoxElem`, and records `prev_line: Some, next_line: None` in `ReferenceLayout`.
    let Some(BlockNode::Enum { items, .. }) = ir.blocks.get(1) else {
        panic!("expected BlockNode::Enum at index 1, got {:?}", ir.blocks.get(1));
    };
    let Some(BlockNode::Paragraph { content, .. }) = items[0].blocks.first() else {
        panic!("expected Paragraph in enum item");
    };
    let shape = content
        .iter()
        .find_map(|inl| match inl {
            typst2word::ir::InlineNode::ShapeInline(s) => Some(s),
            _ => None,
        })
        .expect("expected ShapeInline for rounded #box");
    assert!((shape.inset.left - 12.0).abs() < 0.05);
    assert!((shape.inset.top - 12.0).abs() < 0.05);
    assert_eq!(shape.corner_radius_pt, Some(6.0));
    let ctx = ir
        .layout(shape.id)
        .and_then(|l| l.inline_box_context.as_ref())
        .expect("expected InlineBoxLineContextIr on ShapeInline");
    assert!(ctx.prev_line.is_some());
    assert!(ctx.next_line.is_none());

    // 3. Partial-border standalone `#box(stroke: (left: 2.5pt + #2563EB))` lowers to `<w:tcBorders>` with `2563EB`.
    assert!(doc_xml.contains("2563EB"));
    assert!(doc_xml.contains("<w:tcBorders>"));
}

#[test]
fn test_exam_outline_pageref_coder_card_spacer_pad_table_and_trailing_linebreak() {
    let typst_src = r##"
#set text(font: "PingFang SC", size: 11pt, lang: "zh")
#set heading(numbering: "1.1")
#set page(numbering: "第1 页/共 1 页")

#outline()
#pagebreak()

= 判断题
+ `3e3` 是 double 型常量。#h(1fr) 【$space space$】

= 简答题
#pad(left: 1em)[
  #table(
    columns: (auto, auto),
    [特性], [说明],
    [封装], [隐藏内部细节],
  )
]

#block(height: 2em)[ ]

= 参考答案
#let coder(code) = block(fill: luma(245), inset: 8pt, radius: 5pt, width: 100%)[
  •••
  #v(-8pt)
  #code
]

#text(weight: "bold")[1. #box([正确答案：A], stroke: 1pt, outset: 2.5pt)]#linebreak()
+ 正确选项解析。

== 编程题
1. #coder(```java
class Sum {
  int sum; // 成员变量
  int max; // 最大值
}
```)
"##;

    let (docx_bytes, ir) = convert_typst_str_to_docx_bytes(typst_src, None)
        .expect("exam constructs convert cleanly");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    // 1. Outline emits `PAGEREF <bm> \h` inside `第 ... 页` without invalid trailing `\* Arabic` after `\h`.
    assert!(
        doc_xml.contains(r#"PAGEREF _typst_loc_"#) && doc_xml.contains(r#" \h""#),
        "expected valid PAGEREF ... \\h field instruction in outline"
    );
    assert!(
        !doc_xml.contains(r#"\h \* "#),
        "PAGEREF format switches must never appear after \\h"
    );

    // 2. `#h(1fr)` + `【$space space$】` emits `<w:tab/>` and OMML `<m:nor/>` non-breaking spaces.
    assert!(doc_xml.contains("<w:tab/>"));
    assert!(doc_xml.contains("<m:nor/>"));
    assert!(
        doc_xml
            .matches("<m:t xml:space=\"preserve\">\u{00A0}</m:t>")
            .count()
            >= 2
    );

    // 3. `#pad(left: 1em)[#table(...)]` propagates horizontal padding to `<w:tblInd>`.
    assert!(
        doc_xml.contains(r#"<w:tblInd w:w="220" w:type="dxa"/>"#),
        "expected 11pt (220 dxa) left table indentation from #pad(left: 1em)"
    );

    // 4. `#block(height: 2em)[ ]` at `11pt` emits an exact `22pt` (`440 dxa`) solid spacer paragraph.
    assert!(doc_xml.contains(r#"w:line="440" w:lineRule="exact""#));

    // 5. `DocumentIr` keeps trailing `InlineNode::LineBreak`, while `.docx` strips the single
    //    redundant `<w:br/>` before `</w:p>` and uses `w:space="0"` on inline `<w:bdr>`.
    let has_ir_trailing_br = ir.sections.iter().flat_map(|s| &s.blocks).chain(&ir.blocks).any(|b| {
        matches!(
            b,
            BlockNode::Paragraph { content, style, .. }
                if style.ends_with_explicit_linebreak
                    && matches!(content.last(), Some(typst2word::ir::InlineNode::LineBreak))
        )
    });
    assert!(has_ir_trailing_br, "DocumentIr must preserve semantic trailing LineBreak");
    assert!(!doc_xml.contains("<w:r><w:br/></w:r></w:p>"));
    assert!(doc_xml.contains(r#"w:space="0""#));

    // 6. `1. #coder(...)` extracts a `roundRect` `ShapeBlock` with exact fixed-pitch code line spacing,
    //    `bIns="0"` so bottom descenders are never hard-clipped, and preserves `<w:numPr>` beside the card.
    assert!(doc_xml.contains(r#"<a:prstGeom prst="roundRect">"#));
    assert!(doc_xml.contains(r#"bIns="0""#));
    assert!(doc_xml.contains("<w:numPr>"));
}

#[test]
fn test_cross_page_coder_card_and_multiline_table_cell_spacing() {
    let typst_src = r##"
#set page(width: 420pt, height: 220pt, margin: 24pt)
#set text(font: "PingFang SC", size: 11pt, lang: "zh")
#let coder(code) = block(width: 100%, inset: 1em, fill: rgb("#F6F8FA"), radius: 8pt)[
  #v(-8pt)
  #text(size: 24pt, weight: 900, fill: rgb("#FF5F56"))[#sym.bullet]
  #text(size: 24pt, weight: 900, fill: rgb("#FFBD2E"))[#sym.bullet]
  #text(size: 24pt, weight: 900, fill: rgb("#27C93F"))[#sym.bullet]
  #v(-5pt)
  #text(size: 12pt)[#code]
]

#table(
  columns: (auto, 1fr),
  [代码1],
  [在这段代码中，Father 是父类，Son 继承并重写了 print 方法，通过多态机制，尽管变量类型是 Father，但实际对象是 Son，最终输出结果为 6。],
)

#coder(```java
class Pigeon {
  void line1() {}
  void line2() {}
  void line3() {}
  void line4() {}
  void line5() {}
  void line6() {
    System.out.println("鸽子在下蛋");
    int splitBoundaryToken = 1;
  }
}
```)
"##;

    let (docx_bytes, ir) = convert_typst_str_to_docx_bytes(typst_src, None)
        .expect("cross-page coder card and multiline table cell convert cleanly");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    // 1. Cross-page split `RawLine` must appear only once across both fragments (never duplicated),
    //    and total extracted code lines across the two split `ShapeBlock`s must equal 11.
    assert_eq!(doc_xml.matches("鸽子在下蛋").count(), 1);
    assert_eq!(doc_xml.matches("splitBoundaryToken").count(), 1);
    let total_code_lines: usize = ir
        .blocks
        .iter()
        .filter_map(|b| match b {
            BlockNode::ShapeBlock(s) => Some(
                s.body
                    .iter()
                    .filter_map(|cb| match cb {
                        BlockNode::CodeBlock { lines, .. } => Some(lines.len()),
                        _ => None,
                    })
                    .sum::<usize>(),
            ),
            _ => None,
        })
        .sum();
    assert_eq!(
        total_code_lines, 11,
        "cross-page split RawElem must preserve exact 11 code lines without phantom shell duplication"
    );

    // 2. Both the first and continuation `#coder` fragments must emit `w:lineRule="exact"` and `bIns="0"`.
    assert!(doc_xml.contains(r#"bIns="0""#));
    assert!(
        doc_xml.matches(r#"w:lineRule="exact""#).count() >= 10,
        "continuation CodeBlock lines must also use calibrated exact line spacing"
    );

    // 3. Multi-line CJK+Latin `#table` cell paragraph must retain calibrated multi-line leading (`w:line="199"`),
    //    not single-line compact cell trim (`w:line="154"`).
    assert!(
        doc_xml.contains(r#"w:line="199" w:lineRule="auto""#),
        "expected calibrated multi-line w:line=\"199\" on wrapped CJK+Latin table cell paragraph"
    );
}

#[test]
fn test_centered_padded_block_upright_h4_textbox_lists_and_post_box_spacing() {
    let typst_src = r##"
#set text(font: "PingFang SC", lang: "zh", size: 11pt)

#align(center)[
  #block(inset: (y: 2cm))[
    #text(size: 28pt, weight: "bold")[主标题文本]\
    #v(0.5cm)
    #text(size: 14pt)[副标题说明]
  ]
]
#pagebreak()

==== 第四级小节标题

#block(width: 100%, inset: 12pt, fill: rgb("#F5F5F7"), radius: 8pt)[
  *要点概述*
  1. 第一项措施
    - 子要点甲
    - 子要点乙
  2. 第二项措施
]
#v(1em)

== 下一节标题

#block(width: 100%, inset: 12pt, fill: rgb("#F3F4F6"), stroke: (left: 4pt + rgb("#6B7280")), radius: 8pt)[
  *注意事项*
  需要关注后续影响。
]
#v(1em)

后续正文段落内容。

#v(16cm)
#block(width: 100%, inset: (x: 1.5em, y: 1.2em), fill: rgb("#F5F5F7"), radius: 12pt)[
  *跨页圆角卡片*

  #lorem(120)
]
"##;

    let (docx_bytes, _ir) = convert_typst_str_to_docx_bytes(typst_src, None)
        .expect("centered padded block, upright H4, textbox lists, and post-box spacing convert cleanly");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);
    let styles_xml = read_zip_entry(&docx_bytes, "word/styles.xml");

    // 1. Heading4 style in `word/styles.xml` must not force `<w:i/>` faux italics.
    let h4_chunk = styles_xml
        .split(r#"w:styleId="Heading4""#)
        .nth(1)
        .and_then(|s| s.split("</w:style>").next())
        .expect("Heading4 style exists");
    assert!(
        !h4_chunk.contains("<w:i/>"),
        "Heading4 must remain upright bold by default: {h4_chunk}"
    );

    // 2. `#align(center)[#block(inset: (y: 2cm))[...]]` must center title paragraphs and apply ~2cm (`1134 dxa`) top inset.
    assert!(
        doc_xml.contains(r#"<w:jc w:val="center"/><w:spacing w:before="1134""#),
        "expected centered title paragraph with 2cm (1134 dxa) top inset"
    );

    // 3. `<w:txbxContent>` inside rounded `ShapeBlock` must preserve `<w:numPr>` numbering for `Enum` and nested `List`.
    let txbx_chunk = doc_xml
        .split("<w:txbxContent>")
        .nth(1)
        .and_then(|s| s.split("</w:txbxContent>").next())
        .expect("rounded ShapeBlock emits <w:txbxContent>");
    assert!(
        txbx_chunk.matches("<w:numPr>").count() >= 4,
        "expected <w:numPr> markers on enum and nested bullet items inside <w:txbxContent>: {txbx_chunk}"
    );

    // 4. `ShapeBlock` outer wrapper `<w:p>` uses a 1pt paragraph mark (`w:sz="2"`) so `Normal` font
    //    descent does not bloat `<wp:inline>` height at page boundaries.
    assert!(
        doc_xml.contains(r#"w:line="240" w:lineRule="auto"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr></w:pPr><w:r>"#),
        "expected 1pt paragraph mark on ShapeBlock wrapper paragraph"
    );

    // 5. Post-`Container` (`<w:tbl>`) `#v(1em)` spacing must propagate onto the following paragraph's `w:before` (> 200 dxa).
    let after_tbl_para = doc_xml
        .split("</w:tbl>")
        .nth(1)
        .and_then(|s| s.split("</w:p>").next())
        .expect("trailing paragraph after callout table exists");
    assert!(
        !after_tbl_para.contains(r#"<w:spacing w:before="0""#),
        "trailing paragraph after Container + #v(1em) must carry non-zero w:before: {after_tbl_para}"
    );

    // 6. Multi-page split rounded `ShapeBlock` must not double-apply `inset` as `<wp:anchor>` offset.
    assert!(
        !doc_xml.contains("<wp:anchor"),
        "multi-page split rounded block must not double-apply inset as <wp:anchor> placed_offset_pt"
    );
}

#[test]
fn test_slide_sections_background_valign_full_bleed_furniture_and_inline_pills() {
    let typst_src = r##"
#set page(
  paper: "presentation-16-9",
  margin: (x: 36pt, top: 54pt, bottom: 27pt),
  fill: rgb("#FAFAFA"),
  header: pad(x: -36pt, block(width: 100%, height: 36pt, fill: rgb("#23373B"), inset: (x: 18pt, y: 6pt))[
    #set text(fill: white, size: 18pt, weight: "bold")
    1. 演示标题
  ]),
  footer: pad(x: -36pt, grid(
    columns: (30%, 70%),
    rows: 2pt,
    rect(width: 100%, height: 100%, fill: rgb("#EB811B")),
    rect(width: 100%, height: 100%, fill: rgb("#E6DDD6")),
  )),
)
#set text(font: "PingFang SC", lang: "zh", size: 18pt)
#show heading.where(level: 2): none
#show raw.where(block: false): box.with(
  fill: luma(238),
  inset: (x: 4pt, y: 0pt),
  outset: (y: 3pt),
  radius: 3pt,
)
#show raw.where(block: true): block.with(
  fill: luma(242),
  stroke: 0.5pt + luma(210),
  inset: 10pt,
  radius: 6pt,
  width: 100%,
)

== 隐藏的小节标题

#align(horizon)[
  正文包含中文胶囊 `苹方-简` 与英文胶囊 `PingFang SC`：

  ```python
  # 第一行中文注释
  x = 1
  # 第二行中文注释
  y = 2
  ```
]

#page(fill: rgb("#23373B"), header: none, footer: none, margin: 54pt)[
  #set align(center + horizon)
  #set text(fill: white, size: 28pt, weight: "bold")
  感谢观看！
]
"##;

    let (docx_bytes, ir) = convert_typst_str_to_docx_bytes(typst_src, None)
        .expect("slide constructs convert cleanly");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);
    let settings_xml = read_zip_entry(&docx_bytes, "word/settings.xml");

    // 1. `#show heading.where(level: 2): none` must collapse hidden empty headings cleanly.
    assert!(
        !ir.blocks.iter().any(|b| matches!(b, BlockNode::Heading { .. })),
        "hidden empty heading must be stripped from DocumentIr"
    );

    // 2. Both `#align(horizon)` and `#set align(center + horizon)` emit `<w:vAlign w:val="center"/>`.
    assert!(
        doc_xml.matches(r#"<w:vAlign w:val="center"/>"#).count() >= 2,
        "expected <w:vAlign w:val=\"center\"/> on both horizon-aligned slide sections"
    );

    // 3. Per-section `#page(fill: ...)` enables `<w:displayBackgroundShape/>` and anchors a
    //    behind-doc full-page background rectangle inside the dark slide's header part.
    assert!(settings_xml.contains("<w:displayBackgroundShape/>"));
    let focus_hdr = read_zip_entry(&docx_bytes, "word/header2.xml");
    assert!(
        focus_hdr.contains(r#"behindDoc="1""#) && focus_hdr.contains("23373B"),
        "expected full-page behindDoc background shape in focus-slide header: {focus_hdr}"
    );

    // 4. Full-bleed `#pad(x: -36pt)` header/footer emit `-720 dxa` outdent and exact `2pt` (`40 dxa`) bar row.
    let slide1_hdr = read_zip_entry(&docx_bytes, "word/header1.xml");
    let slide1_ftr = read_zip_entry(&docx_bytes, "word/footer1.xml");
    assert!(
        slide1_hdr.contains(r#"<w:tblInd w:w="-720" w:type="dxa"/>"#),
        "expected -720 dxa full-bleed outdent on slide header: {slide1_hdr}"
    );
    assert!(
        slide1_ftr.contains(r#"<w:tblInd w:w="-720" w:type="dxa"/>"#)
            && slide1_ftr.contains(r#"<w:trHeight w:val="40" w:hRule="exact"/>"#),
        "expected -720 dxa full-bleed outdent and exact 40 dxa 2pt row on slide footer: {slide1_ftr}"
    );

    // 5. Single-line inline pills emit `wrap="none"` without negative `<w:position>`, and boxed
    //    `raw(block: true)` uses calibrated `w:lineRule="exact"` without a phantom trailing blank `<w:p>`.
    assert!(doc_xml.contains(r#"wrap="none""#));
    assert!(doc_xml.contains(r#"w:lineRule="exact""#));
}

#[test]
fn test_font_fallback_warning_and_exam_math_constructs() {
    // Deliberately omit `#set text(font: ...)` so default `Libertinus Serif` /
    // `New Computer Modern Math` fall back to a host CJK font on Chinese glyphs.
    let typst_src = r##"
#set page(paper: "a4")
#set text(size: 10.5pt)
#set heading(numbering: "I.")

= 判断题,正确的请在题号前的括号内打勾,错误的打叉,本大题共五个小题每小题两分

7. 已知密度函数 $f(x) = cases(a x + b\, & 0 < x < 1, 0\, & "其它")$，证明统计量 $display(Y = (sqrt(n - 1) X_i) / sqrt(X_2^2 + X_n^2))$ 服从分布.

#figure(
  table(columns: 2, [X], [1]),
  caption: [总体概率分布表],
)
"##;

    let (docx_bytes, ir) = convert_typst_str_to_docx_bytes(typst_src, None)
        .expect("exam constructs and font fallback convert cleanly");
    let doc_xml = assert_valid_docx_xml(&docx_bytes);

    // 1. Font fallback warnings are reported without altering realized font family names.
    assert!(
        ir.report
            .font_fallback_warnings
            .iter()
            .any(|w| w.contains("字体回退") && w.contains("未更改原字体名称")),
        "expected font fallback warning in ConversionReport, got: {:?}",
        ir.report.font_fallback_warnings
    );
    let cjk_families: Vec<&String> = ir
        .font_metrics
        .iter()
        .filter(|(_, m)| m.has_cjk_codepage)
        .map(|(family, _)| family)
        .collect();
    assert!(
        !cjk_families.is_empty(),
        "expected fallback CJK font to be detected with has_cjk_codepage=true (including AAT/cmap fonts)"
    );
    for family in cjk_families {
        assert!(
            doc_xml.contains(&format!(r#"w:eastAsia="{family}""#)),
            "expected verbatim realized CJK font family `{family}` in <w:rFonts>"
        );
    }

    // 2. Numbered heading emits hanging indent for wrapped lines and compact `w:after="16"` before `Enum`,
    //    while continued `7.` enum item emits `w:before="0"` instead of `180`.
    let parsed = roxmltree::Document::parse(&doc_xml).expect("valid document.xml");
    let paragraphs: Vec<_> = parsed
        .descendants()
        .filter(|n| n.has_tag_name("p"))
        .collect();

    let heading_p = paragraphs
        .iter()
        .find(|p| {
            p.descendants().any(|n| {
                n.has_tag_name("pStyle")
                    && n.attribute(("http://schemas.openxmlformats.org/wordprocessingml/2006/main", "val"))
                        == Some("Heading1")
            })
        })
        .expect("Heading1 paragraph present");
    let heading_xml = &doc_xml[heading_p.range()];
    assert!(
        heading_xml.contains("w:hanging="),
        "expected hanging indent on numbered Heading1: {heading_xml}"
    );

    let enum_p = paragraphs
        .iter()
        .find(|p| doc_xml[p.range()].contains("已知密度函数"))
        .expect("continued enum paragraph 7 present");
    let enum_xml = &doc_xml[enum_p.range()];
    assert!(
        enum_xml.contains(r#"w:before="0""#),
        "expected continued enum item 7 to emit w:before=\"0\": {enum_xml}"
    );

    // 3. Bottom caption after a table figure emits `w:before="100"` (5pt gap).
    let caption_p = paragraphs
        .iter()
        .find(|p| doc_xml[p.range()].contains("总体概率分布表"))
        .expect("table figure caption paragraph present");
    let caption_xml = &doc_xml[caption_p.range()];
    assert!(
        caption_xml.contains(r#"w:before="100""#),
        "expected w:before=\"100\" on bottom caption after table figure: {caption_xml}"
    );

    // 4. `cases(...)` left-aligns every piecewise column with zero `<m:cSp>`, and inline `display(...)`
    //    fraction compensates Word's inline fraction shrink even with nested subscripts.
    assert!(
        doc_xml.contains(
            r#"<m:mPr><m:rSpRule m:val="0"/><m:cGpRule m:val="3"/><m:cSp m:val="0"/><m:cGp m:val="80"/><m:mcs><m:mc><m:mcPr><m:count m:val="1"/><m:mcJc m:val="left"/></m:mcPr></m:mc><m:mc><m:mcPr><m:count m:val="1"/><m:mcJc m:val="left"/></m:mcPr></m:mc></m:mcs></m:mPr>"#
        ),
        "expected left-aligned per-column <m:mPr> for cases(...) matrix"
    );
    let frac_node = parsed
        .descendants()
        .find(|n| n.has_tag_name("f"))
        .expect("inline display fraction <m:f> present");
    let max_frac_sz: u32 = frac_node
        .descendants()
        .filter(|n| n.has_tag_name("sz"))
        .filter_map(|n| {
            n.attribute(("http://schemas.openxmlformats.org/wordprocessingml/2006/main", "val"))
                .and_then(|v| v.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0);
    assert!(
        max_frac_sz > 24,
        "expected inline display(...) fraction run <w:sz> compensation (>24 half-pt), got {max_frac_sz}"
    );
}

