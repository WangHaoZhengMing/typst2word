//! Stage 4: Status classification (`Pass` / `Drift` / `Fail` / `IrOnly`), prose
//! line-break parity analysis, aggregate telemetry, and `report.json` / `SUMMARY.md` writers.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::RunConfig;
use super::compile::CompiledCaseArtifacts;
use super::inspector::InspectorJobOutput;
use crate::model::{AggregateStats, CaseReport, CaseStatus, Category, LabReport, LineMetric};

pub(super) fn build_and_persist_report(
    config: &RunConfig,
    artifacts: Vec<CompiledCaseArtifacts>,
    word_pdfs: &HashMap<String, PathBuf>,
    word_degraded: bool,
    mut insp_map: HashMap<String, InspectorJobOutput>,
) -> LabReport {
    let mut case_reports = Vec::with_capacity(artifacts.len());
    let mut pass_count = 0usize;
    let mut drift_count = 0usize;
    let mut fail_count = 0usize;
    let mut ir_only_count = 0usize;

    let mut sum_dl = 0.0;
    let mut sum_dt = 0.0;
    let mut sum_dw = 0.0;
    let mut sum_dh = 0.0;
    let mut sum_raw_iou = 0.0;
    let mut sum_aligned_iou = 0.0;
    let mut cmp_n = 0usize;

    for art in artifacts {
        let mut reasons = Vec::new();
        if let Some(err) = &art.compile_error {
            reasons.push(err.clone());
        }
        for chk in &art.checks {
            if !chk.passed {
                reasons.push(format!("{}: {}", chk.name, chk.detail));
            }
        }

        let insp = insp_map.remove(&art.spec.id);
        if let Some(ref io) = insp
            && let Some(ref e) = io.error
        {
            reasons.push(e.clone());
        }

        let typst_visual = insp.as_ref().and_then(|i| i.typst.clone());
        let word_visual = insp.as_ref().and_then(|i| i.word.clone());
        let comparison = insp.as_ref().and_then(|i| i.comparison.clone());

        let status = if !reasons.is_empty() {
            fail_count += 1;
            CaseStatus::Fail
        } else if config.skip_word || (word_degraded && comparison.is_none()) {
            ir_only_count += 1;
            CaseStatus::IrOnly
        } else if let Some(ref cmp) = comparison {
            cmp_n += 1;
            sum_dl += cmp.delta_left_pt.abs();
            sum_dt += cmp.delta_top_pt.abs();
            sum_dw += cmp.delta_width_pt.abs();
            sum_dh += cmp.delta_height_pt.abs();
            sum_raw_iou += cmp.raw_iou;
            sum_aligned_iou += cmp.aligned_iou;

            let exp = &art.spec.expectation;
            let mut drift_reasons = Vec::new();
            if cmp.delta_left_pt.abs() > exp.max_delta_left_pt {
                drift_reasons.push(format!(
                    "|Δleft| = {:.2}pt > {:.1}pt",
                    cmp.delta_left_pt, exp.max_delta_left_pt
                ));
            }
            if cmp.delta_top_pt.abs() > exp.max_delta_top_pt {
                drift_reasons.push(format!(
                    "|Δtop| = {:.2}pt > {:.1}pt",
                    cmp.delta_top_pt, exp.max_delta_top_pt
                ));
            }
            if cmp.delta_width_pt.abs() > exp.max_delta_width_pt {
                drift_reasons.push(format!(
                    "|Δwidth| = {:.2}pt > {:.1}pt",
                    cmp.delta_width_pt, exp.max_delta_width_pt
                ));
            }
            if cmp.delta_height_pt.abs() > exp.max_delta_height_pt {
                drift_reasons.push(format!(
                    "|Δheight| = {:.2}pt > {:.1}pt",
                    cmp.delta_height_pt, exp.max_delta_height_pt
                ));
            }
            if let (Some(tv), Some(wv)) = (&typst_visual, &word_visual) {
                if tv.page_count != wv.page_count {
                    drift_reasons.push(format!(
                        "page_count Typst={} vs Word={}",
                        tv.page_count, wv.page_count
                    ));
                }
                if matches!(
                    art.spec.target_func.as_str(),
                    "par" | "text" | "heading" | "title"
                ) && let Some(line_diff) = detect_prose_linebreak_mismatch(&tv.lines, &wv.lines)
                {
                    drift_reasons.push(line_diff);
                }
            }

            if drift_reasons.is_empty() {
                pass_count += 1;
                CaseStatus::Pass
            } else {
                drift_count += 1;
                reasons = drift_reasons;
                CaseStatus::Drift
            }
        } else {
            fail_count += 1;
            reasons.push("Missing Word PDF comparison output".to_string());
            CaseStatus::Fail
        };

        let word_pdf_path = word_pdfs
            .get(&art.spec.id)
            .map(|p| p.to_string_lossy().to_string());

        case_reports.push(CaseReport {
            id: art.spec.id,
            category: art.spec.category,
            suite_rel_path: art.spec.suite_rel_path,
            target_func: art.spec.target_func,
            target_param: art.spec.target_param,
            title: art.spec.title,
            isolated_variable: art.spec.isolated_variable,
            status,
            status_reasons: reasons,
            typst_source: art.spec.typst_source,
            case_dir: art.case_dir.to_string_lossy().to_string(),
            ir_json_path: art.ir_json_path.to_string_lossy().to_string(),
            document_xml_path: art.document_xml_path.to_string_lossy().to_string(),
            docx_path: art.docx_path.to_string_lossy().to_string(),
            typst_pdf_path: art.typst_pdf_path.to_string_lossy().to_string(),
            word_pdf_path,
            ir_summary: art.ir_summary,
            checks: art.checks,
            typst_visual,
            word_visual,
            comparison,
        });
    }

    let denom = (cmp_n.max(1)) as f64;
    let stats = AggregateStats {
        total_cases: case_reports.len(),
        pass_count,
        drift_count,
        fail_count,
        ir_only_count,
        mean_abs_delta_left_pt: round2(sum_dl / denom),
        mean_abs_delta_top_pt: round2(sum_dt / denom),
        mean_abs_delta_width_pt: round2(sum_dw / denom),
        mean_abs_delta_height_pt: round2(sum_dh / denom),
        mean_raw_iou: round4(sum_raw_iou / denom),
        mean_aligned_iou: round4(sum_aligned_iou / denom),
    };

    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let report = LabReport {
        generated_at_unix: now_unix,
        out_dir: config.out_dir.to_string_lossy().to_string(),
        word_enabled: !config.skip_word && !word_degraded,
        stats,
        cases: case_reports,
    };

    persist_report_files(config, &report);
    report
}

