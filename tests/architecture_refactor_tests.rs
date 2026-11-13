//! Integration tests for the semantic / provenance / reference-layout architecture refactor:
//! realized `#show` semantics, unified `NodeId` + `NodeOrigin`, 1-to-N `LayoutFragmentIr`,
//! strict separation of source intent vs. Typst layout vs. Word output decisions.

use std::io::Read;

use typst2word::convert_typst_str_to_docx_bytes;
use typst2word::ir::{BlockNode, DocumentIr, InlineNode, SemanticRoleIr};

fn zip_entry(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("valid docx zip");
    let mut s = String::new();
    zip.by_name(name)
        .unwrap_or_else(|_| panic!("missing zip entry {name}"))
        .read_to_string(&mut s)
        .unwrap();
    s
}

fn zip_names(bytes: &[u8]) -> Vec<String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned())
        .collect()
}

fn paragraphs(ir: &DocumentIr) -> Vec<&BlockNode> {
    ir.blocks
        .iter()
        .filter(|b| matches!(b, BlockNode::Paragraph { .. }))
        .collect()
}

fn paragraph_text(block: &BlockNode) -> String {
    let BlockNode::Paragraph { content, .. } = block else {
        return String::new();
    };
    content
        .iter()
        .filter_map(|i| match i {
            InlineNode::Text(r) => Some(r.text.as_str()),
            _ => None,
        })
        .collect()
}

