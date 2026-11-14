#!/usr/bin/env python3
"""
Automated Typst-to-Word Parity Test Runner & Layout Comparator.

Usage:
  python3 test/run_tests.py [--overlay] [--filter SUBSTR] [--skip-word]

For every categorized `.typ` test under `test/01_paragraphs/` .. `test/09_misc/`:
  1. Compiles `<name>.typ` -> `<name>.typst.pdf` using `typst compile`.
  2. Converts `<name>.typ` -> `<name>.docx` using `target/debug/typst2word`.
  3. Batch-converts all `<name>.docx` -> `<name>.word.pdf` via Microsoft Word
     (staged through `/private/tmp/typst2word_test_stage` to avoid macOS sandbox prompts).
  4. Runs `test/tools/pdf_inspector` to extract exact layout telemetry
     (page count, per-line text, baseline y, width, first/last token, ink bbox,
     and pixel overlap) into `<name>.report.json`.
  5. When `--overlay` is passed, generates a single `<name>.overlay.pdf` per test
     (and a combined `test/all_overlays.pdf` summary) superimposing Typst PDF (Blue)
     and Word PDF (Red) with dark charcoal overlap for direct visual inspection.
"""

import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent
TEST_DIR = Path(__file__).resolve().parent
INSPECTOR_BIN = TEST_DIR / "tools" / "pdf_inspector"
INSPECTOR_SRC = TEST_DIR / "tools" / "pdf_inspector.swift"
TYPST2WORD_BIN = ROOT_DIR / "target" / "debug" / "typst2word"


def resolve_word_stage_dir() -> Path:
    container_data = Path.home() / "Library/Containers/com.microsoft.Word/Data"
    if container_data.is_dir():
        return container_data / "tmp" / f"typst2word_test_stage_{os.getpid()}"
    return Path(f"/private/tmp/typst2word_test_stage_{os.getpid()}")


STAGE_DIR = resolve_word_stage_dir()


def ensure_binaries():
    if not INSPECTOR_BIN.exists() or (
        INSPECTOR_SRC.stat().st_mtime > INSPECTOR_BIN.stat().st_mtime
    ):
        print("[build] Compiling test/tools/pdf_inspector.swift ...")
        subprocess.run(
            ["swiftc", "-O", str(INSPECTOR_SRC), "-o", str(INSPECTOR_BIN)],
            check=True,
        )

    print("[build] Ensuring debug target/debug/typst2word is up to date ...")
    subprocess.run(
        ["cargo", "build", "--bin", "typst2word"],
        cwd=str(ROOT_DIR),
        check=True,
    )


def discover_tests(filter_substr=None):
    tests = []
    for category_dir in sorted(TEST_DIR.iterdir()):
        if not category_dir.is_dir():
            continue
        if not category_dir.name[0].isdigit():
            continue
        for typ_file in sorted(category_dir.glob("*.typ")):
            rel_id = f"{category_dir.name}/{typ_file.stem}"
            if filter_substr and filter_substr not in rel_id:
                continue
            tests.append((rel_id, typ_file))
    return tests


