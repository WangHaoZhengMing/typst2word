// @func: align
// @docs: https://typst.app/docs/reference/layout/align/

// === @case par_align_left ===
// @title: Alignment: Left
// @param: left
// @var: align(left)
// @size: 300x100
// @ir: Paragraph
#set text(font: "Times New Roman", size: 12pt)
#align(left)[Left aligned line]

// === @case par_align_center ===
// @title: Alignment: Center
// @param: center
// @var: align(center)
// @size: 300x100
// @ir: Paragraph
// @xml: w:val="center"
#set text(font: "Times New Roman", size: 12pt)
#align(center)[Centered aligned line]

// === @case par_align_right ===
// @title: Alignment: Right
// @param: right
// @var: align(right)
// @size: 300x100
// @ir: Paragraph
// @xml: w:val="right"
#set text(font: "Times New Roman", size: 12pt)
#align(right)[Right aligned line]
