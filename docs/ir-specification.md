# Intermediate Representation (`DocumentIr`) Specification

`crates/ir` defines the target-independent Intermediate Representation produced by `extract` and consumed immutably by `word-construct`. The schema is serializable to JSON via `--dump-ir <ir.json>` (excluding binary embedded font payloads).

---

## 1. Top-Level Structure (`DocumentIr`)

```rust
pub struct DocumentIr {
    pub sections: Vec<DocumentSectionIr>,          // Sections 2..N (first section in page_setup + blocks)
    pub metadata: MetadataIr,                      // title, authors, description, keywords, locale, dir
    pub page_setup: PageSetupIr,                   // First section page geometry, margins, headers/footers
    pub blocks: Vec<BlockNode>,                    // First section top-level block flow
    pub report: ConversionReport,                  // Counts, compiler warnings, font notices, unsupported list
    pub font_metrics: BTreeMap<String, FontFaceMetricsIr>,
    pub math_font_family: Option<String>,          // Resolved OpenType MATH family (None -> Cambria Math)
    #[serde(skip)]
    pub embedded_fonts: Vec<EmbeddedFontFaceIr>,   // Standalone SFNT faces for .odttf packaging
    pub origins: BTreeMap<NodeId, NodeOrigin>,
    pub reference_layout: BTreeMap<NodeId, NodeReferenceLayout>,
    pub anchors: Vec<AnchorLineIr>,                // Read-only PDF line anchors for verification
}
```

### Page & Section Geometry (`PageSetupIr`)

Each section carries `width_pt`, `height_pt`, per-side margins (`margin_top_pt`, `margin_bottom_pt`, `margin_left_pt`, `margin_right_pt`), `header_margin_pt`, `footer_margin_pt`, `mirror_margins` (two-sided inside/outside binding), `columns`, optional page background `fill`, vertical alignment (`v_align`), default/first-page/even-page headers and footers (`different_first_page`, `different_odd_even`), and page numbering (`page_number_format`, `page_number_start`).

---

## 2. Block-Level Nodes (`BlockNode`)

| Variant | Key Fields | Purpose |
| :--- | :--- | :--- |
| `Title` | `id`, `content`, `align`, `bookmark` | Document title (`#title[...]`). |
| `Heading` | `id`, `level` (1..6), `content`, `align`, `numbering`, `bookmark` | Semantic heading mapped to Word `Heading 1..6`. |
| `Paragraph` | `id`, `content`, `style: ParagraphStyleIr`, `bookmark` | Reflowable text & inline-object paragraph. |
| `List` | `id`, `items: Vec<ListItemIr>`, `tight`, `level`, `marker` | Unordered bullet list (supports arbitrary nesting). |
| `Enum` | `id`, `items: Vec<EnumItemIr>`, `tight`, `level`, `start`, `numbering` | Ordered numbered list with explicit start index and format. |
| `Terms` | `id`, `items: Vec<TermItemIr>`, `tight` | Definition / term list (`/ Term: Description`). |
| `Quote` | `id`, `body`, `attribution`, `block` | Block quotation (`#quote(block: true)`). |
| `CodeBlock` | `id`, `lang`, `lines: Vec<Vec<TextRunIr>>`, `fill`, `stroke`, `inset`, `breakable` | Syntax-highlighted fenced code block with token colors. |
| `Container` | `ContainerIr` (`id`, `style`, `width_pt`, `height_pt`, `align`, `breakable`, `sticky`, `body`) | Styled `#block(...)` / `#rect(...)` container lowered to a borderless/bordered Word table or shape. |
| `Table` | `TableIr` (`id`, `kind`, `col_widths_pt`, `row_heights_pt`, `header_rows`, `footer_rows`, `rows`) | Semantic `TableKindIr::SemanticTable`, `LayoutGrid`, or `StyledContainer` with `colspan`/`rowspan`, borders, fills, and insets. |
| `Figure` | `id`, `kind: FigureKindIr`, `body`, `caption`, `caption_position`, `bookmark` | Captioned figure (`Image`, `Table`, `Code`, or `Custom`). |
| `EquationBlock` | `EquationIr` | Display equation (centered or 3-column numbered display row). |
| `ShapeBlock` | `ShapeIr` | Block-level geometric primitive (`Rect`, `Ellipse`, `Line`, `Polygon`, `CustomPath`). |
| `CetzCanvas` | `CetzCanvasIr` | CeTZ vector canvas with paths, arrowheads, fills, and text/math labels. |
| `ImageBlock` | `ImageIr` | Block-level raster or vector image (`Png`, `Jpeg`, `Gif`, `Svg`). |
| `Outline` | `title`, `entries: Vec<OutlineEntryIr>` | Table of contents (`#outline()`). |
| `Bibliography` | `title`, `entries: Vec<BibEntryIr>` | Rendered bibliography entries (`#bibliography(...)`). |
| `HorizontalRule` | `id`, `length_pt`, `stroke`, `align`, `left_indent_pt` | Horizontal divider (`#line(length: ...)`), participating in `flow_spacing` via `id`. |
| `Columns` | `id`, `count`, `gutter_pt`, `body` | Scoped multi-column flow (`#columns(N)[...]`). |
| `PageBreak` / `ColumnBreak` | — | Explicit page or column break. |
| `UnsupportedBlock` | `UnsupportedIr` | Visible placeholder marker for unsupported primitives. |

