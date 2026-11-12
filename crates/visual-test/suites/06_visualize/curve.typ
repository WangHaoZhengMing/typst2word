// @func: curve
// @docs: https://typst.app/docs/reference/visualize/curve/

// === @case shape_curve_bezier ===
// @title: Curve Cubic Bezier Closed Shape
// @param: move + cubic + line + close
// @var: curve(move, cubic, line, close)
// @size: 220x120
// @ir: ShapeBlock
// @xml: <a:cubicBezTo>
#curve(
  fill: rgb("#CCFBF1"),
  stroke: 1.4pt + rgb("#0F766E"),
  curve.move((0pt, 44pt)),
  curve.cubic((36pt, 0pt), (84pt, 76pt), (120pt, 18pt)),
  curve.line((120pt, 62pt)),
  curve.line((0pt, 62pt)),
  curve.close(mode: "straight"),
)
