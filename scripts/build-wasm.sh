#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:-web}"
WASM_TARGET_BIN="target/wasm32-unknown-unknown/wasm-release/typst2word_wasm.wasm"

echo "==> Building typst2word-wasm (wasm32-unknown-unknown, wasm-release)..."
cargo build --locked -p typst2word-wasm --target wasm32-unknown-unknown --profile wasm-release

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "Error: wasm-bindgen CLI not found on PATH." >&2
  exit 1
fi

optimize_wasm() {
  local wasm_file="$1"
  if command -v wasm-opt >/dev/null 2>&1; then
    echo "==> Optimizing $(basename "$wasm_file") with wasm-opt -Os..."
    if wasm-opt -Os --all-features "$wasm_file" -o "${wasm_file}.opt"; then
      mv "${wasm_file}.opt" "$wasm_file"
    else
      echo "Warning: wasm-opt failed; keeping unoptimized wasm binary." >&2
      rm -f "${wasm_file}.opt"
    fi
  fi
}

if [[ "$MODE" == "web" || "$MODE" == "--web" ]]; then
  OUT_DIR="$ROOT_DIR/web/src/lib/wasm-pkg"
  rm -rf "$OUT_DIR"
  mkdir -p "$OUT_DIR"

  echo "==> Generating wasm-bindgen bindings -> web/src/lib/wasm-pkg..."
  wasm-bindgen "$WASM_TARGET_BIN" \
    --out-dir "$OUT_DIR" \
    --target web \
    --out-name typst2word_wasm

  optimize_wasm "$OUT_DIR/typst2word_wasm_bg.wasm"
  rm -f "$OUT_DIR/.gitignore"
  echo "==> Ready: $OUT_DIR"
elif [[ "$MODE" == "--release-dist" ]]; then
  STAGE_DIR="$ROOT_DIR/target/wasm-dist/typst2word-wasm"
  rm -rf "$ROOT_DIR/target/wasm-dist"
  mkdir -p "$STAGE_DIR/web" "$STAGE_DIR/bundler" "$STAGE_DIR/nodejs"

  echo "==> Generating multi-target WASM SDK bindings (web, bundler, nodejs)..."
  wasm-bindgen "$WASM_TARGET_BIN" --out-dir "$STAGE_DIR/web" --target web --out-name typst2word_wasm
  wasm-bindgen "$WASM_TARGET_BIN" --out-dir "$STAGE_DIR/bundler" --target bundler --out-name typst2word_wasm
  wasm-bindgen "$WASM_TARGET_BIN" --out-dir "$STAGE_DIR/nodejs" --target nodejs --out-name typst2word_wasm

  optimize_wasm "$STAGE_DIR/web/typst2word_wasm_bg.wasm"
  cp "$STAGE_DIR/web/typst2word_wasm_bg.wasm" "$STAGE_DIR/bundler/typst2word_wasm_bg.wasm"
  cp "$STAGE_DIR/web/typst2word_wasm_bg.wasm" "$STAGE_DIR/nodejs/typst2word_wasm_bg.wasm"
  rm -f "$STAGE_DIR"/*/.gitignore

  VERSION="$(grep -m1 '^version = ' "$ROOT_DIR/Cargo.toml" | sed -E 's/version = "(.*)"/\1/')"

  cat > "$STAGE_DIR/package.json" <<EOF
{
  "name": "typst2word-wasm",
  "version": "${VERSION}",
  "description": "WebAssembly SDK for typst2word: semantic + visual Typst (.typ) to Microsoft Word (.docx) converter with live SVG preview",
  "license": "MIT OR Apache-2.0",
  "type": "module",
  "main": "./bundler/typst2word_wasm.js",
  "types": "./web/typst2word_wasm.d.ts",
  "exports": {
    ".": {
      "browser": "./web/typst2word_wasm.js",
      "node": "./nodejs/typst2word_wasm.js",
      "default": "./bundler/typst2word_wasm.js"
    },
    "./web": "./web/typst2word_wasm.js",
    "./bundler": "./bundler/typst2word_wasm.js",
    "./nodejs": "./nodejs/typst2word_wasm.js"
  },
  "files": [
    "web",
    "bundler",
    "nodejs",
    "README.md"
  ]
}
EOF

  cat > "$STAGE_DIR/README.md" <<'EOF'
# typst2word-wasm

WebAssembly bindings for **typst2word** — convert Typst (`.typ`) documents into native, semantically editable Microsoft Word (`.docx`) files (WordprocessingML, OMML equations, DrawingML shapes, and CeTZ vector graphics) entirely in a browser Web Worker or Node.js.

## Directory Targets

- `web/` — ES Module target for browsers, Web Workers, and Vite (`await init()`).
- `bundler/` — Bundler target for Webpack / Rollup (`vite-plugin-wasm`).
- `nodejs/` — CommonJS/Node target for Node.js / Bun CLI tools.

## Browser / Web Worker Quickstart

```js
import init, { WasmWorkspace, convert_typst_to_docx } from "./web/typst2word_wasm.js";

await init();

// 1. Stateless one-liner (.typ string -> Uint8Array .docx)
const docxBytes = convert_typst_to_docx("= Hello\nInline math $E = m c^2$!");

// 2. Stateful multi-file workspace with @preview packages, fonts & SVG preview
const ws = new WasmWorkspace();
ws.set_main_source("main.typ", "= Report\n$ integral_0^1 x^2 dif x = 1/3 $");

// Live SVG preview + semantic conversion telemetry
const preview = JSON.parse(ws.compile_preview_json());
console.log(preview.page_count, preview.report);

// Export .docx bytes
const out = ws.compile_and_convert(false);
if (out.status === "ok") {
  const blob = new Blob([out.docx_bytes()], {
    type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  });
}
```
EOF

  ARCHIVE_BASE="typst2word-wasm32-unknown-unknown"
  tar -C "$ROOT_DIR/target/wasm-dist" -czf "$ROOT_DIR/${ARCHIVE_BASE}.tar.gz" typst2word-wasm
  if command -v zip >/dev/null 2>&1; then
    (cd "$ROOT_DIR/target/wasm-dist" && zip -rq "$ROOT_DIR/${ARCHIVE_BASE}.zip" typst2word-wasm)
  elif command -v python3 >/dev/null 2>&1; then
    python3 -c "import shutil; shutil.make_archive('$ROOT_DIR/${ARCHIVE_BASE}', 'zip', '$ROOT_DIR/target/wasm-dist', 'typst2word-wasm')"
  fi

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${ARCHIVE_BASE}.tar.gz" > "${ARCHIVE_BASE}.tar.gz.sha256"
    [[ -f "${ARCHIVE_BASE}.zip" ]] && shasum -a 256 "${ARCHIVE_BASE}.zip" > "${ARCHIVE_BASE}.zip.sha256"
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${ARCHIVE_BASE}.tar.gz" > "${ARCHIVE_BASE}.tar.gz.sha256"
    [[ -f "${ARCHIVE_BASE}.zip" ]] && sha256sum "${ARCHIVE_BASE}.zip" > "${ARCHIVE_BASE}.zip.sha256"
  fi

  echo "==> Built WASM release archives:"
  ls -lh "${ARCHIVE_BASE}".*
else
  echo "Usage: $0 [web|--release-dist]" >&2
  exit 1
fi