fn persist_report_files(config: &RunConfig, report: &LabReport) {
    // Only overwrite global report.json / SUMMARY.md when running unfiltered suite
    // (or if report.json does not exist yet) so subset runs merge into the full report.
    let report_json_path = config.out_dir.join("report.json");
    if !config.is_filtered() || !report_json_path.exists() {
        if let Ok(js) = serde_json::to_string_pretty(report) {
            let _ = fs::write(&report_json_path, js);
        }
        let summary_md = render_markdown_summary(report);
        let _ = fs::write(config.out_dir.join("SUMMARY.md"), summary_md);
    } else if let Ok(existing_str) = fs::read_to_string(&report_json_path)
        && let Ok(mut existing_rep) = serde_json::from_str::<LabReport>(&existing_str)
    {
        if report.word_enabled {
            existing_rep.word_enabled = true;
        }
        existing_rep.generated_at_unix = report.generated_at_unix;
        for updated in &report.cases {
            if let Some(slot) = existing_rep.cases.iter_mut().find(|c| c.id == updated.id) {
                *slot = updated.clone();
            } else {
                existing_rep.cases.push(updated.clone());
            }
        }
        recompute_aggregate_stats(&mut existing_rep);
        if let Ok(js) = serde_json::to_string_pretty(&existing_rep) {
            let _ = fs::write(&report_json_path, js);
        }
        let summary_md = render_markdown_summary(&existing_rep);
        let _ = fs::write(config.out_dir.join("SUMMARY.md"), summary_md);
    }
}

