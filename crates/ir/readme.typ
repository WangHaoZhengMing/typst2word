#import "@preview/cetz:0.5.3": canvas, draw

#set page(
  paper: "a4",
  margin: (top: 2.5cm, bottom: 2.5cm, left: 2.8cm, right: 2.8cm),
  header: align(right)[
    #text(size: 8.5pt, fill: rgb("666666"))[typst2word · Document IR 架构设计规范]
  ],
  footer: align(center)[
    #context text(size: 8.5pt, fill: rgb("888888"))[
      #counter(page).display("1 / 1", both: true)
    ]
  ],
)

#set text(
  font: ("PingFang SC", "Microsoft YaHei", "Arial", "Liberation Sans"),
  size: 10.5pt,
  lang: "zh",
)

#set heading(numbering: "1.1")

#align(center)[
  #v(1em)
  #text(size: 22pt, weight: "bold")[Typst2Word IR (中间表示) 架构与设计指南] \
  #v(0.5em)
  #text(size: 10pt, fill: rgb("555555"))[crates/ir · 语义表示、身份溯源与物理布局度量三层架构]
  #v(1em)
]

#block(
  fill: rgb("f7f8fa"),
  inset: 12pt,
  radius: 6pt,
  stroke: 1pt + rgb("e1e4e8"),
  width: 100%,
)[
  *文档元信息 & 版本追踪*
  #v(4pt)
  - *Base Git Commit*: `4067f095932ebc45b9aba831f4450e7357dcaae3`
  - *Commit Message*: `release(v0.2.3): bump workspace version to 0.2.3`
  - *Commit Date*: `2026-11-15 10:30:25 +0800`
  - *Crate Path*: `crates/ir`
  - *Status*: Stable Specification
]

#v(1em)

= 引言与设计背景

== 为什么需要独立的 IR？
在 Typst 文档转 Microsoft Word (`.docx`) 的任务中，传统转换器（如 Pandoc）通常直接将源语言 AST 映射到目标标记语言。然而，Word 和 Typst 的排版模型存在根本性差异：
1. *几何度量差异*：Typst 采用基于 cap-height 和 leading 的精确基线步长模型，而 Word 依赖自带排版引擎的 `<w:line>`、`<w:lineRule>` 和 `<w:spacing>`；
2. *跨页切片断裂*：Typst 在编译后会将一个逻辑段落或高亮块切片跨多页输出，若直接读取排版碎片，Word 会生成重复或断裂的边框；
3. *语义重写降级*：Typst 强大的 `#show` 规则会将语义标题降级包裹成普通的 `#block`，导致转换后丢失 Word 大纲和目录导航。

为此，`crates/ir` 设计了一套*逻辑语义完整*、*带有空间几何感知*且*与目标 Word 格式严格解耦*的中间表示——`DocumentIr`。

= 核心三层解耦架构

IR 的整体设计严格贯彻了*三权分立*原则：

