# Fonts & OOXML Embedding

This document describes how `typst2word` resolves font metrics, handles Typst font fallbacks, subsets and packages font binaries into `.docx`, and handles OpenType Math fonts.

---

## 1. Three-Layer Font Architecture

1. **Compile-Time Shaping (`compiler` & `extract`)**:
   Typst shapes every `FrameItem::Text` using the document's active `TextElem::font` fallback chain (system fonts + custom fonts + Typst built-in fonts: `Libertinus Serif`, `DejaVu Sans Mono`, `New Computer Modern Math`). Whenever Typst falls back to a font outside the configured `TextElem::font` list, `extract` records a deduplicated `⚠️` warning in `ConversionReport::font_fallback_warnings` without altering the realized font family name.
2. **Binary OpenType Metrics (`crates/extract/src/font_metrics.rs`)**:
   For every realized font family, `extract` parses the actual font binary with `ttf-parser` to extract `upem`, `hhea` (`ascender`, `descender`, `line_gap`), `OS/2` (`sCapHeight`, `sTypoAscender`, `sTypoDescender`, unsigned `usWinAscent`, `usWinDescent`, `ulCodePageRange1` CJK bits `17..=20`), `GSUB` `hani` script support, monospace status, and whether the family is installed on the host OS.
3. **Verbatim Naming & OOXML Packaging (`word-construct` & `docx-writer`)**:
   Realized font names are written verbatim into `<w:rFonts w:ascii="..." w:hAnsi="..." w:eastAsia="..." w:cs="..."/>`. Depending on `FontEmbedMode`, `docx-writer` embeds `.odttf` payloads into `word/fonts/` and emits dynamic `word/fontTable.xml` entries with `<w:altName>` fallbacks.

---

## 2. Font Embedding Modes (`FontEmbedMode`)

| Mode | CLI Flag | Behavior | Typical Size | Trade-offs |
| :--- | :--- | :--- | :--- | :--- |
| **`Subset`** *(default)* | `--embed-fonts=subset` | Subsets every realized text/code/CJK font face to only the glyphs actually used in the document. | ~50–300 KB | Very compact `.docx` while preserving exact visual fidelity on any machine. Typing new characters not present in the document causes Word to fall back to system fonts for those new characters. |
| **`Full`** | `--embed-fonts=full` | Embeds complete standalone SFNT files for every realized face (unpacking single faces from `.ttc` collections). | 1–30 MB | Full unrestricted editability in Word even on machines without the fonts installed; significantly larger files when CJK fonts are used. |
| **`None`** | `--embed-fonts=none` / `--no-embed-fonts` | Embeds no font binaries; emits only `word/fontTable.xml` with `<w:altName>` hints. | <20 KB | Smallest output; requires target machines to have matching fonts installed, otherwise Word falls back to system fonts. |

In `Subset` mode, `typst2word` emits an explicit `ℹ️` notice summarizing the embedded font count, approximate payload size, and reminding users to pass `--embed-fonts=full` if full editability is required.

---

## 3. Technical Details of Font Packaging

### Supported Font Containers

- **TrueType (`.ttf`, `0x00010000`)**: Supported in both `Subset` and `Full` modes.
- **OpenType CFF (`.otf`, `OTTO`)**: Supported in both `Subset` and `Full` modes (empirically verified with Microsoft Word for Mac / Microsoft 365 using non-installed `LibertinusSerif-Regular.otf`). Legacy Windows Word builds (Office 2010/2013) that do not support CFF embedding gracefully fall back via `<w:altName>`.
- **TrueType Collections (`.ttc`, `ttcf`)**: Common on macOS (`PingFang SC`, `Songti SC`, `Heiti SC`). In `Full` mode, `extract_standalone_sfnt` extracts the exact `face_index` into a standalone SFNT binary with recalculated table offsets and `head.checkSumAdjustment`. In `Subset` mode, `allsorts` directly subsets the target face index into a standalone SFNT.

### Pure-Rust Subsetting & Windows `cmap` Aliasing

Subsetting is performed by the pure-Rust `allsorts` crate (`default-features = false, features = ["flate2_rust"]`, zero C dependencies, fully `wasm32-unknown-unknown` compatible) using `SubsetProfile::Minimal` (`cmap`, `head`, `hhea`, `hmtx`, `maxp`, `name`, `OS/2`, `post`, plus `glyf`/`loca` or `CFF `) and `CmapTarget::Unicode`.

Because `allsorts`'s `CmapTarget::Unicode` writes only Platform `0` (`Unicode`) `cmap` subtables (`(0, 3)` / `(0, 4)`), while Microsoft Word requires a **Platform `3` (Windows)** `cmap` subtable (`(3, 1)` BMP format 4 or `(3, 10)` full Unicode format 12) to map characters correctly without rendering tofu/shifted glyphs, `crates/extract/src/font_embed.rs` runs `ensure_windows_cmap_subtable` on every subsetted font to alias Platform `0` subtables to Platform `3` and recompute table and `head.checkSumAdjustment` checksums. If subsetting fails on an unusual font table, `extract` automatically falls back to full standalone SFNT extraction for that face.

### ECMA-376 §17.8.1 `.odttf` Obfuscation

In `crates/docx-writer/src/parts/fonts.rs`:
1. Each embedded face receives a deterministic RFC 4122 v4 GUID derived from `(family, slot, payload_sample)` so `.docx` outputs remain bit-for-bit reproducible.
2. The first 32 bytes of the standalone SFNT payload are XOR-obfuscated with the 16-byte GUID in **reversed byte order** and stored at `word/fonts/fontN.odttf`.
3. `[Content_Types].xml` registers `<Default Extension="odttf" ContentType="application/vnd.openxmlformats-officedocument.obfuscatedFont"/>`.
4. `word/_rels/fontTable.xml.rels` links each `rIdN` to `fonts/fontN.odttf`.
5. `word/fontTable.xml` emits `<w:embedRegular>`, `<w:embedBold>`, `<w:embedItalic>`, and `<w:embedBoldItalic>` with `w:fontKey="{GUID}"` and `w:subsetted="1"` (when subsetted), alongside `<w:altName>` fallbacks.
6. `word/settings.xml` emits `<w:embedTrueTypeFonts/>`, `<w:embedSystemFonts/>`, and (when subsetted) `<w:saveSubsetFonts/>`.

---

## 4. Mathematics Fonts & OMML Compatibility

The converter reads the OpenType family name from the math face Typst used to lay out each equation. The first detected equation face supplies the document's OMML default. Local equation fonts are emitted on math runs and on operators such as radicals and delimiters.

Both `subset` and `full` embed complete math faces so the OpenType `MATH` table, size variants, and glyph assemblies survive packaging. With `none`, the DOCX references the same font names and the reader needs the fonts installed. Word for Mac versions can differ in their support for embedded CFF math fonts in display equations.

---

## 5. Font Licensing & Cross-Platform Responsibility

`typst2word` intentionally does **not** filter or block font embedding based on OpenType `OS/2.fsType` embedding flags or OS system-font allowlists. Users are responsible for complying with the license terms of any fonts they embed—for example, Apple system fonts (such as `PingFang SC`, `Songti SC`, `Heiti SC`) are licensed for use on Apple platforms only. When distributing `.docx` files cross-platform, prefer open-source fonts (e.g., `Noto Serif SC`, `Noto Sans SC`, `Libertinus Serif`, `DejaVu Sans Mono`, `LXGW WenKai`) or disable embedding with `--no-embed-fonts`.
