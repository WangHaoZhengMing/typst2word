---
name: compare-diff-and-fix
description: Pixel-level visual overlay comparison between original Typst (.typ / .pdf) and converted Microsoft Word (.docx) documents on macOS, plus iterative layout/semantic fixing workflow. Use when comparing Typst vs Word rendering, debugging spacing/margins/line-breaking/tables/equations/shapes, or verifying visual consistency.
---

# Typst vs. Microsoft Word Pixel-Level Visual Comparison & Fix Skill (`compare-diff-and-fix`)

本 Skill 用于将原 Typst 代码（`.typ`）、转换后的 Word 文档（`.docx`）以及排版结果进行**像素级叠图对比（Overlay Diff）**，并根据对比结果定位、修复 `typst2word` 中的排版与语义映射问题。

---

## 1. 核心原理（Paper-Matched Background Overlay）

1. **定位或生成产物**：
   - 找到原 Typst 源码（如 `examples/showcase.typ`）、编译后的 `.docx` 与 `.pdf`；若尚未编译或代码已更新，通过 `typst2word` 重新生成 `.docx`（限制 2 个 CPU 线程）。
   - 生成 `.docx` 时**默认同时导出中间表示 IR**（`typst2word <in.typ> <out.docx> --dump-ir <stem>.ir.json`），输出到 `<out-dir>/<stem>.ir.json`，用于判断问题出在“Typst → IR 提取”还是“IR → Word 写出”。
2. **Word 真实排版导出（AppleScript + Microsoft Word）**：
   - 在 macOS 上通过 `osascript` 调用本地 **Microsoft Word** 打开 `.docx` 并导出为等比例纸张大小的 `<stem>.word.pdf`（脚本会自动通过 `~/Library/Containers/com.microsoft.Word/Data/Documents/` 沙盒目录中转，避免 macOS 沙盒权限错误 `-1708`）。
3. **等尺寸页面栅格化与明度/色调调节（Swift + PDFKit/CoreGraphics）**：
   - 使用 `scripts/pdf_to_images.swift` 将 Word 导出的每一页 PDF 按原纸张 `MediaBox` 尺寸（1:1 比例）导出为高清 PNG：
     - `word_page_raw_<N>.png`：Word 原始页面截图
     - `word_page_bg_<N>.png`：调低明度（默认 `--opacity 0.42`）并叠加浅珊瑚红色调（可用 `--no-tint` 切换纯灰度降明度）的背景图。
4. **Typst 100% 页面背景叠图（`#set page(background: ...)`）**：
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
    ├── visual_compare.py        # 自动化编译 + Word 导出 + Typst 背景叠图主脚本
    └── pdf_to_images.swift      # PDFKit 逐页渲染与明度/色调处理脚本
```

---

## 3. 使用方法

### 3.1 一键重编译并生成像素级叠图对比（默认同时生成 IR）

```bash
python3 .claude/skills/compare-diff-and-fix/scripts/visual_compare.py examples/showcase.typ --out-dir examples/visual_diff
```

该命令会生成 `examples/visual_diff/showcase.docx` 以及对应的 IR 文件 `examples/visual_diff/showcase.ir.json`。

### 3.2 对已有的 `.docx` 直接进行对比（跳过 Cargo 编译）

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

### 3.3 常用参数

| 参数 | 默认值 | 说明 |
| :--- | :--- | :--- |
| `input_typ` | 必填 | 原始 Typst 源码路径（如 `examples/showcase.typ`） |
| `--docx <path>` | 自动生成 | 指定已编译好的 `.docx` 文件路径 |
| `--out-dir <dir>` | `examples/visual_diff` | 输出目录（包含 `overlay_page_*.png`、`overlay_compare.pdf` 等） |
| `--opacity <float>` | `0.42` | Word 背景图的明度/不透明度（`0.0 ~ 1.0`） |
| `--no-tint` | `false` | 使用纯灰度降低明度（默认使用浅珊瑚红以便与黑色前景区分） |
| `--scale <float>` | `2.0` | 导出分辨率倍率（`2.0` 对应 `144 DPI`） |
| `--skip-build` | `false` | 跳过 `cargo run` 转换步骤（此时不生成 IR） |
| `--ir <path>` | `<out-dir>/<stem>.ir.json` | IR（中间表示 JSON）输出路径 |
| `--no-ir` | `false` | 转换时不导出 IR |

---

## 4. 视觉诊断与迭代修复工作流（Compare -> Diagnose -> Fix -> Commit）

1. **查看叠图结果**：
   - 运行 `visual_compare.py` 后，直接读取 `examples/visual_diff/overlay_page_1.png`、`overlay_page_2.png`、`overlay_page_3.png` 检查每一页的像素对齐情况。
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
