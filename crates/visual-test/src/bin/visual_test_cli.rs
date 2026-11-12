use std::path::PathBuf;

use visual_test::model::{CaseStatus, Category};
use visual_test::runner::{RunConfig, run_suite};

fn parse_category(arg: &str) -> Option<Category> {
    let q = arg.to_ascii_lowercase();
    Category::ALL.into_iter().find(|c| {
        c.slug().contains(&q) || c.title().to_ascii_lowercase().contains(&q)
    })
}

fn main() {
    let mut cfg = RunConfig::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--skip-word" => cfg.skip_word = true,
            "--fresh-word" => cfg.fresh_word = true,
            "--category" | "-c" => {
                if let Some(val) = args.next() {
                    match parse_category(&val) {
                        Some(cat) => cfg.category_filter = Some(cat),
                        None => {
                            eprintln!("Unknown category filter: {val}");
                            std::process::exit(2);
                        }
                    }
                }
            }
            "--filter" | "-f" => {
                cfg.id_substring_filter = args.next();
            }
            "--out-dir" | "-o" => {
                if let Some(p) = args.next() {
                    cfg.out_dir = PathBuf::from(p);
                }
            }
            "--scale" => {
                if let Some(s) = args.next()
                    && let Ok(v) = s.parse::<f64>()
                {
                    cfg.raster_scale = v.clamp(1.0, 4.0);
                }
            }
            "--help" | "-h" => {
                println!(
                    "Usage: visual-test-cli [--skip-word] [--fresh-word] [-c <category>] [-f <substr>] [-o <dir>] [--scale <f64>]"
                );
                return;
            }
            other => {
                eprintln!("Unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }

    println!(
        "==> Running Single-Variable (margin: 10pt) Visual Test Suite  [word_pdf={}, out={}]",
        !cfg.skip_word,
        cfg.out_dir.display()
    );

    let report = match run_suite(&cfg) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("ERROR: {err}");
            std::process::exit(1);
        }
    };

    let s = &report.stats;
    println!(
        "\n================================================================================================"
    );
    println!(
        "  TOTAL: {}  |  PASS: {}  |  DRIFT: {}  |  FAIL: {}  |  IR-ONLY: {}",
        s.total_cases, s.pass_count, s.drift_count, s.fail_count, s.ir_only_count
    );
    if report.word_enabled {
        println!(
            "  MEAN |Δ|: left={:.2}pt  top={:.2}pt  width={:.2}pt  height={:.2}pt  |  Raw IoU={:.1}%  Aligned IoU={:.1}%",
            s.mean_abs_delta_left_pt,
            s.mean_abs_delta_top_pt,
            s.mean_abs_delta_width_pt,
            s.mean_abs_delta_height_pt,
            s.mean_raw_iou * 100.0,
            s.mean_aligned_iou * 100.0,
        );
    }
    println!(
        "================================================================================================\n"
    );

    for cat in Category::ALL {
        let items: Vec<_> = report.cases.iter().filter(|c| c.category == cat).collect();
        if items.is_empty() {
            continue;
        }
        let cat_pass = items.iter().filter(|c| c.status == CaseStatus::Pass).count();
        let cat_drift = items.iter().filter(|c| c.status == CaseStatus::Drift).count();
        let cat_fail = items.iter().filter(|c| c.status == CaseStatus::Fail).count();
        println!(
            "--- {}  (pass={}, drift={}, fail={}) ---",
            cat.title(),
            cat_pass,
            cat_drift,
            cat_fail
        );
        for c in items {
            if let Some(ref cmp) = c.comparison {
                println!(
                    "  [{:<5}] {:<32} ΔL={:+6.2}  ΔT={:+6.2}  ΔW={:+6.2}  ΔH={:+6.2}  IoU={:5.1}%/{:5.1}%  {}",
                    c.status.label(),
                    c.id,
                    cmp.delta_left_pt,
                    cmp.delta_top_pt,
                    cmp.delta_width_pt,
                    cmp.delta_height_pt,
                    cmp.raw_iou * 100.0,
                    cmp.aligned_iou * 100.0,
                    if c.status_reasons.is_empty() {
                        String::new()
                    } else {
                        format!("<- {}", c.status_reasons.join("; "))
                    }
                );
            } else {
                println!(
                    "  [{:<5}] {:<32} {}",
                    c.status.label(),
                    c.id,
                    if c.status_reasons.is_empty() {
                        "IR & OOXML verified".to_string()
                    } else {
                        c.status_reasons.join("; ")
                    }
                );
            }
        }
        println!();
    }

    println!(
        "Artifacts written to:\n  - JSON Report: {}/report.json\n  - Markdown Summary: {}/SUMMARY.md",
        report.out_dir, report.out_dir
    );

    if s.fail_count > 0 {
        std::process::exit(1);
    }
}
