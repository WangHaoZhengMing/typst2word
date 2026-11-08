#import "@preview/cetz:0.3.4": canvas, draw

#set document(
  title: "Typst2Word 全量语义与视觉高保真转换示例",
  author: "typst2word",
  keywords: ("Typst", "Word", "OMML", "DrawingML", "CeTZ"),
)
#set page(
  paper: "a4",
  header: align(right)[_Typst2Word 转换引擎示例文档_],
  footer: align(center)[第 1 页],
)
#set heading(numbering: "1.1")
#set math.equation(numbering: "(1)")

#title[Typst2Word 全量语义与视觉转换示例]

#outline()

= 文本与段落语义 <sec-text>

本节演示基础与高级富文本内联样式：*粗体文本*、_斜体文本_、#underline[下划线文本]、#strike[删除线文本]、#overline[上划线文本]、#highlight(fill: rgb("#FFF59D"))[高亮背景文本]、化学式 H#sub[2]O、质能方程 E=mc#super[2]、#smallcaps[SmallCaps 小型大写字母]、行内代码 `typst2word::convert()`、#text(fill: rgb("#1565C0"))[自定义颜色文本]、外部超链接 #link("https://typst.app")[Typst 官方网站]、内部交叉引用 @sec-text 与 @eq-euler，以及原生 Word 脚注#footnote[这是一个转化为 Word 原生 `word/footnotes.xml` 的富文本*脚注*。]。

== 列表、编号与术语定义

- 第一级无序列表项
  - 第二级嵌套无序列表项
- 另一个无序列表项

+ 第一项有序编号列表
+ 第二项有序编号列表

/ 语义还原 (Semantic Restoration): 通过自定义 Typst 编译器后端与 `Introspector` 查询保留完整 AST/Tag 树。
/ 视觉一致 (Visual Consistency): 结合排版后 `Frame` 几何尺寸、颜色、线型与字体信息。

== 引用块与语法高亮代码块

#quote(block: true, attribution: [Donald E. Knuth])[
  Beware of bugs in the above code; I have only proved it correct, not tried it.
]

```rust
pub fn convert_typst_file_to_docx(input: &Path, output: &Path) -> Result<DocumentIr, String> {
    let ir = compile_file_to_ir(input)?;
    write_docx_to_path(&ir, output)?;
    Ok(ir)
}
```

= 表格与跨行跨列结构 <sec-table>

#figure(
  table(
    columns: (110pt, 130pt, 150pt),
    fill: (x, y) => if y == 0 { rgb("#E3F2FD") } else { none },
    table.header([*模块*], [*Typst 元素*], [*Word 原生目标对象*]),
    table.cell(rowspan: 2)[图形与绘图], [`rect` / `circle` / `polygon` / `curve`], [DrawingML `<wps:wsp>`],
    [`cetz.canvas`], [DrawingML `<wpg:wgp>` 组合矢量图],
    [数学公式], [`$ ... $` (`EquationElem`)], [Office Math `<m:oMath>` / `<m:oMathPara>`],
    table.cell(colspan: 2)[*不支持元素兜底策略*], [*显式警示占位块*],
  ),
  caption: [Typst 到 Word 核心元素映射表],
) <fig-mapping>

= 原生图形对象转换 (`DrawingML`) <sec-shapes>

#rect(
  width: 220pt,
  height: 44pt,
  fill: rgb("#E3F2FD"),
  stroke: 1.5pt + rgb("#1E88E5"),
  radius: 6pt,
)[
  *圆角矩形 (`<wps:wsp>`)* — 内部支持富文本与公式 $E = h nu$
]

#circle(radius: 26pt, fill: rgb("#E8F5E9"), stroke: 1.5pt + rgb("#43A047"))[圆形]

#ellipse(width: 110pt, height: 42pt, fill: rgb("#FCE4EC"), stroke: 1.2pt + rgb("#D81B60"))[椭圆图形]

#polygon(
  fill: rgb("#EDE7F6"),
  stroke: 1.5pt + rgb("#5E35B1"),
  (0pt, 36pt),
  (35pt, 0pt),
  (70pt, 36pt),
)

#curve(
  fill: rgb("#E0F7FA"),
  stroke: 1.5pt + rgb("#00838F"),
  curve.move((0pt, 20pt)),
  curve.line((45pt, 0pt)),
  curve.cubic((75pt, 10pt), (75pt, 30pt), (45pt, 40pt)),
  curve.close(),
)

= 数学公式转换 (`Office Math OMML`) <sec-math>

行内公式如 $e^(i pi) + 1 = 0$，以及编号块级公式：

$ integral_(-infinity)^(+infinity) e^(-x^2) dif x = sqrt(pi), quad sum_(n=1)^infinity frac(1, n^2) = frac(pi^2, 6) $ <eq-euler>

$ A = mat(1, 2, 3; 0, 4, 5; 0, 0, 6), quad f(x) = cases(x^2 &"if" x >= 0, -x &"if" x < 0) $

$ overbrace(a_1 + a_2 + dots.c + a_n, n "项") = hat(S)_n + cancel(epsilon) + binom(n, k) + root(3, x^2 + 1) $

= CeTZ 矢量绘图转换 (`<wpg:wgp>`) <sec-cetz>

#canvas(length: 1cm, {
  import draw: *
  grid((0, 0), (5, 3), step: 1, stroke: 0.4pt + luma(200))
  rect((0.3, 0.4), (2.3, 2.4), fill: rgb("#E3F2FD"), stroke: 1.5pt + rgb("#1E88E5"))
  circle((3.7, 1.4), radius: 0.9, fill: rgb("#FFF9C4"), stroke: 1.5pt + rgb("#F57F17"))
  line((2.3, 1.4), (2.8, 1.4), stroke: (paint: rgb("#D32F2F"), thickness: 1.5pt, dash: "dashed"))
  content((1.3, 1.4), [*Input* $x_k$])
  content((3.7, 1.4), [*Transform* $T(x)$])
})

= 不支持元素的可视化兜底展示 <sec-unsupported>

// #html.elem("div")[此处的 HTML 专属标签将被识别并展示为不支持元素占位块]

#skew(ax: 18deg)[2D 错切变换在 Word 中展示为不支持元素提示]

#repeat[. ]
