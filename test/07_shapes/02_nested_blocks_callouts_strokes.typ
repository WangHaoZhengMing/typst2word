// Test 07_shapes/02_nested_blocks_callouts_strokes.typ
// Covers: Styled `#block` callout boxes with left accent borders, full borders,
// background fills, rounded corners, and custom insets.

#set page(
  paper: "a4",
  margin: (top: 2.54cm, bottom: 2.54cm, left: 3.18cm, right: 3.18cm),
)
#set text(font: "Times New Roman", size: 11pt, lang: "en", hyphenate: false)
#set par(
  justify: true,
  linebreaks: "simple",
  justification-limits: (spacing: (min: 100%, max: 500%)),
  leading: 0.65em,
  spacing: 1.2em,
)

= 1. Callout Blocks with Left Accent Border (`stroke: (left: 3pt)`)

Technical documentation frequently uses left-bordered callout blocks with light background shading:

#block(
  width: 100%,
  fill: rgb("#f0f7ff"),
  stroke: (left: 3.5pt + rgb("#0066cc")),
  inset: (x: 12pt, y: 10pt),
)[
  *Architectural Note:* When a `#block` specifies a partial border such as `stroke: (left: 3.5pt + rgb("#0066cc"))` without rounded corners, it maps cleanly to paragraph borders (`<w:pBdr><w:left .../></w:pBdr>`) and shading (`<w:shd>`) or a single-cell callout container.
]

= 2. Rounded Callout Cards (`radius: 6pt`)

#block(
  width: 100%,
  fill: rgb("#fff8e6"),
  stroke: 1.2pt + rgb("#e6a800"),
  radius: 6pt,
  inset: 12pt,
)[
  *Warning — Font Metric Compatibility:* On macOS, `Songti SC` lacks an OpenType `GSUB`/`GPOS` script table (`script_count = 0`), causing Microsoft Word's font validator to reject its East Asian script coverage and route it through `Latin` (`script_id = 5`) in `MicrosoftPTLS7`.
]

= 3. Theorem / Proof Box with Custom Inset and Formulas

#block(
  width: 100%,
  fill: rgb("#f4fbf7"),
  stroke: 1.0pt + rgb("#2e7d32"),
  radius: 4pt,
  inset: 12pt,
)[
  *Theorem 1 (Parseval's Identity).* Let $\{e_n\}_{n=1}^infinity$ be an orthonormal basis of a separable Hilbert space $cal(H)$. Then for every $x in cal(H)$,
  $ norm(x)^2 = sum_(n=1)^infinity abs((chevron.l x, e_n chevron.r))^2. $
]
