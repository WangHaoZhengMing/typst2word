// @func: box
// @docs: https://typst.app/docs/reference/layout/box/

// === @case inline_box_pill ===
// @title: Inline Shaded Pill Box
// @param: fill + inset
// @var: box(fill, inset)
// @size: 360x100
// @ir: Paragraph
// @xml: E0F2FE
#set text(font: "Times New Roman", size: 12pt)
Status #box(fill: rgb("#E0F2FE"), inset: (x: 4pt, y: 1pt))[ACTIVE] verified.

// === @case inline_box_underline_blank ===
// @title: Inline Fixed-Width Bottom-Border Blank
// @param: width + stroke(bottom)
// @var: box(width: 96pt, stroke: (bottom: 0.6pt))
// @size: 360x100
// @ir: Paragraph
#set text(font: "Songti SC", size: 11pt, lang: "zh")
姓名：#box(width: 96pt, inset: (bottom: 2pt), stroke: (bottom: 0.6pt + black))[#align(center)[张三]]

// === @case shape_inline_circle ===
// @title: Inline Shape Badge (#box(circle))
// @param: baseline + shape
// @var: inline ShapeInline
// @size: 280x100
// @ir: Paragraph
#set text(font: "Times New Roman", size: 11pt)
Step #box(baseline: 30%, circle(radius: 8pt, fill: rgb("#DBEAFE"), stroke: 0.8pt + rgb("#1D4ED8"))[1]) begins.
