// ============================================================================
// Microsoft Word 排版算法逆向与 word-construct 数学模型全解 (Apple Design Style)
// 位置：/Users/ole/Code/typst2word/word_layout_algorithms.typ
// ============================================================================

#let apple_blue    = rgb("#007AFF")
#let table_blue    = rgb("#3478F6")
#let apple_green   = rgb("#34C759")
#let apple_orange  = rgb("#FF9500")
#let apple_red     = rgb("#FF3B30")
#let apple_purple  = rgb("#5856D6")
#let light_gray    = rgb("#F5F5F7")
#let card_border   = rgb("#E5E5EA")
#let dark_gray     = rgb("#86868B")
#let text_primary  = rgb("#1D1D1F")
#let divider_color = rgb("#D1D1D6")

#set page(
  paper: "a4",
  margin: (x: 2.2cm, y: 2.4cm),
  numbering: "1",
  header: context {
    let current = counter(page).at(here()).first()
    if current > 1 {
      grid(
        columns: (1fr, auto),
        align(left)[#text(size: 9pt, fill: dark_gray, weight: "medium")[Microsoft Word Layout Engine & PTLS7 Reverse Engineering]],
        align(right)[#text(size: 9pt, fill: apple_blue, weight: "semibold")[word-construct Architecture Guide]],
      )
      v(-4pt)
      line(length: 100%, stroke: 0.6pt + divider_color)
    }
  },
)

#set text(
  font: ("PingFang SC", "Songti SC"),
  size: 10.5pt,
  fill: text_primary,
  lang: "zh",
)

#set par(
  justify: true,
  leading: 0.72em,
  spacing: 1.15em,
)

#show raw.where(block: false): box.with(
  fill: rgb("#F0F2F5"),
  inset: (x: 4pt, y: 0pt),
  outset: (y: 2.5pt),
  radius: 3pt,
)
#show raw: set text(font: ("Menlo", "PingFang SC"), size: 9.2pt)

#show heading: it => {
  set par(justify: false)
  if it.level == 1 {
    v(0.8em)
    block(
      width: 100%,
      inset: (bottom: 6pt),
      stroke: (bottom: 1.2pt + divider_color),
      text(size: 20pt, weight: "bold", fill: text_primary, it.body),
    )
    v(0.4em)
  } else if it.level == 2 {
    v(0.6em)
    text(size: 14.5pt, weight: "bold", fill: text_primary, it.body)
    v(0.25em)
  } else if it.level == 3 {
    v(0.4em)
    text(size: 12pt, weight: "semibold", fill: apple_blue, it.body)
    v(0.15em)
  } else {
    text(size: 10.5pt, weight: "semibold", fill: text_primary, it.body)
  }
}

// ----------------------------------------------------------------------------
// Apple Developer Documentation 风格组件库
// ----------------------------------------------------------------------------

#let hero_card(title, subtitle, tags: ()) = {
  block(
    width: 100%,
    inset: (x: 18pt, y: 16pt),
    fill: light_gray,
    stroke: 1pt + card_border,
    radius: 12pt,
    [
      #text(size: 9pt, weight: "bold", fill: apple_blue)[APPLE ENGINEERING TECH NOTE · WORD-CONSTRUCT]
      #v(4pt)
      #text(size: 24pt, weight: "bold", fill: text_primary)[#title]
      #v(6pt)
      #text(size: 11.5pt, fill: dark_gray)[#subtitle]
      #if tags.len() > 0 {
        v(10pt)
        line(length: 100%, stroke: 0.5pt + divider_color)
        v(6pt)
        for (i, t) in tags.enumerate() {
          if i > 0 { text(fill: divider_color)[ #h(4pt) | #h(4pt) ] }
          box(
            fill: white,
            stroke: 0.6pt + card_border,
            inset: (x: 6pt, y: 3pt),
            radius: 4pt,
            text(size: 8.5pt, weight: "medium", fill: text_primary)[#t],
          )
        }
      }
    ],
  )
  v(0.8em)
}

#let concept_box(title, body) = {
  block(
    width: 100%,
    breakable: false,
    inset: (x: 14pt, y: 12pt),
    fill: light_gray,
    stroke: 0.8pt + card_border,
    radius: 10pt,
    [
      #text(weight: "bold", size: 12pt, fill: text_primary)[#title]
      #v(4pt)
      #body
    ],
  )
}

#let callout_box(title: "Note", accent: apple_blue, bg: rgb("#E8F4F9"), body) = {
  block(
    width: 100%,
    breakable: false,
    inset: (x: 14pt, y: 10pt),
    fill: bg,
    stroke: (left: 3.5pt + accent),
    radius: (right: 8pt, left: 2pt),
    [
      #text(weight: "bold", size: 10.5pt, fill: accent.darken(15%))[#title]
      #v(3pt)
      #body
    ],
  )
}

#let tip_box(title: "Tip", body) = callout_box(
  title: title,
  accent: apple_blue,
  bg: rgb("#EBF5FF"),
  body,
)

