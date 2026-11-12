// @func: rotate
// @docs: https://typst.app/docs/reference/layout/rotate/

// === @case shape_rotate ===
// @title: Rotated Shape (12deg)
// @param: angle: 12deg
// @var: rotate(12deg, rect(...))
// @size: 240x130
// @ir: ShapeBlock
// @xml: rot=
#set text(font: "Times New Roman", size: 11pt)
#pad(left: 16pt, top: 14pt)[#rotate(12deg, rect(width: 110pt, height: 40pt, fill: rgb("#E0E7FF"), stroke: 1pt + rgb("#4338CA"))[Rotated])]

// === @case shape_scale ===
// @title: Scaled Shape (x: 125%, y: 90%)
// @param: x + y
// @var: scale(x: 125%, y: 90%, rect(...))
// @size: 240x120
// @ir: ShapeBlock
#set text(font: "Times New Roman", size: 11pt)
#scale(x: 125%, y: 90%, origin: top + left, rect(width: 100pt, height: 40pt, fill: rgb("#FEE2E2"), stroke: 1pt + rgb("#B91C1C"))[Scaled])

// === @case shape_move ===
// @title: Translated Shape move(dx: 30pt, dy: 8pt)
// @param: dx + dy
// @var: move(dx: 30pt, dy: 8pt, rect(...))
// @size: 240x120
#set text(font: "Times New Roman", size: 11pt)
#move(dx: 30pt, dy: 8pt, rect(width: 100pt, height: 36pt, fill: rgb("#F1F5F9"), stroke: 1pt + black)[Moved])
