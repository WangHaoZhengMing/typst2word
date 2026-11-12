// @func: pad
// @docs: https://typst.app/docs/reference/layout/pad/

// === @case par_pad_left_right ===
// @title: Horizontal Padding: left 24pt, right 24pt
// @param: left + right
// @var: pad(left: 24pt, right: 24pt)
// @size: 320x150
// @ir: Paragraph
// @xml: w:left=
#set text(font: "Times New Roman", size: 12pt, hyphenate: false)
#set par(justify: true)
#pad(left: 24pt, right: 24pt)[In olden times when wishing still helped one, there lived a king whose daughters were all beautiful, and the youngest was so beautiful that the sun itself wondered.]
