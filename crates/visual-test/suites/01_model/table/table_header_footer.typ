// @func: table
// @docs: https://typst.app/docs/reference/model/table/#definitions-header

// === @case table_header_repeat ===
// @title: Table: Semantic Header Row (table.header)
// @param: header + footer
// @var: table.header + table.footer
// @size: 320x150
// @ir: Table
// @xml: <w:tbl>
#set text(font: "Times New Roman", size: 11pt)
#table(
  columns: (130pt, 130pt),
  table.header([*Column*], [*Type*]),
  [id], [u64],
  table.footer([*Total*], [*1*]),
)
