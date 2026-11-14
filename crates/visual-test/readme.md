# `visual-test`

Atomic single-variable (`margin: 10pt`) Typst-to-Word IR, OpenXML, and pixel-diff inspection suite +  `gpui-kit` desktop studio.

## Directory Layout

```text
crates/visual-test/
├── Cargo.toml
├── build.rs                            # Precompiles tools/pdf_lab_inspector.swift -> $OUT_DIR/pdf_lab_inspector
├── fixtures/
│   ├── probe.png                       # Deterministic raster probe for #image tests
│   └── refs.bib                        # BibTeX fixture for #bibliography & #cite tests
├── tools/
│   └── pdf_lab_inspector.swift         # PDFKit page slicer (`split`) + CoreGraphics ink bbox & IoU diff (`inspect`)
├── src/
│   ├── lib.rs
│   ├── model.rs
│   ├── verify.rs                       # IR, <w:pgMar> [T,R,B,L], mirrorMargins & OOXML checks
│   ├── runner/                         # Layered 4-stage execution pipeline
│   │   ├── mod.rs                      # Public API (`RunConfig`, `run_suite`) & pipeline orchestration
│   │   ├── compile.rs                  # Stage 1: 16 MiB Rayon pool -> Typst PDF, IR, WordDocument & OOXML
│   │   ├── word_applescript.rs         # Silent macOS Word sandbox driver (`Data/tmp`, no focus steal, alerts none)
│   │   ├── word_batch.rs               # Stage 2: Multi-Section Mega-Doc assembly, PDFKit split & bisect fallback
│   │   ├── inspector.rs                # Stage 3: Rust <-> Swift `pdf_lab_inspector` (`split` & `inspect`) bridge
│   │   └── report.rs                   # Stage 4: Pass/Drift/Fail classification, report.json & SUMMARY.md
│   ├── catalog/
│   │   ├── mod.rs                      # Discovers suites/**/*.typ
│   │   ├── parser.rs                   # Parses // === @case <id> === directives
│   │   └── schema.rs                   # Typst typesetting standard-library function schema
│   ├── ui/                             # Modular native macOS 3-column gpui-kit desktop studio
│   │   ├── mod.rs                      # App bootstrap, theme sync & report fallback loader
│   │   ├── actions.rs                  # macOS System Menu Bar (`cx.set_menus`) & ⌘ shortcuts
│   │   ├── macos_theme.rs              # macOS Sequoia/Sonoma Light & Dark studio palettes
│   │   ├── docx_parts.rs               # `.docx` OpenXML ZIP reader + Word/Preview/Finder openers
│   │   ├── typst_highlight.rs          # Custom Typst syntax highlighter & OpenXML pretty-formatter
│   │   ├── toolbar.rs                  # Xcode-style Run Scheme Pop-Up (`▶ Run` + `Scheme ▾`) & `Open ▾`
│   │   ├── status_bar.rs               # Bottom case stepper (`‹ n / total ›`) & suite telemetry
│   │   ├── state/                      # `ParityStudioView` state, silent batch bridge & navigation
│   │   │   ├── mod.rs
│   │   │   ├── runner_bridge.rs
│   │   │   └── navigation.rs
│   │   ├── sidebar/                    # Single-row Search + `Filter ▾` Pop-Up & stdlib source list
│   │   │   ├── mod.rs
│   │   │   └── tree_row.rs
│   │   ├── stage/                      # Preview-style paper canvas (`Side-by-Side ▾` & `100% ▾`)
│   │   │   ├── mod.rs
│   │   │   ├── paper_card.rs
│   │   │   └── summary_bar.rs
│   │   ├── diagnostics/                # Collapsible drawer (`Geometry Telemetry ▾`, Checks, Lines)
│   │   │   ├── mod.rs
│   │   │   ├── geometry.rs
│   │   │   └── checks_lines.rs
│   │   └── inspector/                  # Right Code Inspector (`Typst / IR / 8 OpenXML Parts ▾`)
│   │       └── mod.rs
│   └── bin/
│       ├── visual_test_cli.rs
│       └── visual_test_gui.rs
└── suites/                             # Pure typesetting standard-library & package suites
    ├── 01_model/                       # Heading, par, list, enum, terms, table/*, figure, quote, footnote, outline, cite, ref, link
    ├── 02_text/                        # text/*, raw, smallcaps, sub/super, underline, strike, overline, highlight, smartquote, linebreak
    ├── 03_math/                        # equation, frac/binom, roots, attach, mat, cases, lr/mid, accent/underover, op, spacing, styles
    ├── 04_symbols/                     # sym.*, emoji.*
    ├── 05_layout/                      # page/{margins,dimensions,header_footer}, pagebreak, columns, grid, stack, block, box, pad, align, place, transforms
    ├── 06_visualize/                   # rect, square, circle, ellipse, line, polygon, curve, image, color/gradient
    ├── 90_compositions/                # Real-world multi-element regression cases (e.g. rounded box header card)
    └── 99_packages/cetz/               # CeTZ vector canvas -> Word DrawingML group mapping
```

## Usage

```bash
# Run IR & OpenXML verification across all 176 typesetting cases without opening Word
cargo run -p visual-test --bin visual-test-cli -- --skip-word

# Run exhaustive page(margin: ...) suite silently via Multi-Section Word batch
cargo run -p visual-test --bin visual-test-cli -- --suite 05_layout/page/margins.typ

# Run a single case silently via Word
cargo run -p visual-test --bin visual-test-cli -- --case margin_uniform_10pt --fresh-word

# Launch native macOS desktop inspection studio
cargo run -p visual-test --bin visual-test-gui
```

## macOS Studio Shortcuts

| Shortcut | Action |
|---|---|
| `⌘R` | Run active scheme (`Selected Case` / `Current Suite` / `Category` / `All 176`) |
| `⇧⌘R` | Force fresh silent Word render for selected case |
| `⌥⌘R` | Run current `.typ` suite with silent Word batch |
| `⇧⌘I` | Verify all 176 cases (`IR & OpenXML` only) |
| `⌘1` – `⌘4` | Inspect `.typ`, `ir.json`, `word/document.xml`, `word/settings.xml` |
| `⌘K` / `⌘F` | Focus case search / Find in code inspector |
| `⌘[` / `⌘]` | Previous / Next case (`⌥↑` / `⌥↓` also supported) |
| `⌘+` / `⌘-` / `⌘0` | Zoom in / Zoom out / Actual size (`100%`) |
| `⌥⌘S` / `⌥⌘I` / `⌥⌘D` | Toggle Navigator / Code Inspector / Telemetry Drawer |
| `⇧⌘A` | Toggle macOS Light / Dark appearance |
