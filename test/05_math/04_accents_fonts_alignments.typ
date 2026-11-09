// Test 05_math/04_accents_fonts_alignments.typ
// Covers: Math accents (`hat`, `tilde`, `bar`, `vec`, `dot`, `ddot`),
// overbrace/underbrace (`<m:groupChr>`), cancel (`<m:borderBox>`),
// math alphabets (`bold`, `bb`, `cal`, `frak`), and multi-line aligned equations.

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

= 1. Diacritical Math Accents (`<m:acc>` and `<m:bar>`)

Math accents above variables and multi-character expressions:

$ hat(theta) + tilde(omega) + bar(z) + arrow(v) + dot(q) + dot.double(q) = overline(z_1 + z_2) + underline(w_1 - w_2) $

= 2. Horizontal Braces (`overbrace` / `underbrace`) and Cancellation

Horizontal grouping braces (`<m:groupChr>`) and strikethrough cancellation (`<m:borderBox>`):

$ overbrace(a_1 + a_2 + dots.c + a_k, k "terms") + underbrace(b_1 + b_2 + dots.c + b_m, m "terms") = S_(k+m) + cancel(epsilon_0) $

= 3. Mathematical Alphabets (`bb`, `cal`, `bold`, `mono`)

Blackboard bold (`bb`), calligraphic (`cal`), and bold upright symbols map to Unicode Mathematical Alphanumeric Symbols in Cambria Math:

$ bb(R)^n times bb(C)^m -> cal(H), quad bb(E)[X] = integral_Omega X dif bb(P), quad bold(F) = m bold(a) = nabla times bold(A) $

= 4. Multi-Line Aligned Equations (`<m:eqArr>`)

Multi-line derivations aligned at `&` relation points:

$ (a + b)^3 &= (a + b) (a^2 + 2 a b + b^2) \
  &= a^3 + 3 a^2 b + 3 a b^2 + b^3 $
