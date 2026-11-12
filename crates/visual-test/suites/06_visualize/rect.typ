// @func: rect
// @docs: https://typst.app/docs/reference/visualize/rect/

// === @case shape_rect_basic ===
// @title: Rectangle with Fill & Stroke
// @param: width + height + fill + stroke
// @var: rect(width: 140pt, height: 54pt)
// @size: 240x110
// @ir: ShapeBlock
// @xml: <wps:wsp | DBEAFE
#set text(font: "Times New Roman", size: 11pt)
#rect(width: 140pt, height: 54pt, fill: rgb("#DBEAFE"), stroke: 1pt + rgb("#1D4ED8"))[Rect Body]

// === @case shape_rect_rounded ===
// @title: Rounded Rectangle (radius: 8pt)
// @param: radius: 8pt
// @var: rect(radius: 8pt)
// @size: 240x110
// @ir: ShapeBlock
// @xml: roundRect
#set text(font: "Times New Roman", size: 11pt)
#rect(width: 140pt, height: 54pt, radius: 8pt, fill: rgb("#FEF3C7"), stroke: 1pt + rgb("#D97706"))[Rounded]
