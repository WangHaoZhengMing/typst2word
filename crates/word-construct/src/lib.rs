//! `word-construct` — Spatial, Line-Breaking, Pagination, and Verification Model for Microsoft Word & Typst.
//!
//! Models:
//! 1. **Font & Line Box Metrics ([`font_metrics`])**:
//!    `OS/2` `usWinAscent` / `usWinDescent`, CJK codepage `word_cjk_factor = 1.3`, and `WordLineBox`.
//! 2. **Line-Breaking Engines ([`linebreak`])**:
//!    - Word's greedy [`FirstFitBreaker`] + CJK [`KinsokuSet`] (避头尾) backtracking.
//!    - Typst's optimal [`KnuthPlassBreaker`] with two-stage approximate pruning (`HYPH_COST = 0.5`,
//!      `RUNT_COST = 0.5`, `CONSECUTIVE_DASH_COST = 0.3`).
//! 3. **Pagination & Widow/Orphan Solver ([`paginator`])**:
//!    Implements `word-rules` (P0–P10) and `word-rules2` (Q1–Q7): `WidowOrphanPolicy`, `KeepLinesPolicy`,
//!    `KeepWithNextPolicy` chains, `SpaceBeforeTopPolicy`, `ContextualSpacingPolicy`, and `<w:tblHeader/>`.
//! 4. **Read-Only PDF Line Anchor Verification Harness ([`anchor`])**:
//!    Decoupled `AnchorLineIr` and LCS-based `compare_line_anchors` diagnostic comparator.
//! 5. **Paragraph, List, and Table Box Solvers ([`paragraph_box`], [`list_box`], [`table_box`], [`units`])**.

pub use ir as model;
pub mod anchor;
pub mod document;
pub mod policy;
pub mod units;
pub mod equation_box;
pub mod font_metrics;
pub mod linebreak;
pub mod list_box;
pub mod objects;
pub mod paginator;
pub mod page_calibration;
pub mod paragraph_box;
pub mod pipeline;
pub mod table_box;
mod table_lowering;

