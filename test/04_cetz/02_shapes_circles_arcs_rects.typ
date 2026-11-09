// Test 04_cetz/02_shapes_circles_arcs_rects.typ
// Covers: CeTZ rectangles, rounded rectangles, circles, ellipses, circular arcs,
// and concentric geometric compositions.

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

= 1. CeTZ Circles, Ellipses, and Concentric Rings

This test verifies that CeTZ `circle`, `arc`, and `rect` primitives preserve exact aspect ratios, stroke widths, fill colors, and center coordinates when converted to DrawingML custom geometry `<a:custGeom>` paths inside `<wpg:wgp>`.

#canvas(length: 1cm, {
  import draw: *
  // Concentric circles
  circle((2.0, 2.0), radius: 1.6, fill: rgb("#e3f2fd"), stroke: 1.5pt + rgb("#1565c0"))
  circle((2.0, 2.0), radius: 1.0, fill: rgb("#bbdefb"), stroke: 1.2pt + rgb("#1976d2"))
  circle((2.0, 2.0), radius: 0.4, fill: rgb("#90caf9"), stroke: 1.0pt + rgb("#0d47a1"))
  content((2.0, 2.0), [$O$])

  // Ellipse with horizontal and vertical semi-axes
  circle((6.5, 2.0), radius: (1.8, 1.1), fill: rgb("#fce4ec"), stroke: 1.5pt + rgb("#c2185b"))
  line((4.7, 2.0), (8.3, 2.0), stroke: (paint: rgb("#880e4f"), thickness: 0.8pt, dash: "dashed"))
  line((6.5, 0.9), (6.5, 3.1), stroke: (paint: rgb("#880e4f"), thickness: 0.8pt, dash: "dashed"))
  content((6.5, 2.35), [$a = 1.8, b = 1.1$])

  // Circular arcs
  arc((9.8, 1.0), start: 0deg, stop: 180deg, radius: 1.3, stroke: 1.6pt + rgb("#00796b"), fill: rgb("#e0f2f1"))
  content((11.1, 1.5), [Arc $180 degree$])
})

= 2. Nested Rectangles and Geometric Overlays

#canvas(length: 1cm, {
  import draw: *
  rect((0.2, 0.2), (5.2, 3.2), fill: rgb("#f5f5f5"), stroke: 1.2pt + rgb("#616161"))
  rect((0.7, 0.6), (2.7, 2.8), fill: rgb("#fff9c4"), stroke: 1.4pt + rgb("#fbc02d"), radius: 0.2)
  rect((3.0, 0.6), (4.8, 2.8), fill: rgb("#e8eaf6"), stroke: 1.4pt + rgb("#3949ab"), radius: 0.2)
  content((1.7, 1.7), [Module A])
  content((3.9, 1.7), [Module B])

  rect((6.0, 0.4), (11.2, 3.0), fill: rgb("#efebe9"), stroke: 1.4pt + rgb("#5d4037"), radius: 0.25)
  circle((7.3, 1.7), radius: 0.8, fill: rgb("#d7ccc8"), stroke: 1.2pt + rgb("#4e342e"))
  content((7.3, 1.7), [$q_0$])
  line((8.1, 1.7), (9.2, 1.7), mark: (end: ">"), stroke: 1.3pt + rgb("#3e2723"))
  circle((10.0, 1.7), radius: 0.8, fill: rgb("#bcaaa4"), stroke: 1.2pt + rgb("#4e342e"))
  content((10.0, 1.7), [$q_1$])
})
