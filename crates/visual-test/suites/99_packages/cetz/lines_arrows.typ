// @func: cetz
// @docs: https://cetz-package.github.io/docs/

// === @case cetz_line_basic ===
// @title: CeTZ: Diagonal Line
// @param: draw.line
// @var: cetz draw.line
// @size: 240x130
// @ir: CetzCanvas
// @xml: <wpg:wgp>
#import "@preview/cetz:0.5.3": canvas, draw
#canvas(length: 1cm, {
  import draw: *
  line((0, 0), (4.2, 1.8), stroke: 1.5pt + rgb("#1D4ED8"))
})

// === @case cetz_arrow_end ===
// @title: CeTZ: Directed Arrow (mark end: ">")
// @param: line.mark(end)
// @var: cetz line(mark: (end: ">"))
// @size: 240x110
// @ir: CetzCanvas
// @xml: <wpg:wgp>
#import "@preview/cetz:0.5.3": canvas, draw
#canvas(length: 1cm, {
  import draw: *
  line((0, 0), (4.5, 0), mark: (end: ">"), stroke: 1.4pt + rgb("#0F172A"))
})

// === @case cetz_arrow_both ===
// @title: CeTZ: Bidirectional Arrow
// @param: line.mark(start, end)
// @var: cetz line(mark: (start: ">", end: ">"))
// @size: 240x110
// @ir: CetzCanvas
// @xml: <wpg:wgp>
#import "@preview/cetz:0.5.3": canvas, draw
#canvas(length: 1cm, {
  import draw: *
  line((0, 0), (4.5, 0), mark: (start: ">", end: ">"), stroke: 1.4pt + rgb("#B91C1C"))
})