#let warning_box(title: "Warning", body) = callout_box(
  title: title,
  accent: apple_orange,
  bg: rgb("#FFF6E8"),
  body,
)

#let important_box(title: "Important", body) = callout_box(
  title: title,
  accent: apple_red,
  bg: rgb("#FEF1F1"),
  body,
)

#let code_card(title: none, body) = {
  block(
    width: 100%,
    fill: rgb("#F6F8FA"),
    stroke: 0.8pt + rgb("#E1E4E8"),
    radius: 8pt,
    inset: (x: 12pt, y: 10pt),
    [
      #if title != none [
        #text(size: 8.5pt, weight: "bold", fill: dark_gray)[#title]
        #v(4pt)
        #line(length: 100%, stroke: 0.5pt + rgb("#E1E4E8"))
        #v(4pt)
      ]
      #body
    ],
  )
}

// ============================================================================
// 正文开始
// ============================================================================

#hero_card(
  "Microsoft Word 排版引擎逆向与算法全解",
  "从 MicrosoftPTLS7 段落断行截断、OpenType 'DFLT' 脚本覆写，到行盒度量、孤行寡行控制与公式校准的完整数学与工程模型",
  tags: (
    "Microsoft Word 16.x (x86_64)",
    "MicrosoftPTLS7.framework",
    "Songti SC 截断逆向",
    "First-Fit + Kinsoku",
    "Knuth-Plass 对照",
    "crates/word-construct",
  ),
)

#concept_box("文档导读与核心目标")[
  本文档系统性总结了我们对 *Microsoft Word* 主程序（`Microsoft Word.app/Contents/MacOS/Microsoft Word`）与微软核心排版引擎 *`MicrosoftPTLS7.framework`*（Page & Text Layout Services 7）的二进制逆向工程结论，以及基于探针文档（`word-rules` P0–P10、`word-rules2` Q1–Q7）还原的全部排版规则。所有规则均已在工作区的 `crates/word-construct` 中以纯 Rust 强类型枚举与链式调用架构落地实现，并在测试集中通过真实 Word 导出 PDF 的逐行校验。
]

= 一、 核心逆向实证：为什么 `Songti SC` 在 Word 中发生异常断行与数字腰斩截断？

== 1.1 四大诡异视觉现象还原

将 `examples/Typst/Note/香港.typ` 的正文字体设为 macOS 系统自带的 `Songti SC`（`11pt`，A4 纸左右页边距 `2.5cm`，正文栏宽 $W = 453.54 "pt"$）并在 Microsoft Word 中打开时，文档同时出现以下 4 类严重排版异常：

#grid(
  columns: (1fr, 1fr),
  gutter: 10pt,
  important_box(title: "现象 1：行首出现避头尾禁则标点")[
    - 第 1 段第 3 行以全角逗号开头：`，一起向大陆模式转型`
    - 第 2 段第 5 行以顿号开头：`、对欧美以及对香港自己来说`
    - 第 3 段第 2 行以逗号开头：`，今日来说其实已经非常小了`
    Word 的中文避头尾（Kinsoku Shori）在 `Songti SC` 下完全失效。
  ],
  important_box(title: "现象 2：遇到空格或破折号立刻提前换行")[
    - 第 4 段第 4 行在 `"非常普通的 seaside "` 后直接换行，整行仅宽 $90.2 "pt"$，右侧留出 $363 "pt"$ 空白；
    - 第 8 段第 1 行在 `"问题还不止这些，香港剩下的最后那条就业通道——"`（仅 23 字，$253 "pt"$）后直接提前换行。
  ],
  important_box(title: "现象 3：多位阿拉伯数字在行尾被硬生生劈开")[
    - 第 5 段行尾将年份 `2022` 截断为行尾 `20` 与下一行行首 `22年香港股市...`（屏幕视图下 `2020` 亦被截断为 `202` / `0`）；
    - 第 8 段行尾将 `3120亿美元` 截断为行尾 `312` 与下一行行首 `0亿美元`。
  ],
  important_box(title: "现象 4：中英自动间距与标点压缩失效")[
    - Word 默认开启的 `<w:autoSpaceDE/>`（中日韩与西文自动留白）、`<w:autoSpaceDN/>`（中日韩与数字自动留白）以及标点宽度压缩在 `Songti SC` 段落中全部不生效。
  ],
)

#v(4pt)
#tip_box(title: "一语道破根因")[
  以上 4 个现象并非 4 个独立的 Bug，而是*同一个底层机制的必然结果*：Word 在 OpenType 字体塑形校验阶段，将 `Songti SC` 的内部文本流（`plsrun`）脚本标识强制覆写成了 *`script_id = 5`（`Latin` 西文）* 与 *`ot_script_tag = 'DFLT'`*。这导致 `MicrosoftPTLS7` 把整段除 ASCII 空格和破折号之外的所有中文字符、全角标点、阿拉伯数字全部当成*同一个长达数百字符、不可拆分的拉丁单词（Breaking Class `0`）*！
]

== 1.2 `Microsoft Word` + `MicrosoftPTLS7` 五步反汇编调用链证据

