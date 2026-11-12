// @func: circle
// @docs: https://typst.app/docs/reference/visualize/circle/

// === @case shape_circle ===
// @title: Circle Primitive (radius: 32pt)
// @param: radius: 32pt
// @var: circle(radius: 32pt)
// @size: 200x120
// @ir: ShapeBlock
// @xml: ellipse
#set text(font: "Times New Roman", size: 11pt)
#circle(radius: 32pt, fill: rgb("#FCE7F3"), stroke: 1pt + rgb("#BE185D"))[C]
