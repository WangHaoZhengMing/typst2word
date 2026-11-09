// Test 06_tables/03_strokes_fills_and_headers.typ
// Covers: Academic three-line tables (`table.hline`), alternating zebra row fills,
// custom per-cell borders, and repeating `table.header`.

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

= 1. Academic Three-Line Table (`booktabs` Style)

Academic publications omit vertical rules and use a heavy top rule (`1.5pt`), a medium header rule (`0.75pt`), and a heavy bottom rule (`1.5pt`):

#table(
  columns: (115pt, 100pt, 100pt, 100pt),
  stroke: none,
  inset: (x: 8pt, y: 6pt),
  align: (left, right, right, right),
  table.hline(stroke: 1.5pt + black),
  table.header(
    [*Model Architecture*], [*Params (M)*], [*Top-1 Acc (%)*], [*Latency (ms)*],
  ),
  table.hline(stroke: 0.75pt + black),
  [ResNet-50 Baseline], [25.6], [76.15], [4.12],
  [ConvNeXt-Tiny], [28.6], [82.10], [4.85],
  [Swin-Transformer-T], [28.3], [81.30], [6.40],
  [ViT-Base/16], [86.6], [84.52], [11.20],
  table.hline(stroke: 1.5pt + black),
)

= 2. Zebra-Striped Data Table with Custom Header Shading

#table(
  columns: (85pt, 170pt, 80pt, 80pt),
  inset: 6pt,
  stroke: 0.5pt + rgb("#b0bec5"),
  fill: (x, y) => if y == 0 {
    rgb("#1565c0")
  } else if calc.odd(y) {
    rgb("#f5f9ff")
  } else {
    rgb("#ffffff")
  },
  table.header(
    [#text(fill: white)[*Region*]],
    [#text(fill: white)[*Financial Hub Classification*]],
    [#text(fill: white)[*2021 IPO*]],
    [#text(fill: white)[*2023 IPO*]],
  ),
  [Hong Kong], [International Offshore Gateway], [7000 HKD], [1507 HKD],
  [Shanghai], [Mainland Domestic Capital Market], [5400 CNY], [3100 CNY],
  [Singapore], [ASEAN Wealth Management Hub], [1900 SGD], [1650 SGD],
  [London], [European Cross-Border Clearing], [4200 GBP], [2100 GBP],
)