pub fn recompute_aggregate_stats(rep: &mut LabReport) {
    let mut pass = 0usize;
    let mut drift = 0usize;
    let mut fail = 0usize;
    let mut ir_only = 0usize;
    let mut dl = 0.0;
    let mut dt = 0.0;
    let mut dw = 0.0;
    let mut dh = 0.0;
    let mut riou = 0.0;
    let mut aiou = 0.0;
    let mut n = 0usize;

    for c in &rep.cases {
        match c.status {
            CaseStatus::Pass => pass += 1,
            CaseStatus::Drift => drift += 1,
            CaseStatus::Fail => fail += 1,
            CaseStatus::IrOnly => ir_only += 1,
        }
        if let Some(ref cmp) = c.comparison {
            n += 1;
            dl += cmp.delta_left_pt.abs();
            dt += cmp.delta_top_pt.abs();
            dw += cmp.delta_width_pt.abs();
            dh += cmp.delta_height_pt.abs();
            riou += cmp.raw_iou;
            aiou += cmp.aligned_iou;
        }
    }
    let denom = (n.max(1)) as f64;
    rep.stats = AggregateStats {
        total_cases: rep.cases.len(),
        pass_count: pass,
        drift_count: drift,
        fail_count: fail,
        ir_only_count: ir_only,
        mean_abs_delta_left_pt: round2(dl / denom),
        mean_abs_delta_top_pt: round2(dt / denom),
        mean_abs_delta_width_pt: round2(dw / denom),
        mean_abs_delta_height_pt: round2(dh / denom),
        mean_raw_iou: round4(riou / denom),
        mean_aligned_iou: round4(aiou / denom),
    };
}

pub fn render_markdown_summary(report: &LabReport) -> String {
    let mut md = String::with_capacity(16384);
    let s = &report.stats;
    let _ = writeln!(
        md,
        "# Typst-to-Word Single-Variable (`margin: 10pt`) Visual Test Report\n"
    );
    let _ = writeln!(
        md,
        "- **Total Atomic Cases**: `{}`  |  **PASS**: `{}`  |  **DRIFT**: `{}`  |  **FAIL**: `{}`  |  **IR-Only**: `{}`",
        s.total_cases, s.pass_count, s.drift_count, s.fail_count, s.ir_only_count
    );
    let _ = writeln!(
        md,
        "- **Mean Absolute BBox Deltas**: `|Δleft| = {:.2}pt`, `|Δtop| = {:.2}pt`, `|Δwidth| = {:.2}pt`, `|Δheight| = {:.2}pt`",
        s.mean_abs_delta_left_pt,
        s.mean_abs_delta_top_pt,
        s.mean_abs_delta_width_pt,
        s.mean_abs_delta_height_pt
    );
    let _ = writeln!(
        md,
        "- **Mean Ink Overlap**: `Raw IoU = {:.1}%`, `Aligned Shape IoU = {:.1}%`\n",
        s.mean_raw_iou * 100.0,
        s.mean_aligned_iou * 100.0
    );

    for cat in Category::ALL {
        let cat_cases: Vec<&CaseReport> =
            report.cases.iter().filter(|c| c.category == cat).collect();
        if cat_cases.is_empty() {
            continue;
        }
        let _ = writeln!(md, "## {}\n", cat.title());
        let _ = writeln!(
            md,
            "| Status | Case ID | Isolated Variable | Typst Ink (L,T,W,H) | Word Ink (L,T,W,H) | ΔL / ΔT / ΔW / ΔH (pt) | Raw / Aligned IoU | Notes |"
        );
        let _ = writeln!(md, "|---|---|---|---|---|---|---|---|");
        for c in cat_cases {
            let tb = c
                .typst_visual
                .as_ref()
                .and_then(|v| v.ink_bbox)
                .map(|b| {
                    format!(
                        "({:.1}, {:.1}, {:.1}, {:.1})",
                        b.left_pt, b.top_pt, b.width_pt, b.height_pt
                    )
                })
                .unwrap_or_else(|| "-".to_string());
            let wb = c
                .word_visual
                .as_ref()
                .and_then(|v| v.ink_bbox)
                .map(|b| {
                    format!(
                        "({:.1}, {:.1}, {:.1}, {:.1})",
                        b.left_pt, b.top_pt, b.width_pt, b.height_pt
                    )
                })
                .unwrap_or_else(|| "-".to_string());
            let deltas = c
                .comparison
                .as_ref()
                .map(|cmp| {
                    format!(
                        "{:+.1} / {:+.1} / {:+.1} / {:+.1}",
                        cmp.delta_left_pt, cmp.delta_top_pt, cmp.delta_width_pt, cmp.delta_height_pt
                    )
                })
                .unwrap_or_else(|| "-".to_string());
            let ious = c
                .comparison
                .as_ref()
                .map(|cmp| {
                    format!(
                        "{:.0}% / {:.0}%",
                        cmp.raw_iou * 100.0,
                        cmp.aligned_iou * 100.0
                    )
                })
                .unwrap_or_else(|| "-".to_string());
            let note = if c.status_reasons.is_empty() {
                "OK".to_string()
            } else {
                c.status_reasons.join("; ")
            };
            let _ = writeln!(
                md,
                "| **{}** | `{}` | `{}` | {} | {} | {} | {} | {} |",
                c.status.label(),
                c.id,
                c.isolated_variable.replace('|', "\\|"),
                tb,
                wb,
                deltas,
                ious,
                note.replace('|', "\\|")
            );
        }
        let _ = writeln!(md);
    }

    md
}

