// @func: pagebreak
// @docs: https://typst.app/docs/reference/layout/pagebreak/

// === @case pagebreak_explicit ===
// @title: Explicit Page Break (#pagebreak)
// @param: weak: false
// @var: pagebreak()
// @size: 280x110
// @ir: Paragraph, PageBreak
#set text(font: "Times New Roman", size: 12pt)
First page line.
#pagebreak()
Second page line.
