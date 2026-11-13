use ir::{BlockNode, DocumentIr};
use word_construct::WordDocument;

use crate::model::{BBoxPt, CheckItem, IrSummary, TestCaseSpec};

pub fn block_kind_name(b: &BlockNode) -> &'static str {
    match b {
        BlockNode::Title { .. } => "Title",
        BlockNode::Heading { .. } => "Heading",
        BlockNode::Paragraph { .. } => "Paragraph",
        BlockNode::List { .. } => "List",
        BlockNode::Enum { .. } => "Enum",
        BlockNode::Terms { .. } => "Terms",
        BlockNode::Container(_) => "Container",
        BlockNode::Table(_) => "Table",
        BlockNode::Quote { .. } => "Quote",
        BlockNode::CodeBlock { .. } => "CodeBlock",
        BlockNode::Figure { .. } => "Figure",
        BlockNode::EquationBlock(_) => "EquationBlock",
        BlockNode::ShapeBlock(_) => "ShapeBlock",
        BlockNode::CetzCanvas(_) => "CetzCanvas",
        BlockNode::ImageBlock(_) => "ImageBlock",
        BlockNode::Outline { .. } => "Outline",
        BlockNode::Bibliography { .. } => "Bibliography",
        BlockNode::HorizontalRule { .. } => "HorizontalRule",
        BlockNode::PageBreak => "PageBreak",
        BlockNode::ColumnBreak => "ColumnBreak",
        BlockNode::Columns { .. } => "Columns",
        BlockNode::UnsupportedBlock(_) => "UnsupportedBlock",
    }
}

#[derive(Debug, Clone)]
struct ParsedPgMar {
    top: i64,
    right: i64,
    bottom: i64,
    left: i64,
    header: i64,
    footer: i64,
    summary: String,
}

fn dxa_matches(actual: i64, expected_pt: f64) -> bool {
    let exp_dxa = (expected_pt * 20.0).round() as i64;
    (actual - exp_dxa).abs() <= 1
}

