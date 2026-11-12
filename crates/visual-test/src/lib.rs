pub mod catalog;
pub mod model;
pub mod runner;
#[cfg(feature = "gui")]
pub mod ui;
#[cfg(feature = "gui")]
pub use ui as gui;
pub mod verify;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use compiler::{TypstWorld, compile_world};
    use extract::extract_document_ir;
    use ir::BlockNode;

    use crate::catalog::build_catalog;
    use crate::model::{CaseStatus, Category};
    use crate::runner::{RunConfig, run_suite};

    #[test]
    fn catalog_covers_all_stdlib_categories_with_unique_ids() {
        let specs = build_catalog();
        assert!(
            specs.len() >= 165,
            "expected at least 165 atomic typesetting cases, got {}",
            specs.len()
        );

        let mut seen_ids = HashSet::new();
        for cat in Category::ALL {
            let count = specs.iter().filter(|s| s.category == cat).count();
            assert!(
                count >= 1,
                "category {:?} ({}) must have at least 1 suite case",
                cat,
                cat.slug()
            );
        }

        let margin_cases = specs
            .iter()
            .filter(|s| s.suite_rel_path == "05_layout/page/margins.typ")
            .count();
        assert!(
            margin_cases >= 24,
            "expected >= 24 exhaustive page(margin: ...) cases, got {margin_cases}"
        );

        for s in &specs {
            assert!(
                seen_ids.insert(s.id.clone()),
                "duplicate test case id: {}",
                s.id
            );
            assert!(
                !s.suite_rel_path.is_empty(),
                "missing suite_rel_path for {}",
                s.id
            );
            if !s.suite_rel_path.starts_with("05_layout/page/") {
                assert_eq!(
                    s.expectation.expected_margins_pt,
                    [10.0, 10.0, 10.0, 10.0],
                    "non-page case {} must use uniform 10pt baseline margins",
                    s.id
                );
            }
        }
    }

    #[test]
    fn two_sided_mirror_margins_preserve_zero_false_indent_on_even_pages() {
        let src = concat!(
            "#set page(width: 300pt, height: 120pt, margin: (inside: 40pt, outside: 14pt, y: 12pt), binding: left)\n",
            "#set text(font: \"Times New Roman\", size: 11pt)\n",
            "Odd page paragraph.\n",
            "#pagebreak()\n",
            "Even page paragraph.\n",
        );
        let world = TypstWorld::from_str(src, None);
        let compiled = compile_world(&world).expect("compile two-sided Typst document");
        let ir = extract_document_ir(&compiled, &world);

        assert!(ir.page_setup.mirror_margins);
        assert!((ir.page_setup.margin_left_pt - 40.0).abs() <= 0.05);
        assert!((ir.page_setup.margin_right_pt - 14.0).abs() <= 0.05);
        assert!(
            ir.sections.is_empty(),
            "two-sided mirror margins must remain a single Word section without per-page splits"
        );

        let par_indents: Vec<f64> = ir
            .blocks
            .iter()
            .filter_map(|b| match b {
                BlockNode::Paragraph { style, .. } => Some(style.left_indent_pt),
                _ => None,
            })
            .collect();
        assert_eq!(par_indents.len(), 2);
        assert!(
            par_indents[0].abs() <= 0.5 && par_indents[1].abs() <= 0.5,
            "even-page paragraph must not acquire false negative/positive left indent: {par_indents:?}"
        );
    }

    #[test]
    fn all_single_variable_cases_compile_and_pass_ir_ooxml_verification() {
        let out_dir = std::env::temp_dir().join("typst2word_visual_test_unit");
        let cfg = RunConfig {
            out_dir,
            category_filter: None,
            id_substring_filter: None,
            skip_word: true,
            fresh_word: false,
            raster_scale: 1.5,
        };
        let report = run_suite(&cfg).expect("visual-test IR/OOXML suite must succeed");
        let failed: Vec<_> = report
            .cases
            .iter()
            .filter(|c| c.status == CaseStatus::Fail)
            .map(|c| format!("{} ({}) => {:?}", c.id, c.suite_rel_path, c.status_reasons))
            .collect();
        assert!(
            failed.is_empty(),
            "{} single-variable cases failed IR/OOXML verification:\n{}",
            failed.len(),
            failed.join("\n")
        );
    }
}
