// @func: cetz
// @docs: https://cetz-package.github.io/docs/

// === @case cetz_grid ===
// @title: CeTZ: Cartesian Grid
// @param: draw.grid
// @var: cetz draw.grid
// @size: 240x140
// @ir: CetzCanvas
// @xml: <wpg:wgp>
#import "@preview/cetz:0.5.3": canvas, draw
#canvas(length: 1cm, {
  import draw: *
  grid((0, 0), (4, 2), step: 1, stroke: 0.7pt + rgb("#64748B"))
})

// === @case cetz_group_transform ===
// @title: CeTZ: Group Translate + Rotate
// @param: draw.group + rotate
// @var: cetz group(translate + rotate)
// @size: 240x150
// @ir: CetzCanvas
// @xml: <wpg:wgp>
#import "@preview/cetz:0.5.3": canvas, draw
#canvas(length: 1cm, {
  import draw: *
  group({
    translate((2.2, 1.3))
    rotate(20deg)
    rect((-1.3, -0.6), (1.3, 0.6), fill: rgb("#FFE4E6"), stroke: 1.2pt + rgb("#E11D48"))
  })
})
