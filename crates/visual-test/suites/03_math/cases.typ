// @func: math.cases
// @docs: https://typst.app/docs/reference/math/cases/

// === @case math_cases_binom ===
// @title: Binomial & Piecewise Cases
// @param: cases
// @var: binom + cases
// @size: 340x140
// @ir: EquationBlock
// @xml: <m:d>
// @tol: 13.0, 4.5, 25.0, 12.0
#set text(font: "Times New Roman", size: 12pt)
$ f(x) = cases(x^2 &"if" x >= 0, -x &"if" x < 0) $
