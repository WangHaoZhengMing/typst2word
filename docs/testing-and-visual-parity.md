# Testing & Visual Parity Suite

`typst2word` validates correctness at three levels: Rust unit tests, structural IR/OpenXML invariant verification, and automated Typst-PDF vs. Word-PDF pixel/geometry visual parity testing.

---

## 1. Workspace Unit Tests

Run fast pure-Rust unit tests across the core crates (including `WordLineBox` CJK metrics, `FirstFitBreaker` vs. `KnuthPlassBreaker` glue compensation, pagination policies, and equation 2-DoF box calibration):

```bash
cargo test --lib -p ir -p extract -p word-construct -p docx-writer -p typst2word
```

Verify WebAssembly target compilation (`wasm32-unknown-unknown`):

```bash
cargo check -p typst2word-wasm --target wasm32-unknown-unknown
```

---

## 2. Visual Parity Harness (`crates/visual-test`)

`crates/visual-test` is a single-variable (`margin: 10pt`) Typst-to-Word IR, OpenXML, and pixel-diff regression harness paired with an optional native macOS desktop inspection studio (`gpui-kit`).

### Four-Stage Pipeline

1. **Stage 1 — Parallel Compile (`runner/compile.rs`)**: Compiles all discovered `suites/**/*.typ` cases on a 16 MiB-stack Rayon pool to produce reference Typst PDFs, `DocumentIr`, `WordDocument`, and `.docx` packages, then runs deterministic IR & OpenXML assertions (`verify.rs`).
2. **Stage 2 — Silent macOS Word Batch Export (`runner/word_batch.rs`, `runner/word_applescript.rs`)**: Merges compatible cases into Multi-Section Mega-Documents staged inside Microsoft Word's App Sandbox temporary directory (`~/Library/Containers/com.microsoft.Word/Data/tmp/...`), invokes Word in the background with alerts suppressed (`display alerts = alerts none`, no focus steal), exports to PDF, and splits pages back per case with automatic bisect fallback.
3. **Stage 3 — Swift PDF & CoreGraphics Inspection (`tools/pdf_lab_inspector.swift`, `runner/inspector.rs`)**: Uses macOS `PDFKit` and `CoreGraphics` to slice pages, measure ink bounding boxes, compute pixel/IoU visual overlap, and generate side-by-side diff artifacts.
4. **Stage 4 — Classification & Reporting (`runner/report.rs`)**: Classifies each case as `Pass`, `Drift`, or `Fail` and emits `report.json` and `SUMMARY.md`.

### Test Suite Organization (`crates/visual-test/suites/`)

- `01_model/` — Headings, paragraphs, lists, enums, terms, tables, figures, quotes, footnotes, outlines, citations, references, links
- `02_text/` — Text styling, raw code, smallcaps, sub/super, underline/strike/overline/highlight, smartquotes, linebreaks
- `03_math/` — Equations, fractions/binomials, roots, scripts, matrices, cases, delimiters, accents, operators, `comprehensive_gauntlet.typ` (362 equations)
- `04_symbols/` — `sym.*` and `emoji.*`
- `05_layout/` — Page margins/dimensions/headers/footers, pagebreak, columns, grid, stack, block, box, pad, align, place, transforms
- `06_visualize/` — Rect, square, circle, ellipse, line, polygon, curve, image, color/gradient
- `90_compositions/` — Multi-element real-world regression cases
- `99_packages/cetz/` — CeTZ vector canvas to Word DrawingML group mapping

### CLI & Studio Usage

```bash
# Verify IR & OpenXML invariants across all cases without launching Microsoft Word
cargo run -p visual-test --bin visual-test-cli --no-default-features -- --skip-word

# Run a specific suite silently through Microsoft Word on macOS
cargo run -p visual-test --bin visual-test-cli --no-default-features -- --suite 05_layout/page/margins.typ

# Run a single case with a fresh Word instance
cargo run -p visual-test --bin visual-test-cli --no-default-features -- --case margin_uniform_10pt --fresh-word

# Launch the native macOS 3-column visual inspection studio
cargo run -p visual-test --bin visual-test-gui
```
