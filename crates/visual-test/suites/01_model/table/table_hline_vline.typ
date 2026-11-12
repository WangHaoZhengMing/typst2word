// @func: table
// @docs: https://typst.app/docs/reference/model/table/#definitions-hline

// === @case table_three_line ===
// @title: Table: Academic Three-Line Table (hline)
// @param: hline
// @var: table(stroke: none, table.hline)
// @size: 340x150
// @ir: Table
// @xml: <w:tbl>
#set text(font: "Times New Roman", size: 11pt)
#table(
  columns: (100pt, 100pt, 100pt),
  stroke: none,
  table.hline(stroke: 1.2pt + black),
  [*Model*], [*Score*], [*Time*],
  table.hline(stroke: 0.6pt + black),
  [Baseline], [94.2], [12ms],
  [Optimized], [98.7], [8ms],
  table.hline(stroke: 1.2pt + black),
)