pub use anchor::{
    AnchorDiffEntry, AnchorDiffReport, AnchorLineIr, AnchorMismatchKind, compare_line_anchors,
};
pub use document::{SolvedNodeLayout, SolvedParagraphLineDxa, WordDocument, build_word_document};
pub use equation_box::{
    EquationBoxInput, EquationVerticalProfile, FractionScaleMode, WordEquationBox,
};
pub use font_metrics::{
    FontMetricsProvider, FontMetricsResolver, FontParadigm, ParagraphInlineProfile, RawFontMetrics,
    ShapingWorkaround, WORD_CJK_FACTOR, WordFontClass, WordFontMetricsProvider, WordLineBox,
    WordLineBoxSpec, estimate_text_advance_pt, is_cjk_or_fullwidth_char, lookup_known_font,
    str_has_cjk_chars,
};
pub use linebreak::{
    BrokenLine, FirstFitBreaker, InlineSpan, InlineToken, KnuthPlassBreaker, LineBreakCause,
    LineBreaker, Ptls7BreakClass, Ptls7PairAction, TokenKind, WordScriptOverridePolicy,
    break_paragraph_with_policy, is_aat_only_cjk_font_triggering_dflt_override,
    is_aat_only_cjk_font_with_metrics, tokenize_spans, tokenize_spans_with_script_policy,
};
pub use pipeline::flow_spacing::{BlockMarginCarrier, PageVerticalBand, WordBlockVerticalBox};
pub use list_box::{
    EnumMarkerWidthKind, FollowingBlockKind, ListItemLayoutInput, WordListItemLayout,
};
pub use objects::{
    BlockSequenceNeighborContext, WordBlockObjectKind, cetz, code, equation, figure, heading, list,
    paragraph, shape, table, text_run,
};
pub use paginator::{
    LayoutBlock, LayoutParagraph, LayoutTableRow, PageConstraint, PaginatedLine, PaginatedPage,
    paginate_blocks,
};
pub use paragraph_box::{
    InterBlockGapAction, NextBlockSpacingContext, ParagraphScaleProfile, WordJc,
    WordParagraphConstruct,
};
pub use policy::{
    ContextualSpacingPolicy, DEFAULT_KINSOKU_LINE_END_FORBIDDEN,
    DEFAULT_KINSOKU_LINE_START_FORBIDDEN, KeepLinesPolicy, KeepWithNextPolicy, KinsokuSet,
    LineBreakPolicy, ParagraphLayoutConstraint, RowBreakPolicy, RowHeaderRepeatPolicy,
    SpaceBeforeTopPolicy, TYPST_CONSECUTIVE_DASH_COST, TYPST_HYPH_COST, TYPST_MAX_COST,
    TYPST_MIN_RATIO, TYPST_RUNT_COST, WidowOrphanPolicy,
};
pub use table_box::{TableCellRole, WordCellMarginsDxa};
pub use units::{
    Dxa, EighthPt, Emu, HalfPt, Pt, PtWordExt, WLine, dxa_to_pt, pt_to_dxa, pt_to_eighth_pt,
    pt_to_emu, pt_to_half_pt,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cjk_line_box_metrics() {
        // `PingFang SC` has `hhea = 1.400`, CJK `1.3x` -> `M_word = 1.820` (`20.02pt` at 11pt),
        // and `sCapHeight = 0.860` (`9.46pt` at 11pt). With `leading = 7.15pt`, Typst steps by
        // `9.46 + 7.15 = 16.61pt`, which maps to `w:line = round(240 * 16.61 / 20.02) = 199`.
        let lb = WordLineBox::from_typst_metrics(
            Some("PingFang SC"),
            true,
            false,
            false,
            11.0,
            Some(7.15),
        );
        assert_eq!(lb.w_line_240, 199);
        assert!((lb.line_box_height_pt - 16.60).abs() < 0.15);
        assert!((lb.ink_height_pt - 9.46).abs() < 0.1);
    }

    #[test]
    fn test_t5_word_cjk_factor_simsun_12pt_single_spacing() {
        let provider = WordFontMetricsProvider::new();
        let line_h = provider.single_line_height_pt(Some("SimSun"), 12.0);
        assert!(
            (line_h - 15.6).abs() < 0.2,
            "SimSun 12pt single line spacing should be ~15.6pt, got {line_h}"
        );
        let songti_h = provider.single_line_height_pt(Some("宋体"), 12.0);
        assert!((songti_h - 15.6).abs() < 0.2);
    }

    #[test]
    fn test_t3_first_fit_explicit_breaks_and_chinese_kinsoku() {
        let metrics = WordFontMetricsProvider::new();
        let kinsoku = KinsokuSet::default();

        // 1. Explicit `\n` and consecutive `\n\n`
        let spans = vec![InlineSpan::new("第一行\n\n第三行", 10.0)];
        let lines = break_paragraph_with_policy(
            &spans,
            100.0,
            false,
            LineBreakPolicy::FirstFit,
            &kinsoku,
            &metrics,
        );
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].visible_text(), "第一行");
        assert_eq!(lines[1].visible_text(), "");
        assert_eq!(lines[2].visible_text(), "第三行");

        // 2. Kinsoku line-start forbidden (`，`): 5 CJK chars (50pt) + `，` (10pt) in a 50pt line.
        // Without kinsoku, `，` would land at the start of line 2. With kinsoku, the 5th char
        // (`五`) is backtracked so line 2 starts with `五，`.
        let spans_kinsoku = vec![InlineSpan::new("一二三四五，六七", 10.0)];
        let lines_k = break_paragraph_with_policy(
            &spans_kinsoku,
            50.0,
            false,
            LineBreakPolicy::FirstFit,
            &kinsoku,
            &metrics,
        );
        assert_eq!(lines_k.len(), 2);
        assert_eq!(lines_k[0].visible_text(), "一二三四");
        assert_eq!(lines_k[1].visible_text(), "五，六七");

        // 3. Kinsoku line-end forbidden (`（`): 4 CJK chars + `（` at the end of a 50pt line.
        // `（` must not stay at the end of line 1; it moves to the start of line 2.
        let spans_open = vec![InlineSpan::new("一二三四（五六）", 10.0)];
        let lines_open = break_paragraph_with_policy(
            &spans_open,
            50.0,
            false,
            LineBreakPolicy::FirstFit,
            &kinsoku,
            &metrics,
        );
        assert_eq!(lines_open.len(), 2);
        assert_eq!(lines_open[0].visible_text(), "一二三四");
        assert_eq!(lines_open[1].visible_text(), "（五六）");
    }

    #[test]
    fn test_t6_knuth_plass_vs_first_fit_justified_balance() {
        let metrics = WordFontMetricsProvider::new();
        let kinsoku = KinsokuSet::default();
        let text = "Some texts feature many longer words that challenge simple greedy breakers.";
        let spans = vec![InlineSpan::new(text, 10.0)];

        let kp_lines = break_paragraph_with_policy(
            &spans,
            115.0,
            true,
            LineBreakPolicy::typst_knuth_plass(),
            &kinsoku,
            &metrics,
        );
        assert!(kp_lines.len() >= 3);
        assert_eq!(kp_lines.first().unwrap().first_token(), "Some");
        assert_eq!(kp_lines.last().unwrap().last_token(), "breakers.");
    }

    #[test]
    fn test_t4_paginator_widow_orphan_and_keep_with_next() {
        let metrics = WordFontMetricsProvider::new();
        // Page with 50pt content height; each line is 15pt -> at most 3 lines fit on Page 1.
        let page = PageConstraint::a4_default().with_content_height(50.0);

        // A 4-line paragraph (each char 10pt, width 20pt -> 2 chars per line -> 4 lines for 8 chars).
        // 3 lines (45pt) would fit on Page 1, leaving 1 widow line on Page 2.
        // With default WidowOrphanPolicy (min_top=2, min_bottom=2), the paginator must backtrack
        // 1 line so Page 1 has 2 lines and Page 2 has 2 lines!
        let mut p = LayoutParagraph::new(vec![InlineSpan::new("一二三四五六七八", 10.0)])
            .with_line_height(15.0);
        p.left_indent_pt = page.content_width_pt() - 20.0;

        let pages = paginate_blocks(&[LayoutBlock::Paragraph(p)], &page, &metrics);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].lines.len(), 2, "Page 1 should backtrack from 3 to 2 lines");
        assert_eq!(pages[1].lines.len(), 2, "Page 2 should receive 2 lines (no widow)");
    }

    #[test]
    fn test_t1_anchor_lcs_diff_harness() {
        let golden = vec![
            AnchorLineIr::new(1, 0, "一", "四").with_geometry(70.0, 85.0, 40.0),
            AnchorLineIr::new(1, 1, "五", "七").with_geometry(70.0, 100.0, 30.0),
        ];
        let candidate = vec![
            AnchorLineIr::new(1, 0, "一", "四").with_geometry(70.0, 85.5, 40.0),
            AnchorLineIr::new(1, 1, "五", "七").with_geometry(70.0, 100.5, 30.0),
        ];
        let report = compare_line_anchors(&golden, &candidate, 2.0, 2.0);
        assert_eq!(report.exact_token_matches, 2);
        assert!((report.token_parity_rate - 1.0).abs() < 1e-6);
        assert!(report.passes_acceptance(0.99));
    }

    #[test]
    fn test_single_block_list_item_preserves_explicit_space_after() {
        let layout = WordListItemLayout::solve(ListItemLayoutInput {
            ilvl: 0,
            tight: false,
            indent_pt: 14.17,
            hanging_pt: 19.4,
            marker_width: EnumMarkerWidthKind::SingleDigitDecimal,
            item_gap_dxa: 121,
            item_end_after_dxa: 121,
            before_dxa: 0,
            num_blocks: 1,
            first_para_space_after_pt: Some(118.35),
            first_para_line_spacing_pt: Some(7.15),
            following_block: FollowingBlockKind::None,
        });
        assert_eq!(layout.first_after_dxa, pt_to_dxa(118.35));
        assert_eq!(layout.indent_left_dxa, 671);
        assert_eq!(layout.hanging_dxa, 300);
    }

    #[test]
    fn test_font_paradigm_and_word_line_box_spec() {
        let font_size = Pt(11.0);
        let typst_leading = Pt(7.15); // 0.65em

        let pingfang = lookup_known_font("PingFang SC").expect("PingFang SC");
        let spec_pf = WordLineBoxSpec::compute("PingFang SC", pingfang, font_size, typst_leading);
        assert_eq!(spec_pf.w_line, WLine(199));
        assert_eq!(spec_pf.workaround, ShapingWorkaround::None);

        let songti = lookup_known_font("Songti SC").expect("Songti SC");
        let spec_st = WordLineBoxSpec::compute("Songti SC", songti, font_size, typst_leading);
        assert_eq!(spec_st.w_line, WLine(177));
        assert_eq!(spec_st.workaround, ShapingWorkaround::ForceCjkScriptBinding);

        // Verify binary `FontFaceMetricsIr` + `ParagraphLineMetricsIr` calibration:
        // 1) System-installed CJK font (`PingFang SC`) uses binary `hhea_span * 1.3` (`1.820`)
        //    and measured `uniform_baseline_step_pt` (`16.61pt`) -> `w:line = 199`.
        let pf_face = ir::FontFaceMetricsIr {
            upem: 1000,
            hhea_ascender: 1060,
            hhea_descender: -340,
            hhea_line_gap: 0,
            s_cap_height: Some(860),
            s_typo_ascender: Some(860),
            s_typo_descender: Some(-140),
            us_win_ascent: Some(1060),
            us_win_descent: Some(340),
            has_cjk_codepage: true,
            has_gsub_hani: true,
            is_monospaced: false,
            is_system_installed: true,
        };
        let pf_lines = ir::ParagraphLineMetricsIr {
            line_count: 4,
            first_baseline_y_pt: 78.79,
            last_baseline_y_pt: 128.62,
            uniform_baseline_step_pt: Some(16.61),
            has_tall_inline_object: false,
        };
        let lb_cal = WordLineBox::from_paragraph_metrics(
            Some("PingFang SC"),
            Some(&pf_face),
            Some(&pf_lines),
            true,
            false,
            false,
            11.0,
            Some(7.15),
        );
        assert_eq!(lb_cal.w_line_240, 199);

        // 2) Embedded-only font (`Libertinus Serif`, `is_system_installed == false`) falls back
        //    to Word's `fontTable.xml` substitute (`Times New Roman`, `M_word = 1.150`) -> `274`.
        let lib_face = ir::FontFaceMetricsIr {
            upem: 1000,
            hhea_ascender: 891,
            hhea_descender: -216,
            hhea_line_gap: 0,
            s_cap_height: Some(662),
            s_typo_ascender: Some(689),
            s_typo_descender: Some(-216),
            us_win_ascent: Some(975),
            us_win_descent: Some(254),
            has_cjk_codepage: false,
            has_gsub_hani: false,
            is_monospaced: false,
            is_system_installed: false,
        };
        let lb_emb = WordLineBox::from_paragraph_metrics(
            Some("Libertinus Serif"),
            Some(&lib_face),
            None,
            false,
            false,
            false,
            12.0,
            Some(7.80),
        );
        assert_eq!(lb_emb.w_line_240, 274);

        let raw_custom = RawFontMetrics {
            family_name: "CustomAppleSong",
            upm: 1000,
            win_ascent: 1060,
            win_descent: 340,
            hhea_ascent: 1060,
            hhea_descent: -340,
            has_cjk_codepage: true,
            is_serif: true,
            is_monospace: false,
            has_valid_gsub_hani: false,
        };
        let dyn_paradigm = FontParadigm::from(&raw_custom);
        assert!(matches!(dyn_paradigm, FontParadigm::AppleCjkQuirk { .. }));
        assert_eq!(
            dyn_paradigm.shaping_workaround(),
            ShapingWorkaround::ForceCjkScriptBinding
        );
    }

    #[test]
    fn test_inter_block_gap_before_large_display_paragraph() {
        let action = WordParagraphConstruct::solve_inter_block_gap(
            19.5,
            11.0,
            19.8,
            NextBlockSpacingContext::Paragraph,
        );
        match action {
            InterBlockGapAction::SetExactSpaceAfter(pt) => {
                assert!((pt - 10.18).abs() < 0.2);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn test_word_equation_box_2dof_height_and_width_calibration() {
        // 1. 15pt inline equation (`$2024-2025$` inside `#text(15pt)`): preserves exact 15pt (`w:sz="30"`)
        let eq_15pt = WordEquationBox::solve(EquationBoxInput {
            is_block: false,
            typst_width_pt: Some(76.6),
            typst_height_pt: Some(15.0),
            typst_max_glyph_pt: Some(15.0),
            typst_min_glyph_pt: Some(15.0),
            context_font_size_pt: 12.0,
            max_available_width_pt: Some(453.5),
            row_count: 1,
            max_fraction_depth: 0,
            has_tall_construct: false,
            all_glyphs_in_fraction_or_script: false,
            expected_em_width: 5.1,
        });
        assert_eq!(eq_15pt.run_font_size_pt, 15.0);

        // 2. Inline fraction-only equation (`$a/b$` in 12pt text where Typst glyphs are 8.4pt = 0.7 * 12pt):
        //    restores 12pt base size so Word does not double-shrink the fraction.
        let eq_inline_frac = WordEquationBox::solve(EquationBoxInput {
            is_block: false,
            typst_width_pt: Some(18.0),
            typst_height_pt: Some(18.5),
            typst_max_glyph_pt: Some(8.4),
            typst_min_glyph_pt: Some(8.4),
            context_font_size_pt: 12.0,
            max_available_width_pt: Some(453.5),
            row_count: 1,
            max_fraction_depth: 1,
            has_tall_construct: true,
            all_glyphs_in_fraction_or_script: true,
            expected_em_width: 1.5,
        });
        assert_eq!(eq_inline_frac.run_font_size_pt, 12.0);

        // 3. Inline `$ display(a/b) $` where Typst laid out fraction glyphs at 12pt (100%):
        //    compensates `fraction_font_size_pt` (`12.0 / 0.73 = 16.5pt`) so Word inline `<m:f>` renders at 12pt.
        let eq_display_in_inline = WordEquationBox::solve(EquationBoxInput {
            is_block: false,
            typst_width_pt: Some(24.0),
            typst_height_pt: Some(24.0),
            typst_max_glyph_pt: Some(12.0),
            typst_min_glyph_pt: Some(12.0),
            context_font_size_pt: 12.0,
            max_available_width_pt: Some(453.5),
            row_count: 1,
            max_fraction_depth: 1,
            has_tall_construct: true,
            all_glyphs_in_fraction_or_script: true,
            expected_em_width: 2.0,
        });
        assert!(eq_display_in_inline.fraction_font_size_pt.unwrap() >= 16.0);
    }

    #[test]
    fn test_t7_ptls7_songti_sc_dflt_override_and_emergency_mid_word_truncation_hong_kong() {
        let metrics = WordFontMetricsProvider::new();
        let kinsoku = KinsokuSet::default();
        let avail_width_pt = 453.54_f64;

        // Verify AAT-only font detection (`0x103ebda81` -> `0x10044afdc` `script_id = 5` `'DFLT'`)
        assert!(is_aat_only_cjk_font_triggering_dflt_override(Some("Songti SC")));
        assert!(!is_aat_only_cjk_font_triggering_dflt_override(Some("SimSun")));
        assert!(!is_aat_only_cjk_font_triggering_dflt_override(Some("宋体")));

        // Verify PTLS7 22x22 pair table (`0x1d24e0`) collapses CJK + `，` to class 0 (`LatinWord`)
        // under `OpenTypeDfltLatinOverride`, prohibiting breaks between `(0, 0)`.
        let c_ideograph = Ptls7BreakClass::classify(
            '内',
            WordScriptOverridePolicy::OpenTypeDfltLatinOverride,
            &kinsoku,
        );
        let c_comma = Ptls7BreakClass::classify(
            '，',
            WordScriptOverridePolicy::OpenTypeDfltLatinOverride,
            &kinsoku,
        );
        assert_eq!(c_ideograph, Ptls7BreakClass::LatinWord);
        assert_eq!(c_comma, Ptls7BreakClass::LatinWord);
        assert_eq!(
            c_ideograph.pair_action(c_comma),
            Ptls7PairAction::Prohibited
        );

        // 1. Paragraph 1 of `香港.typ` with `"Songti SC"` at 11pt:
        //    Line 2 overflows at 41 chars (`451.0pt <= 453.54pt`) and emergency-truncates right
        //    before `，`, placing `，一起向大陆模式转型...` at the start of Line 3!
        let p1 = "2023年的香港可以用一句话总结：社会管理体系全面向大陆模式转型。这一转型动作非常的坚决和彻底，从行政体系到立法体系、司法体系，乃至包括最底层的区议员选举，全都包含在内，一起向大陆模式转型。而香港版本的区议员，其实就跟大陆的居委会主任差不多，应对的就是社区居民之间的家长里短，其实基本上没有立法权，也没啥参政议政的权利。即便是这样，也没躲过大陆化的命运，同样被管束的死死的。香港当局对这种转型趋势也完全不加掩饰，香港市长李家超的2023年度施政报告开篇的整整38页，都是说如何加强爱国主义，如何调整香港的管理模式去适应大陆，如何融入大湾区，也就是全面向大陆。";
        let p1_lines = break_paragraph_with_policy(
            &[InlineSpan::new(p1, 11.0).with_font("Songti SC")],
            avail_width_pt,
            true,
            LineBreakPolicy::word_first_fit_auto_font(),
            &kinsoku,
            &metrics,
        );
        assert_eq!(p1_lines.len(), 7);
        assert_eq!(
            p1_lines[0].visible_text(),
            "2023年的香港可以用一句话总结：社会管理体系全面向大陆模式转型。这一转型动作非常的坚"
        );
        assert!(p1_lines[0].is_emergency_truncated());
        assert_eq!(
            p1_lines[1].visible_text(),
            "决和彻底，从行政体系到立法体系、司法体系，乃至包括最底层的区议员选举，全都包含在内"
        );
        assert!(p1_lines[1].is_emergency_truncated());
        assert_eq!(
            p1_lines[2].visible_text(),
            "，一起向大陆模式转型。而香港版本的区议员，其实就跟大陆的居委会主任差不多，应对的就"
        );
        assert!(p1_lines[2].visible_text().starts_with('，'));

        // 2. Paragraph 4 of `香港.typ` with `"Songti SC"` at 11pt:
        //    - Line 4 (`Line 22` on Page 1) breaks prematurely right after `"非常普通的 seaside"`
        //      (`NormalWrap` at the ASCII space `U+0020`) because the following 69-char CJK string
        //      `"小城。基于...万幸了。"` (`759.0pt`) has zero internal break opportunities!
        //    - Line 5 (`Line 23` on Page 1) starts `"小城。基于..."` at `x = 0` and emergency-truncates
        //      it at 41 chars (`...死了就是个广西北海的水` | `平，靠廉价的海鲜...`).
        let p4 = "就是说，如果香港不再具备独特的桥梁作用，其金融体系无法再维持的话，那么就香港的人口和经济体量来说，根本就不足以撑起“特区”。香港没有制造业、航运业主要也就是服务于大陆的转口贸易，连金融也都垮掉的话，那么香港的经济规模也会迅速缩减，最终退化为一个非常非常普通的 seaside 小城。基于他已经彻彻底底的产业空心化，所以他一定比不上厦门，死了就是个广西北海的水平，靠廉价的海鲜吸引一下东北游客，这都已经算是万幸了。";
        let p4_lines = break_paragraph_with_policy(
            &[InlineSpan::new(p4, 11.0).with_font("Songti SC")],
            avail_width_pt,
            true,
            LineBreakPolicy::word_first_fit_auto_font(),
            &kinsoku,
            &metrics,
        );
        assert_eq!(p4_lines.len(), 6);
        assert_eq!(p4_lines[3].visible_text(), "非常普通的 seaside");
        assert_eq!(p4_lines[3].break_cause, LineBreakCause::NormalWrap);
        assert!(p4_lines[3].width_pt < 100.0);
        assert_eq!(
            p4_lines[4].visible_text(),
            "小城。基于他已经彻彻底底的产业空心化，所以他一定比不上厦门，死了就是个广西北海的水"
        );
        assert!(p4_lines[4].is_emergency_truncated());
        assert_eq!(
            p4_lines[5].visible_text(),
            "平，靠廉价的海鲜吸引一下东北游客，这都已经算是万幸了。"
        );

        // 3. Paragraph 5 of `香港.typ` with `"Songti SC"` at 11pt:
        //    Emergency mid-word character truncation slices right through multi-digit year `"2022"`
        //    at the end of Line 9 (`...资金迅速的撤离香港。20` | `22年香港股市的融资规模...`).
        let p5 = "接下来我会系统性地介绍香港最新的经济数据，详细讲述香港丧失自己的桥梁优势，从国际金融中心沦落为普通的大陆地级市的过渡过程。这里首先要讲述的就是香港的核心金融数据——香港股市数据。港交所每个月都会发布详细的市场数据报告，我这里将他从2018年到今年一季度的核心数据全部摘取出来，形成了下表。注意最重要的数据就是香港股票市场的融资额，这个数据就是反映一个地区金融活跃度最重要的指标，没有之一。金融活跃度越高，优质企业和投资资金就越聚集，相对应的股票市场的融资规模当然也就越高。从数据表中可以看出，2020年和2021年是香港股市融资规模最高的两年，融资规模都超过了7000亿港币。这个时候，很多在美国上市的中国科技企业因为中美决裂的缘故回归到香港上市，因此大幅拉高了当年度的融资规模。然而此后的情况就急转直下，伴随着香港桥梁地位的丧失，资金迅速的撤离香港。2022年香港股市的融资规模迅速下降到2509亿，这种下降已经不能用腰斩来形容了，只能说是脚踝斩。2023年继续暴跌到1507亿，就这种融资规模来说，香港已经完全称不上是国际金融中心了。";
        let p5_lines = break_paragraph_with_policy(
            &[InlineSpan::new(p5, 11.0).with_font("Songti SC")],
            avail_width_pt,
            true,
            LineBreakPolicy::word_first_fit_auto_font(),
            &kinsoku,
            &metrics,
        );
        assert!(p5_lines[8].visible_text().ends_with("资金迅速的撤离香港。20"));
        assert!(p5_lines[8].is_emergency_truncated());
        assert!(p5_lines[9].visible_text().starts_with("22年香港股市的融资规模"));

        // 4. Paragraph 9 of `香港.typ` with `"Songti SC"` at 11pt:
        //    - Line 1 breaks prematurely right after `——` (`问题还不止这些，香港剩下的最后那条就业通道——`,
        //      only 23 chars = 253pt) because PTLS7 classifies `—` (`U+2014`) as class 5 (`Hyphen`),
        //      forbidding `(5, 5)` between consecutive dashes but allowing a `NormalWrap` after `——`
        //      when the following 267-char CJK token overflows `453.54pt`!
        //    - Line 6 slices `"3120"` into `"312"` | `"0亿美元"` via `EmergencyMidWordTruncation`.
        let p9 = "问题还不止这些，香港剩下的最后那条就业通道——转口贸易，现在也陷入了剧烈的萎缩状态。长期以来，香港就是大陆最大的转口贸易港，现在这个身份也在迅速丧失。欧美世界开始对进出香港的货物执行与大陆一视同仁的政策，取消了香港在外贸上的优惠待遇。因此，此前通过香港转一下手，以规避欧美的贸易禁令的做法现在基本上行不通了。这种影响体现在数据上就是香港对大陆的贸易数据日益萎缩。2021年香港对大陆的出口规模3785亿美元，到2023年锐减到了2975亿美元；2021年香港自大陆进口的规模3120亿美元，到2023年锐减到了2593亿美元。就现在整个欧美世界动不动就取消香港的优惠待遇，并强化对港货品的监察力度的趋势来看，2024年香港的转口贸易数据还要继续剧烈萎缩。";
        let p9_lines = break_paragraph_with_policy(
            &[InlineSpan::new(p9, 11.0).with_font("Songti SC")],
            avail_width_pt,
            true,
            LineBreakPolicy::word_first_fit_auto_font(),
            &kinsoku,
            &metrics,
        );
        assert_eq!(
            p9_lines[0].visible_text(),
            "问题还不止这些，香港剩下的最后那条就业通道——"
        );
        assert_eq!(p9_lines[0].break_cause, LineBreakCause::NormalWrap);
        assert!(p9_lines[5].visible_text().ends_with("进口的规模312"));
        assert!(p9_lines[5].is_emergency_truncated());
        assert!(p9_lines[6].visible_text().starts_with("0亿美元，到2023年锐减到了2593亿美元。"));
    }

    #[test]
    fn test_t8_knuth_plass_shrunk_line_glue_compensation_latin_and_cjk() {
        use ir::{HAlignIr, InlineNode, LinebreaksIr, ParagraphStyleIr, TextRunIr};
        use crate::paragraph::{ParagraphConstruct, ParagraphInlineConstruct};

        // 1. `par_pad_left_right`: 320pt page, left=24pt, right=24pt -> avail=272pt, 12pt Times New Roman
        let latin_run = InlineNode::Text(TextRunIr {
            text: "In olden times when wishing still helped one, there lived a king whose daughters were all beautiful, and the youngest was so beautiful that the sun itself wondered.".to_string(),
            font_family: Some("Times New Roman".to_string()),
            font_size_pt: Some(12.0),
            ..Default::default()
        });
        let mut pad_style = ParagraphStyleIr::default().with_align(HAlignIr::Justify);
        pad_style.justify = true;
        pad_style.linebreaks = LinebreaksIr::Auto;
        pad_style.left_indent_pt = 24.0;
        pad_style.right_indent_pt = 24.0;

        let solved_pad = ParagraphConstruct::solve_inlines(
            std::slice::from_ref(&latin_run),
            &pad_style,
            320 * 20,
        )
        .expect("Knuth-Plass shrunk lines should produce glue adjustments");
        assert!(solved_pad.iter().any(|it| matches!(
            it,
            ParagraphInlineConstruct::AdjustedText {
                char_spacing_dxa: Some(d),
                ..
            } if *d < 0
        )));

        // 2. `par_cjk_latin_mixed`: 300pt page, 11pt Songti SC
        let mixed_run = InlineNode::Text(TextRunIr {
            text: "本项目通过 Spring Boot 集成 MyBatis-Plus 与 Redis 缓存组件，在 macOS 平台上验证 OpenXML 排版一致性。".to_string(),
            font_family: Some("Songti SC".to_string()),
            font_size_pt: Some(11.0),
            ..Default::default()
        });
        let mut mix_style = ParagraphStyleIr::default().with_align(HAlignIr::Justify);
        mix_style.justify = true;
        mix_style.linebreaks = LinebreaksIr::Auto;

        let solved_mix = ParagraphConstruct::solve_inlines(
            std::slice::from_ref(&mixed_run),
            &mix_style,
            300 * 20,
        )
        .expect("CJK+Latin mixed paragraph should shrink spaces and fullwidth comma on Line 1");
        assert!(solved_mix.iter().any(|it| matches!(
            it,
            ParagraphInlineConstruct::AdjustedText {
                run,
                char_spacing_dxa: Some(d),
            } if run.text == "，" && *d < -40
        )));

        // 3. `par_cjk_kinsoku`: 240pt page, 12pt Songti SC — Line 2 attaches negative dxa to `号` preceding `（`
        let kinsoku_run = InlineNode::Text(TextRunIr {
            text: "测试中文排版避头尾规则，确保句号、逗号、感叹号不会出现在行首，并且左括号（如“引号”与《书名号》）不会孤立在行尾。".to_string(),
            font_family: Some("Songti SC".to_string()),
            font_size_pt: Some(12.0),
            ..Default::default()
        });
        let solved_kin = ParagraphConstruct::solve_inlines(
            std::slice::from_ref(&kinsoku_run),
            &mix_style,
            240 * 20,
        )
        .expect("CJK kinsoku paragraph should shrink fullwidth punctuation on Lines 1 & 2");
        assert!(solved_kin.iter().any(|it| matches!(
            it,
            ParagraphInlineConstruct::AdjustedText {
                run,
                char_spacing_dxa: Some(d),
            } if run.text == "号" && *d < -60
        )));

        // 4. `linebreaks: "simple"` must NEVER produce Knuth-Plass glue shrinkage!
        let mut simple_style = mix_style.clone();
        simple_style.linebreaks = LinebreaksIr::Simple;
        assert!(
            ParagraphConstruct::solve_inlines(
                std::slice::from_ref(&mixed_run),
                &simple_style,
                300 * 20
            )
            .is_none()
        );
    }

    #[test]
    fn test_inline_and_block_equation_single_line_height_calibration() {
        use std::collections::BTreeMap;
        use crate::equation::EquationBlockConstruct;
        use crate::paragraph::{compute_line_spacing_dxa_with_metrics, primary_font_size_pt};
        use ir::{EquationIr, EquationLayoutMetricsIr, InlineNode, ParagraphLineMetricsIr, TextRunIr};

        // 9.5pt Times New Roman inline-math paragraph prefixed by 7pt Menlo `[001]` badges:
        // Dominant size must resolve to 9.5pt (not 7.0pt), dominant font to `Times New Roman`,
        // and script-expanded `uniform_baseline_step_pt = 13.41pt` must NOT be double-counted.
        let badge = InlineNode::Text(TextRunIr {
            text: "[001]".to_string(),
            font_family: Some("Menlo".to_string()),
            font_size_pt: Some(7.0),
            ..Default::default()
        });
        let sep = InlineNode::Text(TextRunIr {
            text: " · ".to_string(),
            font_family: Some("Times New Roman".to_string()),
            font_size_pt: Some(9.5),
            ..Default::default()
        });
        let inline_eq = InlineNode::EquationInline(EquationIr {
            id: ir::NodeId::UNSET,
            math_font_family: None,
            is_block: false,
            body: Vec::new(),
            source_text: "$x_i^2 + y_i^2$".to_string(),
            numbering_text: None,
            bookmark: None,
            font_size_pt: Some(9.5),
            layout_metrics: EquationLayoutMetricsIr::Unmeasured {
                context_font_size_pt: 9.5,
                max_glyph_pt: Some(9.5),
                min_glyph_pt: Some(6.7),
                max_available_width_pt: None,
            },
        });
        let content = vec![badge, inline_eq, sep];
        assert!((primary_font_size_pt(&content) - 9.5).abs() < 1e-6);

        let para_lines = ParagraphLineMetricsIr {
            line_count: 4,
            first_baseline_y_pt: 20.0,
            last_baseline_y_pt: 60.23,
            uniform_baseline_step_pt: Some(13.41),
            has_tall_inline_object: false,
        };
        let w_line = compute_line_spacing_dxa_with_metrics(
            &content,
            Some(0.52 * 9.5),
            &BTreeMap::new(),
            Some(&para_lines),
        );
        assert!(
            (244..=258).contains(&w_line),
            "Expected calibrated single-line inline-math w:line ~247..255, got {w_line}"
        );

        let block_eq = EquationIr {
            id: ir::NodeId::UNSET,
            math_font_family: None,
            is_block: true,
            body: Vec::new(),
            source_text: "$ a + b = c $".to_string(),
            numbering_text: None,
            bookmark: None,
            font_size_pt: Some(9.5),
            layout_metrics: EquationLayoutMetricsIr::Unmeasured {
                context_font_size_pt: 9.5,
                max_glyph_pt: Some(9.5),
                min_glyph_pt: None,
                max_available_width_pt: None,
            },
        };
        let solved_eq = EquationBlockConstruct::solve(&block_eq, 450.0, 0, 0, 0);
        assert_eq!(
            solved_eq,
            EquationBlockConstruct::CenteredDisplayParagraph {
                indent_left_dxa: 0,
                before_dxa: 0,
                after_dxa: 0,
                mark_sz_half_pt: 19,
            }
        );
    }

    #[test]
    fn test_p1_font_metrics_resolver_binary_first_and_unified_cjk() {
        use std::collections::BTreeMap;

        // 1. Unified CJK character predicate includes Hangul and CJK Ext B (previously missed).
        assert!(is_cjk_or_fullwidth_char('中'));
        assert!(is_cjk_or_fullwidth_char('한'));
        assert!(is_cjk_or_fullwidth_char('𠀀'));
        assert!(!is_cjk_or_fullwidth_char('A'));

        // 2. WordFontMetricsProvider aligns with WordLineBox on PingFang SC (`1.82x`) & SimSun/宋体 (`1.30x`).
        let provider = WordFontMetricsProvider::new();
        assert!((provider.single_line_height_pt(Some("PingFang SC"), 10.0) - 18.2).abs() < 0.05);
        assert!((provider.single_line_height_pt(Some("SimSun"), 10.0) - 13.0).abs() < 0.05);
        assert!((provider.single_line_height_pt(Some("宋体"), 10.0) - 13.0).abs() < 0.05);

        // 3. Binary OpenType metrics drive first-line overhang & AAT detection for unknown fonts.
        let mut faces = BTreeMap::new();
        faces.insert(
            "CustomCjkAat".to_string(),
            ir::FontFaceMetricsIr {
                upem: 1000,
                hhea_ascender: 900,
                hhea_descender: -200,
                hhea_line_gap: 0,
                s_cap_height: Some(750),
                s_typo_ascender: Some(880),
                s_typo_descender: Some(-120),
                us_win_ascent: Some(900),
                us_win_descent: Some(200),
                has_cjk_codepage: true,
                has_gsub_hani: false,
                is_monospaced: false,
                is_system_installed: true,
            },
        );
        faces.insert(
            "CustomLatinSans".to_string(),
            ir::FontFaceMetricsIr {
                upem: 2048,
                hhea_ascender: 1854,
                hhea_descender: -434,
                hhea_line_gap: 67,
                s_cap_height: Some(1467),
                s_typo_ascender: Some(1491),
                s_typo_descender: Some(-431),
                us_win_ascent: Some(1854),
                us_win_descent: Some(434),
                has_cjk_codepage: false,
                has_gsub_hani: false,
                is_monospaced: false,
                is_system_installed: true,
            },
        );

        let resolver = FontMetricsResolver::new(&faces);
        assert!(resolver.is_cjk_font(Some("CustomCjkAat")));
        assert!(!resolver.is_cjk_font(Some("CustomLatinSans")));
        assert!(is_aat_only_cjk_font_with_metrics(Some("CustomCjkAat"), &faces));
        assert!(!is_aat_only_cjk_font_with_metrics(
            Some("CustomLatinSans"),
            &faces
        ));

        // CustomCjkAat at 12pt: base_asc=0.90, hhea_span=1.10, word_baseline=0.90 + 0.15*1.10 = 1.065
        // overhang = 12 * (1.065 - 0.750) = 3.78pt (NOT fallback 4.80pt!)
        let cjk_delta = resolver.first_line_top_overhang_pt(Some("CustomCjkAat"), Some(12.0), true);
        assert!(
            (cjk_delta - 3.78).abs() < 0.02,
            "expected ~3.78pt binary overhang, got {cjk_delta}"
        );

        // CustomLatinSans (Arial binary metrics) at 12pt: (1854+67)/2048 - 1467/2048 = 0.22168 => 2.66pt
        let latin_delta =
            resolver.first_line_top_overhang_pt(Some("CustomLatinSans"), Some(12.0), false);
        assert!(
            (latin_delta - 2.66).abs() < 0.05,
            "expected ~2.66pt binary overhang, got {latin_delta}"
        );
    }

    #[test]
    fn test_p1_word_block_vertical_box_unifies_inter_block_air() {
        let para = ir::BlockNode::Paragraph {
            id: ir::NodeId::UNSET,
            content: Vec::new(),
            style: ir::ParagraphStyleIr {
                resolved_font_size_pt: Some(10.0),
                ..Default::default()
            },
            bookmark: None,
        };
        let shape = ir::BlockNode::ShapeBlock(ir::ShapeIr {
            id: ir::NodeId::UNSET,
            kind: ir::ShapeKindIr::Rect,
            width_pt: 100.0,
            height_pt: 20.0,
            fill: None,
            fill_rule_even_odd: false,
            stroke: None,
            borders: Default::default(),
            corner_radius_pt: None,
            inset: Default::default(),
            rotation_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            body: Vec::new(),
            align: ir::HAlignIr::Left,
            placed_offset_pt: None,
            baseline_shift_pt: None,
        });
        let hr = ir::BlockNode::HorizontalRule {
            id: ir::NodeId::UNSET,
            stroke: Default::default(),
            length_pt: 200.0,
            align: ir::HAlignIr::Left,
            left_indent_pt: 0.0,
        };

        let box_para = WordBlockVerticalBox::for_block(&para, 10.0);
        let box_shape = WordBlockVerticalBox::for_block(&shape, 10.0);
        let box_hr = WordBlockVerticalBox::for_block(&hr, 10.0);

        // Shape -> Shape subtracts 0.02 + 0.02 = 0.04 * 10 = 0.4pt
        assert!((WordBlockVerticalBox::needed_gap_pt(box_shape, box_shape, 10.0) - 9.6).abs() < 1e-6);
        // Shape -> Paragraph subtracts 0.02 + 0.20 = 0.22 * 10 = 2.2pt
        assert!((WordBlockVerticalBox::needed_gap_pt(box_shape, box_para, 10.0) - 7.8).abs() < 1e-6);
        // HorizontalRule -> Paragraph subtracts 0.0 + 0.20 = 2.0pt
        assert!((WordBlockVerticalBox::needed_gap_pt(box_hr, box_para, 10.0) - 8.0).abs() < 1e-6);
        // Paragraph -> HorizontalRule subtracts 0.18 + 0.0 = 1.8pt
        assert!((WordBlockVerticalBox::needed_gap_pt(box_para, box_hr, 10.0) - 8.2).abs() < 1e-6);
    }

    #[test]
    fn test_single_line_cjk_auto_table_column_expands_for_word_fullwidth_punctuation() {
        let mut ir = ir::DocumentIr::default();
        ir.page_setup.width_pt = 595.28;
        ir.page_setup.margin_left_pt = 70.87;
        ir.page_setup.margin_right_pt = 70.87;

        let make_para = |id: u32, text: &str, bold: bool| ir::BlockNode::Paragraph {
            id: ir::NodeId(id),
            content: vec![ir::InlineNode::Text(ir::TextRunIr {
                text: text.to_string(),
                font_family: Some("Hei".to_string()),
                font_size_pt: Some(11.0),
                bold,
                ..Default::default()
            })],
            style: ir::ParagraphStyleIr {
                resolved_font_size_pt: Some(11.0),
                ..Default::default()
            },
            bookmark: None,
        };
        let inset_5 = ir::SidesPtIr {
            top: 5.0,
            bottom: 5.0,
            left: 5.0,
            right: 5.0,
        };
        for (id, w_pt, lines) in [(1, 27.5, 1), (2, 122.2, 1), (3, 27.5, 1), (4, 390.0, 3)] {
            ir.reference_layout.insert(
                ir::NodeId(id),
                ir::NodeReferenceLayout {
                    fragments: vec![ir::LayoutFragmentIr {
                        page_idx: 0,
                        x_pt: 0.0,
                        top_y_pt: 0.0,
                        width_pt: w_pt,
                        height_pt: 11.0 * lines as f64,
                    }],
                    paragraph_lines: Some(ir::ParagraphLineMetricsIr {
                        line_count: lines,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            );
        }

        // Case 1: Narrow `columns: (auto, auto)` table where col 0 was 41pt in Typst
        // (`姓名：` compressed to 27.5pt + 10pt inset + 3.5pt slack), which wraps in Word.
        let narrow_tbl = ir::TableIr {
            id: ir::NodeId(10),
            kind: ir::TableKindIr::SemanticTable,
            cols: vec![
                ir::TableColTrackIr::Auto { width_pt: 41.0 },
                ir::TableColTrackIr::Auto { width_pt: 138.2 },
            ],
            row_heights_pt: vec![21.75],
            header_rows: 0,
            footer_rows: 0,
            rows: vec![ir::TableRowIr {
                height_pt: None,
                is_header: false,
                is_footer: false,
                cells: vec![
                    ir::TableCellIr {
                        x: 0,
                        y: 0,
                        colspan: 1,
                        rowspan: 1,
                        fill: None,
                        borders: Default::default(),
                        inset: inset_5,
                        halign: ir::HAlignIr::Right,
                        valign: ir::VAlignIr::Center,
                        is_header_cell: false,
                        blocks: vec![make_para(1, "姓名：", true)],
                    },
                    ir::TableCellIr {
                        x: 1,
                        y: 0,
                        colspan: 1,
                        rowspan: 1,
                        fill: None,
                        borders: Default::default(),
                        inset: inset_5,
                        halign: ir::HAlignIr::Center,
                        valign: ir::VAlignIr::Center,
                        is_header_cell: false,
                        blocks: vec![make_para(2, "计算机学科学与技术2301", false)],
                    },
                ],
            }],
            align: ir::HAlignIr::Center,
            left_indent_pt: 0.0,
            summary: None,
        };

        // Case 2: Full-width `columns: (auto, 1fr)` table (total 453.54pt) borrows deficit
        // from the flexible `Fraction` column without exceeding page content width.
        let full_tbl = ir::TableIr {
            id: ir::NodeId(11),
            kind: ir::TableKindIr::SemanticTable,
            cols: vec![
                ir::TableColTrackIr::Auto { width_pt: 41.0 },
                ir::TableColTrackIr::Fraction {
                    fr: 1.0,
                    width_pt: 412.54,
                },
            ],
            row_heights_pt: vec![44.0],
            header_rows: 0,
            footer_rows: 0,
            rows: vec![ir::TableRowIr {
                height_pt: None,
                is_header: false,
                is_footer: false,
                cells: vec![
                    ir::TableCellIr {
                        x: 0,
                        y: 0,
                        colspan: 1,
                        rowspan: 1,
                        fill: None,
                        borders: Default::default(),
                        inset: inset_5,
                        halign: ir::HAlignIr::Right,
                        valign: ir::VAlignIr::Center,
                        is_header_cell: false,
                        blocks: vec![make_para(3, "姓名：", true)],
                    },
                    ir::TableCellIr {
                        x: 1,
                        y: 0,
                        colspan: 1,
                        rowspan: 1,
                        fill: None,
                        borders: Default::default(),
                        inset: inset_5,
                        halign: ir::HAlignIr::Left,
                        valign: ir::VAlignIr::Top,
                        is_header_cell: false,
                        blocks: vec![make_para(4, "这是一段会在多行内自动换行的长正文内容", false)],
                    },
                ],
            }],
            align: ir::HAlignIr::Left,
            left_indent_pt: 0.0,
            summary: None,
        };

        // Case 3: Explicit `Fixed` columns (`columns: (41pt, 140pt)`) preserve author-specified
        // widths verbatim and solve to `TableLayoutMode::Fixed`.
        let fixed_tbl = ir::TableIr {
            id: ir::NodeId(12),
            kind: ir::TableKindIr::SemanticTable,
            cols: vec![
                ir::TableColTrackIr::Fixed { width_pt: 41.0 },
                ir::TableColTrackIr::Fixed { width_pt: 140.0 },
            ],
            row_heights_pt: vec![21.75],
            header_rows: 0,
            footer_rows: 0,
            rows: narrow_tbl.rows.clone(),
            align: ir::HAlignIr::Left,
            left_indent_pt: 0.0,
            summary: None,
        };

        ir.blocks = vec![
            ir::BlockNode::Table(narrow_tbl),
            ir::BlockNode::Table(full_tbl),
            ir::BlockNode::Table(fixed_tbl),
        ];
        let doc = build_word_document(&ir);

        let ir::BlockNode::Table(solved_narrow) = &doc.blocks[0] else {
            panic!("expected narrow table");
        };
        assert!(solved_narrow.cols.iter().all(|c| c.is_auto()));
        assert!(
            solved_narrow.col_width_pt(0) >= 45.5,
            "narrow auto col 0 should expand past Word bold CJK+colon width + tcMar, got {}",
            solved_narrow.col_width_pt(0)
        );
        assert!((solved_narrow.col_width_pt(1) - 138.2).abs() < 1e-6);
        assert_eq!(
            crate::table::TableConstruct::solve(solved_narrow).layout_mode,
            crate::table::TableLayoutMode::Autofit
        );

        let ir::BlockNode::Table(solved_full) = &doc.blocks[1] else {
            panic!("expected full-width table");
        };
        assert!(
            solved_full.col_width_pt(0) >= 45.5,
            "full-width auto col 0 should borrow from Fraction sibling, got {}",
            solved_full.col_width_pt(0)
        );
        let full_sum: f64 = solved_full.total_width_pt();
        assert!(
            (full_sum - 453.54).abs() < 0.05,
            "full-width table must preserve total content width, got {full_sum}"
        );
        assert_eq!(
            crate::table::TableConstruct::solve(solved_full).layout_mode,
            crate::table::TableLayoutMode::Fixed
        );

        let ir::BlockNode::Table(solved_fixed) = &doc.blocks[2] else {
            panic!("expected fixed table");
        };
        assert!((solved_fixed.col_width_pt(0) - 41.0).abs() < 1e-6);
        assert!((solved_fixed.col_width_pt(1) - 140.0).abs() < 1e-6);
        assert_eq!(
            crate::table::TableConstruct::solve(solved_fixed).layout_mode,
            crate::table::TableLayoutMode::Fixed
        );
    }
}
