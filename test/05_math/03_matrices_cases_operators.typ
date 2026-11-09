// Test 05_math/03_matrices_cases_operators.typ
// Covers: Matrices (`mat`), determinants (`det`), vectors (`vec`),
// piecewise `cases`, and n-ary big operators (`sum`, `prod`, `integral`, `union`, `sect`).

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

= 1. Matrices, Determinants, and Column Vectors (`<m:m>`)

Matrices with parentheses, square brackets, and vertical determinant bars translate to `<m:d>` wrapping `<m:m>`:

$ A = mat(
  a_11, a_12, a_13;
  a_21, a_22, a_23;
  a_31, a_32, a_33
), quad det(A - lambda I) = mat(delim: "|",
  a_11 - lambda, a_12, a_13;
  a_21, a_22 - lambda, a_23;
  a_31, a_32, a_33 - lambda
) = 0 $

$ vec(y_1, y_2, y_3) = mat(delim: "[",
  cos theta, -sin theta, 0;
  sin theta, cos theta, 0;
  0, 0, 1
) vec(x_1, x_2, x_3) $

= 2. Piecewise Functions (`cases`)

Piecewise function definitions use left curly brace delimiters (`<m:begChr m:val="{"/>`, `<m:endChr m:val=""/>`) around an equation array:

$ f(x) = cases(
  frac(sin x, x) &"if" x != 0,
  1 &"if" x = 0
), quad abs(x) = cases(
  x &"for" x >= 0,
  -x &"for" x < 0
) $

= 3. N-ary Summation, Product, Integral, and Set Operators (`<m:nary>`)

$ sum_(n=1)^infinity frac(1, n^s) = product_(p "prime") frac(1, 1 - p^(-s)), quad integral.double_D (frac(partial Q, partial x) - frac(partial P, partial y)) dif x dif y = integral.cont_(partial D) (P dif x + Q dif y) $

$ union.big_(i=1)^n A_i = inter.big_(j=1)^m B_j, quad lim_(n -> infinity) (1 + frac(z, n))^n = sum_(k=0)^infinity frac(z^k, k!) $
