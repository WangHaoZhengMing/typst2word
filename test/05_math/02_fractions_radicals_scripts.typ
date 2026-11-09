// Test 05_math/02_fractions_radicals_scripts.typ
// Covers: Nested fractions, continued fractions, square/nth roots,
// simultaneous subscripts/superscripts, pre-scripts, and binomial coefficients.

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

= 1. Nested Fractions and Continued Fractions (`<m:f>`)

Complex rational expressions and continued fractions test nested `<m:f>` elements and delimiter auto-scaling (`<m:d>`):

$ c_0 + frac(1, c_1 + frac(1, c_2 + frac(1, c_3 + frac(1, c_4 + dots.c)))) = frac(P_n, Q_n) $

$ frac(frac(x^2 - y^2, x y), frac(x + y, x^2 y^2)) = x y (x - y), quad binom(n + k - 1, k) = frac((n + k - 1)!, k! (n - 1)!) $

= 2. Square Roots and Higher-Index Radicals (`<m:rad>`)

Square roots omit the degree element (`<m:degHide m:val="on"/>`), whereas $n$-th roots display the explicit radical index:

$ sqrt(1 + sqrt(1 + sqrt(1 + x))) + root(3, a^3 + b^3 + 3 a b (a + b)) - root(n, frac(x^n + y^n, 2)) $

= 3. Subscripts, Superscripts, and Tensor Indices (`<m:sSubSup>`)

Simultaneous sub- and superscripts on tensors and Christoffel symbols:

$ R^rho_(sigma mu nu) = partial_mu Gamma^rho_(nu sigma) - partial_nu Gamma^rho_(mu sigma) + Gamma^rho_(mu lambda) Gamma^lambda_(nu sigma) - Gamma^rho_(nu lambda) Gamma^lambda_(mu sigma) $

$ x_1^(2 k) + x_2^(2 k) + dots.c + x_n^(2 k) >= n (root(n, x_1 x_2 dots.c x_n))^(2 k) $
