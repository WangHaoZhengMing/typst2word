// @func: figure
// @docs: https://typst.app/docs/reference/model/figure/

// === @case figure_table_top_caption ===
// @title: Figure: Table with Top Caption
// @param: caption + kind
// @var: figure(table, caption) [top]
// @size: 340x150
// @ir: Figure
// @xml: Latency Summary
#set text(font: "Times New Roman", size: 11pt)
#show figure.where(kind: table): set figure.caption(position: top)
#figure(
  table(columns: (100pt, 100pt), [Metric], [Value], [Latency], [4ms]),
  caption: [Latency Summary],
)

// === @case figure_shape_bottom_caption ===
// @title: Figure: Shape with Bottom Caption
// @param: caption
// @var: figure(rect, caption) [bottom]
// @size: 340x150
// @ir: Figure
// @xml: System Diagram
#set text(font: "Times New Roman", size: 11pt)
#figure(
  rect(width: 140pt, height: 44pt, fill: rgb("#E0F2FE"), stroke: 0.8pt + rgb("#0284C7"))[Module],
  caption: [System Diagram],
)