我们通过对 `Microsoft Word`（`x86_64` slice）的 `0x1004431fc` $->$ `0x10044ac52` 塑形管线及 `MicrosoftPTLS7.framework` 进行反汇编，完整还原了触发该覆写的 5 个步骤：

=== Step 1：乐观开启 OpenType 塑形标志（`0x1004435e8..0x1004435f1`）

在函数 `0x1004431fc` 中，Word 调用 `0x100447869`（内部调用 `0x100447898` 检查 `0x78(%legacy_chp) == 0`，对所有普通非公式文本恒返回 `1`）。随后在 `0x1004435f1` 处，Word *乐观地将 OpenType 启用标志 `0x13(%r12)`（`fUseOpenType`）置为 `1`*，而此时代表“当前字体是否支持该脚本 OpenType 特性”的标志位 `0x2a(%r15)`（`fFontSupportsRunScript`）仍为初始值 `0`：

#code_card(title: "Microsoft Word (x86_64) @ 0x1004435e8..0x1004435f5")[
```asm
00000001004435e8: callq  0x100447869              ; 普通文本恒返回 al = 1
00000001004435ed: testb  %al, %al
00000001004435ef: je     0x1004436ae
00000001004435f1: movb   $0x1, 0x13(%r12)         ; fUseOpenType = 1 (乐观置位), 此时 0x2a(%r15) == 0
```
]

=== Step 2：跳过 OpenType 表回退清除 + AAT 字体无 `GSUB`/`GPOS`（`0x1004437f4..0x100443803`）

- 在 `0x1004437f4` 处，Word 调用 `0x100447c05(%rbx)` 检查当前 `CHP`（字符属性）是否显式开启了 OpenType 字距调整（`0x7a(%rbx)`）或连字（`0x80(%rbx)`），并检查 `-0x2a(%rbp)`（由 `0x103ebb56c` 写入的 `(font_face->flags_0xd4 >> 12) & 1`）。
- 当未显式请求 OpenType 连字/字距（或 `flags_0xd4` 第 12 位为 `1`）时，`0x100443803` 的 `jne 0x1004436ae` *直接跳过了 `0x103ebd8c1`*（该函数原本负责在字体不支持 OpenType 时把 `0x13(%r12)` 清零回退），导致 `0x13(%r12)` 保持为 `1`、而 `0x2a(%r15)` 保持为 `0`！
- 更关键的是，即便在 `settings.xml` 中写入 `compatibilityMode = 15` 强制进入 `0x103ebd8c1`，macOS 自带的 `Songti SC`（位于 `/System/Library/Fonts/Supplemental/Songti.ttc`）是一个*纯 Apple AAT（Apple Advanced Typography）字体*——它只包含苹果专有的 `feat` 与 `morx` 变形表，*完全没有 OpenType 的 `GSUB` 和 `GPOS` 表*！因此 OpenType `'hani'` 脚本校验必然失败，`0x2a(%r15)` 依然为 `0`。

=== Step 3：东亚字体分类掩码检查失败（`0x100443791..0x1004437a7` $->$ `0x103ebda81`）

对于东亚文字 run（`0x1a(%r15) == 1`），Word 在 `0x100443791` 提供了最后一次将 `0x2a(%r15)` 救回 `1` 的机会——调用 `0x103ebda81(%r14, %r15)`：

#code_card(title: "Microsoft Word (x86_64) @ 0x100443791..0x1004437a7 & 0x103ebda81")[
```asm
; --- 调用处 (0x100443791) ---
0000000100443791: cmpb   $0x1, 0x1a(%r15)         ; 是否为 EastAsian 文本 run?
0000000100443796: jne    0x1004437c0
0000000100443798: movq   %r14, %rdi
000000010044379b: movq   %r15, %rsi
000000010044379e: callq  0x103ebda81              ; 检查东亚字体子类型 0x64(%r15)
00000001004437a3: testb  %al, %al
00000001004437a5: jne    0x1004437c0              ; 若返回 1 则跳过!
00000001004437a7: movb   $0x1, 0x2a(%r15)         ; 仅当返回 0 时才置 fFontSupportsRunScript = 1

; --- 被调函数 (0x103ebda81) ---
0000000103ebda81: movl   0x64(%rsi), %eax         ; 读取字体子类型 (Songti SC 为 -1 / 0xffffffff)
0000000103ebda84: incl   %eax                     ; ecx = eax + 1
0000000103ebda86: cmpl   $0x6, %eax
0000000103ebda89: ja     0x103ebda96
0000000103ebda8b: movl   $0x47, %edx              ; 位掩码 0b0100_0111 (bit 0,1,2,6 为 1; bit 3,4,5 为 0)
0000000103ebda90: btl    %eax, %edx               ; 测试第 (0x64(%rsi) + 1) 位
0000000103ebda93: setb   %al                      ; 若为 -1, 0, 1, 5 则返回 al = 1 (失败)
```
]

