// @func: table
// @docs: https://typst.app/docs/reference/model/table/

// === @case table_fixed_cols ===
// @title: Table: Fixed Column Widths (90pt, 120pt, 90pt)
// @param: columns: (pt)
// @var: table(columns: (90pt, 120pt, 90pt))
// @size: 340x140
// @ir: Table
// @xml: <w:tbl> | <w:gridCol
#set text(font: "Times New Roman", size: 11pt)
#table(
  columns: (90pt, 120pt, 90pt),
  [Key], [Description], [Unit],
  [Alpha], [Baseline metric], [pt],
)

// === @case table_fr_cols ===
// @title: Table: Fractional Widths (1fr, 2fr, 1fr)
// @param: columns: (fr)
// @var: table(columns: (1fr, 2fr, 1fr))
// @size: 360x140
// @ir: Table
// @xml: <w:tbl>
#set text(font: "Times New Roman", size: 11pt)
#table(
  columns: (1fr, 2fr, 1fr),
  [ID], [Component Role], [State],
  [01], [Layout Engine], [Ready],
)

// === @case table_inset_0pt ===
// @title: Table: Zero Cell Inset (0pt)
// @param: inset: 0pt
// @var: table(inset: 0pt)
// @size: 320x120
// @ir: Table
#set text(font: "Times New Roman", size: 11pt)
#table(
  columns: (120pt, 120pt),
  inset: 0pt,
  [ZeroA], [ZeroB],
  [ZeroC], [ZeroD],
)

// === @case table_inset_10pt ===
// @title: Table: Generous Cell Inset (10pt)
// @param: inset: 10pt
// @var: table(inset: 10pt)
// @size: 320x150
// @ir: Table
#set text(font: "Times New Roman", size: 11pt)
#table(
  columns: (120pt, 120pt),
  inset: 10pt,
  [PadA], [PadB],
  [PadC], [PadD],
)

// === @case table_cell_align ===
// @title: Table: Horizontal & Vertical Cell Alignment
// @param: align
// @var: table(align: (left + top, center + horizon, right + bottom))
// @size: 340x140
// @ir: Table
// @xml: <w:vAlign
#set text(font: "Times New Roman", size: 11pt)
#table(
  columns: (100pt, 100pt, 100pt),
  rows: (42pt),
  align: (left + top, center + horizon, right + bottom),
  [TopLeft], [MidCenter], [BotRight],
)

// === @case table_stroke_heavy ===
// @title: Table: Colored Heavy Stroke (1.5pt)
// @param: stroke
// @var: table(stroke: 1.5pt + rgb("#1D4ED8"))
// @size: 320x130
// @ir: Table
// @xml: 1D4ED8
#set text(font: "Times New Roman", size: 11pt)
#table(
  columns: (130pt, 130pt),
  stroke: 1.5pt + rgb("#1D4ED8"),
  [Blue A], [Blue B],
  [Blue C], [Blue D],
)

// === @case table_fill_zebra ===
// @title: Table: Header & Zebra Cell Fill
// @param: fill
// @var: table(fill: (x, y) => ...)
// @size: 340x150
// @ir: Table
// @xml: E2E8F0
#set text(font: "Times New Roman", size: 11pt)
#table(
  columns: (140pt, 140pt),
  fill: (_, y) => if y == 0 { rgb("#E2E8F0") } else if calc.odd(y) { white } else { rgb("#F8FAFC") },
  [*Field*], [*Value*],
  [Row 1], [100],
  [Row 2], [200],
)
