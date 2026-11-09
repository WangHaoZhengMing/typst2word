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

pub mod anchor;
pub mod equation_box;
pub mod font_metrics;
pub mod linebreak;
pub mod list_box;
pub mod paginator;
pub mod paragraph_box;
pub mod policy;
pub mod table_box;
pub mod units;

pub use anchor::{
    AnchorDiffEntry, AnchorDiffReport, AnchorLineIr, AnchorMismatchKind, compare_line_anchors,
};
pub use equation_box::{
    EquationBoxInput, EquationVerticalProfile, FractionScaleMode, WordEquationBox,
};
pub use font_metrics::{
    FontMetricsProvider, FontParadigm, ParagraphInlineProfile, RawFontMetrics, ShapingWorkaround,
    WORD_CJK_FACTOR, WordFontClass, WordFontMetricsProvider, WordLineBox, WordLineBoxSpec,
    is_cjk_or_fullwidth_char, lookup_known_font, str_has_cjk_chars,
};
pub use linebreak::{
    BrokenLine, FirstFitBreaker, InlineSpan, InlineToken, KnuthPlassBreaker, LineBreakCause,
    LineBreaker, Ptls7BreakClass, Ptls7PairAction, TokenKind, WordScriptOverridePolicy,
    break_paragraph_with_policy, is_aat_only_cjk_font_triggering_dflt_override, tokenize_spans,
    tokenize_spans_with_script_policy,
};
pub use list_box::{
    EnumMarkerWidthKind, FollowingBlockKind, ListItemLayoutInput, WordListItemLayout,
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
    Dxa, EighthPt, Emu, HalfPt, Pt, WLine, dxa_to_pt, pt_to_dxa, pt_to_eighth_pt, pt_to_emu,
    pt_to_half_pt,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cjk_line_box_metrics() {
        let lb = WordLineBox::from_typst_metrics(
            Some("PingFang SC"),
            true,
            false,
            false,
            11.0,
            Some(7.15),
        );
        assert_eq!(lb.w_line_240, 278);
        assert!(lb.line_box_height_pt > 16.5 && lb.line_box_height_pt < 18.0);
        assert!((lb.ink_height_pt - 10.56).abs() < 0.1);
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
        let layout = WordListItemLayout::solve_from_input(ListItemLayoutInput {
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
        assert_eq!(spec_pf.w_line, WLine(278));
        assert_eq!(spec_pf.workaround, ShapingWorkaround::None);

        let songti = lookup_known_font("Songti SC").expect("Songti SC");
        let spec_st = WordLineBoxSpec::compute("Songti SC", songti, font_size, typst_leading);
        assert_eq!(spec_st.w_line, WLine(181));
        assert_eq!(spec_st.workaround, ShapingWorkaround::ForceCjkScriptBinding);

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
        let action = WordParagraphConstruct::solve_inter_block_gap_adjustment(
            19.5,
            11.0,
            19.8,
            false,
            true,
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
}
