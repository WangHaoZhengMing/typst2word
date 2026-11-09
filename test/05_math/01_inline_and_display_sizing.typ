// Test 05_math/01_inline_and_display_sizing.typ
// Covers: Inline math ($...$) vs display block math ($ ... $),
// inline fraction 1/0.73 size compensation, numbered equations, and baseline alignment.

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
#set math.equation(numbering: "(1)")

= 1. Inline Math Embedding and Baseline Stability

Inline mathematical expressions such as Euler's identity $e^(i pi) + 1 = 0$, the quadratic roots $x = (-b plus.minus sqrt(b^2 - 4 a c)) / (2 a)$, and inner product notation $chevron.l u, v chevron.r = sum_(k=1)^n u_k overline(v_k)$ are embedded directly within standard Times New Roman body paragraphs. They must align cleanly on the text baseline without disrupting horizontal word wrapping.

When an inline fraction like $frac(a + b, c + d)$ or $frac(partial f, partial x)$ appears inside a paragraph, Microsoft Word normally shrinks inline numerator and denominator script levels by $0.73 times$. Our OMML converter compensates for this shrinkage so that inline fractions retain clear legibility and consistent vertical height.

= 2. Numbered Display Equations (`<m:oMathPara>`)

Block-level display equations are centered on the page with right-aligned equation numbers:

$ integral_(-infinity)^(+infinity) e^(-alpha x^2) dif x = sqrt(frac(pi, alpha)), quad Re(alpha) > 0 $ <eq-gauss>

The Cauchy-Schwarz inequality in Hilbert space is expressed as:

$ abs((chevron.l f, g chevron.r))^2 <= integral_Omega abs(f(x))^2 dif mu(x) dot.op integral_Omega abs(g(x))^2 dif mu(x) $ <eq-cauchy>

And the Fourier transform pair relates time and frequency domains via:

$ hat(f)(xi) = integral_(-infinity)^(+infinity) f(t) e^(-2 pi i xi t) dif t, quad f(t) = integral_(-infinity)^(+infinity) hat(f)(xi) e^(2 pi i xi t) dif xi $ <eq-fourier>
