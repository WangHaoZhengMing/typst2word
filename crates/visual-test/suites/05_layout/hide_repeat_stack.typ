// @func: stack
// @docs: https://typst.app/docs/reference/layout/stack/

// === @case layout_hide_span ===
// @title: Hidden Content Placeholder (#hide)
// @param: hide
// @var: hide[...]
// @size: 300x95
// @ir: Paragraph
#set text(font: "Times New Roman", size: 12pt)
Visible #hide[secret] tail.

// === @case layout_stack_horizontal ===
// @title: Horizontal Stack (dir: ltr, spacing: 12pt)
// @param: dir: ltr + spacing
// @var: stack(dir: ltr, spacing: 12pt)
// @size: 320x95
#set text(font: "Times New Roman", size: 12pt)
#stack(dir: ltr, spacing: 12pt, [Alpha], [Beta], [Gamma])

// === @case layout_stack_shapes_ltr ===
// @title: Horizontal Shape Stack (rect + ellipse + polygon)
// @param: dir: ltr + spacing: 14pt + shapes
// @var: stack(dir: ltr, rect, ellipse, polygon)
// @size: 420x100
// @ir: Table
// @xml: <wps:wsp>
#set text(font: ("PingFang SC", "Times New Roman"))
#stack(
  dir: ltr,
  spacing: 14pt,
  rect(width: 175pt, height: 38pt, fill: rgb("#E8F2FF"), stroke: 1.2pt + rgb("#0071E3"), radius: 6pt)[
    #align(center + horizon)[*圆角矩形* `<wps:wsp>` $E = h nu$]
  ],
  ellipse(width: 105pt, height: 38pt, fill: rgb("#EAFBF0"), stroke: 1.2pt + rgb("#28CD41"))[
    #align(center + horizon)[*原生椭圆*]
  ],
  polygon(
    fill: rgb("#FFF4E5"),
    stroke: 1.2pt + rgb("#FF9F0A"),
    (0pt, 36pt),
    (32pt, 2pt),
    (64pt, 36pt),
  ),
)
