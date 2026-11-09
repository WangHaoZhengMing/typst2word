// Test 04_cetz/03_bezier_and_grids.typ
// Covers: CeTZ coordinate grids, cubic/quadratic Bezier curves,
// and coordinate transformations (translate, rotate, scale).

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

= 1. CeTZ Background Grid and Cubic Bezier Splines

Cubic Bezier segments (`<a:cubicBezTo>` in DrawingML) and Cartesian reference grids test precision path quantization in EMUs ($1"pt" = 12700"EMU"$).

#canvas(length: 1cm, {
  import draw: *
  grid((0, 0), (10, 4), step: 1, stroke: 0.4pt + luma(200))
  line((0, 0), (10.3, 0), mark: (end: ">"), stroke: 1.1pt + rgb("#263238"))
  line((0, 0), (0, 4.3), mark: (end: ">"), stroke: 1.1pt + rgb("#263238"))

  // Quadratic & Cubic Bezier curves
  bezier((0.5, 0.6), (4.5, 3.4), (2.0, 3.8), stroke: 1.6pt + rgb("#d81b60"))
  bezier((4.5, 3.4), (9.5, 0.8), (6.5, 0.1), (8.0, 3.9), stroke: 1.6pt + rgb("#1e88e5"))

  // Control point markers
  circle((0.5, 0.6), radius: 0.09, fill: rgb("#d81b60"), stroke: none)
  circle((4.5, 3.4), radius: 0.09, fill: rgb("#263238"), stroke: none)
  circle((9.5, 0.8), radius: 0.09, fill: rgb("#1e88e5"), stroke: none)

  content((2.4, 3.5), [$B_2(t)$])
  content((7.2, 2.8), [$B_3(t)$])
})

= 2. Coordinate Transformations (Translate and Rotate)

#canvas(length: 1cm, {
  import draw: *
  grid((0, 0), (10, 3), step: 1, stroke: 0.4pt + luma(210))

  group({
    translate((2.0, 1.5))
    rect((-1.0, -0.6), (1.0, 0.6), fill: rgb("#e0f7fa"), stroke: 1.3pt + rgb("#00838f"))
    content((0, 0), [$0 degree$])
  })

  group({
    translate((5.5, 1.5))
    rotate(25deg)
    rect((-1.0, -0.6), (1.0, 0.6), fill: rgb("#fff3e0"), stroke: 1.3pt + rgb("#ef6c00"))
    content((0, 0), [$25 degree$])
  })

  group({
    translate((8.5, 1.5))
    rotate(-20deg)
    rect((-1.0, -0.6), (1.0, 0.6), fill: rgb("#f3e5f5"), stroke: 1.3pt + rgb("#7b1fa2"))
    content((0, 0), [$-20 degree$])
  })
})