---

## 3. Inline-Level Nodes (`InlineNode`)

| Variant | Description |
| :--- | :--- |
| `Text(TextRunIr)` | Styled text run: `text`, `font_family`, `font_size_pt`, `bold`, `italic`, `underline`, `strike`, `overline`, `color`, `highlight`, `border`, `script` (`Normal` / `Sub` / `Super`), `smallcaps`, `tracking_pt`, `baseline_shift_pt`, `text_stroke`, `locale`, `dir`. |
| `EquationInline(EquationIr)` | Inline OMML equation. |
| `ShapeInline(ShapeIr)` | Inline DrawingML shape or `#box(...)` pill with baseline offset. |
| `CetzInline(CetzCanvasIr)` | Inline CeTZ vector group. |
| `ImageInline(ImageIr)` | Inline image. |
| `Link { target, content }` | External URL (`LinkTargetIr::Url`) or internal bookmark (`LinkTargetIr::Anchor`). |
| `Ref { target, content }` | Cross-reference (`@label`). |
| `Citation { key, content }` | Bibliography citation. |
| `Footnote { id, body }` | Footnote anchor referencing `word/footnotes.xml` body blocks. |
| `Field { kind, content }` | Dynamic field (`FieldKindIr::PageNumber`, `TotalPages`, `PageRef`, `Custom`). |
| `HorizontalSpace { width_pt, is_fractional }` | Explicit `#h(...)` horizontal space or fractional flush-right tab (`1fr`). |
| `LineBreak` / `Tab` | Soft line break (`\`) or tab character. |
| `UnsupportedInline(UnsupportedIr)` | Inline fallback badge. |

---

## 4. Mathematics AST (`EquationIr` & `MathNodeIr`)

`EquationIr` stores `id`, `block: bool`, `body: Vec<MathNodeIr>`, `numbering_text`, `bookmark`, `font_size_pt`, `color`, and `source_text`.

`MathNodeIr` models structured mathematical expressions rather than flat glyph strings:
- `Run(MathRunIr)` (text, `MathScriptIr`: `DoubleStruck` / `Script` / `Fraktur` / `Monospace` / `SansSerif`, `MathStyleIr`: `Plain` / `Bold` / `Italic` / `BoldItalic`, font family, size, color)
- `Sequence(Vec<MathNodeIr>)`
- `Fraction { num, denom, style: MathFracStyleIr }` (`Vertical`, `Skewed`, `Horizontal`, `NoBar`)
- `Radical { degree, radicand }`
- `SubSup { base, sub, sup }` and `PreSubSup { base, sub, sup }`
- `Limit { base, lower, upper }`
- `Nary { chr, limits_under_over, sub, sup, operand }`
- `Delimited { open, close, separator, elements }`
- `Matrix { rows, left_aligned }` (also represents piecewise `cases`)
- `EquationArray { rows }` and `AlignPoint`
- `Accent { chr, base }`, `Bar { pos_bottom, body }`, `GroupChar { chr, is_bottom, body, annotation }`
- `Cancel { body, cross, inverted }`

---

## 5. Provenance & Reference Layout

- **`NodeOrigin`** (`crates/ir/src/origin.rs`): Records `source_file`, `source_range`, `source_line_col`, `typst_location` (hex hash of Typst's `Location`), `semantic_parent: Option<NodeId>`, and `SemanticRoleIr`.
- **`NodeReferenceLayout`** (`crates/ir/src/reference_layout.rs`): Read-only Typst layout measurements:
  - `fragments: Vec<LayoutFragmentIr>` (`page_idx`, `x_pt`, `top_y_pt`, `width_pt`, `height_pt`)
  - `span: BlockLayoutSpanIr` (`Unmeasured`, `SinglePage`, or `MultiPage`)
  - `flow: Option<FlowReferenceIr>` (measured sibling gaps and trailing transitions)
  - `paragraph_lines: Option<ParagraphLineMetricsIr>` (visual line count, baseline coordinates, line widths)
  - `equation: Option<EquationLayoutMetricsIr>` (rendered Typst equation height & width for 2-DoF OMML box calibration)

---

## 6. Structural Normalization (`normalize_document_ir`)

Executed once at the end of `extract`:
1. **`canonicalize_bookmark_anchors`**: Deduplicates and normalizes internal bookmark target IDs.
2. **`clean_redundant_pagebreaks`**: Removes trailing or duplicate page breaks that would create blank Word pages.
3. **`coalesce_adjacent_text_runs`**: Merges neighboring `InlineNode::Text` runs with identical formatting properties.
4. **`merge_split_containers`**: Merges adjacent `Container` and `CodeBlock` fragments split across page boundaries—matching by `NodeOrigin.typst_location` first, and falling back to page-adjacent continuity (`start_page == end_page + 1`, matching `fill`/`stroke`, and absent continuation `typst_location`/`lang`) when Typst omits `Tag::Start` on continuation pages.
