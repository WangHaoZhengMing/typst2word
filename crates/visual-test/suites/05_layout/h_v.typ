// @func: h
// @docs: https://typst.app/docs/reference/layout/h/

// === @case inline_hspace ===
// @title: Explicit Horizontal Space h(36pt)
// @param: amount: 36pt
// @var: h(36pt)
// @size: 360x100
// @ir: Paragraph
#set text(font: "Times New Roman", size: 12pt)
Left#h(36pt)Right

// === @case layout_vspace_24pt ===
// @title: Explicit Vertical Space v(24pt)
// @param: v(24pt)
// @var: v(24pt)
// @size: 300x130
// @ir: Paragraph
#set text(font: "Times New Roman", size: 12pt)
Top paragraph.
#v(24pt)
Bottom paragraph.