在 `0x103ebb71b..0x103ebb738` 中，`"Songti SC"` 的 `0x64(%r15)` 被初始化为 `-1`（`0xffffffff`）。传入 `0x103ebda81` 后，`eax + 1 = 0`，命中 `0x47` 的第 0 位，返回 `al = 1`，直接跳过 `0x1004437a7`，导致 `0x2a(%r15)` 最终定格为 `0`（即使在 `.docx` 中注入声明 `<w:charset w:val="86"/>` 的 `word/fontTable.xml` 也无法改变 CoreText 字体对象的 `flags_0xd4`）。

=== Step 4：灾难性的 `'DFLT'` / `script_id = 5`（`Latin`）覆写（`0x10044afdc..0x10044affd`）

进入 `0x10044ac52` 后，由于 `0x13(%r12) == 1`（启用 OpenType）且 `0x2a(%r15) == 0`（字体不支持当前东亚脚本的 OpenType 表），Word 走入 `0x10044afdc` 分支，*将该文本流 `plsrun` 的脚本 ID 强行改写为 `5`（`Latin` 西文），并将 OpenType 脚本标签改写为 `'DFLT'`（`0x44464c54`）*：

#code_card(title: "Microsoft Word (x86_64) @ 0x10044afdc..0x10044affd")[
```asm
000000010044afdc: movb   $0x1, 0x105(%rbx)
000000010044afe3: movb   $0x1, 0x106(%rbx)
000000010044afea: movw   $0x5, 0x19e(%rbx)        ; plsrun->script_id = 5 (Latin 西文脚本!)
000000010044aff3: movl   $0x44464c54, 0x1a4(%rbx) ; plsrun->ot_script_tag = 'DFLT' (0x44464c54)
```
]

=== Step 5：`MicrosoftPTLS7` 22×22 断行对表坍缩与紧急词内截断（`0x1d24e0`）

当 `plsrun->script_id` 被改为 `5`（`Latin`）后，Word 向 `MicrosoftPTLS7.framework` 注册的 `GetBreakingClasses` 回调（`0x10048be5a` $->$ `0x100445724`）不再按东亚字符分类，而是：
1. 将所有汉字、全角标点（`，`、`、`、`。`、`“ `、` ”`）与阿拉伯数字（`0..9`）全部归为 *Class `0`（`Ptls7BreakClass::LatinWord`）*；
2. 将空格（`U+0020`）归为 *Class `4`（`Ptls7BreakClass::Whitespace`）*；
3. 将连字符与破折号（`-`、`–`、`—` `U+2014`）归为 *Class `5`（`Ptls7BreakClass::Hyphen`）*。

查询 `MicrosoftPTLS7` 位于偏移 `0x1d24e0` 的 22×22 断行对表（Pair Table）：

#align(center)[
  #table(
    columns: (25%, 14%, 25%, 36%),
    inset: 7pt,
    align: (left, center, center, left),
    fill: (x, y) => if y == 0 { light_gray } else { white },
    stroke: 0.6pt + card_border,
    [*前驱字符 $->$ 后继字符*], [*PTLS7 类别对*], [*查表动作 (`0x1d24e0`)*], [*在 `Songti SC` 段落中的实际表现*],
    [汉字/标点/数字 $->$ 汉字/标点/数字], [`(0, 0)`], [`Prohibited` \ （禁止断行）], [整段无空格中文与数字粘连成一个数百字的超长“单词”],
    [空格 `U+0020` $->$ 任意非空字符], [`(4, 0)`], [`DirectBreak` \ （允许断行）], [`"非常普通的 seaside "` 后遇到长中文串立即提前换行],
    [破折号 `—` $->$ 破折号 `—`], [`(5, 5)`], [`Prohibited` \ （禁止断行）], [双破折号 `——` 中间绝不断开],
    [破折号 `—` $->$ 汉字/数字], [`(5, 0)`], [`DirectBreak` \ （允许断行）], [`"...最后那条就业通道——"`（23 字）后立即提前换行],
    [行首 `x = 0` 单词超出列宽 $W$], [`ForcedBreak`], [`EmergencyMidWord` \ `Truncation`], [在 $<= 453.54 "pt"$ 的最后一个字符处硬切（无视避头尾、劈开数字）],
  )
]

== 1.3 `Songti SC` 在 CoreText 与 Word 下的精确字形推进宽度表

为什么第 1 段第 2 行恰好在 41 个汉字处截断，而第 5 段第 9 行恰好把 `2022` 劈成 `20` 和 `22`？我们直接调用 macOS `CoreText` 测量了 `/System/Library/Fonts/Supplemental/Songti.ttc`（`UPEM = 1000`）在 $S = 11.0 "pt"$ 下的精确字形推进宽度（Advance Width），并与 Word 导出 PDF 的内容流坐标进行了比对：

