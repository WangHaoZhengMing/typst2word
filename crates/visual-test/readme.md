# `visual-test`

Atomic single-variable (`margin: 10pt`) Typst-to-Word IR, OpenXML, and pixel-diff inspection suite + native macOS `gpui-kit` desktop studio.

## Directory Layout

```text
crates/visual-test/
├── Cargo.toml
├── fixtures/
│   ├── probe.png                       # Deterministic raster probe for #image tests
│   └── refs.bib                        # BibTeX fixture for #bibliography & #cite tests
├── tools/
│   └── pdf_lab_inspector.swift         # PDFKit / CoreGraphics rasterizer, ink bbox & IoU diff
├── src/
│   ├── lib.rs
│   ├── model.rs
│   ├── runner.rs                       # Parallel compile, 25s Word timeout guard, PDF diff
│   ├── verify.rs                       # IR, <w:pgMar> [T,R,B,L], mirrorMargins & OOXML checks
│   ├── catalog/
│   │   ├── mod.rs                      # Discovers suites/**/*.typ
│   │   ├── parser.rs                   # Parses // === @case <id> === directives
│   │   └── schema.rs                   # Typst typesetting standard-library function schema
│   ├── ui/                             # Native macOS 3-column gpui-kit desktop inspector
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

# Run exhaustive page(margin: ...) suite
cargo run -p visual-test --bin visual-test-cli -- --skip-word -f page/margins

# Launch native macOS desktop inspection studio
cargo run -p visual-test --bin visual-test-gui
```