def batch_export_word_pdfs(staged_pairs):
    """
    staged_pairs: list of (staged_docx_path, staged_pdf_path, dest_pdf_path)
    Uses a single silent AppleScript call to Microsoft Word inside Word's sandbox container
    without stealing window focus or showing modal alerts.
    """
    if not staged_pairs:
        return

    was_running = (
        subprocess.run(
            ["pgrep", "-x", "Microsoft Word"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        ).returncode
        == 0
    )
    if not was_running:
        subprocess.run(
            ["open", "-g", "-j", "-a", "Microsoft Word"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=True,
        )
        subprocess.run(
            [
                "osascript",
                "-e",
                'tell application "System Events" to if exists process "Microsoft Word" then set visible of process "Microsoft Word" to false',
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

    script_lines = [
        "with timeout of 60 seconds",
        'tell application "Microsoft Word"',
        "  try",
        "    set startup dialog to false",
        "  end try",
        "  set prevAlerts to display alerts",
        "  set display alerts to alerts none",
        "  try",
    ]
    for docx_p, pdf_p, _ in staged_pairs:
        if pdf_p.exists():
            pdf_p.unlink()
        script_lines.extend(
            [
                f'    open POSIX file "{docx_p}" with read only without add to recent files',
                "    set targetDoc to active document",
                f'    save as targetDoc file name "{pdf_p}" file format format PDF without add to recent files',
                "    close targetDoc saving no",
            ]
        )
    script_lines.extend(
        [
            "  on error errMsg number errNum",
            "    try",
            "      set display alerts to prevAlerts",
            "    end try",
            "    error errMsg number errNum",
            "  end try",
            "  try",
            "    set display alerts to prevAlerts",
            "  end try",
            "end tell",
            "end timeout",
        ]
    )
    script = "\n".join(script_lines)

    print(f"[word] Silently exporting {len(staged_pairs)} .docx files to PDF via Microsoft Word ...")
    try:
        subprocess.run(["osascript", "-e", script], check=True, timeout=70)
    finally:
        if not was_running:
            subprocess.run(
                [
                    "osascript",
                    "-e",
                    'tell application "System Events" to if exists process "Microsoft Word" then set visible of process "Microsoft Word" to false',
                ],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )

    for _, pdf_p, dest_pdf_p in staged_pairs:
        if pdf_p.exists():
            shutil.copy2(pdf_p, dest_pdf_p)
        else:
            print(f"[warn] Word did not produce expected PDF: {pdf_p}", file=sys.stderr)
    shutil.rmtree(STAGE_DIR, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(description="Run Typst-to-Word parity test suite.")
    parser.add_argument(
        "--overlay",
        action="store_true",
        help="Generate single-file <name>.overlay.pdf visual comparisons.",
    )
    parser.add_argument(
        "--filter",
        type=str,
        default=None,
        help="Only run tests whose category/name contains this substring.",
    )
    parser.add_argument(
        "--skip-word",
        action="store_true",
        help="Reuse existing .word.pdf files without invoking Microsoft Word.",
    )
    args = parser.parse_args()

    ensure_binaries()
    STAGE_DIR.mkdir(parents=True, exist_ok=True)

    tests = discover_tests(args.filter)
    if not tests:
        print("No matching tests found.")
        return

    print(f"[suite] Discovered {len(tests)} categorized test cases.")

    staged_word_jobs = []
    compiled_tests = []

    def compile_one(item):
        rel_id, typ_path = item
        typst_pdf = typ_path.with_suffix(".typst.pdf")
        docx_path = typ_path.with_suffix(".docx")
        word_pdf = typ_path.with_suffix(".word.pdf")

        res_typ = subprocess.run(
            ["typst", "compile", str(typ_path), str(typst_pdf)],
            cwd=str(ROOT_DIR),
            capture_output=True,
            text=True,
        )
        if res_typ.returncode != 0:
            print(f"[ERROR] typst compile failed for {rel_id}:\n{res_typ.stderr}")
            return None

        res_d2w = subprocess.run(
            [str(TYPST2WORD_BIN), str(typ_path), str(docx_path)],
            cwd=str(ROOT_DIR),
            capture_output=True,
            text=True,
        )
        if res_d2w.returncode != 0:
            print(f"[ERROR] typst2word failed for {rel_id}:\n{res_d2w.stderr}")
            return None

        safe_slug = rel_id.replace("/", "__")
        staged_docx = STAGE_DIR / f"{safe_slug}.docx"
        staged_pdf = STAGE_DIR / f"{safe_slug}.word.pdf"
        shutil.copy2(docx_path, staged_docx)
        return (rel_id, typ_path, typst_pdf, docx_path, word_pdf, staged_docx, staged_pdf)

    print(f"  -> Compiling {len(tests)} test cases in parallel (.typst.pdf & .docx) ...")
    with ThreadPoolExecutor() as pool:
        results = list(pool.map(compile_one, tests))

    for item in results:
        if item is None:
            continue
        rel_id, typ_path, typst_pdf, docx_path, word_pdf, staged_docx, staged_pdf = item
        if not args.skip_word or not word_pdf.exists():
            staged_word_jobs.append((staged_docx, staged_pdf, word_pdf))
        compiled_tests.append((rel_id, typ_path, typst_pdf, docx_path, word_pdf))

    if staged_word_jobs:
        batch_export_word_pdfs(staged_word_jobs)

    summary_rows = []
    overlay_files = []
    for rel_id, typ_path, typst_pdf, docx_path, word_pdf in compiled_tests:
        if not word_pdf.exists():
            print(f"[ERROR] Missing {word_pdf} for {rel_id}")
            continue

        report_json = typ_path.with_suffix(".report.json")
        overlay_pdf = typ_path.with_suffix(".overlay.pdf")

        cmd = [str(INSPECTOR_BIN), str(typst_pdf), str(word_pdf), "--title", rel_id]
        if args.overlay:
            cmd.extend(["--overlay", str(overlay_pdf)])

        res_insp = subprocess.run(cmd, capture_output=True, text=True, check=True)
        report_json.write_text(res_insp.stdout, encoding="utf-8")
        data = json.loads(res_insp.stdout)

        if args.overlay and overlay_pdf.exists():
            overlay_files.append(str(overlay_pdf))

        t_pages = data["typstPageCount"]
        w_pages = data["wordPageCount"]
        t_lines = data["totalTypstLines"]
        w_lines = data["totalWordLines"]
        exact_lines = data["totalExactLineMatches"]
        exact_pct = 100.0 * data["overallLineParityRate"]
        mean_dy = data["overallMeanAbsDeltaYPt"]
        max_dy = data["overallMaxAbsDeltaYPt"]
        mean_dw = (
            sum(p["meanAbsDeltaWidthPt"] for p in data["pages"]) / len(data["pages"])
            if data["pages"]
            else 0.0
        )
        overlap_pct = (
            100.0
            * sum(p["pixelOverlapRate"] for p in data["pages"])
            / len(data["pages"])
            if data["pages"]
            else 100.0
        )
        ink_top_delta = (
            data["pages"][0]["inkTopDeltaPt"] if data["pages"] else 0.0
        )

        row = {
            "id": rel_id,
            "typstPages": t_pages,
            "wordPages": w_pages,
            "typstLines": t_lines,
            "wordLines": w_lines,
            "exactLines": exact_lines,
            "exactLinePct": round(exact_pct, 2),
            "meanAbsDeltaBaselineYPt": round(mean_dy, 2),
            "maxAbsDeltaBaselineYPt": round(max_dy, 2),
            "meanAbsDeltaWidthPt": round(mean_dw, 2),
            "inkTopDeltaPt": round(ink_top_delta, 2),
            "pixelOverlapPct": round(overlap_pct, 2),
            "overlayPdf": str(overlay_pdf.relative_to(ROOT_DIR)) if args.overlay else None,
        }
        summary_rows.append(row)

    if args.overlay and overlay_files:
        master_overlay = TEST_DIR / "overlay_comparison.pdf"
        subprocess.run(
            [str(INSPECTOR_BIN), "--merge", str(master_overlay)] + overlay_files,
            check=True,
        )
        print(f"[overlay] Merged all {len(overlay_files)} overlay PDFs into single file: {master_overlay}")

    summary_path = TEST_DIR / "test_summary.json"
    summary_path.write_text(
        json.dumps({"totalTests": len(summary_rows), "tests": summary_rows}, indent=2),
        encoding="utf-8",
    )

    print("\n" + "=" * 108)
    print(
        f"{'Test ID':<48} | {'Pages':<5} | {'Lines (T/W)':<11} | {'ExactBrk':<9} | {'Mean dy':<8} | {'Mean dw':<8} | {'InkOvlp':<7}"
    )
    print("-" * 108)
    for r in summary_rows:
        pg_str = f"{r['typstPages']}/{r['wordPages']}"
        ln_str = f"{r['typstLines']}/{r['wordLines']}"
        ex_str = f"{r['exactLinePct']:>6.1f}%"
        dy_str = f"{r['meanAbsDeltaBaselineYPt']:>5.2f}pt"
        dw_str = f"{r['meanAbsDeltaWidthPt']:>5.2f}pt"
        ov_str = f"{r['pixelOverlapPct']:>5.1f}%"
        print(
            f"{r['id']:<48} | {pg_str:<5} | {ln_str:<11} | {ex_str:<9} | {dy_str:<8} | {dw_str:<8} | {ov_str:<7}"
        )
    print("=" * 108)
    print(f"Saved detailed summary to {summary_path}")


if __name__ == "__main__":
    main()