#align(center)[
  #table(
    columns: (23%, 15%, 13%, 15%, 34%),
    inset: 7pt,
    align: (left, center, right, right, left),
    fill: (x, y) => if y == 0 { light_gray } else { white },
    stroke: 0.6pt + card_border,
    [*字符类别*], [*示例字符*], [*UPEM 宽度*], [*11pt 推进宽度*], [*行尾截断数学验算（栏宽 $W = 453.54 "pt"$）*],
    [中日韩汉字 / 全角标点 / 破折号], [`香`, `，`, `。`, `—`], [`1000`], [$11.000 "pt"$], [$41 times 11.0 = 451.0 "pt" <= 453.54 "pt"$（第 42 字 $462 "pt"$ 溢出）],
    [阿拉伯数字 `0`–`9`], [`2`, `0`, `3`, `8`], [`469`], [$5.159 "pt"$], [40 汉字 ($440 "pt"$) + `"20"` ($10.32 "pt"$) = $450.32 "pt"$，第 3 个数字 `"2"` ($455.48 "pt"$) 溢出 $->$ `20` | `22`],
    [西文半角空格 `U+0020`], [`' '`], [`250`], [$2.750 "pt"$], [提供合法 `Whitespace` 断点],
    [中西弯引号 `U+201C` / `U+201D`], [`“`, `”`], [`389 / 412`], [$approx 4.40 "pt"$], [`'DFLT'` 模式下使用西文比例引号宽度而非全角 $11 "pt"$],
    [英文大写字母 `A`–`Z`], [`G`, `D`, `P`], [`703`], [$7.733 "pt"$], [`"GDP"` 宽 $23.20 "pt"$，决定第 3 段第 4 行在 `排在第六位，` 处截断],
    [英文小写字母 `a`–`z`], [`s`, `e`, `a`], [`~430`], [$approx 4.73 "pt"$], [`"seaside"` 宽 $32.4 "pt"$，与前序 5 汉字合计 $90.2 "pt"$],
    [英文句点 / 百分号], [`.`, `%`], [`219 / 823`], [`2.41 / 9.05 pt`], [用于小数 `2.70`、`8.4%`、`20%` 精确行宽累计],
  )
]

基于上述精确规则，`crates/word-construct` 对 `香港.typ`（`Songti SC` `11pt`）全文档 3 页、9 个段落共 *112 行*的断行预测，与真实 Microsoft Word 导出的 PDF 达到了 *112 / 112 行（100.0%）逐字完全一致*。

= 二、 Word 字体度量与行盒（Line Box）空间数学模型

在 Word 中，段落行距 `<w:spacing w:line="W" w:lineRule="auto"/>` 的基准单位是 $1/240$ 行（即 `W = 240` 代表“单倍行距”）。但 *Word 的单倍行距绝不等于 $1.0 times S$（字号）*。

== 2.1 `OS/2` Win 度量与 CJK 代码页 1.3 倍行高放大机制

Word 根据字体的 `OS/2` 表与 `hhea` 表计算单倍行距基准高度 $H_("single")^("Word")(S)$：

1. *常规西文与现代 UI 字体*：读取 `OS/2` 表的 `usWinAscent` 与 `usWinDescent`：
   $ H_("single")^("Word")(S) = ("usWinAscent" + "usWinDescent") / ("unitsPerEm") dot S = M_("win") dot S $
   - 西文比例字体（`Times New Roman`, `Calibri`, `Libertinus Serif`）：$M_("win") approx 1.150$，字形墨水高度比 $M_("ink") approx 0.720$。
   - 现代中日韩 UI 字体（`PingFang SC`, `Heiti SC`）：`unitsPerEm = 1000`，`usWinAscent = 1060`，`usWinDescent = 320`，故 $M_("win") = 1.380$，字形墨水高度比 $M_("ink") approx 0.960$。

2. *声明 CJK 代码页（`CP932` / `CP936` / `CP949` / `CP950`）的传统字体放大 1.3 倍*：
   - 对于声明了东亚代码页的传统衬线字体（如 Windows 的 `SimSun` / `宋体`，`hhea` 高度比为 $1.000$），Word 会在内核度量基础上*额外乘以放大系数 $gamma_("CJK") = 1.3$*（上下平分作为外部行距 External Leading），使得 $12 "pt"$ 宋体的单倍行距等于 $12 times 1.3 = 15.6 "pt"$。
   - 对于 macOS 的 `Songti SC` / `STSong` / `Kaiti SC`（其 `hhea` 度量本身已达 $1.400$），叠加 $gamma_("CJK") = 1.3$ 后，Word 中的单倍行盒高度比高达 $M_("win")^("Songti") = 1.400 times 1.3 = 1.820$（即 $11 "pt"$ 字号下 `w:line="240"` 的行盒高达 $20.02 "pt"$！）。

== 2.2 行盒内部空间分解与 Typst `par.leading` 逆映射公式

在给定字号 $S$ 与 Word 行距参数 $W$（`w:line`）时，`WordLineBox`（位于 `crates/word-construct/src/font_metrics.rs`）的空间几何结构定义为：

$ H_("line")^("Word")(S, W) = M_("win") dot S dot W / 240, wide H_("ink")(S) = M_("ink") dot S $
$ Delta_("internal")(S, W) = max(0, H_("line")^("Word")(S, W) - H_("ink")(S)) = delta_("top") + delta_("bot") $

