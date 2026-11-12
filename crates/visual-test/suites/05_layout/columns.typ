// @func: columns
// @docs: https://typst.app/docs/reference/layout/columns/

// === @case columns_two ===
// @title: Multi-Column Section & Explicit colbreak()
// @param: count + gutter + colbreak
// @var: columns(2, gutter: 16pt) + colbreak()
// @size: 340x130
// @ir: Columns
// @xml: w:num="2"
#set text(font: "Times New Roman", size: 11pt)
#columns(2, gutter: 16pt)[
  Left column content.
  #colbreak()
  Right column content.
]