pub fn summarize_and_verify(
    spec: &TestCaseSpec,
    ir: &DocumentIr,
    word_doc: &WordDocument,
    document_xml: &str,
    all_word_xml_parts: &str,
) -> (IrSummary, Vec<CheckItem>) {
    let ps = &ir.page_setup;
    let mut top_kinds: Vec<String> = ir
        .blocks
        .iter()
        .map(|b| block_kind_name(b).to_string())
        .collect();
    for sec in &ir.sections {
        for b in &sec.blocks {
            top_kinds.push(block_kind_name(b).to_string());
        }
    }

    let anchor_bbox = if ir.anchors.is_empty() {
        None
    } else {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for a in &ir.anchors {
            min_x = min_x.min(a.x_start_pt);
            max_x = max_x.max(a.x_start_pt + a.width_pt);
            min_y = min_y.min((a.baseline_y_pt - 10.0).max(0.0));
            max_y = max_y.max(a.baseline_y_pt);
        }
        if min_x.is_finite() && max_x >= min_x {
            Some(BBoxPt {
                left_pt: round2(min_x),
                top_pt: round2(min_y),
                right_pt: round2(max_x),
                bottom_pt: round2(max_y),
                width_pt: round2(max_x - min_x),
                height_pt: round2(max_y - min_y),
            })
        } else {
            None
        }
    };

    let mut pg_mar_xml = String::new();
    let mut checks = Vec::new();

    // 1. Well-formed XML parse check + `<w:pgMar>` single/multi-section checks
    match roxmltree::Document::parse(document_xml) {
        Ok(xml_doc) => {
            checks.push(CheckItem {
                name: "OOXML Well-Formed".to_string(),
                passed: true,
                detail: "word/document.xml parsed cleanly".to_string(),
            });

            let pg_mars: Vec<ParsedPgMar> = xml_doc
                .descendants()
                .filter(|n| n.is_element() && n.tag_name().name() == "pgMar")
                .map(|pg_mar| {
                    let get_attr = |k: &str| -> i64 {
                        pg_mar
                            .attributes()
                            .find(|a| a.name() == k || a.name().ends_with(&format!(":{k}")))
                            .and_then(|a| a.value().parse::<i64>().ok())
                            .unwrap_or(-1)
                    };
                    let top = get_attr("top");
                    let right = get_attr("right");
                    let bottom = get_attr("bottom");
                    let left = get_attr("left");
                    let header = get_attr("header");
                    let footer = get_attr("footer");
                    ParsedPgMar {
                        top,
                        right,
                        bottom,
                        left,
                        header,
                        footer,
                        summary: format!(
                            "top={top} right={right} bottom={bottom} left={left} header={header} footer={footer}"
                        ),
                    }
                })
                .collect();

            if let Some(first_pm) = pg_mars.first() {
                pg_mar_xml = first_pm.summary.clone();
                let [et, er, eb, el] = spec.expectation.expected_margins_pt;
                let sides_ok = dxa_matches(first_pm.top, et)
                    && dxa_matches(first_pm.right, er)
                    && dxa_matches(first_pm.bottom, eb)
                    && dxa_matches(first_pm.left, el);
                let hf_clamp_ok = first_pm.header >= 0
                    && first_pm.footer >= 0
                    && first_pm.header <= first_pm.top
                    && first_pm.footer <= first_pm.bottom;
                checks.push(CheckItem {
                    name: "OOXML pgMar Margin".to_string(),
                    passed: sides_ok && hf_clamp_ok,
                    detail: if sides_ok && hf_clamp_ok {
                        pg_mar_xml.clone()
                    } else {
                        format!(
                            "expected dxa ~({:.0},{:.0},{:.0},{:.0}) with header<=top & footer<=bottom, got {}",
                            et * 20.0,
                            er * 20.0,
                            eb * 20.0,
                            el * 20.0,
                            pg_mar_xml
                        )
                    },
                });

                if !spec.expectation.expected_section_margins_pt.is_empty() {
                    let exp_secs = &spec.expectation.expected_section_margins_pt;
                    let count_ok = pg_mars.len() == exp_secs.len();
                    let all_sec_ok = count_ok
                        && pg_mars.iter().zip(exp_secs.iter()).all(|(pm, [t, r, b, l])| {
                            dxa_matches(pm.top, *t)
                                && dxa_matches(pm.right, *r)
                                && dxa_matches(pm.bottom, *b)
                                && dxa_matches(pm.left, *l)
                                && pm.header <= pm.top
                                && pm.footer <= pm.bottom
                        });
                    let sec_summary: Vec<String> = pg_mars
                        .iter()
                        .map(|pm| format!("[{},{},{},{}]", pm.top, pm.right, pm.bottom, pm.left))
                        .collect();
                    checks.push(CheckItem {
                        name: "OOXML Multi-Section pgMar".to_string(),
                        passed: all_sec_ok,
                        detail: format!(
                            "sections={} dxa={}",
                            pg_mars.len(),
                            sec_summary.join(" -> ")
                        ),
                    });
                }
            } else {
                checks.push(CheckItem {
                    name: "OOXML pgMar Presence".to_string(),
                    passed: false,
                    detail: "Missing <w:pgMar> in word/document.xml".to_string(),
                });
            }
        }
        Err(e) => {
            checks.push(CheckItem {
                name: "OOXML Well-Formed".to_string(),
                passed: false,
                detail: format!("XML parse error: {e}"),
            });
        }
    }

    // 2. IR margin preservation check (4-sided + multi-section)
    let [exp_t, exp_r, exp_b, exp_l] = spec.expectation.expected_margins_pt;
    let ir_margin_ok = (ps.margin_top_pt - exp_t).abs() <= 0.06
        && (ps.margin_right_pt - exp_r).abs() <= 0.06
        && (ps.margin_bottom_pt - exp_b).abs() <= 0.06
        && (ps.margin_left_pt - exp_l).abs() <= 0.06;
    checks.push(CheckItem {
        name: "IR PageSetup Margin".to_string(),
        passed: ir_margin_ok,
        detail: format!(
            "IR margins (T={:.2}, R={:.2}, B={:.2}, L={:.2}) pt",
            ps.margin_top_pt, ps.margin_right_pt, ps.margin_bottom_pt, ps.margin_left_pt
        ),
    });

    let all_ir_sections: Vec<[f64; 4]> = std::iter::once([
        round2(ps.margin_top_pt),
        round2(ps.margin_right_pt),
        round2(ps.margin_bottom_pt),
        round2(ps.margin_left_pt),
    ])
    .chain(ir.sections.iter().map(|s| {
        [
            round2(s.page_setup.margin_top_pt),
            round2(s.page_setup.margin_right_pt),
            round2(s.page_setup.margin_bottom_pt),
            round2(s.page_setup.margin_left_pt),
        ]
    }))
    .collect();

    if !spec.expectation.expected_section_margins_pt.is_empty() {
        let exp_secs = &spec.expectation.expected_section_margins_pt;
        let ir_secs_ok = all_ir_sections.len() == exp_secs.len()
            && all_ir_sections
                .iter()
                .zip(exp_secs.iter())
                .all(|(actual, exp)| {
                    (actual[0] - exp[0]).abs() <= 0.08
                        && (actual[1] - exp[1]).abs() <= 0.08
                        && (actual[2] - exp[2]).abs() <= 0.08
                        && (actual[3] - exp[3]).abs() <= 0.08
                });
        checks.push(CheckItem {
            name: "IR Multi-Section Margins".to_string(),
            passed: ir_secs_ok,
            detail: format!("IR sections={:?}", all_ir_sections),
        });
    }

    // 3. Mirror margins verification (`inside` / `outside` + `binding`)
    let any_ir_mirror =
        ps.mirror_margins || ir.sections.iter().any(|s| s.page_setup.mirror_margins);
    if let Some(exp_mirror) = spec.expectation.expected_mirror_margins {
        let xml_has_mirror = all_word_xml_parts.contains("<w:mirrorMargins/>");
        let mirror_ok = (any_ir_mirror == exp_mirror) && (xml_has_mirror == exp_mirror);
        checks.push(CheckItem {
            name: "Two-Sided Mirror Margins".to_string(),
            passed: mirror_ok,
            detail: format!(
                "expected={exp_mirror}, ir.mirror_margins={any_ir_mirror}, settings.xml <w:mirrorMargins/>={xml_has_mirror}"
            ),
        });
    }

    // 4. Expected IR top-level block kinds
    for req_kind in &spec.expectation.required_ir_kinds {
        let found = top_kinds.iter().any(|k| k == req_kind);
        checks.push(CheckItem {
            name: format!("IR Block Kind `{req_kind}`"),
            passed: found,
            detail: if found {
                format!("Found in {:?}", top_kinds)
            } else {
                format!("Missing `{req_kind}`, actual IR blocks: {:?}", top_kinds)
            },
        });
    }

    // 5. Required OOXML snippets
    for snippet in &spec.expectation.required_xml_snippets {
        let found = all_word_xml_parts.contains(snippet);
        checks.push(CheckItem {
            name: format!("OOXML Contains `{snippet}`"),
            passed: found,
            detail: if found {
                "Present in generated Word XML parts".to_string()
            } else {
                format!("Substring `{snippet}` not found in generated Word XML")
            },
        });
    }

    // 6. Unsupported elements check
    if !spec.expectation.allow_unsupported {
        let no_unsup = ir.report.unsupported_elements.is_empty();
        checks.push(CheckItem {
            name: "Zero Unexpected Unsupported Elements".to_string(),
            passed: no_unsup,
            detail: if no_unsup {
                "0 unsupported elements".to_string()
            } else {
                format!("Unsupported: {:?}", ir.report.unsupported_elements)
            },
        });
    }

    let summary = IrSummary {
        page_width_pt: round2(ps.width_pt),
        page_height_pt: round2(ps.height_pt),
        margins_pt: [
            round2(ps.margin_top_pt),
            round2(ps.margin_right_pt),
            round2(ps.margin_bottom_pt),
            round2(ps.margin_left_pt),
        ],
        mirror_margins: any_ir_mirror,
        section_margins_pt: all_ir_sections,
        header_footer_margin_pt: [round2(ps.header_margin_pt), round2(ps.footer_margin_pt)],
        sections_count: 1 + ir.sections.len(),
        top_block_kinds: top_kinds,
        anchor_lines_count: ir.anchors.len(),
        anchor_bbox,
        has_cjk: word_doc.has_cjk,
        has_cetz_canvas: word_doc.has_cetz_canvas,
        pg_mar_xml,
    };

    (summary, checks)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Lightweight XML pretty-printer for inspector viewing.
pub fn pretty_format_xml(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() + xml.len() / 4);
    let mut indent: usize = 0;
    let mut i = 0;
    let bytes = xml.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<'
            && let Some(rel_end) = xml[i..].find('>')
        {
            let tag = &xml[i..=i + rel_end];
            let is_decl = tag.starts_with("<?") || tag.starts_with("<!");
            let is_close = tag.starts_with("</");
            let is_self_close = tag.ends_with("/>");

            if is_close {
                indent = indent.saturating_sub(1);
            }
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            for _ in 0..indent {
                out.push_str("  ");
            }
            out.push_str(tag);
            out.push('\n');
            if !is_decl && !is_close && !is_self_close {
                indent += 1;
            }
            i += rel_end + 1;
            continue;
        }
        let next_lt = xml[i..].find('<').unwrap_or(xml.len() - i);
        let text = xml[i..i + next_lt].trim();
        if !text.is_empty() {
            for _ in 0..indent {
                out.push_str("  ");
            }
            out.push_str(text);
            out.push('\n');
        }
        i += next_lt;
    }
    out
}