#align(center)[
#canvas({
  import draw: *

  // Layer 1: Semantic AST
  rect((-6.5, 2.2), (6.5, 4.4), fill: rgb("eef5ff"), stroke: 1.2pt + rgb("4a90e2"), radius: 4pt)
  content((0, 3.8), [*第一层：源意图与逻辑语义 (Source Intent & Resolved Semantics)*])
  content((0, 3.0), [
    #text(size: 8.5pt)[
      `BlockNode`（段落、标题、表格、公式、容器）· `InlineNode`（文字、公式、超链接、域）\
      *目标无关 (Target-Agnostic)*，表示纯粹的文档逻辑流，屏蔽底层 OOXML 特有属性
    ]
  ])

  // Center link id
  rect((-2.5, 0.7), (2.5, 1.7), fill: rgb("fff8e6"), stroke: 1.2pt + rgb("f5a623"), radius: 4pt)
  content((0, 1.35), [*第二层：统一身份与溯源*])
  content((0, 0.95), [#text(size: 8pt)[`NodeId` · `NodeOrigin` · `SemanticRoleIr`]])

  // Layer 3: Reference layout
  rect((-6.5, -2.0), (6.5, 0.2), fill: rgb("f0f9eb"), stroke: 1.2pt + rgb("67c23a"), radius: 4pt)
  content((0, -0.4), [*第三层：只读 Typst 排版度量 (Read-Only Reference Layout)*])
  content((0, -1.2), [
    #text(size: 8.5pt)[
      `NodeReferenceLayout` · `LayoutFragmentIr` (1-to-N 跨页切片) · `FlowReferenceIr` (垂直间距) \
      记录 Typst 真实几何坐标、真实字号与基线步长，为 Word 生成器提供校准基准
    ]
  ])

  // Arrows
  line((0, 2.2), (0, 1.7), stroke: 1.2pt + rgb("888888"), mark: (end: ">"))
  line((0, 0.7), (0, 0.2), stroke: 1.2pt + rgb("888888"), mark: (end: ">"))
  line((-4.5, 2.2), (-4.5, 0.2), stroke: (paint: rgb("999999"), dash: "dashed"), mark: (end: ">"))
  content((-5.5, 1.2), [#text(size: 7.5pt, fill: rgb("666666"))[通过 NodeId 索引]])

  line((4.5, 2.2), (4.5, 0.2), stroke: (paint: rgb("999999"), dash: "dashed"), mark: (end: ">"))
  content((5.5, 1.2), [#text(size: 7.5pt, fill: rgb("666666"))[保持树与几何解耦]])
})
]

== 1. 语义层 (AST Layer)
仅表示“逻辑上有什么”。不包含任何 OOXML 专用标签（如 `w:spacing`、`dxa`、`w:rFonts`），这些转换决策全部归入下游 `word-construct`。

== 2. 身份与溯源层 (Identity & Provenance)
每个逻辑节点分配唯一的 `NodeId`。
- 解决 `#for` 循环或模板展开时相同源码位置产生多份实例的碰撞问题；
- 维护 `SemanticRoleIr`：即便标题被 `#show` 规则包裹成 `#block(fill: red)[...]`，其原始身份仍保留为 `SemanticRoleIr::Heading`。

== 3. 只读布局度量层 (Reference Layout)
记录 Typst 编译器排版后的真实物理坐标和基线信息（以 `pt` 为单位）。生成器需要计算行距或段间距时，无需盲目猜测，直接按此真实数据做补偿校准。

= 数据结构详解

== 顶层结构 `DocumentIr`
`DocumentIr` 采用类似于 ECS（实体组件系统）的扁平 Sidecar 设计：
```rust
pub struct DocumentIr {
    pub sections: Vec<DocumentSectionIr>, // 分节信息与页面设置
    pub metadata: MetadataIr,             // 标题、作者、日期等
    pub page_setup: PageSetupIr,         // 纸张大小、边距、方向
    pub blocks: Vec<BlockNode>,           // 正文主块级节点列表
    pub report: ConversionReport,         // 诊断与转换统计报告

    // --- Sidecar 外部索引表 ---
    pub font_metrics: BTreeMap<String, FontFaceMetricsIr>,       // 字体度量表
    pub origins: BTreeMap<NodeId, NodeOrigin>,                   // 节点身份与来源
    pub reference_layout: BTreeMap<NodeId, NodeReferenceLayout>,// Typst排版几何信息
    pub anchors: Vec<AnchorLineIr>,                              // 视觉对齐比对锚线
}
```

== 块级节点 `BlockNode`
`BlockNode` 涵盖了所有可能独立占行或形成区域的排版块：

#table(
  columns: (2.2cm, 4.2cm, 1fr),
  fill: (x, y) => if y == 0 { rgb("f0f2f5") } else { none },
  stroke: 0.5pt + rgb("d0d5dd"),
  inset: 7pt,
  [*变体*], [*对应 Typst 概念*], [*核心字段与职责*],
  [`Title`], [页面/文档标题], [`content: Vec<InlineNode>`, `align`],
  [`Heading`], [`= Heading` 章节标题], [
    `level` (标题级别), `numbering`, `content`, `bookmark` (供超链接书签跳转), `box_style` (修饰背景框)
  ],
  [`Paragraph`], [普通文本段落], [`content`, `style` (行距、首行缩进、对齐方式)],
  [`List`], [`- item` 无序项目列表], [`items: Vec<ListItemIr>`, `tight` (紧凑度), `level`, `hanging_pt`],
  [`Enum`], [`+ item` 有序编号列表], [`items: Vec<EnumItemIr>`, 格式与挂起缩进],
  [`Terms`], [`/ Term: Desc` 定义列表], [`items: Vec<TermItemIr>`, 术语及其展开描述],
  [`Container`], [`#block(...)` 容器], [`fill`, `stroke`, `inset`, `radius`, `body: Vec<BlockNode>` 嵌套块],
  [`Table`], [`#table(...)` / `#grid(...)`], [行列网格、跨行跨列合并 (`colspan`/`rowspan`)、边框与对齐],
  [`Quote`], [`#quote(block: true)` 引述], [`body: Vec<BlockNode>`, `attribution: Option<Vec<InlineNode>>`],
  [`CodeBlock`], [```rust ... ``` 原生代码块], [`lang`, `lines: Vec<Vec<TextRunIr>>` (携带语法高亮 run), `fill`],
  [`Figure`], [`#figure(..., caption: ...)`], [图表主体、`caption`、`kind` (Image/Table/Code)、书签],
  [`EquationBlock`], [`$ ... $` 独立块级公式], [`EquationIr`，OMML/MathML 抽象，支持编号与书签],
  [`ShapeBlock`], [矢量图形基语], [`ShapeIr` (矩形、圆形、椭圆、多边形、贝塞尔曲线路径)],
  [`CetzCanvas`], [`cetz.canvas(...)` 绘图画布], [完整保留 CeTZ 矢量指令流以翻译为 Word 矢量图/DrawingML],
  [`ImageBlock`], [`#image(...)` 独立图像], [图像字节或路径、宽/高、缩放比例与适配模式],
  [`Outline`], [`#outline(...)` 目录], [目录项条目与页码参考],
  [`Bibliography`], [`#bibliography(...)` 参考文献], [参考文献引用条目列表],
  [`HorizontalRule`], [`#line(length: 100%)` 分割线], [线长、笔触 `stroke`、左右对齐],
  [`PageBreak`], [`#pagebreak()` 显式分页符], [强制段落从新页开始],
  [`Columns`], [`#columns(n)[...]` 多栏布局], [`count` (栏数), `gutter_pt` (栏间距), `body` 栏内块],
)

== 行内节点 `InlineNode`
`InlineNode` 是段落、标题、单元格内部的基本单元：

#table(
  columns: (2.5cm, 4cm, 1fr),
  fill: (x, y) => if y == 0 { rgb("f0f2f5") } else { none },
  stroke: 0.5pt + rgb("d0d5dd"),
  inset: 7pt,
  [*变体*], [*对应语义*], [*说明*],
  [`Text`], [文本片段 `TextRunIr`], [包含纯文本字符串、字体族、字号、粗斜体、下划线、字距、颜色],
  [`EquationInline`], [`$x+y$` 行内数学公式], [行内公式树，转译为 Word OMML 行内结构],
  [`Field`], [动态文档域], [`Page` (当前页码), `NumPages` (总页数), `PageRef` (页码引用)],
  [`Link`], [超链接], [支持外部 URL 网页跳转或文档内部 `_typst_loc_` 书签锚点跳转],
  [`Ref`], [交叉引用], [`@label` / `#ref(<label>)` 引用标题、表格或图表编号],
  [`Citation`], [参考文献引用], [`@key` 文献上标或括号引用],
  [`Footnote`], [`#footnote[...]` 脚注], [脚注编号文字与内部富文本 `content: Vec<BlockNode>`],
  [`ShapeInline`], [行内小图标/小图形], [如内联在文字里的 `#box(rect(...))` 标签],
  [`CetzInline`], [行内微型矢量画布], [支持文字行内混排 CeTZ 标记],
  [`ImageInline`], [行内图片], [如行内 emoji、小徽章图标],
  [`LineBreak`], [`\` 软换行符], [Word `<w:br/>` 换行不换段],
  [`Tab`], [`\t` 制表符], [Word `<w:tab/>`],
  [`HorizontalSpace`], [`#h(12pt)` 水平显式间距], [显式空白宽度],
)

= 通俗易懂：设计背后的思考

== 1. 为什么“逻辑 1 对 几何 N”很重要？
想象你在 Typst 里写了一个带灰色背景的醒目高亮框：
#align(center)[
#canvas({
  import draw: *

  // Page 1
  rect((-5, -1.8), (-1, 2.2), fill: rgb("ffffff"), stroke: 1pt + rgb("999999"))
  content((-3, 1.8), [#text(size: 8pt, fill: rgb("888888"))[Typst 第 1 页]])
  rect((-4.6, -1.5), (-1.4, 0.5), fill: rgb("f0f0f0"), stroke: 1pt + rgb("cccccc"))
  content((-3, -0.5), [#text(size: 8pt)[高亮框前半部分\n(Fragment 1)]])

  // Page 2
  rect((1, -1.8), (5, 2.2), fill: rgb("ffffff"), stroke: 1pt + rgb("999999"))
  content((3, 1.8), [#text(size: 8pt, fill: rgb("888888"))[Typst 第 2 页]])
  rect((1.4, 0.2), (4.6, 1.4), fill: rgb("f0f0f0"), stroke: 1pt + rgb("cccccc"))
  content((3, 0.8), [#text(size: 8pt)[高亮框后半部分\n(Fragment 2)]])

  // Word doc
  line((0, -2.5), (0, -3.2), stroke: 1.5pt + rgb("4a90e2"), mark: (end: ">"))
  content((0, -2.8), [#text(size: 8pt, fill: rgb("4a90e2"))[IR 映射到 Word]])

  rect((-4, -5.2), (4, -3.6), fill: rgb("eef5ff"), stroke: 1.2pt + rgb("4a90e2"))
  content((0, -4.4), [
    *Word 目标文档* \
    #text(size: 8.5pt)[
      单一连续的 `<w:p>` / `<w:tbl>` 高亮区域（由 Word 自身引擎负责在屏幕上分页）
    ]
  ])
})
]

如果 IR 记录的是 Typst 物理切片，那到了 Word 就会变成两个独立的灰色方框，上下边框重复闭合；
而如果 IR 只记录纯语义，又无法得知该元素在实际排版中的真实高度与间距。
*解决方案*：AST 里的 `BlockNode::Container` 永远只有 1 个，它的 `NodeId` 在 `reference_layout` 中映射到包含了 2 个 `LayoutFragmentIr` 的切片列表。

== 2. 如何实现“字节级可复现（Deterministic Output）”？
Typst 内部的位置哈希（`Location::hash()`）取决于编译时的内部状态和文件绝对路径。如果直接将这类哈希写入 Word 书签名称，每次在不同电脑编译生成的 `.docx` 文件哈希都会变动。
IR 引入了 `ir::normalize::canonicalize_bookmark_anchors` 规范化步骤：在 AST 遍历时，按文档从头到右出现的顺序，将所有 `_typst_loc_<hash>` 重写为确定性的 `_typst_loc_0001`、`_typst_loc_0002`。这保证了相同的文档在任何环境下转换出的二进制 docx 内容完全相同。

== 3. 为什么有字体度量缓存？
Word 在不同操作系统（Windows、macOS）上的渲染机制存在差异，尤其是行距计算。Typst2Word 在编译期会提取文档所用 OpenType 字体的精确字形表（Cap-Height、x-Height、Ascender/Descender），并保存在 `DocumentIr::font_metrics` 中。
下游的 `word-construct` 可以直接利用这组客观数学参数计算出 Word `<w:line>` 所需的最优参数，摆脱依赖平台魔数的猜测行为。

= 目录结构索引

`crates/ir/src` 的源码模块职责划分清晰：
- `lib.rs`: 核心枚举定义 (`BlockNode`, `InlineNode`, `DocumentIr`) 与导出；
- `document.rs`: 页面尺寸、边距与分节配置 (`PageSetupIr`, `MetadataIr`)；
- `origin.rs`: 统一节点身份 (`NodeId`)、源码范围与原始角色 (`SemanticRoleIr`)；
- `reference_layout.rs`: Typst 真实几何测量结构体 (`LayoutFragmentIr`, `ParagraphLineMetricsIr`)；
- `normalize.rs`: 规范化管道（合并文字 run、消除冗余空页、重新规范化书签）；
- `table.rs` / `equation.rs` / `cetz.rs` / `shape.rs`: 各专项排版元素的语义结构定义。
