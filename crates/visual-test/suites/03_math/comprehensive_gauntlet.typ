// @func: math.equation
// @docs: https://typst.app/docs/reference/math/

// === @case math_comprehensive_gauntlet ===
// @title: Comprehensive Formula Gauntlet (362 Equations: Simple -> Torture)
// @func: math.equation
// @param: comprehensive
// @var: 362 equations (atoms, shorthands, scripts, operators, matrices, Mozilla/TeXbook/OMML torture, surreal chaos)
// @size: 900x1080
// @ir: Paragraph, EquationBlock, Table
// @xml: <m:oMath> | <m:f> | <m:rad> | <m:nary> | <m:m> | <m:eqArr> | <m:acc> | <m:groupChr> | <m:borderBox> | <m:sPre>
// @tol: 20.0, 10.0, 40.0, 160.0
#set text(font: ("Times New Roman", "PingFang SC"), size: 9.5pt)
#set par(leading: 0.52em, spacing: 0.55em)

#let id(n) = text(font: "Menlo", size: 7pt, fill: rgb("#64748B"))[[#n]]
#let sec(title) = block(above: 0.65em, below: 0.3em, sticky: true)[
  #text(size: 10pt, weight: "bold", fill: rgb("#1E3A8A"))[#title]
]
#let cell(n, body) = [#id(n) #body]

#sec[§01 · Atomic Primitives, Numbers, Primes & All 38 Math Shorthands (001–045)]
#id("001") $0$ · #id("002") $1$ · #id("003") $-1$ · #id("004") $x$ · #id("005") $a + b$ · #id("006") $a - b$ · #id("007") $a * b$ · #id("008") $a / b$ · #id("009") $3.1415926$ · #id("010") $6.022 times 10^23$ · #id("011") $n!$ · #id("012") $(2 n)!!$ · #id("013") $0!$ · #id("014") $x'$ · #id("015") $x''$ · #id("016") $x'''$ · #id("017") $x''''$ · #id("018") $x'''''$ · #id("019") $a plus.minus b$ · #id("020") $a minus.plus b$ · #id("021") $a div b$ · #id("022") $a dot.op b$ · #id("023") $a circle.stroked.tiny b$ · #id("024") $a star.op b$ · #id("025") $a plus.o b$ · #id("026") $a minus.o b$ · #id("027") $a times.o b$ · #id("028") $a slash.o b$ · #id("029") $a dot.o b$ · #id("030") $a ast.op.o b$ · #id("031") $a plus.square b$ · #id("032") $a minus.square b$ · #id("033") $a times.square b$ · #id("034") $a dot.square b$ · #id("035") $x != y$ · #id("036") $x := y$ · #id("037") $x ::= y$ · #id("038") $x =: y$ · #id("039") $x <= y >= z$ · #id("040") $a << b <<< c >> d >>> e$ · #id("041") $A -> B --> C |-> D >-> E ->> F$ · #id("042") $A <- B <-- C <-< D <<- E <-> F <--> G$ · #id("043") $p ~> q ~~> r <~ s <~~ t$ · #id("044") $P => Q |=> R ==> S <== T <=> U <==> V$ · #id("045") $[| k |] + || v || + (1, 2, ..., n)$

#sec[§02 · Greek, Hebrew, Letter-Likes, Set Theory, Logic & Exotic Relations (046–090)]
#id("046") $alpha + beta + gamma + delta + epsilon + epsilon.alt$ · #id("047") $zeta + eta + theta + theta.alt + iota + kappa + kappa.alt$ · #id("048") $lambda + mu + nu + xi + omicron + pi + pi.alt$ · #id("049") $rho + rho.alt + sigma + sigma.alt + tau + upsilon$ · #id("050") $phi + phi.alt + chi + psi + omega + digamma$ · #id("051") $Alpha Beta Gamma Delta Epsilon Zeta Eta Theta Theta.alt$ · #id("052") $Iota Kappa Lambda Mu Nu Xi Omicron Pi Rho$ · #id("053") $Sigma Tau Upsilon Phi Chi Psi Omega Omega.inv Digamma$ · #id("054") $aleph_0 < beth_1 <= gimel_2 < daleth_3$ · #id("055") $ell + planck + pee(z) + Re(z) + Im(z) + angstrom$ · #id("056") $dotless.i + dotless.j + partial_x + nabla^2 + laplace u + oo$ · #id("057") $forall x in A, exists y in.not B : x in.rev C$ · #id("058") $exists.not z in emptyset$ · #id("059") $A subset B subset.eq C subset.neq D supset.eq E supset F$ · #id("060") $X subset.eq.sq Y supset.eq.sq Z$ · #id("061") $A union B inter C without D^complement = nothing$ · #id("062") $A union.plus B union.sq C inter.sq D$ · #id("063") $p and q or not r xor s$ · #id("064") $Gamma tack.r phi tack.rr psi tack.rr.not chi$ · #id("065") $A tack.l B, top != bot, therefore P because Q quad qed$ · #id("066") $a equiv b equiv.not c approx d approx.eq e tilde.equiv f$ · #id("067") $x prop y asymp z eq.def w eq.delta u eq.quest v$ · #id("068") $a prec b prec.eq c succ d succ.curly.eq e$ · #id("069") $p divides q, p divides.not r, L_1 parallel L_2 perp L_3, L_1 parallel.not L_4$ · #id("070") $angle A B C = 90 degree = angle.right, angle.spheric + angle.arc$ · #id("071") $G times.l H times.r K wreath S_n$ · #id("072") $R bowtie.stroked.big S bowtie.stroked.big.l T bowtie.stroked.big.r U$ · #id("073") $f : X arrow.r.hook Y arrow.r.twohead Z arrow.r.tail W$ · #id("074") $A arrow.l.hook B arrow.r.loop C arrow.l.r.wave D$ · #id("075") $x arrows.rl y arrows.lr z arrows.tb w arrows.rr u arrows.lll v$ · #id("076") $A + B harpoons.rtlb C + D harpoons.ltrb E$ · #id("077") $u harpoon.rt v harpoon.rb w harpoon.lt p harpoon.lb q$ · #id("078") $x arrow.tr y arrow.br z arrow.tl w arrow.bl v$ · #id("079") $P arrow.r.not Q arrow.r.double.not R arrow.l.r.double.not S$ · #id("080") $f multimap g, A plus.dot B minus.dot C$ · #id("081") $sha(E / QQ) + Sha$ · #id("082") $euro 10 + yen 5 + pound 3 + bitcoin 1 + \$ 99$ · #id("083") $suit.spade.stroked + suit.heart.stroked + suit.diamond.stroked + suit.club.stroked$ · #id("084") $flat + natural + sharp + star.stroked + diamond.stroked + lozenge.stroked$ · #id("085") $square.stroked P <=> lozenge.stroked Q$ · #id("086") $x eq.dots y eq.dots.down z eq.dots.up w$ · #id("087") $a lt.eq.slant b gt.eq.slant c lt.gt d$ · #id("088") $x tilde.op y tilde.eq z tilde.rev w tilde.triple u$ · #id("089") $A subset.double B supset.double C union.double D inter.double E$ · #id("090") $die.one + die.two + die.three + die.four + die.five + die.six = 21$

