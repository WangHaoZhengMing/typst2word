# CLI & Rust Library Reference

This document specifies the command-line interface (`typst2word`) and the top-level Rust library API exposed by the root `typst2word` crate.

---

## 1. Command-Line Interface

### Synopsis

```bash
typst2word <input.typ> [output.docx] [OPTIONS]
```

If `[output.docx]` is omitted, `typst2word` writes the output to `<input>.docx` in the same directory as `<input.typ>`.

### Options

| Option | Argument | Default | Description |
| :--- | :--- | :--- | :--- |
| `--embed-fonts=<mode>` | `subset` \| `full` \| `none` | `subset` | Controls font embedding in the generated `.docx` package. Also accepts space-separated form (`--embed-fonts full`) and bare `--embed-fonts` (equivalent to `subset`). |
| `--no-embed-fonts` | — | — | Shorthand for `--embed-fonts=none`. Disables font embedding and emits only `word/fontTable.xml` `<w:altName>` fallback hints. |
| `--input <key=value>` | `key=value` | — | Passes a string entry into Typst's `sys.inputs` dictionary. May be specified multiple times. |
| `--dump-ir <path>` | File path | — | Serializes the extracted, Word-independent `DocumentIr` as pretty-printed JSON to `<path>` for inspection or debugging. Embedded font binaries are excluded (`#[serde(skip)]`). |
| `-h`, `--help` | — | — | Prints usage information and exits with status `0`. |

### Examples

```bash
# Default conversion (writes thesis.docx with subset font embedding enabled)
typst2word thesis.typ

# Explicit output path with full font embedding for unrestricted editing in Word
typst2word report.typ dist/report.docx --embed-fonts=full

# Minimal file size without embedded font binaries
typst2word draft.typ draft.docx --no-embed-fonts

# Supply sys.inputs variables (repeatable)
typst2word paper.typ paper.docx --input blind_review=true --input version=2.1

# Export intermediate DocumentIr JSON alongside .docx generation
typst2word report.typ report.docx --dump-ir report.ir.json
```

### Terminal Diagnostics & Exit Codes

Upon successful conversion (`exit 0`), `typst2word` prints:

1. **Structural Summary (`stdout`)**: Counts of pages, headings, paragraphs, lists, tables, OMML equations, DrawingML shapes, CeTZ canvases, images, quotes, code blocks, figures, footnotes, links, and unsupported elements.
2. **Font Embedding Notice (`stdout`, `ℹ️`)**: Reports the number of embedded font faces and approximate payload size. In `subset` mode, reminds the user that newly typed characters absent from the source document will fall back to system fonts unless `--embed-fonts=full` is used.
3. **Compiler & Font Fallback Warnings (`stderr`, `⚠️`)**: Deduplicated Typst compiler warnings and explicit notices whenever Typst shapes glyphs via a fallback font or references a non-system font that was not embedded. Original font family names are preserved verbatim in the IR and `.docx`.
4. **Unsupported Element Markers (`stdout`, `!`)**: Lists any unsupported Typst primitives that were lowered to visible in-document placeholder badges.

On argument errors, Typst compilation failures, or I/O errors, `typst2word` prints the diagnostic to `stderr` and exits with a non-zero exit code (`ExitCode::FAILURE`).

---

## 2. Rust Library API

Add `typst2word` as a dependency (workspace path or git dependency) to invoke the compiler, extractor, layout solver, and `.docx` serializer programmatically.

### Re-exported Workspace Crates

The root `typst2word` crate re-exports all five core pipeline crates:

- `typst2word::compiler`
- `typst2word::extract`
- `typst2word::ir`
- `typst2word::word_construct`
- `typst2word::docx_writer`

### High-Level Functions (`src/lib.rs`)

#### File-Based Conversion (`#[cfg(not(target_arch = "wasm32"))]`)

```rust
pub fn compile_file_to_ir(input_path: &Path) -> Result<DocumentIr, String>

pub fn compile_file_to_ir_with_inputs<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
) -> Result<DocumentIr, String>

pub fn compile_file_to_ir_with_options<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
    embed_mode: FontEmbedMode,
) -> Result<DocumentIr, String>

pub fn convert_typst_file_to_docx(
    input_path: &Path,
    output_path: &Path,
) -> Result<DocumentIr, String>

pub fn convert_typst_file_to_docx_with_inputs<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    output_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
) -> Result<DocumentIr, String>

pub fn convert_typst_file_to_docx_with_options<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    output_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
    embed_mode: FontEmbedMode,
) -> Result<DocumentIr, String>
```

#### In-Memory Conversion (Native & WebAssembly)

```rust
pub fn compile_str_to_ir(
    source: &str,
    root_dir: Option<&Path>,
) -> Result<DocumentIr, String>

pub fn compile_str_to_ir_with_inputs<K: AsRef<str>, V: AsRef<str>>(
    source: &str,
    root_dir: Option<&Path>,
    inputs: impl IntoIterator<Item = (K, V)>,
) -> Result<DocumentIr, String>

pub fn convert_typst_str_to_docx_bytes(
    source: &str,
    root_dir: Option<&Path>,
) -> Result<(Vec<u8>, DocumentIr), String>

pub fn convert_typst_str_to_docx_bytes_with_options(
    source: &str,
    root_dir: Option<&Path>,
    options: DocxWriteOptions,
) -> Result<(Vec<u8>, DocumentIr), String>
```

### Usage Example

```rust
use std::path::Path;
use typst2word::ir::FontEmbedMode;
use typst2word::{
    compile_file_to_ir,
    convert_typst_file_to_docx_with_options,
    convert_typst_str_to_docx_bytes,
};

fn main() -> Result<(), String> {
    // Convert a file on disk with full font embedding and sys.inputs
    let ir = convert_typst_file_to_docx_with_options(
        Path::new("report.typ"),
        Path::new("dist/report.docx"),
        [("draft", "false")],
        FontEmbedMode::Full,
    )?;
    println!(
        "Pages: {}, Equations: {}, Tables: {}",
        ir.report.pages_count, ir.report.equations_count, ir.report.tables_count
    );

    // Convert an in-memory Typst source string directly to .docx bytes
    let (bytes, _ir) = convert_typst_str_to_docx_bytes(
        "= Summary\n\nEinstein's relation $E = m c^2$.",
        None,
    )?;
    std::fs::write("summary.docx", bytes).map_err(|e| e.to_string())?;

    // Inspect the pure, Word-independent DocumentIr
    let _pure_ir = compile_file_to_ir(Path::new("report.typ"))?;
    Ok(())
}
```
