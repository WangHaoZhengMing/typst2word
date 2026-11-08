---
name: compare-diff-and-fix
description: Pixel-level visual overlay comparison between original Typst (.typ / .pdf) and converted Microsoft Word (.docx) documents on macOS, plus iterative layout/semantic fixing workflow. Use when comparing Typst vs Word rendering, debugging spacing/margins/line-breaking/tables/equations/shapes, or verifying visual consistency.
---

# Typst vs. Microsoft Word Pixel-Level Visual Comparison & Fix Skill (`compare-diff-and-fix`)

本 Skill 用于将原 Typst 代码（`.typ`）、转换后的 Word 文档（`.docx`）以及排版结果进行**像素级叠图对比（Overlay Diff）**，并根据对比结果定位、修复 `typst2word` 中的排版与语义映射问题。

---

## 1. 核心原理（Glyph-Ink Coordinate Diff + Paper-Matched Background Overlay）

1. **定位或生成产物**：
   - 找到原 Typst 源码（如 `examples/showcase.typ`）、编译后的 `.docx` 与 `.pdf`；若尚未编译或代码已更新，通过 `typst2word` 重新生成 `.docx`（限制 2 个 CPU 线程）。
   - 生成 `.docx` 时**默认同时导出中间表示 IR**（`typst2word <in.typ> <out.docx> --dump-ir <stem>.ir.json`），输出到 `<out-dir>/<stem>.ir.json`，用于判断问题出在“Typst → IR 提取”还是“IR → Word 写出”。
2. **Word 真实排版导出（AppleScript + Microsoft Word）**：
   - 在 macOS 上通过 `osascript` 调用本地 **Microsoft Word** 打开 `.docx` 并导出为等比例纸张大小的 `<stem>.word.pdf`（脚本会自动通过 `~/Library/Containers/com.microsoft.Word/Data/Documents/` 沙盒目录中转，避免 macOS 沙盒权限错误 `-1708`）。
3. **逐行/分栏真实字形墨迹坐标对比（Swift + PDFKit + CoreGraphics Bitmap Ink Bounds）**：
   - 使用 `scripts/pdf_layout_diff.swift` 对 `<stem>.orig.pdf`（Typst 原生 PDF）与 `<stem>.word.pdf`（Word 导出 PDF）同时进行 `PDFKit` 文本提取与 `144 DPI` 栅格化**真实字形墨迹边界（Glyph Ink Bounds）**测量，输出 `<out-dir>/layout_diff.txt`。
   - **为什么结合 PDFKit 与真实墨迹边界（`inkTop..inkBottom, inkLeft..inkRight`）最精确**：
     - 单纯使用 `PDFSelection.bounds` 会受到 Typst 与 Word 导出 PDF 时字体 `/FontBBox`（ascent/descent 行高框）差异的干扰（例如 `12pt` 思源黑体在 Typst PDF 中行框高 `12.0pt`，而在 Word PDF 中行框高 `16.4pt`，即使两者的字形像素完全重合也会报假偏移）。
     - `pdf_layout_diff.swift` 先通过 `PDFKit` 提取每行及水平分栏（如 `#grid` 的 `A. / B. / C. / D.` 选项或双栏页面）的文本，再在页面位图上收缩出**非白像素的真实字形墨迹边界**（精度 `0.5pt`），自动匹配并计算出每一项的 `dY = word.inkTop - orig.inkTop`、`dX = word.inkLeft - orig.inkLeft`、`dW = word.inkWidth - orig.inkWidth`。
     - 无需每次打开多张 PNG 大图肉眼比对，直接阅读 `layout_diff.txt` 即可在毫秒级定位到具体哪一行、哪一栏偏移了多少 `pt`（以及是否发生了跨页 `PAGE->P*` 或断行差异）。
4. **等尺寸页面栅格化与明度/色调调节（Swift + PDFKit/CoreGraphics）**：
   - 使用 `scripts/pdf_to_images.swift` 将 Word 导出的每一页 PDF 按原纸张 `MediaBox` 尺寸（1:1 比例）导出为高清 PNG（可通过 `--coords-only` 跳过以加速迭代）：
     - `word_page_raw_<N>.png`：Word 原始页面截图
     - `word_page_bg_<N>.png`：调低明度（默认 `--opacity 0.42`）并叠加浅珊瑚红色调（可用 `--no-tint` 切换纯灰度降明度）的背景图。