const KANGXI_RADICALS_TO_UNIFIED: [char; 214] = [
    '一', '丨', '丶', '丿', '乙', '亅', '二', '亠', '人', '儿', '入', '八', '冂', '冖', '冫', '几',
    '凵', '刀', '力', '勹', '匕', '匚', '匸', '十', '卜', '卩', '厂', '厶', '又', '口', '囗', '土',
    '士', '夂', '夊', '夕', '大', '女', '子', '宀', '寸', '小', '尢', '尸', '屮', '山', '巛', '工',
    '己', '巾', '干', '幺', '广', '廴', '廾', '弋', '弓', '彐', '彡', '彳', '心', '戈', '戶', '手',
    '支', '攴', '文', '斗', '斤', '方', '无', '日', '曰', '月', '木', '欠', '止', '歹', '殳', '毋',
    '比', '毛', '氏', '气', '水', '火', '爪', '父', '爻', '爿', '片', '牙', '牛', '犬', '玄', '玉',
    '瓜', '瓦', '甘', '生', '用', '田', '疋', '疒', '癶', '白', '皮', '皿', '目', '矛', '矢', '石',
    '示', '禸', '禾', '穴', '立', '竹', '米', '糸', '缶', '网', '羊', '羽', '老', '而', '耒', '耳',
    '聿', '肉', '臣', '自', '至', '臼', '舌', '舛', '舟', '艮', '色', '艸', '虍', '虫', '血', '行',
    '衣', '襾', '見', '角', '言', '谷', '豆', '豕', '豸', '貝', '赤', '走', '足', '身', '車', '辛',
    '辰', '辵', '邑', '酉', '釆', '里', '金', '長', '門', '阜', '隶', '隹', '雨', '靑', '非', '面',
    '革', '韋', '韭', '音', '頁', '風', '飛', '食', '首', '香', '馬', '骨', '高', '髟', '鬥', '鬯',
    '鬲', '鬼', '魚', '鳥', '鹵', '鹿', '麥', '麻', '黃', '黍', '黑', '黹', '黽', '鼎', '鼓', '鼠',
    '鼻', '齊', '齒', '龍', '龜', '龠',
];

