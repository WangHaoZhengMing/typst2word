// @func: math.equation
// @docs: https://typst.app/docs/reference/math/

// === @case math_multiline_align ===
// @title: Multi-Line Aligned Display Equation
// @param: align point &
// @var: aligned &= \ &=
// @size: 340x140
// @ir: EquationBlock
// @xml: <m:eqArr>
// @tol: 13.0, 4.5, 25.0, 12.0
#set text(font: "Times New Roman", size: 12pt)
$ f(x) &= (x + 1)^2 \
  &= x^2 + 2x + 1 $