5. **Typst 100% 页面背景叠图（`#set page(background: ...)`）**：
   - 自动生成 `overlay_compare.typ`，在原 `.typ` 的 `#set page(...)` 之后注入：
     ```typst
     #set page(background: context {
       let p = here().page()
       let bg_pages = ("word_page_bg_1.png", "word_page_bg_2.png", ...)
       if p >= 1 and p <= bg_pages.len() {
         image(bg_pages.at(p - 1), width: 100%, height: 100%)
       }
     })
     ```
   - 编译生成 `overlay_compare.pdf` 与逐页 `overlay_page_<N>.png`：
     - **前景深黑色** = Typst 原生排版基准
     - **背景浅珊瑚红/浅灰** = Microsoft Word 实际渲染结果
     - 两者完全重合处即为像素级一致；任何偏移（页边距、行距、段距、断行、表格高度、公式与图形位置）都会直观显示为重影偏移。

---

## 2. 目录结构

```text
.claude/skills/compare-diff-and-fix/
├── SKILL.md                     # 标准 Skill 说明文件
└── scripts/
    ├── visual_compare.py        # 自动化编译 + Word 导出 + 坐标对比 + Typst 背景叠图主脚本
    ├── pdf_layout_diff.swift    # PDFKit + 真实字形墨迹坐标 (ink bounds) 提取与逐项对比脚本
    └── pdf_to_images.swift      # PDFKit 逐页渲染与明度/色调处理脚本
```

---

## 3. 使用方法

### 3.1 快速坐标对比模式（推荐日常迭代调试使用，`--coords-only`）

只生成 `.docx`、`.ir.json`、`.orig.pdf`、`.word.pdf` 和精确坐标对比报告 `layout_diff.txt`，跳过耗时的高清叠图 PNG 渲染：

```bash
python3 .claude/skills/compare-diff-and-fix/scripts/visual_compare.py examples/showcase.typ \
  --out-dir examples/visual_diff \
  --coords-only \
  --max-pages 5
```

或者对已有的 `<orig.pdf>` 与 `<word.pdf>` 单独运行坐标对比脚本：

```bash
swift .claude/skills/compare-diff-and-fix/scripts/pdf_layout_diff.swift \
  examples/visual_diff/showcase.orig.pdf \
  examples/visual_diff/showcase.word.pdf \
  examples/visual_diff/layout_diff.txt \
  5
```

### 3.2 完整生成坐标对比 + 像素级叠图 PNG

```bash
python3 .claude/skills/compare-diff-and-fix/scripts/visual_compare.py examples/showcase.typ --out-dir examples/visual_diff
```

该命令会生成 `examples/visual_diff/showcase.docx`、IR 文件 `examples/visual_diff/showcase.ir.json`、坐标对比表 `examples/visual_diff/layout_diff.txt` 以及逐页叠图 `overlay_page_*.png`。

### 3.3 对已有的 `.docx` 直接进行对比（跳过 Cargo 编译）

> 使用 `--docx` 或 `--skip-build` 时不会运行转换，因此**不会生成/更新 IR**。如需 IR，请去掉这两个参数重新转换，或手动执行：
>
> ```bash
> RAYON_NUM_THREADS=2 cargo run -j 2 -- examples/showcase.typ examples/visual_diff/showcase.docx \
>   --dump-ir examples/visual_diff/showcase.ir.json
> ```

```bash
python3 .claude/skills/compare-diff-and-fix/scripts/visual_compare.py examples/showcase.typ \
  --docx examples/showcase.docx \
  --out-dir examples/visual_diff
```

### 3.4 常用参数

| 参数 | 默认值 | 说明 |
| :--- | :--- | :--- |
| `input_typ` | 必填 | 原始 Typst 源码路径（如 `examples/showcase.typ`） |
| `--docx <path>` | 自动生成 | 指定已编译好的 `.docx` 文件路径 |
| `--out-dir <dir>` | `examples/visual_diff` | 输出目录（包含 `layout_diff.txt`、`overlay_page_*.png` 等） |
| `--coords-only` | `false` | 仅导出 PDF 并计算精确坐标对比表 `layout_diff.txt`，跳过叠图 PNG 渲染 |
| `--max-pages <int>` | `0`（全部） | 限制坐标对比报告仅分析前 N 页 |
| `--opacity <float>` | `0.42` | Word 背景图的明度/不透明度（`0.0 ~ 1.0`） |
| `--no-tint` | `false` | 使用纯灰度降低明度（默认使用浅珊瑚红以便与黑色前景区分） |
| `--scale <float>` | `2.0` | 导出分辨率倍率（`2.0` 对应 `144 DPI`） |
| `--skip-build` | `false` | 跳过 `cargo run` 转换步骤（此时不生成 IR） |
| `--ir <path>` | `<out-dir>/<stem>.ir.json` | IR（中间表示 JSON）输出路径 |
| `--no-ir` | `false` | 转换时不导出 IR |

---

## 4. 坐标与视觉诊断迭代修复工作流（Compare -> Diagnose -> Fix -> Commit）

