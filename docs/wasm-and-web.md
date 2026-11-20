# WebAssembly SDK & Web Converter

`typst2word` compiles to `wasm32-unknown-unknown` with zero native C dependencies (including pure-Rust `allsorts` font subsetting with `flate2_rust`), powering both a standalone JavaScript/TypeScript WebAssembly SDK (`crates/wasm`) and the browser-local web converter (`web/`).

---

## 1. WebAssembly SDK (`crates/wasm`)

### Building the WASM Package

```bash
# Build for web (ES module), bundler, or nodejs via wasm-pack
./scripts/build-wasm.sh web
./scripts/build-wasm.sh bundler
./scripts/build-wasm.sh nodejs
```

### `WasmWorkspace` API (`wasm-bindgen`)

`WasmWorkspace` is a stateful in-memory workspace designed to run inside a browser Dedicated Web Worker:

| Method | Signature | Description |
| :--- | :--- | :--- |
| `new` | `new WasmWorkspace()` | Initializes a workspace preloaded with Typst's embedded Latin/Mono/Math fonts. |
| `set_embed_fonts_mode` | `set_embed_fonts_mode(mode: string)` | Sets `.docx` font embedding mode: `"subset"` (default), `"full"`, or `"none"`. |
| `set_main_source` | `set_main_source(path: string, source: string)` | Sets the entry `.typ` file path and UTF-8 source text. |
| `put_file` / `remove_file` / `clear_files` | `put_file(path, Uint8Array)` | Manages virtual project files (`.typ`, `.bib`, `.csv`, `.png`, `.jpg`, `.svg`, etc.). |
| `set_main_path` / `main_path` | `set_main_path(path: string)` | Switches or reads the active entry `.typ` path. |
| `set_inputs_json` | `set_inputs_json(json: string)` | Populates `sys.inputs` from a JSON object of string key-value pairs. |
| `set_today_ymd` | `set_today_ymd(year, month, day)` | Sets deterministic compilation date for `datetime.today()`. |
| `add_font_bytes` | `add_font_bytes(bytes: Uint8Array, is_system: bool): number` | Registers raw `.ttf`, `.otf`, or `.ttc` font binaries into the workspace font book. Returns face count added. |
| `list_font_families_json` | `list_font_families_json(): string` | Returns JSON array of `{ family, is_system_installed }`. |
| `register_package_tar_gz` | `register_package_tar_gz(ns, name, ver, Uint8Array)` | Unpacks and registers a `@preview/name:version` `.tar.gz` archive in memory. |
| `compile_and_convert` | `compile_and_convert(include_svg_preview: bool): WasmConversionOutput` | Compiles the workspace and returns `.docx` bytes, `report_json`, optional `pages_svg_json`, and `missing_packages_json`. |
| `export_openxml_json` | `export_openxml_json(): string` | Returns all generated OpenXML parts (`word/document.xml`, `word/fontTable.xml`, `word/settings.xml`, etc.) as JSON. |
| `export_ir_json` | `export_ir_json(): string` | Returns pretty-printed `DocumentIr` JSON. |

A stateless convenience function is also exported:

```ts
import init, { convert_typst_to_docx } from 'typst2word-wasm';

await init();
const docxBytes: Uint8Array = convert_typst_to_docx('= Hello\n\n$E = m c^2$');
```

---

## 2. Minimalist Web Converter (`web/`)

Deployed automatically to GitHub Pages via `.github/workflows/pages.yml` (`https://wanghaozhengming.github.io/typst2word/`).

### Architecture

- **Single-Purpose Converter UI (`web/src/App.svelte`)**: Centered 680 px single-column converter focused strictly on `.typ` / `.zip` → `.docx` conversion (no Typst preview pane or IDE chrome).
  - **Upload Mode**: Drag-and-drop or file picker accepting `.typ`, `.zip` project archives (unpacked client-side via `jszip`), raster/SVG images, data files, and custom `.ttf`/`.otf`/`.ttc` fonts, with automatic entry-file detection.
  - **Paste Mode**: Direct monospace code input with custom output filename.
  - **Font Embedding Control**: Segmented selector (`Subset` / `Full` / `None`) wired to `WasmWorkspace::set_embed_fonts_mode`.
  - **Result Card & OpenXML Inspector**: Displays output filename, `.docx` size, elapsed time, non-zero element counts, embedding & fallback notices, one-click download, and a modal OpenXML package inspector (`XRayInspector.svelte`).
- **Dedicated Web Worker (`web/src/lib/compiler.worker.ts`)**:
  - Streams the WASM binary via `fetch` + `ReadableStream` to emit real-time download & initialization progress (`%` and MB) without blocking the UI thread.
  - Lazily fetches unresolved `@preview` packages from `packages.typst.org` and caches them in IndexedDB (`web/src/lib/idb-cache.ts`).
- **On-Demand Local Font Access (`web/src/lib/local-fonts.ts`)**:
  - On Chromium browsers (Chrome, Edge, Arc, Brave), supports the **Local Font Access API (`window.queryLocalFonts()`)**.
  - After one user-triggered permission grant, scans active `.typ` files for `font:` declarations (including common CJK aliases such as `宋体`/`SimSun`/`Songti SC`, `微软雅黑`/`Microsoft YaHei`, `苹方`/`PingFang SC`) and reads `face.blob()` **only for matched families** (up to 4 core styles per family).
  - On Safari/Firefox (where `queryLocalFonts()` is unavailable), gracefully falls back to manual `.ttf`/`.otf`/`.ttc` font upload.
