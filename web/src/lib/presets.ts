import type { StudioPreset } from './types';

export const STUDIO_PRESETS: StudioPreset[] = [
  {
    id: 'showcase',
    title: '全量语义与视觉转换',
    subtitle: '公式 OMML · 原生表格 · DrawingML 图形 · CeTZ 画布',
    badge: '综合全景',
    mainPath: 'showcase.typ',
    files: {
      'showcase.typ': `#import "@preview/cetz:0.5.3": canvas, draw

#set document(
  title: "typst2word 全量语义与视觉高保真转换示例",
  author: "typst2word Studio",
  keywords: ("Typst", "Word", "OMML", "DrawingML", "CeTZ"),
)
#set page(
  paper: "a4",
  header: align(right)[#text(size: 9pt, fill: luma(110))[_typst2word 语义与视觉高保真转换示例_]],
  footer: context align(center)[#text(size: 9pt, fill: luma(110))[第 #counter(page).display() 页]],
)
#set text(
  font: ("Libertinus Serif", "Noto Serif SC"),
  size: 10.5pt,
  lang: "zh",
)
#set par(justify: true, leading: 0.68em, spacing: 1.15em)
#set heading(numbering: "1.1")
#set math.equation(numbering: "(1)")

#title[typst2word 全量语义与视觉转换示例]

#outline(indent: auto)

= 文本语义与内联富格式 <sec-text>

本示例在浏览器内通过 WebAssembly 直接运行完整 *Typst 0.15* 编译器与 *Word 布局补偿引擎*。支持 *粗体*、_斜体_、#underline[下划线]、#strike[删除线]、#overline[上划线]、#highlight(fill: rgb("#FFF3B0"))[荧光高亮]、化学式 H#sub[2]O、相对论能量 E=mc#super[2]、#smallcaps[SmallCaps 小型大写]、行内代码 \`typst2word::convert()\`、#text(fill: rgb("#0066CC"))[主题色文本]、超链接 #link("https://typst.app")[Typst 官网]、交叉引用（见 @sec-math 的公式 @eq-gauss）以及原生脚注#footnote[转化为 Word 原生 \`word/footnotes.xml\` 部件，而非页面底部普通文本。]。

== 列表与术语定义

- 语义可编辑性（Semantic Editability）
  - 标题映射为原生 \`Heading 1..6\` 样式，支持 Word 导航窗格与动态目录
  - 公式映射为原生 \`<m:oMath>\`，可在 Word 公式编辑器中二次修改
+ 视觉高保真校准（Visual Calibration）
+ 读取字体二进制 \`OS/2\` 度量并补偿中文字体 \`1.3×\` 行盒扩张与段间间距折叠

/ 双源提取 (Dual Extraction): 同时关联排版前 \`Content\` 语义节点与排版后 \`Frame\` 物理坐标。
/ 贪心断行补偿 (Glue Compensation): 在不插入硬回车的前提下对齐 Knuth-Plass 段落行数。

== 引用块与语法高亮代码

#quote(block: true, attribution: [Donald E. Knuth])[
  Beware of bugs in the above code; I have only proved it correct, not tried it.
]

\`\`\`rust
pub fn convert_typst_to_docx(source: &str) -> Result<Vec<u8>, String> {
    let (bytes, ir) = typst2word::convert_typst_str_to_docx_bytes(source, None)?;
    assert!(ir.report.equations_count > 0);
    Ok(bytes)
}
\`\`\`

= 复合表格与跨行列合并 <sec-table>

#figure(
  table(
    columns: (1.1fr, 1.3fr, 1.6fr),
    inset: (x: 8pt, y: 6pt),
    stroke: 0.6pt + rgb("#D2D2D7"),
    fill: (x, y) => if y == 0 { rgb("#F0F6FF") } else { none },
    table.header([*能力维度*], [*Typst 语法构造*], [*Word 原生 OpenXML 映射*]),
    table.cell(rowspan: 2, align: horizon)[*矢量图形*],
    [\`rect\` / \`circle\` / \`curve\`], [DrawingML \`<wps:wsp>\` 自定义几何路径],
    [\`cetz.canvas\` 绘图画布], [DrawingML \`<wpg:wgp>\` 组合矢量对象（非位图）],
    [*数学公式*], [\`$ ... $\` 行内与编号块公式], [Office Math \`<m:oMath>\` / \`<m:oMathPara>\`],
    table.cell(colspan: 2)[*未支持图元安全降级策略*], [生成显式可读占位提示块，绝不静默丢失],
  ),
  caption: [Typst 核心构造到 Microsoft Word 原生对象映射一览],
) <fig-map>

= 数学公式转换（Office Math OMML） <sec-math>

行内欧拉恒等式 $e^(i pi) + 1 = 0$ 与高斯积分、级数展开编号公式：

$ integral_(-infinity)^(+infinity) e^(-x^2) dif x = sqrt(pi), quad sum_(n=1)^infinity frac(1, n^2) = frac(pi^2, 6) $ <eq-gauss>

矩阵、分段函数与上下花括号批注均可输出为完全可编辑的 OMML 结构：

$ A = mat(1, lambda, 0; 0, 1, -lambda; 0, 0, 1), quad f(x) = cases(x^2 sin(1/x) &"if" x != 0, 0 &"if" x = 0) $

$ overbrace(a_1 + a_2 + dots.c + a_n, n "项求和") = hat(S)_n + cancel(epsilon_0) + binom(n, k) + root(3, x^2 + 1) $

= 原生矢量图形与 CeTZ 画布 <sec-cetz>

#stack(
  dir: ltr,
  spacing: 14pt,
  rect(width: 175pt, height: 38pt, fill: rgb("#E8F2FF"), stroke: 1.2pt + rgb("#0071E3"), radius: 6pt)[
    #align(center + horizon)[*圆角矩形* \`<wps:wsp>\` $E = h nu$]
  ],
  ellipse(width: 105pt, height: 38pt, fill: rgb("#EAFBF0"), stroke: 1.2pt + rgb("#28CD41"))[
    #align(center + horizon)[*原生椭圆*]
  ],
  polygon(
    fill: rgb("#FFF4E5"),
    stroke: 1.2pt + rgb("#FF9F0A"),
    (0pt, 36pt),
    (32pt, 2pt),
    (64pt, 36pt),
  ),
)

#v(6pt)

#align(center)[
  #canvas(length: 1cm, {
    import draw: *
    grid((0, 0), (10, 2.6), step: 1, stroke: 0.35pt + luma(225))
    rect((0.3, 0.5), (2.9, 2.1), fill: rgb("#E8F2FF"), stroke: 1.3pt + rgb("#0071E3"), radius: 0.16)
    content((1.6, 1.3), [*Typst AST*])

    line((2.9, 1.3), (4.1, 1.3), mark: (end: ">"), stroke: 1.3pt + rgb("#48484A"))
    rect((4.1, 0.5), (6.7, 2.1), fill: rgb("#F2EEFF"), stroke: 1.3pt + rgb("#5E5CE6"), radius: 0.16)
    content((5.4, 1.3), [*DocumentIr*])

    line((6.7, 1.3), (7.9, 1.3), mark: (end: ">"), stroke: (paint: rgb("#0071E3"), thickness: 1.3pt, dash: "dashed"))
    circle((8.9, 1.3), radius: 0.85, fill: rgb("#EAFBF0"), stroke: 1.3pt + rgb("#248A3D"))
    content((8.9, 1.3), [*.docx*])
  })
]
`,
    },
  },
  {
    id: 'academic-math',
    title: '多文件学术论文与参考文献',
    subtitle: '含 refs.bib 引用 · 矩阵算子 · 双栏摘要 · 公式交叉引用',
    badge: '多文件 + BibTeX',
    mainPath: 'paper.typ',
    files: {
      'paper.typ': `#set document(
  title: "Layout-Calibrated Semantic Lowering from Typst to Office OpenXML",
  author: ("Linus Chen", "Ada Zhang"),
)
#set page(paper: "a4", margin: (x: 2.4cm, y: 2.5cm))
#set text(font: ("Libertinus Serif", "Noto Serif SC"), size: 10.5pt, lang: "en")
#set par(justify: true, leading: 0.62em, spacing: 1.05em)
#set heading(numbering: "1.")
#set math.equation(numbering: "(1)")

#align(center)[
  #text(size: 16pt, weight: "bold")[Layout-Calibrated Semantic Lowering from Typst to Office OpenXML]
  #v(4pt)
  #text(size: 10pt, fill: luma(90))[Department of Computer Systems · Technical Report]
]

#v(6pt)
#block(
  fill: rgb("#F5F5F7"),
  inset: 12pt,
  radius: 5pt,
  stroke: 0.5pt + rgb("#D2D2D7"),
)[
  *Abstract.* Optimal paragraph breaking @knuth1984 and modern attention-based sequence architectures @vaswani2017 motivate high-precision document representations. We demonstrate how pre-layout semantic introspection fused with post-layout frame geometry produces native Office MathML (\`<m:oMath>\`) without sacrificing baseline rhythm.
]

= Variational Formulation <sec-var>

Given an input token sequence $X in RR^(n times d)$, scaled dot-product attention @vaswani2017 evaluates:

$ "Attention"(Q, K, V) = "softmax"(frac(Q K^top, sqrt(d_k))) V $ <eq-attn>

When coupled with an $n$-th order differential operator and boundary constraints, the Euler–Lagrange stationarity condition expands into the block system:

$ cal(L)(u, lambda) = integral_Omega (frac(1, 2) norm(nabla u)_2^2 - f u) dif x + sum_(i=1)^m lambda_i g_i (u) $ <eq-lagrange>

= Spectral Stability Matrix

Combining Eq. @eq-attn and Eq. @eq-lagrange yields the symmetric saddle-point Jacobian:

$ cal(H) = mat(
  nabla^2 cal(E)(u), B^top, 0;
  B, -alpha I, C^top;
  0, C, -beta I
), quad det(cal(H) - mu I) = prod_(k=1)^N (sigma_k^2 - mu) $

#bibliography("refs.bib", style: "ieee")
`,
      'refs.bib': `@article{knuth1984,
  author  = {Knuth, Donald E. and Plass, Michael F.},
  title   = {Breaking Paragraphs into Lines},
  journal = {Software: Practice and Experience},
  volume  = {11},
  number  = {11},
  pages   = {1119--1184},
  year    = {1981}
}

@inproceedings{vaswani2017,
  author    = {Vaswani, Ashish and Shazeer, Noam and Parmar, Niki and Uszkoreit, Jakob},
  title     = {Attention Is All You Need},
  booktitle = {Advances in Neural Information Processing Systems},
  volume    = {30},
  year      = {2017}
}
`,
    },
  },
  {
    id: 'scientific-tables',
    title: '科研三线表与实验数据报告',
    subtitle: '三线表边框消解 · 跨行跨列 · 图表混排',
    badge: '表格专场',
    mainPath: 'tables.typ',
    files: {
      'tables.typ': `#set document(title: "科研三线表与结构化排版测试")
#set page(paper: "a4", margin: 2.5cm)
#set text(font: ("Libertinus Serif", "Noto Serif SC"), size: 10.5pt, lang: "zh")
#set par(justify: true, leading: 0.68em)
#set heading(numbering: "1.1")

= 学术出版级三线表（Per-Edge 边框冲突消解）

在学术期刊与学位论文中，三线表依赖顶底粗线（\`1.2pt\`）与表头细线（\`0.6pt\`），且竖向无分隔线。\`typst2word\` 精确消解每个单元格四边线型并输出原生 \`<w:tcBorders>\`：

#figure(
  table(
    columns: (1.3fr, 0.9fr, 0.9fr, 0.9fr, 1.1fr),
    align: (left, right, right, right, center),
    inset: (x: 8pt, y: 6.5pt),
    stroke: (x, y) => (
      top: if y == 0 { 1.2pt + black } else if y == 1 { 0.6pt + black } else { none },
      bottom: 1.2pt + black,
    ),
    table.header(
      [*模型架构*], [*参数量 (M)*], [*吞吐 (tok/s)*], [*困惑度 $downarrow$*], [*收敛状态*]
    ),
    [Baseline Transformer], [124.0], [1,420], [18.42], [稳定],
    [Linear-Attention v2], [126.5], [2,890], [17.95], [稳定],
    [*Hybrid-State (Ours)*], [*128.2*], [*3,140*], [*16.08*], [*最优 $star$*],
  ),
  caption: [不同序列建模架构在标准语料上的吞吐率与困惑度对比],
)

= 跨行跨列分组实验矩阵

#figure(
  table(
    columns: (1fr, 1fr, 1fr, 1fr),
    inset: 7pt,
    stroke: 0.5pt + rgb("#C7C7CC"),
    fill: (x, y) => if y < 2 { rgb("#F5F5F7") } else { none },
    table.header(
      table.cell(rowspan: 2, align: horizon)[*测试集*],
      table.cell(colspan: 2, align: center)[*视觉还原指标*],
      table.cell(rowspan: 2, align: horizon + center)[*Word 可编辑性*],
      [*行高误差 (pt)*], [*IoU 重合率*],
    ),
    [西文混排段落], [$<= 0.15$], [99.1%], [原生 \`w:p\`],
    [中文 1.3× 行盒], [$<= 0.20$], [98.7%], [槽位路由 \`eastAsia\`],
    [多行矩阵公式], [$<= 0.35$], [97.9%], [原生 \`m:oMathPara\`],
  ),
  caption: [双层表头合并（\`colspan\` + \`rowspan\`）转换验证],
)
`,
    },
  },
  {
    id: 'cetz-diagrams',
    title: 'CeTZ 矢量信号与几何绘图',
    subtitle: '自动拉取 @preview/cetz · 转化为原生可编辑 DrawingML 组',
    badge: 'CeTZ 矢量',
    mainPath: 'diagrams.typ',
    files: {
      'diagrams.typ': `#import "@preview/cetz:0.5.3": canvas, draw

#set document(title: "CeTZ 矢量图形转 Word DrawingML 演示")
#set page(paper: "a4", margin: 2.5cm)
#set text(font: ("Libertinus Serif", "Noto Serif SC"), size: 10.5pt, lang: "zh")
#set heading(numbering: "1.")

= 坐标系、曲线与箭头标记

以下 \`@preview/cetz:0.5.3\` 画布不会栅格化为模糊图片，而是直接编译为 Word 原生 \`<wpg:wgp>\` 矢量形状组与数学公式文本框：

#align(center)[
  #canvas(length: 1cm, {
    import draw: *
    grid((0, 0), (9, 4), step: 1, stroke: 0.3pt + luma(225))
    line((0, 0), (9.4, 0), mark: (end: ">"), stroke: 1.2pt + rgb("#1D1D1F"))
    line((0, 0), (0, 4.4), mark: (end: ">"), stroke: 1.2pt + rgb("#1D1D1F"))
    content((9.7, 0), [$t$])
    content((0, 4.7), [$u(t)$])

    bezier((0.2, 0.4), (3.2, 3.6), (1.4, 3.8), (2.2, 0.5), stroke: 1.6pt + rgb("#0071E3"))
    bezier((3.2, 3.6), (6.5, 1.4), (4.4, 3.6), (5.2, 1.4), stroke: 1.6pt + rgb("#0071E3"))
    bezier((6.5, 1.4), (8.8, 2.8), (7.5, 1.4), (8.0, 2.8), stroke: 1.6pt + rgb("#0071E3"))

    line((0, 2.8), (9.0, 2.8), stroke: (paint: rgb("#FF3B30"), thickness: 1pt, dash: "dashed"))
    content((7.6, 3.25), [#text(fill: rgb("#FF3B30"), size: 9pt)[稳态收敛阈值 $u_*$]])
  })
]

= 闭环反馈控制系统框图

#align(center)[
  #canvas(length: 1cm, {
    import draw: *
    content((0.4, 1.5), [$R(s)$])
    line((0.9, 1.5), (1.9, 1.5), mark: (end: ">"), stroke: 1.2pt)
    circle((2.3, 1.5), radius: 0.38, fill: rgb("#F5F5F7"), stroke: 1.2pt)
    content((2.3, 1.5), [$+$])

    line((2.68, 1.5), (3.9, 1.5), mark: (end: ">"), stroke: 1.2pt)
    rect((3.9, 0.85), (6.3, 2.15), fill: rgb("#E8F2FF"), stroke: 1.3pt + rgb("#0071E3"), radius: 0.14)
    content((5.1, 1.5), [*控制器* $C(s)$])

    line((6.3, 1.5), (7.5, 1.5), mark: (end: ">"), stroke: 1.2pt)
    rect((7.5, 0.85), (9.9, 2.15), fill: rgb("#EAFBF0"), stroke: 1.3pt + rgb("#248A3D"), radius: 0.14)
    content((8.7, 1.5), [*被控对象* $P(s)$])

    line((9.9, 1.5), (11.3, 1.5), mark: (end: ">"), stroke: 1.2pt)
    content((11.8, 1.5), [$Y(s)$])

    line((10.6, 1.5), (10.6, 0.1), (2.3, 0.1), (2.3, 1.12), mark: (end: ">"), stroke: 1.1pt + luma(80))
    content((6.4, -0.28), [#text(size: 9pt, fill: luma(80))[单位负反馈通道 $H(s) = 1$]])
  })
]
`,
    },
  },
  {
    id: 'exam-layout',
    title: '高校期末试卷模板（动态参数）',
    subtitle: '演示 sys.inputs 模板变量 · 登分栏 · 中英混排题干',
    badge: '试卷 + 变量',
    mainPath: 'exam.typ',
    inputs: {
      course: '数据结构与算法分析',
      semester: '2026-2027 学年第一学期',
      duration: '120 分钟',
    },
    files: {
      'exam.typ': `#let course = sys.inputs.at("course", default: "数据结构与算法分析")
#let semester = sys.inputs.at("semester", default: "2026-2027 学年第一学期")
#let duration = sys.inputs.at("duration", default: "120 分钟")

#set document(title: course + " 期末试题")
#set page(
  paper: "a4",
  margin: (top: 2.2cm, bottom: 2.2cm, left: 2.4cm, right: 2.4cm),
  footer: context align(center)[#text(size: 9pt, fill: luma(100))[《#course》试卷 · 第 #counter(page).display() 页]],
)
#set text(font: ("Libertinus Serif", "Noto Serif SC"), size: 10.5pt, lang: "zh")
#set par(justify: true, leading: 0.72em)

#align(center)[
  #text(size: 12pt)[#semester 期末考试试题（A 卷）]
  #v(4pt)
  #text(size: 17pt, weight: "bold")[《#course》]
  #v(6pt)
  #text(size: 9.5pt, fill: luma(80))[考试时长：#duration #h(1.5em) 闭卷笔试 #h(1.5em) 满分：100 分]
]

#v(6pt)
#table(
  columns: (1fr, 1fr, 1fr, 1fr, 1fr, 1.2fr),
  align: center + horizon,
  inset: 6pt,
  stroke: 0.6pt + black,
  [*题号*], [一], [二], [三], [四], [*总分*],
  [*满分*], [20], [20], [30], [30], [*100*],
  [*得分*], [], [], [], [], [],
)

= 一、单项选择题（本大题共 2 小题，每小题 5 分）

+ 设具有 $n$ 个顶点的有向无环图（DAG）采用邻接表存储，执行 Kahn 拓扑排序算法的时间复杂度为（ #h(1.6em) ）。
  #grid(
    columns: (1fr, 1fr, 1fr, 1fr),
    row-gutter: 6pt,
    [A. $O(n log n)$], [B. $O(n + e)$], [C. $O(n^2)$], [D. $O(e log n)$],
  )

+ 若一棵平衡二叉树（AVL 树）的高度为 $h$（单个根节点高度记为 $1$），则其最少包含的结点数 $N(h)$ 满足递推关系（ #h(1.6em) ）。
  #grid(
    columns: (1fr, 1fr),
    row-gutter: 6pt,
    [A. $N(h) = N(h-1) + N(h-2) + 1$],
    [B. $N(h) = 2^(h-1)$],
  )

= 二、算法设计与综合推导题（30 分）

已知实对称正定矩阵 $A in RR^(n times n)$ 满足条件数 $kappa(A) = lambda_max / lambda_min$。试证明最速下降法在能量范数 $norm(e_k)_A = sqrt(e_k^top A e_k)$ 下的收敛率上界：

$ norm(x_(k+1) - x_*)_A <= (frac(kappa(A) - 1, kappa(A) + 1)) norm(x_k - x_*)_A $
`,
    },
  },
];
