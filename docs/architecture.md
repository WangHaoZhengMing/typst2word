# System Architecture

`typst2word` is structured as a multi-crate Rust workspace enforcing strict separation between Typst compilation, semantic/geometric extraction, target-independent IR, Microsoft Word layout modeling, and OpenXML serialization.

---

## 1. Design Philosophy

Converting Typst to editable Microsoft Word documents spans a fundamental gap between two representations:

- **Pre-layout syntax / AST converters** cannot evaluate Typst scripting, `#show` / `#set` rules, dynamic counters, BibTeX bibliographies, or packages such as CeTZ, and lack physical bounding boxes or baseline coordinates.
- **Post-layout PDF / Frame converters** observe only positioned glyphs and vector paths, losing semantic structure (heading hierarchy, numbered lists, merged table cells, editable OMML equations, and reflowable paragraphs).

`typst2word` resolves this by embedding the full **Typst 0.15** compiler and performing **dual-source extraction**, correlating pre-layout realized `Content` elements with post-layout `PagedDocument` `Frame` geometry through a unified `NodeId`.

---

## 2. Five-Stage Pipeline

```text
  .typ source + assets + @preview packages + fonts
                        │
                        ▼
 ┌─────────────────────────────────────────────────────┐
 │ 1. compiler  (crates/compiler)                      │
 │    TypstWorld, CustomFontRegistry, CUSTOM_ROUTINES  │
 │    -> CompiledTypstDocument (Content + Frames)      │
 └──────────────────────┬──────────────────────────────┘
                        │
                        ▼
 ┌─────────────────────────────────────────────────────┐
 │ 2. extract   (crates/extract)                       │
 │    Frame-tree builder, semantic extractors,         │
 │    OpenType metric reader, font subsetter (allsorts)│
 │    -> DocumentIr (normalized, immutable)            │
 └──────────────────────┬──────────────────────────────┘
                        │
                        ▼
 ┌─────────────────────────────────────────────────────┐
 │ 3. ir        (crates/ir)                            │
 │    Word-agnostic Semantic + ReferenceLayout IR      │
 │    (zero dxa / w:line / Word-specific mutations)    │
 └──────────────────────┬──────────────────────────────┘
                        │
                        ▼
 ┌─────────────────────────────────────────────────────┐
 │ 4. word-construct (crates/word-construct)           │
 │    WordFontMetricsProvider, WordLineBox, PTLS7      │
 │    line-break & glue compensation, flow_spacing,    │
 │    table lowering, OMML & DrawingML geometry solvers│
 │    -> WordDocument (with SolvedNodeLayout side-tbl) │
 └──────────────────────┬──────────────────────────────┘
                        │
                        ▼
 ┌─────────────────────────────────────────────────────┐
 │ 5. docx-writer (crates/docx-writer)                 │
 │    Pure OpenXML serializer (document, styles,       │
 │    settings, numbering, footnotes, headers/footers, │
 │    fontTable + obfuscated .odttf) -> .docx ZIP      │
 └─────────────────────────────────────────────────────┘
```

---

## 3. Workspace Crate Responsibilities

| Crate | Path | Responsibility |
| :--- | :--- | :--- |
| `typst2word` | `src/` | CLI binary (`src/main.rs`) and top-level orchestration API (`src/lib.rs`). |
| `compiler` | `crates/compiler/` | Self-contained Typst 0.15 `World` implementation (`TypstWorld`), system/custom font discovery (`CustomFontRegistry`), `@preview` package resolution & tar.gz unpacking, and `CUSTOM_ROUTINES` instrumentation preserving pre-layout realized semantics alongside laid-out `PagedDocument` frames. |
| `extract` | `crates/extract/` | Walks `Tag::Start` / `Tag::End` frame trees to reconstruct semantic blocks and inlines (`Paragraph`, `Heading`, `List`, `Enum`, `Terms`, `Table`, `Equation`, `Shape`, `CetzCanvas`, `CodeBlock`, `Figure`, `Footnote`, `Outline`, `Bibliography`, `HorizontalRule`), captures 1-to-N `ReferenceLayout` fragments, extracts binary OpenType metrics (`ttf-parser`), and prepares embedded/subsetted SFNT fonts (`allsorts`). |
| `ir` | `crates/ir/` | Defines the canonical, serializable, Word-agnostic `DocumentIr` schema, `NodeId` provenance (`NodeOrigin`), `NodeReferenceLayout`, unit primitives (`Pt`), and structural normalization passes (`normalize_document_ir`). |
| `word-construct` | `crates/word-construct/` | Consumes `&DocumentIr` immutably and produces a solved `WordDocument`. Owns all Word-specific modeling: `WordLineBox` & CJK `1.3×` expansion, `PTLS7` tallest-run line-height denominator, Knuth-Plass glue compensation, `compatibilityMode=15` inter-block `flow_spacing`, container-to-table lowering, 2-DoF OMML equation calibration, and DrawingML `<wps:bodyPr>` / `<wp:effectExtent>` solvers. |
| `docx-writer` | `crates/docx-writer/` | Pure OpenXML and OPC ZIP serializer. Emits `[Content_Types].xml`, `_rels/.rels`, `word/document.xml`, `word/styles.xml`, `word/numbering.xml`, `word/settings.xml`, `word/footnotes.xml`, `word/header*.xml`, `word/footer*.xml`, `word/fontTable.xml`, `word/_rels/fontTable.xml.rels`, `word/fonts/*.odttf`, and `word/media/*`. |
| `typst2word-wasm` | `crates/wasm/` | `wasm-bindgen` bindings exposing `WasmWorkspace` for browser Web Workers, virtual file trees, `@preview` package caching, OpenXML inspection, and `.docx` export. |
| `visual-test` | `crates/visual-test/` | Single-variable visual parity harness (`visual-test-cli`) and native macOS `gpui-kit` visual inspection studio (`visual-test-gui`). |

---

## 4. Key Architectural Invariants

1. **Strict IR Immutability**: `build_word_document(&DocumentIr) -> WordDocument` takes `&DocumentIr` by shared reference and never mutates IR nodes. All Word-specific layout decisions (`paragraph_line`, `code_line_spacing`, `shape`, `before_dxa`, `after_dxa`) are stored in `WordDocument::solved_nodes: BTreeMap<NodeId, SolvedNodeLayout>`.
2. **Unified `NodeId` Provenance**: Every extracted semantic block (including `HorizontalRule`, `ShapeBlock`, `EquationBlock`, `Table`, `Container`, `CodeBlock`, `Paragraph`, `Heading`, etc.) is assigned a monotonic `NodeId` linking three parallel maps:
   - The tree node itself (`BlockNode` / `ShapeIr` / `TableIr`)
   - `DocumentIr::origins: BTreeMap<NodeId, NodeOrigin>` (source file, byte span, line/col, Typst `Location`, semantic role)
   - `DocumentIr::reference_layout: BTreeMap<NodeId, NodeReferenceLayout>` (1-to-N page fragments, vertical spans, line baselines, inter-block flow measures)
3. **Structural Normalization (`normalize_document_ir`)**: Runs at the end of `extract` to canonicalize bookmark anchors, clean redundant page breaks, coalesce adjacent compatible `TextRunIr` spans, and merge cross-page split `Container` and `CodeBlock` fragments (matching by `NodeOrigin.typst_location` first, with cross-page continuity fallback when Typst omits `Tag::Start` on continuation pages).
4. **Verbatim Font Preservation**: Neither `extract` nor `word-construct` silently renames user-requested font families; realized family names are preserved verbatim in `<w:rFonts>` while embedding or emitting `<w:altName>` fallbacks and explicit diagnostics.