#sec[§03 · Subscripts, Superscripts, Pre-Scripts (attach), Tensors & Towers (091–130)]
#id("091") $x_1$ · #id("092") $x^2$ · #id("093") $x_1^2$ · #id("094") $x^2_1$ · #id("095") $x_(i, j)^(k, l)$ · #id("096") $a_n'$ · #id("097") $a'_n$ · #id("098") $y_3'''$ · #id("099") $f''''_0 (x)$ · #id("100") $(x'_i)^2$ · #id("101") $a^b'_c^d$ · #id("102") $x^2 y^2$ · #id("103") $x^(2 y)$ · #id("104") $x^(2^y)$ · #id("105") $y^(x^2)$ · #id("106") $2^(2^(2^x))$ · #id("107") $2^(2^(2^(2^n)))$ · #id("108") $a_(b_(c_(d_e)))$ · #id("109") $x_(i_1)^(j_1) y_(i_2)^(j_2)$ · #id("110") $e^(-x^2 / (2 sigma^2))$ · #id("111") $x^(y / b) a^(z^(c / d))$ · #id("112") $x^(9^2) 31415 + pi$ · #id("113") $A^top B^(-1) C^* D^dagger E^(upright("H"))$ · #id("114") $Gamma^lambda_(mu nu) = 1/2 g^(lambda sigma) (partial_mu g_(nu sigma) + partial_nu g_(mu sigma) - partial_sigma g_(mu nu))$ · #id("115") $R^rho_(sigma mu nu) = partial_mu Gamma^rho_(nu sigma) - partial_nu Gamma^rho_(mu sigma) + Gamma^rho_(mu lambda) Gamma^lambda_(nu sigma) - Gamma^rho_(nu lambda) Gamma^lambda_(mu sigma)$ · #id("116") $attach(F, bl: 2, br: 3) (a_1, a_2; b_1, b_2, b_3; z)$ · #id("117") $attach(F, bl: p, br: q) (alpha_1, ..., alpha_p; beta_1, ..., beta_q; x)$ · #id("118") $attach("U", tl: 235, bl: 92) + attach(n, tl: 1, bl: 0) -> attach("Ba", tl: 141, bl: 56) + attach("Kr", tl: 92, bl: 36) + 3 attach(n, tl: 1, bl: 0)$ · #id("119") $attach(C, tl: 14, bl: 6, tr: 4-, br: "aq")$ · #id("120") $attach(X, tl: alpha, bl: beta, tr: gamma, br: delta)$ · #id("121") $attach(Pi, t: alpha, b: beta, tl: 1, tr: 2+3, bl: 4+5, br: 6)$ · #id("122") $attach(Omega, tl: (i), bl: [j], tr: {k}, br: |l|)$ · #id("123") $T^(i_1 i_2)_(j_1 j_2) S_(k_1)^(l_1)$ · #id("124") $x_(n+1) = x_n - f(x_n) / f'(x_n)$ · #id("125") $delta^i_j epsilon_(i j k) = 0$ · #id("126") $Lambda^(k) (V^*) tilde.equiv (Lambda^k V)^*$ · #id("127") $W^(k, p)_0 (Omega)$ · #id("128") $C^oo_c (RR^n)$ · #id("129") $ell^p (NN), L^q (mu)$ · #id("130") $(a_n)_(n=1)^oo, {x_alpha}_(alpha in I)$