fn coalesce_visual_rows(lines: &[LineMetric]) -> Vec<String> {
    let mut sorted: Vec<&LineMetric> =
        lines.iter().filter(|l| !l.text.trim().is_empty()).collect();
    sorted.sort_by(|a, b| {
        a.page
            .cmp(&b.page)
            .then_with(|| a.y_top_pt.total_cmp(&b.y_top_pt))
            .then_with(|| a.x_pt.total_cmp(&b.x_pt))
    });

    type LineRow<'a> = (usize, f64, Vec<(f64, &'a str)>);
    let mut rows: Vec<LineRow<'_>> = Vec::new();
    for lm in sorted {
        if let Some(last) = rows.last_mut()
            && last.0 == lm.page
            && (lm.y_top_pt - last.1).abs() <= 3.0
        {
            last.2.push((lm.x_pt, lm.text.as_str()));
        } else {
            rows.push((lm.page, lm.y_top_pt, vec![(lm.x_pt, lm.text.as_str())]));
        }
    }

    rows.into_iter()
        .map(|(_, _, mut segs)| {
            segs.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut joined = String::new();
            for (_, txt) in segs {
                joined.push_str(txt);
            }
            joined
        })
        .collect()
}

fn detect_prose_linebreak_mismatch(
    typst_lines: &[LineMetric],
    word_lines: &[LineMetric],
) -> Option<String> {
    let normalize = |s: &str| -> String {
        let mut out = String::with_capacity(s.len());
        for raw_c in s.chars() {
            if raw_c.is_whitespace() {
                continue;
            }
            match raw_c {
                'ﬀ' => {
                    out.push_str("ff");
                    continue;
                }
                'ﬁ' => {
                    out.push_str("fi");
                    continue;
                }
                'ﬂ' => {
                    out.push_str("fl");
                    continue;
                }
                'ﬃ' => {
                    out.push_str("ffi");
                    continue;
                }
                'ﬄ' => {
                    out.push_str("ffl");
                    continue;
                }
                _ => {}
            }
            let cp = raw_c as u32;
            let c = if (0x2F00..=0x2FD5).contains(&cp) {
                KANGXI_RADICALS_TO_UNIFIED[(cp - 0x2F00) as usize]
            } else {
                raw_c
            };
            let mapped = match c {
                '⺜' => '日',
                '⺝' | '⺼' => '月',
                '⺟' => '母',
                '⺠' => '民',
                '⺩' => '王',
                '⺫' => '目',
                '⻄' => '西',
                '⻅' => '见',
                '⻆' => '角',
                '⻉' => '贝',
                '⻋' => '车',
                '⻓' => '长',
                '⻔' => '门',
                '⻗' => '雨',
                '⻘' | '靑' => '青',
                '⻙' => '韦',
                '⻚' => '页',
                '⻛' => '风',
                '⻜' => '飞',
                '⻝' => '食',
                '⻢' => '马',
                '⻣' => '骨',
                '⻤' => '鬼',
                '⻥' => '鱼',
                '⻦' => '鸟',
                '⻧' => '卤',
                '⻨' => '麦',
                '⻩' | '黃' => '黄',
                '⻪' => '黾',
                '⻬' => '齐',
                '⻮' => '齿',
                '⻰' => '龙',
                '⻳' => '龟',
                '戶' => '户',
                '，' => ',',
                '（' => '(',
                '）' => ')',
                '：' => ':',
                '；' => ';',
                '！' => '!',
                '？' => '?',
                other => other.to_ascii_lowercase(),
            };
            out.push(mapped);
        }
        out
    };

    let t_norm: Vec<String> = coalesce_visual_rows(typst_lines)
        .into_iter()
        .map(|s| normalize(&s))
        .filter(|s| !s.is_empty())
        .collect();
    let w_norm: Vec<String> = coalesce_visual_rows(word_lines)
        .into_iter()
        .map(|s| normalize(&s))
        .filter(|s| !s.is_empty())
        .collect();

    if t_norm.is_empty() || w_norm.is_empty() || t_norm == w_norm {
        return None;
    }

    let max_len = t_norm.len().max(w_norm.len());
    for i in 0..max_len {
        let ts = t_norm.get(i).map(String::as_str).unwrap_or("<EOF>");
        let ws = w_norm.get(i).map(String::as_str).unwrap_or("<EOF>");
        if ts != ws {
            return Some(format!(
                "line_break[{}] mismatch: Typst=\"{}\" vs Word=\"{}\"",
                i + 1,
                ts,
                ws
            ));
        }
    }
    None
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}
