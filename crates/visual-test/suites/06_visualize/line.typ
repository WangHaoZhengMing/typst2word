// @func: line
// @docs: https://typst.app/docs/reference/visualize/line/

// === @case shape_hline_rule ===
// @title: Horizontal Rule (line length: 100%)
// @param: length: 100%
// @var: line(length: 100%)
// @size: 320x90
// @ir: HorizontalRule
#line(length: 100%, stroke: 1pt + black)

// === @case shape_line_diagonal ===
// @title: Diagonal Line Segment
// @param: start + end
// @var: line(start: (0pt, 0pt), end: (140pt, 50pt))
// @size: 240x110
// @ir: ShapeBlock
// @xml: <wps:wsp
#line(start: (0pt, 0pt), end: (140pt, 50pt), stroke: 1.5pt + rgb("#2563EB"))

// === @case shape_line_dashed ===
// @title: Dashed Line Stroke
// @param: stroke.dash
// @var: line(stroke: (dash: "dashed"))
// @size: 260x90
#line(length: 180pt, stroke: (paint: rgb("#DC2626"), thickness: 1.5pt, dash: "dashed"))
