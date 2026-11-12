// @func: ellipse
// @docs: https://typst.app/docs/reference/visualize/ellipse/

// === @case shape_ellipse ===
// @title: Ellipse Primitive (120pt x 56pt)
// @param: width + height
// @var: ellipse(width: 120pt, height: 56pt)
// @size: 220x120
// @ir: ShapeBlock
// @xml: ellipse
#set text(font: "Times New Roman", size: 11pt)
#ellipse(width: 120pt, height: 56pt, fill: rgb("#EDE9FE"), stroke: 1pt + rgb("#6D28D9"))[Ellipse]
