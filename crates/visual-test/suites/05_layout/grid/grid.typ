// @func: grid
// @docs: https://typst.app/docs/reference/layout/grid/

// === @case grid_2col_gutter ===
// @title: Grid: 2-Column Layout with 16pt Gutter
// @param: columns + column-gutter
// @var: grid(columns: (1fr, 1fr), column-gutter: 16pt)
// @size: 360x130
// @ir: Table
#set text(font: "Times New Roman", size: 11pt)
#grid(
  columns: (1fr, 1fr),
  column-gutter: 16pt,
  [Left grid column text block.],
  [Right grid column text block.],
)

// === @case grid_underline_fields ===
// @title: Grid: Bottom-Bordered Form Fields (No Black Box)
// @param: column-gutter + row-gutter
// @var: grid + block(stroke: (bottom: 0.5pt + black))
// @size: 360x120
// @ir: Table
// @xml: <w:tcBorders>
#set text(font: "Songti SC", size: 10.5pt, lang: "zh")
#let field(body) = block(width: 100%, above: 0pt, below: 0pt, inset: (bottom: 2pt), stroke: (bottom: 0.5pt + black), align(center, body))
#grid(
  columns: (48pt, 1fr, 56pt, 1fr),
  column-gutter: 6pt,
  row-gutter: 8pt,
  [院　系：], field[*信息学院*], [班　级：], field[*计科2401*],
  [姓　名：], field[*李颂尧*], [学　号：], field[*2310401*],
)

// === @case grid_stacked_flow_boxes ===
// @title: Grid: Vertically Stacked Flowchart Rounded Nodes
// @param: cells with stacked blocks
// @var: grid cell with stacked block(radius: 3pt)
// @size: 360x180
// @ir: Table
#set text(font: "Times New Roman", size: 10pt)
#let node(t) = block(width: 100%, inset: (x: 6pt, y: 4pt), radius: 3pt, fill: rgb("#EEF4F6"), stroke: 0.5pt + rgb("#8AA2AC"), align(center, t))
#let arr = align(center)[↓]
#grid(
  columns: (1fr, 1fr),
  column-gutter: 14pt,
  [#node[Controller]#arr#node[Service]],
  [#node[Repository]#arr#node[Redis]],
)
