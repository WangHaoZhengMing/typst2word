# typst2word Documentation

This directory contains the formal technical documentation for **`typst2word`**, a Rust compiler-backed CLI, library, and WebAssembly engine that converts Typst (`.typ`) documents into native, editable Microsoft Word (`.docx`) packages.

## Documentation Index

| Document | Scope | Audience |
| :--- | :--- | :--- |
| [CLI & Rust Library Reference](./cli-and-library.md) | Command-line options, font embedding modes, exit behavior, and Rust crate entry points | End users, Rust integrators |
| [System Architecture](./architecture.md) | Five-stage conversion pipeline (`compiler` → `extract` → `ir` → `word-construct` → `docx-writer`), dual-source extraction, and workspace layout | Contributors, maintainers |
| [Intermediate Representation (IR) Specification](./ir-specification.md) | `DocumentIr` schema, `BlockNode` / `InlineNode` / `MathNodeIr` / `ShapeIr` types, `NodeOrigin`, `NodeReferenceLayout`, and normalization passes | Contributors, tooling authors |
| [Fonts & OOXML Embedding](./fonts-and-embedding.md) | Font resolution, OpenType binary metrics extraction, `allsorts` subsetting, `.ttc` face unpacking, `.odttf` obfuscation, OMML math font handling, and licensing notes | End users, maintainers |
| [Layout Calibration & Word Modeling](./layout-calibration.md) | `WordLineBox` & CJK `1.3×` expansion, `PTLS7` tallest-run line-height model, Knuth-Plass glue compensation, `compatibilityMode=15` spacing, DrawingML insets, and page margin validation | Maintainers, layout researchers |
| [Feature Coverage & Limitations](./feature-matrix.md) | Complete mapping from Typst constructs to WordprocessingML, OMML, and DrawingML, plus known constraints and graceful degradation behavior | End users, evaluators |
| [WebAssembly SDK & Web Converter](./wasm-and-web.md) | `typst2word-wasm` (`wasm-bindgen`) API, Web Worker architecture, streaming WASM progress, Local Font Access API, and GitHub Pages deployment | Web integrators, contributors |
| [Testing & Visual Parity Suite](./testing-and-visual-parity.md) | Workspace unit tests, `visual-test` 4-stage pipeline, macOS silent Word batching, Swift PDF/CoreGraphics inspector, and desktop GUI studio | Contributors, QA |

## Quick Links

- **Repository Root**: [README.md](../README.md)
- **Web Converter (WASM)**: [wanghaozhengming.github.io/typst2word](https://wanghaozhengming.github.io/typst2word/)
- **Prebuilt Releases**: [GitHub Releases](https://github.com/WangHaoZhengMing/typst2word/releases)
