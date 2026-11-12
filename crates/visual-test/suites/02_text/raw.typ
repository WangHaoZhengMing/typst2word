// @func: raw
// @docs: https://typst.app/docs/reference/text/raw/

// === @case inline_raw_code ===
// @title: Inline Raw Monospace
// @param: block: false
// @var: `raw` inline
// @size: 360x100
// @ir: Paragraph
#set text(font: "Times New Roman", size: 12pt)
Call `compute_delta(x, y)` now.

// === @case code_plain ===
// @title: Code Block: Plain Unboxed
// @param: block: true
// @var: raw(block: true)
// @size: 320x120
// @ir: CodeBlock
#show raw: set text(font: "Courier New")
```
alpha = 10
beta = 20
```

// === @case code_rust_syntax ===
// @title: Code Block: Rust Syntax Highlighting
// @param: lang: rust
// @var: raw(block: true, lang: "rust")
// @size: 340x130
// @ir: CodeBlock
// @xml: `<w:color `
#show raw: set text(font: "Courier New")
```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

// === @case code_shaded_sharp ===
// @title: Code Block: Shaded Rect Background
// @param: show + block(fill)
// @var: show raw: block(fill: luma(245), inset: 8pt)
// @size: 340x130
// @ir: CodeBlock
// @xml: F1F5F9
#show raw: set text(font: "Courier New")
#show raw.where(block: true): block.with(width: 100%, fill: rgb("#F1F5F9"), inset: 8pt)
```
server.port=8080
spring.redis.database=2
```

// === @case code_shaded_rounded ===
// @title: Code Block: Rounded Shaded Background (Curve)
// @param: show + block(radius)
// @var: show raw: block(fill, radius: 4pt, inset: 8pt)
// @size: 340x130
// @ir: CodeBlock
// @xml: F5F5F5 | <w:tbl>
#show raw: set text(font: "Courier New")
#show raw.where(block: true): block.with(width: 100%, fill: rgb("#F5F5F5"), radius: 4pt, inset: 8pt)
```
datasource:
  database: 2
  timeout: 2s
```
