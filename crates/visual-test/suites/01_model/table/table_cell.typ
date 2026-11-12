// @func: table
// @docs: https://typst.app/docs/reference/model/table/#definitions-cell

// === @case table_colspan ===
// @title: Table: Horizontal Cell Merge (colspan: 2)
// @param: cell.colspan
// @var: table.cell(colspan: 2)
// @size: 320x140
// @ir: Table
// @xml: <w:gridSpan
#set text(font: "Times New Roman", size: 11pt)
#table(
  columns: (130pt, 130pt),
  table.cell(colspan: 2, align: center)[*Merged Header*],
  [Left Cell], [Right Cell],
)

// === @case table_rowspan ===
// @title: Table: Vertical Cell Merge (rowspan: 2)
// @param: cell.rowspan
// @var: table.cell(rowspan: 2)
// @size: 320x140
// @ir: Table
// @xml: <w:vMerge
#set text(font: "Times New Roman", size: 11pt)
#table(
  columns: (110pt, 150pt),
  table.cell(rowspan: 2, align: horizon)[*Group A*], [Item 1],
  [Item 2],
)
