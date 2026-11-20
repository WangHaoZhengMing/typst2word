# typst2word

**`typst2word`** is a Rust CLI tool and library that converts **Typst (`.typ`)** documents into **Microsoft Word (`.docx`)** while preserving both **semantic editability** and **visual layout fidelity**.

Unlike pure syntax-tree converters (which cannot evaluate Typst scripting, `#show` rules, counters, or packages like CeTZ) or PDF-to-Word tools (which flatten everything into floating text boxes and raster images), `typst2word` embeds the full **Typst 0.15** compiler and lowers documents into **native, editable WordprocessingML, Office MathML (OMML), and DrawingML** constructs—calibrating Word's line-box metrics, paragraph spacing, and line-breaking behavior against Typst's laid-out reference geometry.

> **Try Online (WebAssembly Converter)**: [**wanghaozhengming.github.io/typst2word**](https://wanghaozhengming.github.io/typst2word/) — convert `.typ` files, `.zip` project archives, or pasted snippets directly to `.docx` locally in your browser (zero server upload, supports subset/full font embedding, Local Font Access API on Chromium, and OpenXML inspection).
>
> **Documentation**: See [`docs/`](./docs/README.md) for the complete technical reference ([CLI & Library](./docs/cli-and-library.md), [Architecture](./docs/architecture.md), [IR Specification](./docs/ir-specification.md), [Fonts & Embedding](./docs/fonts-and-embedding.md), [Layout Calibration](./docs/layout-calibration.md), [Feature Matrix](./docs/feature-matrix.md), [WebAssembly & Web](./docs/wasm-and-web.md), [Testing & Visual Parity](./docs/testing-and-visual-parity.md)).

---

## Getting Started

### 1. Installation

#### Prebuilt Binaries & WebAssembly SDK

Download the archive for your platform from [GitHub Releases](https://github.com/WangHaoZhengMing/typst2word/releases), extract it, and place `typst2word` (`typst2word.exe` on Windows) on your `PATH`:

- **macOS**: `aarch64-apple-darwin` (Apple Silicon), `x86_64-apple-darwin` (Intel)
- **Linux**: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`
- **Windows**: `x86_64-pc-windows-msvc`
- **WebAssembly (`wasm32-unknown-unknown`)**: `typst2word-wasm32-unknown-unknown.tar.gz` / `.zip` (ES Module `web/`, `bundler/`, and `nodejs/` targets)

#### Build from Source (Rust 1.85+ / Edition 2024)

```bash
git clone https://github.com/WangHaoZhengMing/typst2word.git
cd typst2word

# Install into ~/.cargo/bin
cargo install --path .

# Or build a release binary at target/release/typst2word
cargo build --release
```

---

### 2. Command-Line Usage

```bash
typst2word <input.typ> [output.docx] [--embed-fonts=<subset|full|none>] [--no-embed-fonts] [--dump-ir <ir.json>] [--input key=value]
```

**Examples:**

```bash
# Convert input.typ -> input.docx in the same directory (subset font embedding enabled by default)
typst2word thesis.typ

# Specify an explicit output path with full font embedding for unrestricted editing in Word
typst2word report.typ dist/report.docx --embed-fonts=full

# Disable font embedding for minimal file size
typst2word draft.typ draft.docx --no-embed-fonts

# Pass key-value pairs into Typst's sys.inputs (repeatable)
typst2word paper.typ paper.docx --input blind_review=true --input version=2.1

# Export the pure semantic + reference-layout IR as JSON for inspection
typst2word report.typ report.docx --dump-ir debug_ir.json
```

After conversion, `typst2word` prints a structural summary (pages, headings, paragraphs, lists, tables, OMML equations, DrawingML shapes, CeTZ canvases, footnotes, links, etc.), an embedded-font summary (`ℹ️`), and any font fallback warnings (`⚠️`). `typst2word` preserves original font family names verbatim and does not filter font embedding by `fsType` or OS vendor—users are responsible for complying with font license terms (e.g., Apple system fonts are licensed for macOS/Apple platforms only; see [Fonts & OOXML Embedding](./docs/fonts-and-embedding.md)).

---

### 3. Rust Library Usage

`typst2word` can also be embedded directly in Rust applications to convert files or in-memory strings to `.docx` or inspect the intermediate `DocumentIr`:

```rust
use std::path::Path;
use typst2word::{
    compile_file_to_ir,
    convert_typst_file_to_docx,
    convert_typst_str_to_docx_bytes,
};

fn main() -> Result<(), String> {
    // 1. Convert a .typ file on disk directly to .docx
    let ir = convert_typst_file_to_docx(
        Path::new("input.typ"),
        Path::new("output.docx"),
    )?;
    println!(
        "Converted {} pages and {} equations",
        ir.report.pages_count, ir.report.equations_count
    );

    // 2. Convert an in-memory Typst string into .docx bytes
    let source = r#"
    = Introduction
    Mass-energy equivalence $E = m c^2$ expands in relativistic form as:
    $ E^2 = (p c)^2 + (m_0 c^2)^2 $
    "#;
    let (docx_bytes, _ir) = convert_typst_str_to_docx_bytes(source, None)?;
    std::fs::write("in_memory.docx", docx_bytes).map_err(|e| e.to_string())?;

    // 3. Extract the pure, Word-independent DocumentIr
    let _pure_ir = compile_file_to_ir(Path::new("input.typ"))?;
    Ok(())
}
```

---

## Feature Coverage

| Category | Supported Typst Features | Word (`.docx`) Representation |
| :--- | :--- | :--- |
| **Document Structure** | Multi-level headings (`heading` with custom/auto numbering), paragraphs (`par` first-line & hanging indents, alignment, leading & spacing), bullet/numbered/term lists (`list` / `enum` / `terms` with nesting), blockquotes (`quote`), rules, footnotes (`footnote`) | Native `Heading` styles, multi-level `<w:numPr>` numbering definitions, dynamic `<w:footnotes>` |
| **Outlines, Citations & Links** | Table of contents (`outline`), BibTeX / Hayagriva bibliographies (`bibliography` / `cite`), cross-references (`@label` / `ref`), internal & external hyperlinks (`link`) | Dynamic Word `TOC` and `PAGEREF` fields, canonicalized bookmarks (`w:bookmarkStart`), native hyperlinks |
| **Tables & Figures** | Fixed, fractional (`1fr`), percentage, and `auto` column widths; `colspan` & `rowspan` merging; repeating headers (`table.header`); per-cell & three-line custom strokes, fills, and insets; captioned `figure` blocks | Native `<w:tbl>` (`gridSpan`, `vMerge`, `tblHeader`, per-edge border conflict resolution) |
| **Code Blocks** | Inline `raw` and fenced code blocks with Typst syntax-highlighted token colors, monospace metrics, background fills, and borders | Styled inline runs, shaded/bordered compact code paragraphs or container tables |
| **Typography & CJK** | Font families, weights, italics, sizes, colors, highlights, `sub` / `super`, `smallcaps`, `underline` / `strike` / `overline`, `tracking`, `baseline` shift, text outline stroke, RTL directionality, CJK Kinsoku rules & mixed CJK/Latin spacing | Slot-routed `w:ascii` / `w:hAnsi` / `w:eastAsia` / `w:cs` fonts and OpenXML run properties |
| **Mathematics (OMML)** | Inline & display equations, numbered display equations, vertical/skewed fractions & binomials (`frac` / `binom`), square & n-th roots (`sqrt` / `root`), sub/superscripts & pre-scripts (`attach`), 19+ big n-ary operators (`sum`, `prod`, `integral`, etc.) with limit placement, scalable delimiters (`lr` / `mid` / `abs` / `norm` / `floor` / `ceil`), matrices & vectors (`mat` / `vec`), piecewise `cases`, multi-line `&`-aligned equations, accents (`hat`, `tilde`, `vec`, `dot`, etc.), over/under bars & braces with annotations, `cancel` strokes, math alphabets (`upright`, `italic`, `bold`, `bb`, `cal`, `frak`, `sans`, `mono`) | **Native editable Office MathML** (`<m:oMath>`, `<m:oMathPara>`, and 3-column borderless numbered display rows) |
| **Page Layout & Sections** | Page dimensions & orientation, uniform/per-side/inside-outside `mirror` margins, page fills, headers & footers (first-page & odd/even differentiation, dynamic page counters), page/column breaks, full-page & scoped multi-column flows (`columns`), styled `block` containers & inline `box` pills | Multi-section `<w:sectPr>` breaks, `header/footer.xml` parts, `PAGE` / `NUMPAGES` fields, container tables & anchored shapes |
| **Shapes, Images & CeTZ** | Native geometric primitives (`rect`, `square`, `circle`, `ellipse`, `line`, `polygon`, Bézier `curve`), `rotate` / `scale` / `move` transforms, `place` positioning, raster images (`image`), and **CeTZ vector canvases** (paths, arrowheads, fills, text & math labels) | **Native editable Word DrawingML** (`<wps:wsp>` custom geometry `<a:custGeom>` and `<wpg:wgp>` groups—never rasterized bitmaps) |

### Known Limitations & Fallback Behavior

- **Advanced OMML Constraints**: Due to structural limits in Microsoft Word's OMML engine, multi-anchor column alignment within a single `<m:eqArr>` row, internal matrix augmentation lines / mixed per-column matrix alignment, and automatic line-wrapping of ultra-wide display equations inside narrow table cells are still being refined.
- **Graceful Degradation**: All Typst scripts, `#show` / `#set` rules, counters, and packages execute normally inside the embedded Typst compiler. If an unsupported visual primitive cannot yet be lowered into native Word objects, `typst2word` never crashes or silently drops content—it emits a visible placeholder badge in the document and reports its location in the CLI summary.

---

## How It Works

Converting Typst to editable Word sits across a fundamental tension:
- **Syntax-only converters** see raw `.typ` markup before scripting, `#show` rules, counters, and layout run, losing evaluated content and physical dimensions.
- **Frame/PDF-only converters** see positioned glyphs and paths after layout, losing semantic structure (headings, lists, tables, inline vs. display math).

`typst2word` bridges both worlds with a 5-stage workspace pipeline:

```text
  .typ source & packages
            │
            ▼
 ┌───────────────────┐
 │     compiler      │  Embedded Typst 0.15 engine preserving both pre-layout Content
 └─────────┬─────────┘  semantics (Introspector) and post-layout PagedDocument Frames
           │
           ▼
 ┌───────────────────┐
 │      extract      │  Fuses semantic elements with laid-out reference geometry & binary
 └─────────┬─────────┘  OpenType font metrics by unified NodeId -> pure DocumentIr
           │
           ▼
 ┌───────────────────┐
 │        ir         │  Word-agnostic, enum-driven Semantic + ReferenceLayout IR
 └─────────┬─────────┘
           │
           ▼
 ┌───────────────────┐
 │  word-construct   │  Word layout modeling & compensation engine (font-slot routing,
 └─────────┬─────────┘  OS/2 & CJK 1.3x line-box solver, Knuth-Plass glue compensation,
           │            compatibilityMode=15 margin-collapse solver, OMML/DrawingML builders)
           ▼
 ┌───────────────────┐
 │    docx-writer    │  Pure OpenXML serializer packaging styles, settings & document parts
 └───────────────────┘
```

### Key Technical Pillars

1. **Dual-Source Semantic + Physical Extraction (`compiler` & `extract`)**
   Custom `Tag::Start` / `Tag::End` introspection hooks correlate pre-layout `Content` elements (after `#show` rules and scripting evaluate) with post-layout `Frame` fragments via a unified `NodeId`. Every block and inline construct retains both its semantic identity (e.g., level-2 heading, tight list, equation AST) and its exact laid-out bounding boxes and baseline coordinates (`ReferenceLayout`).

2. **Binary OpenType Metrics & CJK `1.3×` Line-Box Calibration (`word-construct`)**
   Typst computes line spacing from a font's `OS/2.sCapHeight` plus `par.leading`, whereas Microsoft Word's single spacing (`w:line="240"`) derives from `hhea` / `OS/2` metrics and applies an extra **`1.3×` expansion factor** to fonts declaring East Asian codepages (`CP932`/`CP936`/`CP949`/`CP950`—pushing macOS fonts like `PingFang SC` and `Songti SC` to `1.82×` em height). `extract` reads binary OpenType tables directly from the resolved font faces, and `word-construct` solves exact `w:line` (`1/240` line units) and paragraph-mark sizes so Latin text, CJK text, and single-line inline/display equations match Typst's baseline rhythm.

3. **Knuth-Plass to Greedy Line-Break Compensation**
   Typst breaks paragraphs globally using Knuth-Plass optimization, whereas Word (`MicrosoftPTLS7`) breaks lines greedily and often spills an extra trailing word onto a new line. By comparing against Typst's laid-out line anchors, `word-construct` applies sub-point `<w:spacing>` character-glue adjustments only on overflowing lines—matching Typst's line count **without inserting hard `<w:br/>` breaks**, so paragraphs remain naturally reflowable when edited in Word.

4. **`compatibilityMode=15` Inter-Block Spacing Solver (`flow_spacing`)**
   Modern Word (`compatibilityMode=15`) collapses adjacent paragraph vertical margins via `max(w:after, w:before)`, does not collapse across `<w:tbl>` boundaries, and injects default `<m:preSp>` / `<m:postSp>` padding around `<m:oMathPara>`. `word-construct` zeroes hidden display-math margins and solves `before_dxa` / `after_dxa` transitions from measured reference gaps rather than fixed magic constants.

---

## Testing & Visual Parity Suite

The repository includes unit tests, integration tests, and an automated Typst-vs-Word PDF visual regression harness (`crates/visual-test`):

```bash
# Run all workspace unit and integration tests
cargo test --workspace

# Verify IR & OpenXML invariants across all single-variable test cases (without launching Word)
cargo run -p visual-test --bin visual-test-cli --no-default-features -- --skip-word

# Silently batch-render .docx via Microsoft Word on macOS and compute PDF ink-bbox & IoU diffs
cargo run -p visual-test --bin visual-test-cli --no-default-features -- -c math

# Launch the native macOS 3-column visual inspection studio
cargo run -p visual-test --bin visual-test-gui
```

## Workspace Layout

```text
typst2word/
├── src/                    # CLI entry point (typst2word) and top-level library API
├── crates/
│   ├── ir/                 # Word-agnostic Semantic + ReferenceLayout IR (DocumentIr)
│   ├── compiler/           # Embedded Typst 0.15 world, font scanner & query engine
│   ├── extract/            # Dual-source Content + Frame semantic/geometric extractor
│   ├── word-construct/     # Word layout compensation, spacing solvers & construct builders
│   ├── docx-writer/        # Pure OpenXML serializer and .docx ZIP writer
│   ├── wasm/               # WebAssembly (wasm-bindgen) workspace, SVG preview & OpenXML bindings
│   └── visual-test/        # Automated Typst-vs-Word PDF diff CLI and desktop GUI studio
├── web/                    # Svelte 5 + Vite online WebAssembly Studio (GitHub Pages)
├── examples/               # Sample Typst documents
└── tests/                  # End-to-end conversion and architectural regression tests
```
