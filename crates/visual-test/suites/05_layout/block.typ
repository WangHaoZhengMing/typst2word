// @func: block
// @docs: https://typst.app/docs/reference/layout/block/

// === @case block_callout_left_bar ===
// @title: Styled Block: Left Accent Callout
// @param: fill + stroke(left)
// @var: block(fill, stroke: (left: 3pt))
// @size: 340x110
// @xml: 2563EB
#set text(font: "Times New Roman", size: 11pt)
#block(width: 100%, fill: rgb("#F8FAFC"), stroke: (left: 3pt + rgb("#2563EB")), inset: 8pt)[Callout container with left accent border only.]

// === @case block_shaded_card ===
// @title: Styled Block: Full-Width Bordered Card
// @param: width + fill + stroke + inset
// @var: block(width: 100%, fill, stroke, inset)
// @size: 340x110
// @xml: F1F5F9
#set text(font: "Times New Roman", size: 11pt)
#block(width: 100%, fill: rgb("#F1F5F9"), stroke: 0.8pt + rgb("#64748B"), inset: 8pt)[Full-width shaded container card.]
