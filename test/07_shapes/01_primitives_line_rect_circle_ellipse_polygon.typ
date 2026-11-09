// Test 07_shapes/01_primitives_line_rect_circle_ellipse_polygon.typ
// Covers: Native Typst geometric primitives (`line`, `rect`, `square`, `circle`,
// `ellipse`, `polygon`, `curve`) converted to DrawingML `<wps:wsp>`.

#set page(
  paper: "a4",
  margin: (top: 2.54cm, bottom: 2.54cm, left: 3.18cm, right: 3.18cm),
)
#set text(font: "Times New Roman", size: 11pt, lang: "en", hyphenate: false)
#set par(
  justify: true,
  linebreaks: "simple",
  justification-limits: (spacing: (min: 100%, max: 500%)),
  leading: 0.65em,
  spacing: 1.2em,
)

= 1. Horizontal Rule (`#line`) and Rectangular Shapes (`#rect`, `#square`)

#line(length: 100%, stroke: 1.2pt + rgb("#1565c0"))

#rect(
  width: 260pt,
  height: 42pt,
  fill: rgb("#e3f2fd"),
  stroke: 1.4pt + rgb("#1e88e5"),
  radius: 6pt,
  inset: 8pt,
)[
  *Rounded Rectangle (`#rect`)* with `radius: 6pt` and inline math $E = m c^2$.
]

#square(
  size: 56pt,
  fill: rgb("#fff8e1"),
  stroke: 1.4pt + rgb("#f57f17"),
  inset: 6pt,
)[
  *Square* \ `56pt`
]

= 2. Circle (`#circle`) and Ellipse (`#ellipse`)

#circle(
  radius: 28pt,
  fill: rgb("#e8f5e9"),
  stroke: 1.5pt + rgb("#43a047"),
)[
  #align(center + horizon)[*Circle*]
]

#ellipse(
  width: 140pt,
  height: 46pt,
  fill: rgb("#fce4ec"),
  stroke: 1.4pt + rgb("#d81b60"),
)[
  #align(center + horizon)[*Ellipse (`140pt x 46pt`)*]
]

= 3. Polygon (`#polygon`) and Bezier Curve (`#curve`)

#polygon(
  fill: rgb("#ede7f6"),
  stroke: 1.5pt + rgb("#5e35b1"),
  (0pt, 42pt),
  (45pt, 0pt),
  (90pt, 42pt),
)

#curve(
  fill: rgb("#e0f7fa"),
  stroke: 1.5pt + rgb("#00838f"),
  curve.move((0pt, 22pt)),
  curve.line((55pt, 0pt)),
  curve.cubic((95pt, 10pt), (95pt, 34pt), (55pt, 44pt)),
  curve.close(),
)
