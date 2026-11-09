// Test 04_cetz/01_lines_arrows_polylines.typ
// Covers: CeTZ lines, dashed/dotted strokes, arrow marks, open polylines,
// closed polygons, and positioned text/math labels.

#import "@preview/cetz:0.5.3": canvas, draw

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

= 1. CeTZ Lines, Stroke Styles, and Arrowheads

The following CeTZ canvas tests horizontal, vertical, and diagonal line segments with solid, dashed, and dotted strokes, arrowhead markers, and coordinate labels converted to native DrawingML `<wpg:wgp>` vector groups.

#canvas(length: 1cm, {
  import draw: *
  // Coordinate axes with arrowheads
  line((0, 0), (8.5, 0), mark: (end: ">"), stroke: 1.2pt + rgb("#2c3e50"))
  line((0, 0), (0, 4.2), mark: (end: ">"), stroke: 1.2pt + rgb("#2c3e50"))
  content((8.8, 0), [$x$])
  content((0, 4.5), [$y$])

  // Solid, dashed, and dotted lines
  line((0.8, 3.4), (4.2, 3.4), stroke: 1.5pt + rgb("#1e88e5"))
  content((5.8, 3.4), [Solid Stroke (1.5pt)])

  line((0.8, 2.4), (4.2, 2.4), stroke: (paint: rgb("#d32f2f"), thickness: 1.5pt, dash: "dashed"))
  content((5.8, 2.4), [Dashed Stroke (1.5pt)])

  line((0.8, 1.4), (4.2, 1.4), stroke: (paint: rgb("#388e3c"), thickness: 1.5pt, dash: "dotted"))
  content((5.8, 1.4), [Dotted Stroke (1.5pt)])

  // Multi-segment polyline and closed triangle
  line((0.8, 0.4), (1.8, 0.9), (2.8, 0.3), (3.8, 0.9), stroke: 1.3pt + rgb("#8e24aa"))
  line((6.2, 0.4), (7.8, 0.4), (7.0, 1.6), close: true, fill: rgb("#f3e5f5"), stroke: 1.3pt + rgb("#6a1b9a"))
})

= 2. Block Diagram Flow with Bidirectional Connectors

#canvas(length: 1cm, {
  import draw: *
  rect((0, 0.5), (2.8, 2.1), fill: rgb("#e3f2fd"), stroke: 1.4pt + rgb("#1565c0"), radius: 0.15)
  content((1.4, 1.3), [*Source* $S_0$])

  line((2.8, 1.3), (4.4, 1.3), mark: (end: ">"), stroke: 1.4pt + rgb("#37474f"))
  content((3.6, 1.7), [$f(x)$])

  rect((4.4, 0.5), (7.2, 2.1), fill: rgb("#e8f5e9"), stroke: 1.4pt + rgb("#2e7d32"), radius: 0.15)
  content((5.8, 1.3), [*Filter* $H(z)$])

  line((7.2, 1.3), (8.8, 1.3), mark: (end: ">"), stroke: 1.4pt + rgb("#37474f"))
  content((8.0, 1.7), [$y[n]$])

  rect((8.8, 0.5), (11.6, 2.1), fill: rgb("#fff8e1"), stroke: 1.4pt + rgb("#f57f17"), radius: 0.15)
  content((10.2, 1.3), [*Sink* $Y_k$])
})
