#!/usr/bin/env python3
"""
Visual pixel-level comparison script between Typst and Microsoft Word (.docx).

Workflow:
1. Convert `<input.typ>` -> `<output.docx>` via `typst2word` (limited to 2 CPU threads),
   dumping the intermediate representation to `<stem>.ir.json` by default (`--dump-ir`;
   disable with `--no-ir`), or use an existing `--docx` file.
2. Use macOS AppleScript (`osascript`) to instruct Microsoft Word to export `<output.docx>`
   to PDF (`<stem>.word.pdf`), preserving exact page geometry.
3. Render each page of `<stem>.word.pdf` to PNG (`word_page_raw_<N>.png`) and a lightness-adjusted
   background PNG (`word_page_bg_<N>.png`) with identical paper aspect ratio.
4. Generate `overlay_compare.typ` that sets `#set page(background: image(..., width: 100%, height: 100%))`
   for each page on top of the original Typst document.
5. Compile `overlay_compare.typ` to `overlay_compare.pdf` and `overlay_page_<N>.png` for pixel-level
   inspection.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path


def find_repo_root() -> Path:
    for parent in Path(__file__).resolve().parents:
        if (parent / "Cargo.toml").exists():
            return parent
    return Path.cwd()


def export_docx_to_pdf_via_word(docx_path: Path, pdf_out_path: Path) -> None:
    container_data = Path.home() / "Library/Containers/com.microsoft.Word/Data"
    word_sandbox_dir = (
        container_data / "tmp" if container_data.is_dir() else Path("/private/tmp")
    )
    word_sandbox_dir.mkdir(parents=True, exist_ok=True)
    pid = os.getpid()
    temp_docx = word_sandbox_dir / f"typst2word_visual_temp_{pid}.docx"
    temp_pdf = word_sandbox_dir / f"typst2word_visual_temp_{pid}.pdf"

    if temp_pdf.exists():
        temp_pdf.unlink()
    shutil.copy2(docx_path, temp_docx)

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

    applescript = f'''
    with timeout of 30 seconds
    tell application "Microsoft Word"
        try
            set startup dialog to false
        end try
        set prevAlerts to display alerts
        set display alerts to alerts none
        try
            open POSIX file "{temp_docx}" with read only without add to recent files
            set targetDoc to active document
            save as targetDoc file name "{temp_pdf}" file format format PDF without add to recent files
            close targetDoc saving no
        on error errMsg number errNum
            try
                set display alerts to prevAlerts
            end try
            error errMsg number errNum
        end try
        try
            set display alerts to prevAlerts
        end try
    end tell
    end timeout
    '''
    try:
        subprocess.run(["osascript", "-e", applescript], check=True, timeout=35)
        if not temp_pdf.exists():
            raise RuntimeError(f"Microsoft Word did not produce PDF at {temp_pdf}")
        pdf_out_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(temp_pdf, pdf_out_path)
    finally:
        temp_docx.unlink(missing_ok=True)
        temp_pdf.unlink(missing_ok=True)
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


def render_pdf_pages_to_images(
    script_dir: Path,
    pdf_path: Path,
    out_dir: Path,
    prefix: str,
    scale: float,
    opacity: float,
    tint: bool,
) -> list[str]:
    swift_script = script_dir / "pdf_to_images.swift"
    cmd = [
        "swift",
        str(swift_script),
        str(pdf_path),
        str(out_dir),
        prefix,
        str(scale),
        str(opacity),
        "1" if tint else "0",
    ]
    res = subprocess.run(cmd, check=True, capture_output=True, text=True)
    bg_files = []
    for line in res.stdout.splitlines():
        print(" ", line)
        m = re.search(r"bg=(\S+)", line)
        if m:
            bg_files.append(Path(m.group(1)).name)
    return bg_files


def build_overlay_typ(
    orig_typ_path: Path,
    out_dir: Path,
    bg_filenames: list[str],
) -> Path:
    orig_src = orig_typ_path.read_text(encoding="utf-8")
    bg_array_literal = ", ".join(f'"{name}"' for name in bg_filenames)
    if len(bg_filenames) == 1:
        bg_array_literal += ","

    bg_rule = f"""
