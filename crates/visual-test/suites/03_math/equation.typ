// @func: math.equation
// @docs: https://typst.app/docs/reference/math/equation/

// === @case math_inline_basic ===
// @title: Inline Equation ($E = m c^2$)
// @param: block: false
// @var: inline $...$
// @size: 340x100
// @ir: Paragraph
// @xml: <m:oMath>
#set text(font: "Times New Roman", size: 12pt)
Mass-energy $E = m c^2$ relation.

// === @case math_display_basic ===
// @title: Display Equation ($ a^2 + b^2 = c^2 $)
// @param: block: true
// @var: display $ ... $
// @size: 340x110
// @ir: EquationBlock
// @xml: <m:oMath
// @tol: 13.0, 4.5, 25.0, 12.0
#set text(font: "Times New Roman", size: 12pt)
$ a^2 + b^2 = c^2 $

// === @case math_numbered ===
// @title: Numbered Display Equation (1)
// @param: numbering
// @var: math.equation(numbering: "(1)")
// @size: 340x110
// @ir: EquationBlock
// @xml: (1)
#set text(font: "Times New Roman", size: 12pt)
#set math.equation(numbering: "(1)")
$ E = h nu $