#sec[§04 · Math Alphabets, Styles, Sizes, Spacings, Colors, CJK & Show Rules (131–165)]
#id("131") $upright(A B C a b c 0 1 2)$ · #id("132") $italic(A B C a b c)$ · #id("133") $bold(A B C a b c alpha beta Gamma)$ · #id("134") $bold(upright(x y z A B C))$ · #id("135") $bold(italic(x y z A B C))$ · #id("136") $serif(R S T)$ · #id("137") $sans(A B C x y z 1 2 3)$ · #id("138") $bold(sans(T E N S O R))$ · #id("139") $cal(A B C D E F G H L M N P S T)$ · #id("140") $bold(cal(F G H L))$ · #id("141") $scr(A B C F H I L M R S)$ · #id("142") $frak(A B C g h m n p q s l S O S U)$ · #id("143") $bold(frak(g h s u))$ · #id("144") $mono(x + y == z_0)$ · #id("145") $bb(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z)$ · #id("146") $NN subset ZZ subset QQ subset RR subset CC subset HH subset OO$ · #id("147") $EE[X], PP(A), FF_(p^n), AA^n, GG_m$ · #id("148") $bb(1)_A (x)$ · #id("149") $a thin b med c thick d quad e wide f$ · #id("150") $display(sum_(k=1)^n 1/k)$ vs $inline(sum_(k=1)^n 1/k)$ vs $script(sum_(k=1)^n 1/k)$ vs $sscript(sum_(k=1)^n 1/k)$ · #id("151") $display(x^(2^k), cramped: #true)$ vs $display(x^(2^k), cramped: #false)$ · #id("152") $#text(fill: rgb("#DC2626"))[$alpha^2$] + #text(fill: rgb("#2563EB"))[$beta^2$] = #text(fill: rgb("#16A34A"))[$gamma^2$]$ · #id("153") $P("事件" A mid(|) "条件" B) = ("联合概率" P(A B)) / ("边缘概率" P(B))$ · #id("154") $"方差"(X) = EE[(X - "均值")^2]$ · #id("155") $#strong[F] = m #emph[a] + #raw("dx/dt") + #hide[hidden] z$ · #id("156") $#box(stroke: 0.4pt + rgb("#2563EB"), inset: 1.5pt)[$E = m c^2$]$ · #id("157") $class("relation", star.op) quad x class("relation", suit.heart.stroked) y class("binary", diamond.stroked) z$ · #id("158") #[
  #show "RRd": $bb(R)^d$
  #let Kxy = $e^(-||x - y||^2 / (2 t))$
  $integral_"RRd" Kxy dif y = (2 pi t)^(d/2)$
] · #id("159") $100% - 50 permille + 7 permyriad$ · #id("160") $upright("d") x and upright("d") y and upright("d") z$ · #id("161") $cal(O)(n log n) + Theta(n^2) + omega(1)$ · #id("162") $frak(R e)(s) > 1/2 => zeta(s) != 0$ · #id("163") $sans("SNR")_(upright("dB")) = 10 log_10 (P_"signal" / P_"noise")$ · #id("164") $mono("0xFF") + mono("0b1010") = 265$ · #id("165") $"若" x >= 0 "且" y != 0, "则" sqrt(x) / y in RR$

#sec[§05 · Fractions (Vertical / Skewed / Horizontal), Binomials & Radicals (166–205)]
#table(
  columns: (1fr, 1fr, 1fr, 1fr),
  stroke: 0.35pt + rgb("#CBD5E1"),
  inset: (x: 4pt, y: 3pt),
  align: horizon,
  cell("166", $ (x + y^2) / (k + 1) $),
  cell("167", $ ((a + b)) / ((c + d)) $),
  cell("168", $ frac(a + b, c + d, style: "skewed") $),
  cell("169", $ frac(a + b, c + d, style: "horizontal") $),

  cell("170", $ (x + y) / (x / k + 1) $),
  cell("171", $ (a / b - c / d) / (e / f + g / h) $),
  cell("172", $ 1 / (1 + 1 / (1 + 1 / (1 + x))) $),
  cell("173", $ a^(b / 2) + x^((n - 1) / (n + 1)) $),

  cell("174", $ binom(n, k) = n! / (k! (n - k)!) $),
  cell("175", $ binom(n, k / 2) + binom(2 n, n) $),
  cell("176", $ binom(n, k_1, k_2, k_3, ..., k_m) $),
  cell("177", $ binom(binom(n, k), binom(m, j)) $),

  cell("178", $ sqrt(x^2 + y^2 + z^2) $),
  cell("179", $ √2 + ∛3 + ∜5 $),
  cell("180", $ root(3, x^3 + y^3) $),
  cell("181", $ root(n, a_1 a_2 dots.c a_n) $),

  cell("182", $ root(n + 1, (1 + sqrt(5)) / 2) $),
  cell("183", $ root(k / m, x^(p / q) + 1) $),
  cell("184", $ sqrt(1 + sqrt(1 + sqrt(1 + x))) $),
  cell("185", $ (-b plus.minus sqrt(b^2 - 4 a c)) / (2 a) $),

  cell("186", $ sqrt(a / b) = sqrt(a) / sqrt(b) $),
  cell("187", $ 1 / sqrt(2 pi sigma^2) e^(-(x - mu)^2 / (2 sigma^2)) $),
  cell("188", $ root(3, -q / 2 + sqrt((q / 2)^2 + (p / 3)^3)) $),
  cell("189", $ frac(dif^n, dif x^n) [x^n ln x] $),

  cell("190", $ (partial^2 u) / (partial t^2) = c^2 (partial^2 u) / (partial x^2) $),
  cell("191", $ frac(frac(1, x) + frac(1, y), 1 - frac(1, x y)) $),
  cell("192", $ x / (y / (z / w)) != ((x / y) / z) / w $),
  cell("193", $ binom(-1/2, n) = ((-1)^n / 4^n) binom(2 n, n) $),

  cell("194", $ frac(sin(x / 2), frac(x, 2), style: "skewed") $),
  cell("195", $ frac(alpha^2 + beta^2, gamma_1 - gamma_2, style: "horizontal") $),
  cell("196", $ root(2 k + 1, -x^(2 k + 1)) = -x $),
  cell("197", $ sqrt((1 - cos theta) / (1 + cos theta)) $),

  cell("198", $ (p / 2) x^2 y^(p - 2) - 1 / (1 - x / (1 - x^2 / 2)) $),
  cell("199", $ a_0 + b_1 / (a_1 + b_2 / (a_2 + b_3 / a_3)) $),
  cell("200", $ root(sqrt(n), root(3, m^2 + 1)) $),
  cell("201", $ (sqrt(a + b) - sqrt(a - b)) / sqrt(a^2 - b^2) $),

  cell("202", $ display(1 / (1 + inline(1 / (1 + script(1 / 2))))) $),
  cell("203", $ binom(alpha + k - 1, k) z^k $),
  cell("204", $ frac(Delta y, Delta x) -> frac(dif y, dif x) $),
  cell("205", $ root(n, product_(i=1)^n x_i) >= n / (sum_(i=1)^n 1 / x_i) $),
)

