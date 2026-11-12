// @func: cetz
// @docs: https://cetz-package.github.io/docs/

// === @case cetz_rect_styled ===
// @title: CeTZ: Filled & Stroked Rectangle
// @param: draw.rect
// @var: cetz draw.rect
// @size: 240x130
// @ir: CetzCanvas
// @xml: DBEAFE
#import "@preview/cetz:0.5.3": canvas, draw
#canvas(length: 1cm, {
  import draw: *
  rect((0, 0), (4.0, 1.8), fill: rgb("#DBEAFE"), stroke: 1.3pt + rgb("#1D4ED8"))
})

// === @case cetz_circle ===
// @title: CeTZ: Circle Primitive
// @param: draw.circle
// @var: cetz draw.circle
// @size: 220x140
// @ir: CetzCanvas
// @xml: <wpg:wgp>
#import "@preview/cetz:0.5.3": canvas, draw
#canvas(length: 1cm, {
  import draw: *
  circle((1.4, 1.4), radius: 1.1, fill: rgb("#DCFCE7"), stroke: 1.3pt + rgb("#15803D"))
})

// === @case cetz_arc ===
// @title: CeTZ: Circular Arc (0deg..150deg)
// @param: draw.arc
// @var: cetz draw.arc
// @size: 220x130
// @ir: CetzCanvas
// @xml: <wpg:wgp>
#import "@preview/cetz:0.5.3": canvas, draw
#canvas(length: 1cm, {
  import draw: *
  arc((0, 0), start: 0deg, stop: 150deg, radius: 1.4, stroke: 1.5pt + rgb("#7E22CE"))
})

// === @case cetz_bezier ===
// @title: CeTZ: Cubic Bezier Spline
// @param: draw.bezier
// @var: cetz draw.bezier
// @size: 240x140
// @ir: CetzCanvas
// @xml: <a:cubicBezTo>
#import "@preview/cetz:0.5.3": canvas, draw
#canvas(length: 1cm, {
  import draw: *
  bezier((0, 0.8), (4.4, 0.8), (1.4, 2.4), (3.0, -0.8), stroke: 1.5pt + rgb("#0D9488"))
})

// === @case cetz_polygon_closed ===
// @title: CeTZ: Closed Filled Polygon
// @param: draw.line(close: true)
// @var: cetz line(..., close: true, fill)
// @size: 240x140
// @ir: CetzCanvas
// @xml: FEF3C7
#import "@preview/cetz:0.5.3": canvas, draw
#canvas(length: 1cm, {
  import draw: *
  line((0, 0), (2.0, 2.0), (4.0, 0), close: true, fill: rgb("#FEF3C7"), stroke: 1.3pt + rgb("#D97706"))
})
