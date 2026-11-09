// Test 06_tables/02_colspan_rowspan_merges.typ
// Covers: Horizontal cell spans (`colspan` -> `<w:gridSpan>`),
// vertical cell spans (`rowspan` -> `<w:vMerge w:val="restart"/>` / `<w:vMerge/>`),
// and combined 2D rectangular cell merges.

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

= 1. Horizontal (`colspan`) and Vertical (`rowspan`) Cell Merges

In OOXML WordprocessingML, horizontal merges emit `<w:gridSpan w:val="N"/>` while vertical merges emit `<w:vMerge w:val="restart"/>` on the top cell and `<w:vMerge/>` continuation cells in subsequent rows at the exact grid column index.

#table(
  columns: (95pt, 110pt, 105pt, 105pt),
  inset: 6pt,
  stroke: 0.6pt + rgb("#546e7a"),
  fill: (x, y) => if y == 0 { rgb("#e3f2fd") } else { none },
  table.header(
    [*Category*], [*Component*], [*Input Spec*], [*Output Spec*],
  ),
  table.cell(rowspan: 3)[*Typography & Layout*],
  [Line Breaker], [Paragraph IR], [Wrapped Lines],
  [Font Metrics], [TrueType `hmtx`], [Advance in `pt`],
  [Page Flow], [Block Sequence], [Paginated Frames],
  table.cell(rowspan: 2)[*Vector & Math*],
  [CeTZ Canvas], [`cetz.canvas`], [`<wpg:wgp>` Group],
  [Math Equations], [`EquationElem`], [`<m:oMath>` XML],
  table.cell(colspan: 3, align: right)[*Total Verified Conversion Subsystems*],
  [*5 Modules*],
)

= 2. Combined 2x2 Block Merge (`colspan: 2, rowspan: 2`)

#table(
  columns: (100pt, 105pt, 105pt, 105pt),
  inset: 6pt,
  stroke: 0.6pt + rgb("#607d8b"),
  table.cell(colspan: 2, rowspan: 2, fill: rgb("#fff8e1"), align: center + horizon)[
    *Merged 2x2 Header Block* \ (`colspan: 2, rowspan: 2`)
  ],
  [*Metric A*], [*Metric B*],
  [99.4%], [100.0%],
  [*Row 3 Col 1*], [*Row 3 Col 2*], [0.02 pt], [0.00 pt],
)