#sec[§06 · Accents (All 19), Bars, All 8 Over/Under Group Braces & Cancel (206–241)]
#table(
  columns: (1fr, 1fr, 1fr),
  stroke: 0.35pt + rgb("#CBD5E1"),
  inset: (x: 4pt, y: 3pt),
  align: horizon,
  cell("206", $ grave(a) + acute(a) + hat(a) + tilde(a) + macron(a) $),
  cell("207", $ dash(a) + breve(a) + dot(a) + dot.double(a) + diaer(a) $),
  cell("208", $ dot.triple(x) + dot.quad(x) + circle(A) + acute.double(o) + caron(z) $),

  cell("209", $ arrow(v) + arrow.l(v) + arrow.l.r(A B) + harpoon(u) + harpoon.lt(w) $),
  cell("210", $ hat(i) + hat(j) != hat(i, dotless: #false) + hat(j, dotless: #false) $),
  cell("211", $ hat(A B C D, size: #140%) + tilde(x + y + z) + arrow(P Q R) $),

  cell("212", $ tilde(E)_p^2 + hat(psi)_n^((0)) + dot(q)_i^2 + dot.double(r)_(mu nu) $),
  cell("213", $ hat(tilde(macron(Phi))) + breve(caron(acute(omega))) + arrow(dot(x)) $),
  cell("214", $ overline(z_1 + z_2) = overline(z_1) + overline(z_2), quad underline(x_1 x_2) $),

  cell("215", $ overline(underline(A union B)) = overline(A) inter overline(B) $),
  cell("216", $ overbrace(a + b + c) + underbrace(d + e + f) $),
  cell("217", $ overbrace(x + dots.c + x, k "times") = k x $),

  cell("218", $ underbrace(1 + 2 + dots.c + n, n "terms") = (n(n + 1)) / 2 $),
  cell("219", $ overbracket(x_1 + x_2, "pair") + underbracket(y_1 + y_2 + y_3, "triple") $),
  cell("220", $ overparen(A B C, "arc") + underparen(D E F, "chord") $),

  cell("221", $ overshell(alpha + beta, "upper") + undershell(gamma + delta, "lower") $),
  cell("222", $ overbrace(a_1 + underbrace(a_2 + a_3, "inner") + a_4, "outer") $),
  cell("223", $ { overbrace(a \, ... \, a, k "a's"), underbrace(b \, ... \, b, ell "b's") } $),

  cell("224", $ (a dot b dot cancel(x)) / cancel(x) = a b $),
  cell("225", $ (a cancel((b + c), inverted: #true)) / cancel(b + c, inverted: #true) = a $),
  cell("226", $ cancel(Pi, cross: #true) + cancel(sum_(i=1)^n x_i, cross: #true) $),

  cell("227", $ (x^2 - 1) / (x - 1) = (cancel((x - 1)) (x + 1)) / cancel(x - 1) = x + 1 $),
  cell("228", $ cancel(alpha / beta, length: #130%) - cancel(sqrt(x + 1), inverted: #true) $),
  cell("229", $ hat(H) lr(| psi chevron.r) = E lr(| psi chevron.r), quad accent(f, ~) (xi) = cal(F){f}(xi) $),

  cell("230", $ accent(x, \u{0302}) + accent(w, "ˆ") + accent(y, ->) + accent(z, <->) $),
  cell("231", $ overline(lim)_(n -> oo) a_n >= underline(lim)_(n -> oo) a_n $),
  cell("232", $ underbracket(overbracket(M_(i j), "col " j), "row " i) $),

  cell("233", $ dot(x) = sigma (y - x), quad dot(y) = x (rho - z) - y $),
  cell("234", $ macron(X)_n = 1/n sum_(i=1)^n X_i, quad hat(sigma)^2 = 1/(n-1) sum_(i=1)^n (X_i - macron(X)_n)^2 $),
  cell("235", $ arrow(nabla) times arrow(E) = -(partial arrow(B)) / (partial t) $),

  cell("236", $ overparen(underparen(p + q, "sub"), "sup") $),
  cell("237", $ overshell(undershell(x_0 + x_1, "shell-b"), "shell-t") $),
  cell("238", $ cancel(a_1) + (cancel(a_2) - cancel(a_1)) + (a_3 - cancel(a_2)) = a_3 $),

  cell("239", $ tilde(nabla)_mu V^nu = partial_mu V^nu + tilde(Gamma)^nu_(mu lambda) V^lambda $),
  cell("240", $ hat(U)(t, t_0) = cal(T) exp(-i / planck integral_(t_0)^t hat(H)(t') dif t') $),
  cell("241", $ macron(underline(overline(hat(tilde(Z)))))_(alpha beta)^(gamma delta) $),
)

#sec[§07 · Delimiters (lr / mid / abs / norm / floor / ceil / round) & Operators (op / limits / stretch) (242–277)]
#table(
  columns: (1fr, 1fr, 1fr),
  stroke: 0.35pt + rgb("#CBD5E1"),
  inset: (x: 4pt, y: 3pt),
  align: horizon,
  cell("242", $ abs(-x / y) + norm(A x - b)_2^2 $),
  cell("243", $ floor(x / 2) + ceil(x / 2) + round(x / 2) $),
  cell("244", $ lr(\[ 0, 1 \)) union lr((1, +oo)) $),

  cell("245", $ lr(\] -oo, a \]) inter lr(\[ b, +oo \[) $),
  cell("246", $ { x in RR mid(|) sum_(i=1)^n w_i abs(f_i (x)) < 1 } $),
  cell("247", $ lr(chevron.l psi |) + lr(| phi chevron.r) + lr(chevron.l psi mid(|) hat(H) mid(|) phi chevron.r) $),

  cell("248", $ lr(⟦ 1, n ⟧) times lr(⟪ u / v, w / z ⟫) $),
  cell("249", $ lr(⟮ (a^2 + b^2) / (c^2 + d^2) ⟯) $),
  cell("250", $ lr((partial L) / (partial dot(q)_i) |)_(q = q_0) = 0 $),

  cell("251", $ lr([ F(x) ])_a^b = F(b) - F(a) $),
  cell("252", $ lr(( 1 + (x / n)^2 ), size: #150%) $),
  cell("253", $ lr(\{ (x, y) mid(|) x > 0 mid(‖) y < 0 \}) $),

  cell("254", $ sin^2 x + cos^2 x = 1, quad 1 + tan^2 x = sec^2 x $),
  cell("255", $ cot x + csc x, quad arcsin u + arccos u = pi / 2 $),
  cell("256", $ arctan x + sinh x + cosh x + tanh x + coth x $),

  cell("257", $ sech^2 x + tanh^2 x = 1, quad csch x, quad sinc(pi x) $),
  cell("258", $ ctg x + tg x + lg 100 + ln e + log_2 8 $),
  cell("259", $ deg p + dim V = dim ker T + dim im T $),

  cell("260", $ hom(A, B) + id_X + tr(A^top B) + det(lambda I - A) $),
  cell("261", $ gcd_(1 <= i <= k) (a_i) dot lcm_(1 <= i <= k) (b_i) $),
  cell("262", $ lim_(n -> oo) a_n, quad liminf_(n -> oo) x_n <= limsup_(n -> oo) x_n $),

  cell("263", $ sup_(x in K) f(x) >= max_(1 <= i <= n) x_i >= min_(1 <= i <= n) x_i >= inf_(x in K) f(x) $),
  cell("264", $ arg max_(theta in Theta) ell(theta) != Pr_(X ~ cal(D)) [X >= t] $),
  cell("265", $ a equiv b mod n, quad Dif f(x) [h] = f'(x) dif x $),

  cell("266", $ op("Res", limits: #true)_(z = z_0) f(z) = 1 / (2 pi i) integral.cont_gamma f(z) dif z $),
  cell("267", $ op("argmin", limits: #true)_(w in RR^d) { 1/(2n) norm(X w - y)_2^2 + lambda norm(w)_1 } $),
  cell("268", $ op("diag")(lambda_1, ..., lambda_n) + op("rank")(A) + op("sgn")(sigma) $),

  cell("269", $ limits(A)_1^2 != A_1^2, quad scripts(sum)_1^2 != sum_1^2 $),
  cell("270", $ limits(op("ess sup"))_(x in Omega) abs(f(x)) < oo $),
  cell("271", $ H stretch(=)^"def" U + p V $),

  cell("272", $ X stretch(->>, size: #160%)^pi_"quotient" X / ~ $),
  cell("273", $ A stretch(->, size: #200%)^(f compose g)_(epsilon -> 0) B $),
  cell("274", $ R stretch(harpoons.rtlb, size: #220%)^(k_1)_(k_(-1)) P $),

  cell("275", $ op("Cov")(X, Y) / sqrt(op("Var")(X) op("Var")(Y)) in [-1, 1] $),
  cell("276", $ op("erf")(z) = 2 / sqrt(pi) integral_0^z e^(-t^2) dif t $),
  cell("277", $ det_(1 <= i, j <= n) [a_(i j)] = sum_(sigma in S_n) op("sgn")(sigma) product_(i=1)^n a_(i, sigma(i)) $),
)

#sec[§08 · All 19+ Big N-ary Operators (Sums, Products, Integrals, Set/Logic/Tensor N-aries) (278–307)]
#table(
  columns: (1fr, 1fr, 1fr),
  stroke: 0.35pt + rgb("#CBD5E1"),
  inset: (x: 4pt, y: 3pt),
  align: horizon,
  cell("278", $ sum_(n=1)^oo 1 / n^2 = pi^2 / 6 $),
  cell("279", $ product_(p "prime") 1 / (1 - p^(-s)) = zeta(s) $),
  cell("280", $ product.co_(i in I) X_i tilde.equiv union.plus.big_(i in I) ({i} times X_i) $),

  cell("281", $ integral_0^oo (sin x) / x dif x = pi / 2 $),
  cell("282", $ limits(integral)_(-oo)^(+oo) e^(-x^2) dif x = sqrt(pi) $),
  cell("283", $ scripts(sum)_(k=0)^n binom(n, k) x^k = (1 + x)^n $),

  cell("284", $ integral.double_D ( (partial Q) / (partial x) - (partial P) / (partial y) ) dif x dif y $),
  cell("285", $ integral.triple_Omega (nabla dot arrow(F)) dif V = integral.surf_(partial Omega) arrow(F) dot dif arrow(S) $),
  cell("286", $ integral.cont_C arrow(F) dot dif arrow(r) = integral.surf_S (nabla times arrow(F)) dot dif arrow(S) $),

  cell("287", $ integral.vol_W phi(x) dif^4 x + integral.quad_(RR^4) psi(x) dif^4 x $),
  cell("288", $ union.big_(n=1)^oo inter.big_(k=n)^oo A_k = liminf_(n -> oo) A_n $),
  cell("289", $ inter.big_(n=1)^oo union.big_(k=n)^oo A_k = limsup_(n -> oo) A_n $),

  cell("290", $ or.big_(i=1)^n (p_i and not q_i) <=> not and.big_(i=1)^n (p_i -> q_i) $),
  cell("291", $ plus.o.big_(k=0)^n V_k, quad times.o.big_(i=1)^m H_i, quad dot.o.big_(alpha in Lambda) S_alpha $),
  cell("292", $ union.sq.big_(x in B) F_x, quad union.plus.big_(j=1)^k C_j $),

  cell("293", $ sum_(mat(delim: #none, 0 <= i <= m; 0 < j < n)) P(i, j) $),
  cell("294", $ sum_(i=1)^p sum_(j=1)^q sum_(k=1)^r a_(i j) b_(j k) c_(k i) $),
  cell("295", $ sum_(p "prime") f(p) = integral_(t > 1) f(t) dif pi(t) $),

  cell("296", $ sum x_i + product y_i + integral f(x) dif x $),
  cell("297", $ sum_(k >= 1) 1 / k^3 + product^(N) (1 + q^k) $),
  cell("298", $ integral_1^x (dif t) / t = ln x $),

  cell("299", $ integral_0^1 integral_0^(1-x) integral_0^(1-x-y) x y z dif z dif y dif x $),
  cell("300", $ limits(integral.cont)_(abs(z) = 1) z^n dif z = 2 pi i delta_(n, -1) $),
  cell("301", $ sum_(n = -oo)^(+oo) abs(hat(f)(n))^2 = integral_0^1 abs(f(t))^2 dif t $),

  cell("302", $ product_(k=1)^(n-1) sin((k pi) / n) = n / 2^(n-1) $),
  cell("303", $ union.big_(alpha) U_alpha supset K => exists alpha_1, ..., alpha_m $),
  cell("304", $ plus.o.big_(p + q = k) H^(p, q) (X; CC) = H^k (X; CC) $),

  cell("305", $ integral_(-pi)^pi cos(m x) cos(n x) dif x = pi delta_(m n) $),
  cell("306", $ sum_(k=0)^oo ((-1)^k (x / 2)^(2 k + nu)) / (k! Gamma(nu + k + 1)) = J_nu (x) $),
  cell("307", $ scripts(union.big)_(i=1)^n E_i != limits(union.big)_(i=1)^n E_i $),
)

#sec[§09 · Vectors, Matrices, Augmented / Jagged Matrices & Piecewise Cases (308–331)]
#table(
  columns: (1fr, 1fr, 1fr),
  stroke: 0.35pt + rgb("#CBD5E1"),
  inset: (x: 4pt, y: 3pt),
  align: horizon,
  cell("308", $ vec(x_1, x_2, x_3) dot vec(delim: "[", y_1, y_2, y_3) = sum_(i=1)^3 x_i y_i $),
  cell("309", $ vec(delim: "‖", u_1, u_2) + vec(delim: #none, a, b) $),
  cell("310", $ mat(1, 2, 3; 0, 4, 5; 0, 0, 6) $),

  cell("311", $ mat(delim: "[", cos theta, -sin theta; sin theta, cos theta) $),
  cell("312", $ mat(delim: "|", a_(1 1), a_(1 2); a_(2 1), a_(2 2)) = a_(1 1) a_(2 2) - a_(1 2) a_(2 1) $),
  cell("313", $ mat(delim: "‖", 1, -1; 2, 3) + mat(delim: "{", p, q; r, s) $),

  cell("314", $ mat(delim: #none, a, b, c; d, e, f) $),
  cell("315", $ mat(delim: #("[", ")"), 1, 2; 3, 4) $),
  cell("316", $ mat(1, 0, 2, -1; 0, 1, 3, 4; augment: #3) $),

  cell("317", $ mat(I_n, A; 0, I_m; augment: #(hline: 1, vline: 1)) $),
  cell("318", $ mat(1; 1, 1; 1, 2, 1; 1, 3, 3, 1) $),
  cell("319", $ mat(1, x_1, dots.h.c, x_1^(n-1); 1, x_2, dots.h.c, x_2^(n-1); dots.v, dots.v, dots.down, dots.v; 1, x_n, dots.h.c, x_n^(n-1)) $),

  cell("320", $ mat(lambda, 1, 0, dots.h.c, 0; 0, lambda, 1, dots.h.c, 0; dots.v, dots.v, dots.down, dots.down, dots.v; 0, 0, dots.h.c, lambda, 1; 0, 0, dots.h.c, 0, lambda) $),
  cell("321", $ mat(0, dots.h.c, 0, a_1; dots.v, dots.up, a_2, 0; 0, a_(n-1), dots.up, dots.v; a_n, 0, dots.h.c, 0) $),
  cell("322", $ abs(x) = cases(x &"if" x >= 0, -x &"if" x < 0) $),

  cell("323", $ f(x) = cases(1/3 &"if" 0 <= x <= 1, 2/3 &"if" 3 <= x <= 4, 0 &"elsewhere") $),
  cell("324", $ cases(delim: "[", u_t - Delta u = f &"in" Omega times (0, T), u = 0 &"on" partial Omega times (0, T), u(dot.c, 0) = u_0 &"in" Omega) $),
  cell("325", $ cases(reverse: #true, x + 2 y = 5, 3 x - y = 1) => (x, y) = (1, 2) $),

  cell("326", $ op("ReLU")(z) = cases(z &z > 0, alpha (e^z - 1) &z <= 0) $),
  cell("327", $ delta_(i j) = cases(1 &i = j, 0 &i != j), quad epsilon_(1 2 3) = +1 $),
  cell("328", $ T(n) = cases(Theta(1) &n = 1, 2 T(n / 2) + Theta(n) &n > 1) $),

  cell("329", $ mat(1/(1+1), 1/(1+2), 1/(1+3); 1/(2+1), 1/(2+2), 1/(2+3); 1/(3+1), 1/(3+2), 1/(3+3)) $),
  cell("330", $ vec(dot(x)_1, dot(x)_2) = mat(0, 1; -omega_0^2, -2 zeta omega_0) vec(x_1, x_2) + vec(0, u(t)) $),
  cell("331", $ cases(x equiv a_1 &(mod m_1), x equiv a_2 &(mod m_2), dots.v, x equiv a_k &(mod m_k)) $),
)

#sec[§10 · Standalone Display, Numbered Equations & Multi-Column Aligned Arrays (332–342)]
#[
  #set math.equation(numbering: "(1)")
  $ #id("332") quad integral_(-oo)^(+oo) e^(-a x^2 + b x) dif x = sqrt(pi / a) exp(b^2 / (4 a)) quad (frak(R e)(a) > 0) $
  $ #id("333") quad sum_(n=0)^oo B_n / n! t^n = t / (e^t - 1), wide zeta(2 k) = ((-1)^(k+1) (2 pi)^(2 k) B_(2 k)) / (2 (2 k)!) $
]
#table(
  columns: (1fr, 1fr),
  stroke: 0.35pt + rgb("#CBD5E1"),
  inset: (x: 5pt, y: 3.5pt),
  align: horizon,
  cell("334", $ cal(L){f'}(s) &= integral_0^oo e^(-s t) f'(t) dif t \
    &= lr([ e^(-s t) f(t) ])_0^oo + s integral_0^oo e^(-s t) f(t) dif t \
    &= s F(s) - f(0^+) $),
  cell("335", $ norm(u + v)^2 + norm(u - v)^2 &= chevron.l u + v, u + v chevron.r + chevron.l u - v, u - v chevron.r &&"(expand)" \
    &= 2 norm(u)^2 + 2 norm(v)^2 &&"(parallelogram)" $),

  cell("336", $ nabla dot arrow(E) &= rho / epsilon_0, & wide nabla dot arrow(B) &= 0 \
    nabla times arrow(E) &= -(partial arrow(B)) / (partial t), & wide nabla times arrow(B) &= mu_0 arrow(J) + mu_0 epsilon_0 (partial arrow(E)) / (partial t) $),
  cell("337", $ D_(upright("KL"))(P || Q) &= sum_(x in cal(X)) P(x) log (P(x) / Q(x)) \
    &= H(P, Q) - H(P) >= 0 $),

  cell("338", $ product_(k=1)^n (1 + a_k) &<= product_(k=1)^n exp(a_k) = exp(sum_(k=1)^n a_k) \
    ln(1 + x) &= x - x^2 / 2 + x^3 / 3 - x^4 / 4 + dots.h.c $),
  cell("339", $ f(x) &= f(a) + f'(a)(x - a) + (f''(a)) / 2! (x - a)^2 + R_2 (x) \
    R_n (x) &= 1 / n! integral_a^x (x - t)^n f^((n+1))(t) dif t $),
)
$ #id("340") quad (partial^2) / (partial x^2) abs(phi(x + i y))^2 + (partial^2) / (partial y^2) abs(phi(x + i y))^2 = 4 abs(phi'(z))^2 >= 0 $
$ #id("341") quad R_(mu nu) - 1/2 R g_(mu nu) + Lambda g_(mu nu) = (8 pi G) / c^4 T_(mu nu), wide #id("342") quad i planck (partial) / (partial t) Psi(arrow(r), t) = lr([ -planck^2 / (2 m) nabla^2 + V(arrow(r), t) ]) Psi(arrow(r), t) $

#sec[§11 · Mozilla MathML Torture Benchmarks & Classical Analysis / Physics Giants (343–352)]
#table(
  columns: (1fr, 1fr),
  stroke: 0.35pt + rgb("#CBD5E1"),
  inset: (x: 5pt, y: 4pt),
  align: horizon,
  cell("343", $ a_0 + 1 / (a_1 + 1 / (a_2 + 1 / (a_3 + 1 / (a_4 + 1 / a_5)))) = [a_0; a_1, a_2, a_3, a_4, a_5] $),
  cell("344", $ 1 / (sqrt(phi sqrt(5)) - phi) e^((2 pi) / 5) = 1 + e^(-2 pi) / (1 + e^(-4 pi) / (1 + e^(-6 pi) / (1 + e^(-8 pi) / (1 + dots.h.c)))) $),

  cell("345", $ sqrt(1 + sqrt(1 + sqrt(1 + sqrt(1 + sqrt(1 + sqrt(1 + sqrt(1 + x))))))) $),
  cell("346", $ lr(( mat(mat(a, b; c, d), mat(e, f; g, h); 0, mat(i, j; k, l)) ))^(-1) = mat(mat(a, b; c, d)^(-1), -mat(a, b; c, d)^(-1) mat(e, f; g, h) mat(i, j; k, l)^(-1); 0, mat(i, j; k, l)^(-1)) $),

  cell("347", $ det mat(delim: "|", c_0, c_1, c_2, dots.h.c, c_n; c_1, c_2, c_3, dots.h.c, c_(n+1); dots.v, dots.v, dots.v, dots.down, dots.v; c_n, c_(n+1), c_(n+2), dots.h.c, c_(2 n)) > 0 $),
  cell("348", $ rho ( (partial arrow(u)) / (partial t) + (arrow(u) dot nabla) arrow(u) ) = -nabla p + mu nabla^2 arrow(u) + 1/3 mu nabla (nabla dot arrow(u)) + arrow(f) $),

  cell("349", $ (dif^n) / (dif x^n) f(g(x)) = sum_(mat(delim: #none, m_1 + 2 m_2 + dots.c + n m_n = n; m_k >= 0)) (n!) / (m_1! 1!^(m_1) m_2! 2!^(m_2) dots.c m_n! n!^(m_n)) f^((m_1 + dots.c + m_n))(g(x)) product_(j=1)^n (g^((j))(x))^(m_j) $),
  cell("350", $ cal(L)_"SM" = -1/4 F_(mu nu) F^(mu nu) + i overline(psi) gamma^mu D_mu psi + "h.c." + psi_i y_(i j) psi_j phi + "h.c." + abs(D_mu phi)^2 - V(phi) $),

  cell("351", $ Z[J] = integral cal(D) phi exp(i / planck integral dif^4 x [ 1/2 (partial_mu phi)(partial^mu phi) - 1/2 m^2 phi^2 - lambda / 4! phi^4 + J(x) phi(x) ]) $),
  cell("352", $ pi / 4 = 4 arctan(1 / 5) - arctan(1 / 239) = sum_(k=0)^oo (-1)^k / (2 k + 1) = 1 / (1 + 1^2 / (2 + 3^2 / (2 + 5^2 / (2 + 7^2 / (2 + dots.h.c))))) $),
)

#sec[§12 · Surreal / Pathological / Non-Semantic Chaos Bosses ("奇奇怪怪极限压力测试") (353–362)]
#table(
  columns: (1fr, 1fr),
  stroke: 0.35pt + rgb("#CBD5E1"),
  inset: (x: 5pt, y: 4pt),
  align: horizon,
  cell("353", $ root(mat(delim: "[", 1, 0; 0, -1), cases(frac(x^2 + 1, y - 1 / (z + 1)) &"if" cancel(alpha, cross: #true) != 0, sqrt(root(3, root(5, omega + 1))) &"otherwise")) $),
  cell("354", $ attach(
    overbrace(underbrace(Omega, b_1 + b_2), t_1 / t_2),
    tl: sqrt(alpha / beta),
    bl: binom(n, k),
    tr: mat(delim: "|", 1, 2; 3, 4),
    br: sum_(i=1)^m i^2,
  ) $),

  cell("355", $ overbrace(underbrace(overbracket(underbracket(overshell(undershell(x_0 + cancel(x_1) + cancel(x_2, inverted: #true) + cancel(x_3, cross: #true), "L6"), "L5"), "L4"), "L3"), "L2"), "L1") $),
  cell("356", $ lr(⟪ sum_(mat(delim: #none, i != j; k in NN)) hat(tilde(macron(breve(psi))))_(i, j)^((k)) mid(|) product.co_(alpha in aleph_1) frac(bb(H)^alpha, frak(g)_alpha, style: "skewed") mid(‖) integral.vol_(partial M) star.op F ⟫) $),

  cell("357", $ 1 + 1 / (2 + 2 / (3 + 3 / (4 + 4 / (5 + 5 / 6)))) = 6 / (5 + 5 / (4 + 4 / (3 + 3 / (2 + 2 / 1)))) $),
  cell("358", $ cases(reverse: #true, delim: "{",
    #text(fill: rgb("#DC2626"))[$frak(X)_1^('''')$] + display(1 / (1 + inline(1 / (1 + script(1 / (1 + sscript(1 / 2))))))) &= "奇异塔",
    #text(fill: rgb("#2563EB"))[$cal(Y)_(attach(F, bl: 2, br: 1))$] times root(n + 1 / m, vec(alpha, beta, gamma)) &= "超根向量",
  ) $),

  cell("359", $ mat(
    delim: "‖",
    augment: #(hline: 1, vline: 1),
    frac(sqrt(a) + root(3, b), overbrace(c + d, "top")), cases(1 &x > 0, -1 &x < 0);
    limits(op("WTF"))_(epsilon -> 0^+)^(Lambda -> oo) lr([ x / y mid(|) z ]), cancel(mat(1, 2; 3, 4), cross: #true);
  ) $),
  cell("360", $ stretch(harpoons.rtlb, size: #240%)^(overbracket(suit.spade.stroked + suit.heart.stroked, "red+black"))_(underbracket(aleph_0^(beth_1^(gimel_2)), "cardinals")) lr(⟦ frac(bold(sans("CHAOS")), mono("OMML_v2"), style: "horizontal") + "混沌极限"_(2^(2^2)) ⟧) $),
)
$ #id("361") quad sum_(k=1)^oo ( (-1)^(k+1) root(k, binom(2 k, k) + attach(Xi, tl: k, bl: k-1, tr: prime.triple, br: ast.op)) ) / ( product_(j=1)^k lr(( 1 + 1 / (j + 1 / (j + 1)) )) ) $
$ #id("362") quad integral.cont_(partial D) ( overline(underline(hat(f)(z))) dot cancel(z - z_0, inverted: #true) ) / ( (z - z_0)^4 sqrt(1 - z^2 / R^2) ) dif z $