其中顶部内留白 $delta_("top") = min(0.06 S, 0.25 Delta_("internal"))$，底部内留白 $delta_("bot") = Delta_("internal") - delta_("top")$。

由于 Typst 的 `par(leading: L)` 定义的是*上一行字形墨水底边到下一行字形墨水顶边的净间距 $L$*（即目标行距为 $H_("ink")(S) + L$），为了在 Word 中精确复现 Typst 的行距 $L$，必须求解逆映射 $W$：

$ H_("line")^("Word")(S, W) = H_("ink")^("eff")(S) + L ==> W = "round"( 240 dot (M_("ink")^("eff") + L / S) / M_("win") ) $

- 对 `PingFang SC`（$M_("win") = 1.380, M_("ink")^("eff") = 0.950$），当 $S = 11 "pt", L = 7.15 "pt"$（$L/S = 0.65$）时，$W = "round"(240 times 1.60 / 1.380) = 278$；
- 对 `Songti SC`（$M_("win") = 1.820, M_("ink")^("eff") = 0.720$），同条件下 $W = "round"(240 times 1.37 / 1.820) = 181$，从而抵消 `Songti SC` 的 $1.820 times$ 行盒膨胀，使 Word 每页行数与 Typst PDF 严格对齐。

= 三、 段落内断行算法：Word (`FirstFit` + `Kinsoku`) vs Typst (`Knuth-Plass`)

== 3.1 Word 贪心 First-Fit + 避头尾回溯 + 紧急截断状态机

Word 的段落断行器（`FirstFitBreaker`）采用从左到右的单遍贪心扫描（Tokenization + Greedy First-Fit），并叠加三层约束处理：

#concept_box("Word 段落断行核心状态机（FirstFitBreaker）")[
  设可用行宽为 $W_max$，当前行已累计 token 序列为 $C = [t_1, t_2, ..., t_k]$，当前行宽为 $w(C) = sum_(i=1)^k w(t_i)$。对于下一个输入 token $t$：
  1. *显式硬回车（`MandatoryBreak`）*：立即将 $C$ 封行输出（标记为 `LineBreakCause::MandatoryBreak`，末行不作两端对齐拉伸），清空 $C$。
  2. *行首空白丢弃与行尾空白悬挂（Hanging Whitespace）*：若 $C$ 为空且 $t$ 为空白，直接丢弃；若 $C$ 非空且 $t$ 为空白，允许 $t$ 悬挂追加到 $C$ 末尾而不触发换行（封行时再剥离尾部空白）。
  3. *贪心装填*：若 $w(C) + w(t) <= W_max + epsilon$，将 $t$ 追加到 $C$。
  4. *溢出换行与避头尾回溯（Kinsoku Backtracking，仅对 `TokenKind::CjkChar` 生效）*：
     当 $C$ 非空且 $w(C) + w(t) > W_max + epsilon$ 时，先剥离 $C$ 尾部空白，随后执行两步回溯：
     - _行尾禁则回溯（Line-End Forbidden，Class `6`）_：若 $C$ 的末尾 token 属于行尾禁则集 $K_("end")$（如 `（“《「『【`）且 $|C| >= 2$，循环将 $C$ 末尾弹出并移入下一行缓冲 `carried`；
     - _行首禁则回溯（Line-Start Forbidden，Class `7`）_：若即将成为下一行首字符的 token（`carried[0]` 或 $t$）属于行首禁则集 $K_("start")$（如 `，、。．！？；：）》”`）且 $|C| >= 2$，从 $C$ 末尾再回退 1 个 token 到 `carried`；
     然后将 $C$ 封行输出（`LineBreakCause::NormalWrap`），令 `carried` 成为新行的起始序列。
  5. *行首超长单词紧急字符截断（PTLS7 `ForcedBreak` / `EmergencyMidWordTruncation`）*：
     当新行在加入 $t$ 后仍满足 $w_("cur") + w(t) > W_max + epsilon$ 且 $t$ 为多字符 `TokenKind::Word` 时，按字符推进宽度数组 `char_widths_pt` 循环切出满足 $sum_(j=1)^m w(c_j) <= W_max - w_("cur")$ 的最长前缀（至少 1 个字符），每次切出均封行输出为 `LineBreakCause::EmergencyMidWordTruncation`，直至剩余后缀可放入当前行。
]

== 3.2 Typst 全局最优 `Knuth-Plass` 动态规划算法对比

与 Word 的局部贪心不同，Typst（`par.rs`）及 `KnuthPlassBreaker` 将段落断行建模为有向无环图（DAG）上的最短路径动态规划问题：

- *伸缩比（Adjustment Ratio $r$）*：设候选行 $[i -> j]$ 的自然宽度为 $w_(i,j)$、可拉伸总量为 $S_(i,j)^+$、可压缩总量为 $S_(i,j)^-$，目标行宽为 $W_max$：
  $ r_(i,j) = cases(
    (W_max - w_(i,j)) / S_(i,j)^+ quad & "if " w_(i,j) < W_max,
    (W_max - w_(i,j)) / S_(i,j)^- quad & "if " w_(i,j) > W_max,
    0 quad & "if " w_(i,j) = W_max,
  ) $