/// 1. `#show heading: it => block(...)[#it.body]` keeps BOTH heading identity and box styling.
#[test]
fn show_rule_heading_keeps_dual_identity() {
    let src = r##"
#show heading: it => block(fill: rgb("#E2E8F0"), stroke: (left: 3pt + rgb("#2563EB")), inset: 6pt)[#it.body]
#outline()
= Styled Heading <styled>
Body paragraph linking to #link(<styled>)[the heading].
"##;
    let (bytes, ir) = convert_typst_str_to_docx_bytes(src, None).expect("converts");

    let heading = ir
        .blocks
        .iter()
        .find_map(|b| match b {
            BlockNode::Heading {
                id,
                level,
                bookmark,
                box_style,
                ..
            } => Some((*id, *level, bookmark.clone(), box_style.clone())),
            _ => None,
        })
        .expect("heading survives show-rule realization");
    let (id, level, bookmark, box_style) = heading;
    assert_eq!(level, 1);
    assert!(bookmark.is_some(), "heading keeps its bookmark anchor");
    let style = box_style.expect("realized block styling retained as box_style");
    assert_eq!(style.fill.as_deref(), Some("E2E8F0"));
    assert_eq!(
        style.borders.left.as_ref().map(|s| s.color.as_str()),
        Some("2563EB")
    );

    let origin = ir.origin(id).expect("heading has a NodeOrigin");
    assert!(matches!(
        origin.role,
        Some(SemanticRoleIr::Heading { level: 1, .. })
    ));
    assert!(origin.typst_location.is_some());

    let outline_has_entry = ir.blocks.iter().any(|b| {
        matches!(b, BlockNode::Outline { entries, .. } if !entries.is_empty())
    });
    assert!(outline_has_entry, "outline still lists the restyled heading");

    let doc_xml = zip_entry(&bytes, "word/document.xml");
    roxmltree::Document::parse(&doc_xml).unwrap();
    assert!(doc_xml.contains(r#"<w:pStyle w:val="Heading1"/>"#));
    assert!(doc_xml.contains(r#"w:fill="E2E8F0""#), "heading shading emitted");
    assert!(doc_xml.contains("<w:pBdr>"));
    assert!(doc_xml.contains(r#"w:color="2563EB""#), "left accent border emitted");
}

/// 2. Pagebreaks produced by show rules become real `PageBreak`s; natural page wraps never
///    inject fake `w:after` spacing derived from page coordinates.
#[test]
fn show_rule_pagebreak_and_natural_wrap_have_no_fake_spacing() {
    let src = r##"
#set page(height: 200pt, margin: 20pt)
#show heading.where(level: 1): it => pagebreak(weak: true) + it
= First
#lorem(20)
= Second
#lorem(20)
"##;
    let (bytes, ir) = convert_typst_str_to_docx_bytes(src, None).expect("converts");
    let first_heading = ir
        .blocks
        .iter()
        .position(|b| matches!(b, BlockNode::Heading { .. }))
        .unwrap();
    assert!(
        !ir.blocks[..first_heading]
            .iter()
            .any(|b| matches!(b, BlockNode::PageBreak)),
        "weak pagebreak before the first heading must not create a blank page"
    );
    let second_heading = ir
        .blocks
        .iter()
        .rposition(|b| matches!(b, BlockNode::Heading { .. }))
        .unwrap();
    assert!(
        ir.blocks[first_heading..second_heading]
            .iter()
            .any(|b| matches!(b, BlockNode::PageBreak)),
        "explicit (show-rule) pagebreak before second heading: {:#?}",
        ir.blocks
    );
    let doc_xml = zip_entry(&bytes, "word/document.xml");
    assert!(doc_xml.contains(r#"<w:br w:type="page"/>"#));

    // Natural multi-page paragraph flow: no `755 - bottom` style page-push spacing.
    let (bytes, ir) = convert_typst_str_to_docx_bytes(
        "#set page(height: 160pt, margin: 20pt)\n#lorem(120)\n\n#lorem(120)\n\n#lorem(40)",
        None,
    )
    .unwrap();
    assert!(ir.report.pages_count > 1);
    let word = typst2word::word_construct::build_word_document(&ir);
    for b in &word.blocks {
        if let BlockNode::Paragraph { style, .. } = b {
            assert!(
                style.space_after_pt < 50.0,
                "natural page wrap injected fake spacing: {}",
                style.space_after_pt
            );
        }
    }
    let doc_xml = zip_entry(&bytes, "word/document.xml");
    assert!(!doc_xml.contains(r#"<w:br w:type="page"/>"#));
}

/// 3. Scoped `#set` rules inside a block resolve against the local `StyleChain`.
#[test]
fn scoped_styles_resolve_against_local_style_chain() {
    let src = r##"
Outside paragraph one.

#block[
  #set text(size: 20pt)
  #set par(first-line-indent: (amount: 2em, all: true), leading: 1em)
  Scoped paragraph one with enough words to be a normal paragraph.

  Scoped paragraph two.
]
"##;
    let (bytes, ir) = convert_typst_str_to_docx_bytes(src, None).expect("converts");
    let mut found = false;
    let mut stack: Vec<&BlockNode> = ir.blocks.iter().collect();
    while let Some(b) = stack.pop() {
        match b {
            BlockNode::Paragraph { style, .. } if paragraph_text(b).contains("Scoped") => {
                assert!(
                    (style.first_line_indent_pt - 40.0).abs() < 0.5,
                    "2em at 20pt => 40pt, got {}",
                    style.first_line_indent_pt
                );
                assert_eq!(style.resolved_font_size_pt.map(|v| v.round()), Some(20.0));
                found = true;
            }
            BlockNode::Container(c) => stack.extend(c.body.iter()),
            BlockNode::Table(t) => {
                for r in &t.rows {
                    for c in &r.cells {
                        stack.extend(c.blocks.iter());
                    }
                }
            }
            _ => {}
        }
    }
    assert!(found, "scoped paragraphs present: {:#?}", ir.blocks);
    let doc_xml = zip_entry(&bytes, "word/document.xml");
    assert!(doc_xml.contains(r#"w:firstLine="800""#));
}

/// 4. Math realization honours `#show` rules and nested `$...$` variables.
#[test]
fn math_realization_applies_show_rules_and_nested_equations() {
    let src = r##"
#show "RR": math.bb("R")
#let inner = $alpha + 1$
$ x in RR, quad inner^2 $
"##;
    let (bytes, _ir) = convert_typst_str_to_docx_bytes(src, None).expect("converts");
    let doc_xml = zip_entry(&bytes, "word/document.xml");
    assert!(doc_xml.contains("<m:oMath>"));
    assert!(
        doc_xml.contains('ℝ') || doc_xml.contains(r#"<m:scr m:val="double-struck"/>"#),
        "blackboard R realized"
    );
    assert!(doc_xml.contains('α'), "nested equation variable inlined");
}

/// 5. Unified identity: loop instances get distinct ids + locations sharing one source span;
///    semantic styles compare equal; cross-page paragraphs keep 1-to-N layout fragments.
#[test]
fn node_identity_provenance_and_one_to_many_fragments() {
    let ir = typst2word::compile_str_to_ir(
        "#for i in range(3) [Loop paragraph #i\n\n]",
        None,
    )
    .unwrap();
    let paras = paragraphs(&ir);
    assert_eq!(paras.len(), 3, "{:#?}", ir.blocks);
    let ids: Vec<_> = paras.iter().map(|b| b.node_id()).collect();
    assert!(ids.iter().all(|id| id.is_set()));
    assert!(ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2]);
    let origins: Vec<_> = ids.iter().map(|id| ir.origin(*id).unwrap()).collect();
    let locs: Vec<_> = origins.iter().map(|o| o.typst_location).collect();
    assert!(locs.iter().all(Option::is_some));
    assert!(locs[0] != locs[1] && locs[1] != locs[2], "loop instances disambiguated by Location");
    assert!(origins.iter().all(|o| matches!(o.role, Some(SemanticRoleIr::Paragraph))));
    let styles: Vec<_> = paras
        .iter()
        .map(|b| match b {
            BlockNode::Paragraph { style, .. } => style.clone(),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(styles[0], styles[1], "ParagraphStyleIr is position-independent");

    // Cross-page paragraph => multiple fragments on consecutive pages.
    let ir = typst2word::compile_str_to_ir(
        "#set page(height: 140pt, margin: 20pt)\n#lorem(150)",
        None,
    )
    .unwrap();
    assert!(ir.report.pages_count > 1);
    let para = paragraphs(&ir)
        .into_iter()
        .max_by_key(|b| paragraph_text(b).len())
        .unwrap();
    let layout = ir.layout(para.node_id()).expect("reference layout recorded");
    let pages: std::collections::BTreeSet<_> =
        layout.fragments.iter().map(|f| f.page_idx).collect();
    assert!(
        pages.contains(&0) && pages.contains(&1),
        "fragments: {:?}",
        layout.fragments
    );
}

/// 6. A `context`-dependent header absent on page 1 becomes a first-page header variant,
///    not a section split.
#[test]
fn dynamic_first_page_header_consolidates_into_title_page_section() {
    let src = r##"
#set page(height: 160pt, margin: 25pt, header: context if here().page() > 1 [Running Header])
#lorem(400)
"##;
    let (bytes, ir) = convert_typst_str_to_docx_bytes(src, None).expect("converts");
    assert!(ir.report.pages_count > 1);
    assert!(ir.sections.is_empty(), "single Word section expected");
    assert!(ir.page_setup.different_first_page);
    let doc_xml = zip_entry(&bytes, "word/document.xml");
    assert!(doc_xml.contains("<w:titlePg/>"));
    assert!(doc_xml.contains(r#"w:type="default""#));
    let header = zip_entry(&bytes, "word/header1.xml");
    assert!(header.contains("Running Header"));
    let names = zip_names(&bytes);
    if let Some(first) = names.iter().find(|n| n.as_str() == "word/header1_first.xml") {
        let first_xml = zip_entry(&bytes, first);
        assert!(!first_xml.contains("Running Header"));
    }
}

/// 7. `table.footer`, 3-tier stroke priority, and run-level locale / tracking / baseline.
#[test]
fn table_footer_stroke_priority_and_run_properties() {
    let src = r##"
#table(
  columns: 2,
  stroke: 0.5pt + rgb("#999999"),
  table.header([H1], [H2]),
  table.cell(stroke: 2pt + rgb("#FF0000"))[A], [B],
  table.hline(stroke: 3pt + rgb("#0000FF")),
  [C], [D],
  table.footer([F1], [F2]),
)

#set text(lang: "zh", region: "CN", tracking: 1pt, baseline: -2pt)
Shifted text.
"##;
    let (bytes, ir) = convert_typst_str_to_docx_bytes(src, None).expect("converts");
    let table = ir
        .blocks
        .iter()
        .find_map(|b| match b {
            BlockNode::Table(t) => Some(t),
            BlockNode::Figure { body, .. } => body.iter().find_map(|b| match b {
                BlockNode::Table(t) => Some(t),
                _ => None,
            }),
            _ => None,
        })
        .expect("table extracted");
    assert_eq!(table.footer_rows, 1);
    assert!(table.rows.last().unwrap().is_footer);
    assert!(table.rows.first().unwrap().is_header);

    let cell_a = table
        .rows
        .iter()
        .flat_map(|r| r.cells.iter())
        .find(|c| {
            c.blocks
                .iter()
                .any(|b| paragraph_text(b) == "A")
        })
        .expect("cell A");
    // Explicit hline (3pt blue) beats cell stroke (2pt red) beats table stroke at the shared edge.
    let bottom = cell_a.borders.bottom.as_ref().expect("bottom edge stroke");
    assert_eq!(bottom.color, "0000FF");
    let left = cell_a.borders.left.as_ref().expect("left edge stroke");
    assert_eq!(left.color, "FF0000");

    let doc_xml = zip_entry(&bytes, "word/document.xml");
    assert!(doc_xml.contains(r#"<w:lang w:val="zh-CN" w:eastAsia="zh-CN"/>"#));
    assert!(doc_xml.contains(r#"<w:spacing w:val="20"/>"#), "1pt tracking = 20 dxa");
    assert!(doc_xml.contains(r#"<w:position w:val="4"/>"#), "baseline -2pt raises 4 half-points");
}

/// 8. `sys.inputs` reach the document through the public API.
#[test]
fn sys_inputs_are_visible_to_typst() {
    let ir = typst2word::compile_str_to_ir_with_inputs(
        "#sys.inputs.at(\"who\", default: \"nobody\")",
        None,
        [("who", "templated")],
    )
    .unwrap();
    assert!(paragraphs(&ir)
        .iter()
        .any(|b| paragraph_text(b).contains("templated")));
}
