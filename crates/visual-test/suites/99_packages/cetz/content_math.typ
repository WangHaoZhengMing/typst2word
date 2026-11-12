// @func: cetz
// @docs: https://cetz-package.github.io/docs/

// === @case cetz_content_text ===
// @title: CeTZ: Text Label Inside Node
// @param: draw.content(text)
// @var: cetz draw.content [Text]
// @size: 240x130
// @ir: CetzCanvas
// @xml: CeTZLabel_ | Node
#import "@preview/cetz:0.5.3": canvas, draw
#set text(font: "Times New Roman", size: 11pt)
#canvas(length: 1cm, {
  import draw: *
  rect((0, 0), (3.8, 1.5), fill: rgb("#F1F5F9"), stroke: 1pt + rgb("#334155"))
  content((1.9, 0.75), [Node A])
})

// === @case cetz_content_math ===
// @title: CeTZ: Math Formula Label Inside Node
// @param: draw.content(math)
// @var: cetz draw.content [$f(x)$]
// @size: 240x130
// @ir: CetzCanvas
// @xml: <m:oMath>
#import "@preview/cetz:0.5.3": canvas, draw
#set text(font: "Times New Roman", size: 11pt)
#canvas(length: 1cm, {
  import draw: *
  rect((0, 0), (3.8, 1.5), fill: rgb("#EEF2FF"), stroke: 1pt + rgb("#4338CA"))
  content((1.9, 0.75), [$y = x^2$])
})