- *代价函数与核心参数*：
  $ "cost"(i -> j) = |r_(i,j)|^3 + P_("hyph") + P_("dash") + P_("runt") $
  其中连字符断行代价 `HYPH_COST = 0.5`，连续两行连字符惩罚 `CONSECUTIVE_DASH_COST = 0.3`，末行过短（Runt）惩罚 `RUNT_COST = 0.5`，最小允许压缩比 `MIN_RATIO = -1.0`，超长行惩罚 `MAX_COST = 1_000_000.0`。对长段落（候选断点 $> 64$），先以近似代价剪枝不可行前驱，再精算最优路径。

= 四、 Word 分页、孤行/寡行控制与表格跨页算法（`word-rules` P0–P10 & Q1–Q7）

`crates/word-construct/src/paginator.rs` 完整实现了通过 `docx/layout-probes` 在真实 Word 中实测验证的分页约束体系：

#align(center)[
  #table(
    columns: (22%, 34%, 44%),
    inset: 7.5pt,
    align: (left, left, left),
    fill: (x, y) => if y == 0 { light_gray } else { white },
    stroke: 0.6pt + card_border,
    [*规则编号与名称*], [*`word-construct` 策略枚举*], [*Word 真实排版行为与分页器实现逻辑*],
    [*P0*：页顶段前距抑制], [`SpaceBeforeTopPolicy::` \ `SuppressAtPageTop`], [自然软分页（Soft Page Break）后的新页第一段自动抑制 `space_before`（置为 $0 "pt"$）；文档第 1 页顶或显式硬分页后则按配置保留。],
    [*P1*：相同样式上下文间距], [`ContextualSpacingPolicy::` \ `SuppressBetweenSameStyle`], [当 `<w:contextualSpacing/>` 开启且相邻两段具有相同 `style_id` 时，抑制前一段的 `space_after` 与后一段的 `space_before`。],
    [*P2*：孤行与寡行控制], [`WidowOrphanPolicy::Enabled` \ `{ min_top: 2, min_bottom: 2 }`], [Word 默认开启 `<w:widowControl/>`：若段落跨页后当前页底仅剩 1 行（孤行 Orphan），则将该段整段推至下页；若下页顶仅剩 1 行（寡行 Widow），则从当前页底回退 1 行推入下页，保证页底 $>= 2$ 行、页顶 $>= 2$ 行。],
    [*P3*：段内不拆行], [`KeepLinesPolicy::` \ `KeepTogether`], [对应 `<w:keepLines/>`：若当前页剩余高度不足以放下该段全部行，且当前页非空，则先触发软分页再在下一页顶整体排版。],
    [*P4*：与下段同页传递链], [`KeepWithNextPolicy::` \ `KeepWithNext`], [对应 `<w:keepNext/>`：分页器向后扫描整个 `KeepWithNext` 传递链 `[idx..chain_end]`，计算链上前序块全部高度 + 链尾块至少 `min_bottom` 行的原子高度和；若当前页放不下则整体前移分页（若超过整页高则自动解开避免死循环）。],
    [*P5 / P8*：表格行跨页与重复表头], [`RowBreakPolicy::CantSplit` + \ `RowHeaderRepeatPolicy::` \ `RepeatHeaderAtTop`], [对应 `<w:cantSplit/>` 与 `<w:tblHeader/>`：表头行保存在 `active_headers` 中，当表格数据行跨入新页时，自动在新页顶部克隆插入表头行并扣除表头高度。],
  )
]

= 五、 Word OMML 数学公式双自由度（2-DOF）空间校准模型

在 Word 的 Office MathML（OMML）引擎中，行内公式（Inline Equation）内的分式 `<m:f>`、上下标、求和号上下限会被 Word 自动缩小约 $0.73 times$。若直接把 Typst 布局树中已缩小的分式字号（如 $12 "pt" times 0.7 = 8.4 "pt"$）写入 `<w:sz>`，Word 会在此基础上*二次缩小*至 $8.4 times 0.73 approx 6.1 "pt"$。

`WordEquationBox::solve`（位于 `crates/word-construct/src/equation_box.rs`）通过两个正交自由度（2-DOF）精确校准公式盒尺寸：
1. *外层公式基准字号 `run_font_size_pt`*：
   - 若行内公式中所有字形均位于分式或上下标内部（`all_glyphs_in_fraction_or_script == true`）且实测字形约为上下文正文字号 $S_("ctx")$ 的 $0.60 tilde.op 0.82$ 倍，则自动恢复外层字号为 $S_("ctx")$（消除二次缩小）；
   - 若用户在 `#text(15pt)[$2024-2025$]` 中显式放大了行内公式，则忠实保留 $15 "pt"$（`w:sz="30"`）。
