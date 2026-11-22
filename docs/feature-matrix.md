# Feature Coverage & Known Limitations

This matrix summarizes the mapping from Typst 0.15 constructs to native Microsoft Word (`.docx`) OpenXML representations.

---

## 1. Supported Features

| Category | Supported Typst Features | Word (`.docx`) Representation |
| :--- | :--- | :--- |
| **Document Structure** | Multi-level headings (`heading` with auto/custom numbering), paragraphs (`par` alignment, first-line & hanging indents, leading & spacing), bullet/numbered/term lists (`list`, `enum`, `terms` with nesting), blockquotes (`quote`), horizontal rules (`line` with `#v(...)` flow spacing), footnotes (`footnote`) | Native `Heading 1..6` styles, multi-level `<w:numPr>` numbering definitions, dynamic `word/footnotes.xml`, bottom-bordered rule paragraphs |
| **Outlines, Citations & Links** | Table of contents (`outline`), BibTeX / Hayagriva bibliographies (`bibliography`, `cite`), cross-references (`@label`, `ref`), internal & external hyperlinks (`link`) | Dynamic Word `TOC` and `PAGEREF` fields, canonicalized bookmarks (`w:bookmarkStart`), native relationship hyperlinks |
| **Tables & Layout Grids** | Fixed, fractional (`1fr`), percentage, and `auto` column widths; `colspan` & `rowspan` merging; repeating headers (`table.header`); per-cell and three-line custom strokes, fills, and insets; horizontal `#stack(dir: ltr/rtl)` and `#grid(...)` layouts; captioned `figure` blocks | Native `<w:tbl>` (`w:gridSpan`, `w:vMerge`, `w:tblHeader`, per-edge border conflict resolution) and borderless `LayoutGrid` tables |
| **Code Blocks** | Inline `raw` and fenced code blocks with Typst syntax-highlighted token colors, monospace metrics, background fills, borders, and seamless cross-page continuation merging | Styled inline runs; breakable shaded/bordered code paragraphs or single-cell container tables |
| **Typography & CJK** | Font families, weights, italics, sizes, colors, highlights, `sub`/`super` (unscaled base size), `smallcaps`, `underline`/`strike`/`overline`, `tracking`, `baseline` shift, text outline stroke, RTL directionality, CJK Kinsoku rules, and default subset/full `.odttf` font embedding (`.ttf`, CFF `.otf`, `.ttc`) | Slot-routed `w:ascii`/`w:hAnsi`/`w:eastAsia`/`w:cs` fonts, OpenXML run properties, dynamic `word/fontTable.xml`, and obfuscated `word/fonts/*.odttf` |
| **Mathematics (OMML)** | Inline & display equations, numbered display equations, vertical/skewed/linear fractions & binomials (`frac`, `binom`), square & n-th roots (`sqrt`, `root`), sub/superscripts & pre-scripts (`attach`), 19+ big n-ary operators (`sum`, `prod`, `integral`, etc.) with limit placement, scalable delimiters (`lr`, `mid`, `abs`, `norm`, `floor`, `ceil`), matrices & vectors (`mat`, `vec`), piecewise `cases`, multi-line `&`-aligned equations, accents (`hat`, `tilde`, `vec`, `dot`, etc.), over/under bars & braces with annotations, `cancel` strokes, math alphabets (`upright`, `italic`, `bold`, `bb`, `cal`, `frak`, `sans`, `mono`) | **Native editable Office MathML** (`<m:oMath>`, `<m:oMathPara>`, and 3-column borderless numbered display rows) |
| **Page Layout & Sections** | Page dimensions & orientation, uniform/per-side/inside-outside `mirror` margins, page fills, headers & footers (first-page & odd/even differentiation, dynamic page counters), page/column breaks, full-page & scoped multi-column flows (`columns`), styled `block` containers & inline `box` pills / underlined fill-in fields | Multi-section `<w:sectPr>` breaks, `header*.xml` / `footer*.xml` parts, `PAGE`/`NUMPAGES` fields, container tables, and anchored shapes |
| **Shapes, Images & CeTZ** | Native geometric primitives (`rect`, `square`, `circle`, `ellipse`, `line`, `polygon`, Bézier `curve`), `rotate`/`scale`/`move` transforms, `place` positioning, raster/SVG images (`image`), and **CeTZ vector canvases** (paths, arrowheads, fills, text & math labels) | **Native editable Word DrawingML** (`<wps:wsp>`, `<a:custGeom>`, and `<wpg:wgp>` groups—never rasterized bitmaps) |

---

## 2. Known Limitations & Graceful Degradation

- **OMML Engine Constraints**: Multi-anchor column alignment within a single `<m:eqArr>` row, internal matrix augmentation lines / mixed per-column alignment, and automatic line-wrapping of ultra-wide display equations inside narrow table cells are constrained by Microsoft Word's OMML engine.
- **Embedded Math Fonts** — Both embedding modes keep complete OpenType math faces. OMML uses the family resolved by Typst, including when font embedding is disabled.
- **Legacy Word CFF Support**: Embedded OpenType CFF (`.otf`) text fonts render natively in Microsoft 365 / Word for Mac, while legacy Windows Word builds (Office 2010/2013) fall back via `<w:altName>`.
- **Unsupported Primitives**: All Typst scripts, `#show`/`#set` rules, counters, and packages execute inside the embedded Typst 0.15 compiler. Any visual primitive that cannot yet be lowered into native Word objects never crashes or silently vanishes—`typst2word` emits an explicit in-document placeholder badge (`UnsupportedIr`) and reports its source location in the conversion report.