1. **优先读取 `layout_diff.txt` 查看精确坐标偏差**：
   - 运行 `visual_compare.py`（可加 `--coords-only` 快速迭代）后，先查看终端输出的 `PAGE N SUMMARY` 以及 `<out-dir>/layout_diff.txt`：
     - `dY = word.inkTop - orig.inkTop`（正值表示 Word 偏下，负值表示 Word 偏上）
     - `dX = word.inkLeft - orig.inkLeft`（正值表示 Word 偏右，负值表示 Word 偏左）
     - `dW = word.inkWidth - orig.inkWidth`（宽度差）
     - 若某一段连续多行 `dY` 逐行累积增大/减小（如每行偏 `-1.5pt`），说明**行距（`w:line`）**或**列表项间距**偏小/偏大；若从某标题或表格后整体跳变一个常数 `dY`，说明该元素的**段前/段后间距（`w:before` / `w:after`）**不准。
   - 当涉及复杂图形（CeTZ 画布、原生形状）或需要最终确认视觉效果时，再读取 `overlay_page_<N>.png` 叠图验证。
2. **先查 IR，判断问题在哪一层**：
   - 打开 `examples/visual_diff/<stem>.ir.json`，找到有差异的元素（如 `Heading`、`Paragraph.style`、`Table.col_widths_pt`、`ShapeBlock`、`EquationBlock.omml_xml`），核对其中的尺寸、对齐、间距、样式等数值是否与 Typst 一致：
     - **IR 数值就不对** → 问题在提取层，修改 `src/extract/`（对应元素在 `src/extract/elements/<元素>.rs`）。
     - **IR 正确但 Word 显示不对** → 问题在写出层，修改 `src/docx/`（对应元素在 `src/docx/blocks/<元素>.rs` 或 `src/docx/inlines/`）。
   - 修复后重新运行脚本，对比前后两次的 IR（例如 `diff` 两份 `.ir.json`）确认改动只影响目标元素。
3. **按模块定位差异并修改对应源码**：
   - **页面尺寸与页边距 / 页眉页脚距离**：
     - 检查 [`src/extract/document.rs`](../../src/extract/document.rs) 与 [`src/docx/parts/document.rs`](../../src/docx/parts/document.rs) 中的 `PageElem::margin`、`w:pgMar`（`w:top` / `w:bottom` / `w:left` / `w:right` / `w:header` / `w:footer`）。
   - **段落间距、行距与首行/悬挂缩进**：
     - 检查 [`src/extract/elements/paragraph.rs`](../../src/extract/elements/paragraph.rs) 与 [`src/docx/blocks/paragraph.rs`](../../src/docx/blocks/paragraph.rs)。
     - 注意 Typst 的 `ParElem::leading` 是行间空隙（inter-line gap），转换为 Word `<w:spacing w:line="..." w:lineRule="auto"/>` 时需加上 `1em` 字高并扣除字体自带行距（`line_mult = (font_size_pt + leading_pt) / (font_size_pt * 1.15)`）。
   - **中西文混排断行与等宽字体宽度**：
     - 检查 [`src/docx/parts/settings.rs`](../../src/docx/parts/settings.rs)（`compressPunctuation`、`autoSpaceDE`、`autoSpaceDN`）与 [`src/docx/inlines/text.rs`](../../src/docx/inlines/text.rs)（macOS 上等宽字体使用 `Menlo` 匹配 Typst 的 `DejaVu Sans Mono` 字符宽度 `0.602em`）。
   - **表格行高与跨页控制**：
     - 检查 [`src/docx/blocks/table.rs`](../../src/docx/blocks/table.rs)（扣除 `<w:tcMar>` 上下内边距并设置 `<w:cantSplit/>`）。
   - **数学公式（OMML）与块级公式右对齐编号**：
     - 检查 [`src/math/`](../../src/math/)、[`src/extract/elements/equation.rs`](../../src/extract/elements/equation.rs) 与 [`src/docx/blocks/equation.rs`](../../src/docx/blocks/equation.rs)（带编号公式使用 `[12% | 76% | 12%]` 三列无边框布局表以保持 `<m:oMathPara>` 完整 Display 模式且编号垂直居中右对齐）。
   - **原生图形（DrawingML `<wps:wsp>`）与 CeTZ 画布（`<wpg:wgp>`）**：
     - 检查 [`src/extract/elements/shape.rs`](../../src/extract/elements/shape.rs)、[`src/docx/drawingml/shape.rs`](../../src/docx/drawingml/shape.rs) 与 [`src/docx/drawingml/cetz.rs`](../../src/docx/drawingml/cetz.rs)。
4. **验证与汇报（重要约束）**：
   - 编译和测试时**必须限制 CPU 数量为 2**：
     ```bash
     RAYON_NUM_THREADS=2 cargo test -j 2 -- --test-threads=2
     ```
   - 再次运行 `visual_compare.py` 确认叠图对齐。
   - **每修完一个地方及时告知用户修改点与对比结果**，方便用户随时执行 `git commit`。