// === Auto-generated Word Page Background Overlay ===
#set page(background: context {{
  let p = here().page()
  let bg_pages = ({bg_array_literal})
  if p >= 1 and p <= bg_pages.len() {{
    image(bg_pages.at(p - 1), width: 100%, height: 100%)
  }}
}})
// ===================================================
"""

    # Insert right after `#set page(...)` if present, otherwise at top of file.
    page_set_match = re.search(r"#set\s+page\s*\((?:[^()]*|\([^()]*\))*\)", orig_src, re.DOTALL)
    if page_set_match:
        insert_pos = page_set_match.end()
        overlay_src = orig_src[:insert_pos] + "\n" + bg_rule + orig_src[insert_pos:]
    else:
        overlay_src = bg_rule + "\n" + orig_src

    overlay_typ_path = out_dir / "overlay_compare.typ"
    overlay_typ_path.write_text(overlay_src, encoding="utf-8")
    return overlay_typ_path


def main() -> int:
    parser = argparse.ArgumentParser(description="Visual pixel-level comparison between Typst and Word.")
    parser.add_argument("input_typ", type=Path, help="Input .typ file")
    parser.add_argument(
        "--docx",
        type=Path,
        default=None,
        help="Optional path to an already-compiled .docx file (otherwise compiled via cargo run -j 2)",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("examples/visual_diff"),
        help="Output directory for generated Word PDF, page images, and overlay Typst/PDF/PNGs",
    )
    parser.add_argument(
        "--opacity",
        type=float,
        default=0.42,
        help="Opacity/darkness of the Word background layer in [0.0, 1.0] (default: 0.42)",
    )
    parser.add_argument(
        "--no-tint",
        action="store_true",
        help="Use pure greyscale/brightness dimming instead of coral tint for the Word background",
    )
    parser.add_argument(
        "--scale",
        type=float,
        default=2.0,
        help="Rasterization scale factor relative to 72pt/inch (2.0 = 144 DPI)",
    )
    parser.add_argument(
        "--skip-build",
        action="store_true",
        help="Skip running cargo run to regenerate .docx if already up to date",
    )
    parser.add_argument(
        "--ir",
        type=Path,
        default=None,
        help="Where to dump the intermediate representation JSON (default: <out-dir>/<stem>.ir.json)",
    )
    parser.add_argument(
        "--no-ir",
        action="store_true",
        help="Do not dump the intermediate representation JSON when converting",
    )
    parser.add_argument(
        "--coords-only",
        action="store_true",
        help="Only generate PDFs and exact coordinate diff (layout_diff.txt), skipping overlay PDF rendering",
    )
    parser.add_argument(
        "--keep-pngs",
        action="store_true",
        help="Also render per-page overlay_page_<N>.png files and keep intermediate word_page_*.png images",
    )
    parser.add_argument(
        "--max-pages",
        type=int,
        default=0,
        help="Limit coordinate diff report to the first N pages (0 = all pages)",
    )
    args = parser.parse_args()

    script_dir = Path(__file__).resolve().parent
    repo_root = find_repo_root()
    input_typ = args.input_typ.resolve()
    out_dir = args.out_dir.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    docx_path = args.docx.resolve() if args.docx else (out_dir / f"{input_typ.stem}.docx")
    word_pdf_path = out_dir / f"{input_typ.stem}.word.pdf"
    orig_pdf_path = out_dir / f"{input_typ.stem}.orig.pdf"
    layout_diff_path = out_dir / "layout_diff.txt"
    ir_path = None if args.no_ir else (args.ir.resolve() if args.ir else out_dir / f"{input_typ.stem}.ir.json")

    env = os.environ.copy()
    env["RAYON_NUM_THREADS"] = "2"

    if not args.skip_build and args.docx is None:
        print(f"[1/5] Converting {input_typ.name} -> {docx_path.name} (CPU threads = 2)...")
        convert_cmd = [
            "cargo",
            "run",
            "-j",
            "2",
            "--",
            str(input_typ),
            str(docx_path),
        ]
        if ir_path is not None:
            ir_path.parent.mkdir(parents=True, exist_ok=True)
            convert_cmd += ["--dump-ir", str(ir_path)]
        subprocess.run(
            convert_cmd,
            cwd=repo_root,
            env=env,
            check=True,
        )
        # Also keep sibling `<stem>.docx` next to `<input_typ>` updated if it exists
        sibling_docx = input_typ.with_suffix(".docx")
        if sibling_docx.exists() and sibling_docx.resolve() != docx_path.resolve():
            shutil.copy2(docx_path, sibling_docx)

    print(f"[2/5] Exporting {docx_path.name} -> {word_pdf_path.name} via Microsoft Word AppleScript...")
    export_docx_to_pdf_via_word(docx_path, word_pdf_path)

    typst_bin = shutil.which("typst") or str(Path.home() / ".cargo/bin/typst")
    subprocess.run(
        [typst_bin, "compile", "--jobs", "2", str(input_typ), str(orig_pdf_path)],
        check=True,
    )

    print(f"[3/5] Computing exact glyph-ink coordinate diff -> {layout_diff_path.name}...")
    diff_script = script_dir / "pdf_layout_diff.swift"
    subprocess.run(
        [
            "swift",
            str(diff_script),
            str(orig_pdf_path),
            str(word_pdf_path),
            str(layout_diff_path),
            str(args.max_pages),
        ],
        check=True,
    )

    bg_files: list[str] = []
    overlay_typ_path = out_dir / "overlay_compare.typ"
    overlay_pdf_path = out_dir / "overlay_compare.pdf"

    if not args.coords_only:
        print(f"[4/5] Rendering Word PDF pages to paper-matched PNGs (opacity={args.opacity})...")
        bg_files = render_pdf_pages_to_images(
            script_dir=script_dir,
            pdf_path=word_pdf_path,
            out_dir=out_dir,
            prefix="word_page",
            scale=args.scale,
            opacity=args.opacity,
            tint=not args.no_tint,
        )

        print("[5/5] Building & compiling overlay_compare.pdf with Word pages as 100% page backgrounds...")
        overlay_typ_path = build_overlay_typ(input_typ, out_dir, bg_files)

        subprocess.run(
            [typst_bin, "compile", "--jobs", "2", str(overlay_typ_path), str(overlay_pdf_path)],
            check=True,
        )
        if args.keep_pngs:
            overlay_png_pattern = out_dir / "overlay_page_{p}.png"
            ppi = int(round(72.0 * args.scale))
            subprocess.run(
                [
                    typst_bin,
                    "compile",
                    "--jobs",
                    "2",
                    "--ppi",
                    str(ppi),
                    str(overlay_typ_path),
                    str(overlay_png_pattern),
                ],
                check=True,
            )
        else:
            overlay_typ_path.unlink(missing_ok=True)
            for png_file in out_dir.glob("word_page_*.png"):
                png_file.unlink(missing_ok=True)

    print(f"\nDone! Visual & coordinate comparison files generated in {out_dir}:")
    if ir_path is not None and ir_path.exists():
        print(f"  - Intermediate IR JSON: {ir_path}")
    print(f"  - Original Typst PDF:   {orig_pdf_path}")
    print(f"  - Word Exported PDF:    {word_pdf_path}")
    print(f"  - Coordinate Diff TXT:  {layout_diff_path}")
    if not args.coords_only:
        print(f"  - Merged Overlay PDF:   {overlay_pdf_path}")
        if args.keep_pngs:
            print(f"  - Overlay Typst source: {overlay_typ_path}")
            for p in range(1, len(bg_files) + 1):
                print(f"  - Overlay Page {p} PNG:   {out_dir / f'overlay_page_{p}.png'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