2. *行内 `math.display(...)` 分式反向补偿 `fraction_font_size_pt`*：
   - 当用户在行内公式中使用 `$ display(a/b) $` 时，Typst 以 $100%$ 正文字号（如 $12 "pt"$）排版分子分母，但 Word 仍因处于行内模式将其缩小 $0.73 times$。此时模型计算反向补偿字号 $S_("frac") = "round_half"(S_("base") / 0.73) = 16.5 "pt"$，写入 `<m:f>` 内部 run，使 Word 渲染出的分子分母视觉高度严格等于 $12 "pt"$。

= 六、 PDF 行级锚点（`AnchorLineIr`）LCS 校验器与代码架构总表

== 6.1 为什么 PDF 行首尾锚点只能做“校验器”而不能做“生成器”？

#warning_box(title: "架构铁律：AnchorIR 与生成逻辑彻底解耦")[
  如果记录 PDF 每行开头和末尾的词并强行在那里插入换行符，等于把“结果对不对”当成了“过程怎么走”，一旦用户在 Word 中增删一个字或微调页边距，硬编码断点就会导致全篇排版崩坏。
  因此，在 `crates/word-construct` 中：
  - *构造者（Generator）完全不读取 `AnchorLineIr`*，只依靠上述逆向出的字体度量、PTLS7 断行规则与分页约束驱动生成；
  - *校验器（`compare_line_anchors`）* 从 PDF 提取每行的 `(page, line_index, first_token, last_token, x_start_pt, baseline_y_pt, width_pt)`，使用*最长公共子序列（LCS）*算法将构造者输出行与 Golden PDF 行配对，自动诊断是 `TokenBoundaryMismatch`（断行参数偏差）、`BaselineDrift`（行距/段距偏差）还是 `MissingInCandidate` / `ExtraInCandidate`。
]

== 6.2 `crates/word-construct` 模块与核心类型速查表

#align(center)[
  #table(
    columns: (24%, 34%, 42%),
    inset: 7pt,
    align: (left, left, left),
    fill: (x, y) => if y == 0 { light_gray } else { white },
    stroke: 0.6pt + card_border,
    [*模块文件路径*], [*核心类型 / 枚举 / 函数*], [*实现的 Word 排版算法与职责*],
    [`src/linebreak/ptls7.rs`], [`WordScriptOverridePolicy`, \ `Ptls7BreakClass`, \ `Ptls7PairAction`, \ `LineBreakCause`], [逆向复刻 Word `0x10044afdc` `'DFLT'` (`script_id = 5`) 覆写、PTLS7 `0x1d24e0` 22×22 断行对表与 AAT 字体自动检测。],
    [`src/linebreak/first_fit.rs`], [`FirstFitBreaker::` \ `break_tokens`, \ `push_with_emergency_` \ `truncation`], [Word 贪心 First-Fit 断行、避头尾（Kinsoku）双向回溯、以及行首超长词 `ForcedBreak` 逐字符紧急截断。],
    [`src/linebreak/knuth_plass.rs`], [`KnuthPlassBreaker`], [Typst 动态规划最优断行（`HYPH_COST`, `RUNT_COST`, `CONSECUTIVE_DASH_COST`）与两阶段近似剪枝。],
    [`src/linebreak/mod.rs`], [`InlineSpan`, `InlineToken`, \ `BrokenLine`, \ `tokenize_spans_with_` \ `script_policy`], [支持 `StandardEastAsian` 与 `OpenTypeDfltLatinOverride` 的分词器及 `split_at_width_limit` 字符级切分。],
    [`src/font_metrics.rs`], [`WordFontClass`, `WordLineBox`, \ `WordFontMetricsProvider`, \ `WORD_CJK_FACTOR`], [`OS/2` Win 度量、CJK 代码页 $1.3 times$ 放大、`Songti SC` CoreText 精确字宽比例与 `w:line` 逆映射。],
    [`src/paginator.rs`], [`PageConstraint`, \ `LayoutParagraph`, \ `paginate_blocks`], [`word-rules` P0–P10 分页引擎：页顶段前距抑制、上下文间距、孤行寡行回溯、`keepLines`、`keepNext` 链、跨页表头重复。],
    [`src/policy.rs`], [`LineBreakPolicy`, `KinsokuSet`, \ `WidowOrphanPolicy`, \ `ParagraphLayoutConstraint`], [全枚举化策略定义与流畅链式调用（Fluent Builder API）。],
    [`src/equation_box.rs`], [`WordEquationBox`, \ `EquationBoxInput`], [OMML 公式双自由度（外层字号 + 行内展示分式 $1/0.73$ 反向补偿）校准器。],
    [`src/paragraph_box.rs` & \ `src/list_box.rs`], [`WordParagraphConstruct`, \ `WordListItemLayout`], [跨字号段间间距补偿与列表悬挂缩进（`671 dxa` / `300 dxa`）空间求解。],
    [`src/anchor.rs`], [`AnchorLineIr`, \ `AnchorDiffReport`, \ `compare_line_anchors`], [基于 LCS 的只读 PDF 行锚点配对与基线/断行奇偶性校验器。],
  )
]
