// @func: place
// @docs: https://typst.app/docs/reference/layout/place/

// === @case shape_place_canvas ===
// @title: 2D Positioned Canvas (#place dx/dy)
// @param: dx + dy
// @var: block + place(dx, dy)
// @size: 260x130
#set text(font: "Times New Roman", size: 10pt)
#block(width: 220pt, height: 90pt)[
  #place(top + left, dx: 8pt, dy: 8pt, rect(width: 72pt, height: 30pt, fill: rgb("#DBEAFE"), stroke: 0.8pt + rgb("#1D4ED8"))[Node A])
  #place(top + left, dx: 120pt, dy: 42pt, rect(width: 72pt, height: 30pt, fill: rgb("#FCE7F3"), stroke: 0.8pt + rgb("#BE185D"))[Node B])
]
