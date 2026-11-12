// @func: math.attach
// @docs: https://typst.app/docs/reference/math/attach/

// === @case math_scripts ===
// @title: Subscripts, Superscripts & Tensor Indices
// @param: sub + sup
// @var: sSub + sSup + sSubSup
// @size: 340x110
// @ir: EquationBlock
// @xml: <m:sSubSup>
// @tol: 13.0, 4.5, 25.0, 12.0
#set text(font: "Times New Roman", size: 12pt)
$ x_i^(2 k) + R_(mu nu)^lambda $

// === @case math_sum_prod ===
// @title: Summation with Upper & Lower Limits
// @param: limits
// @var: sum_(k=1)^n
// @size: 340x130
// @ir: EquationBlock
// @xml: <m:nary>
// @tol: 13.0, 4.5, 25.0, 12.0
#set text(font: "Times New Roman", size: 12pt)
$ sum_(k=1)^n k^2 = (n(n+1)(2n+1)) / 6 $
