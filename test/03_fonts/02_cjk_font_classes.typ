// Test 03_fonts/02_cjk_font_classes.typ
// Covers: macOS/Windows CJK typefaces (Songti SC, Heiti SC, Kaiti SC)
// and mixed CJK + Western dual-font fallback behavior.

#set page(
  paper: "a4",
  margin: (top: 2.54cm, bottom: 2.54cm, left: 3.18cm, right: 3.18cm),
)
#set text(font: "Songti SC", size: 11pt, lang: "zh", cjk-latin-spacing: none)
#set par(
  justify: true,
  linebreaks: "simple",
  justification-limits: (spacing: (min: 100%, max: 500%)),
  leading: 0.65em,
  spacing: 1.2em,
)

= 1. Songti SC（华文宋体 / macOS 系统宋体，11pt）

#text(font: "Songti SC", size: 11pt)[
  华文宋体是苹果操作系统内置的经典衬线中文印刷字体。每个全角汉字与全角标点的水平步进宽度严格等于一倍字号（在十一点字号下为十一点零磅），因此在四百一十五点三磅的标准正文字宽内，每行固定容纳三十七个全角字符（合计四百零七磅）。
]

= 2. Heiti SC（华文黑体 / macOS 无衬线黑体，11pt）

#text(font: "Heiti SC", size: 11pt)[
  华文黑体是苹果操作系统广泛使用的无衬线黑体字族。在文档的字体槽位映射中，通过同时写入西文与东亚字体属性，确保中英文字符均使用目标字体渲染，不会无故回退到默认的等线或宋体。
]

= 3. Kaiti SC（华文楷体，11pt）

#text(font: "Kaiti SC", size: 11pt)[
  华文楷体常用于公文引用、定理叙述、诗词引文与图注说明。楷体的全角字宽同样遵循一倍字号标准网格，但其字面视觉重心更为舒展，可用于检验不同东亚字族的行高与字形度量精度。
]

= 4. 西文字体优先 + 中文字体回退（Times New Roman + Songti SC）

#text(font: ("Times New Roman", "Songti SC"), size: 11pt)[
  当字体列表配置为西文字体优先加中文字体回退时，连续的中文段落自动使用华文宋体渲染，而拉丁字母与阿拉伯数字则使用西文衬线字体，从而验证双字体槽位的精确对应。
]
