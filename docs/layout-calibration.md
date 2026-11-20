# Layout Calibration & Word Modeling

`docx-writer` emits semantic WordprocessingML, OMML, and DrawingML properties, while Microsoft Word performs final line breaking and pagination. `crates/word-construct` bridges the metric and behavioral gaps between Typst's layout engine and Microsoft Word (`MicrosoftPTLS7` / `compatibilityMode=15`).

> **Note on predictors**: `word_construct::linebreak::break_paragraph_with_policy` and `word_construct::paginator::paginate_blocks` are diagnostic predictors. Passing unit tests on those models does not by itself establish visual parity in Microsoft Word; real Word-to-PDF visual validation is always required.

---

## 1. Line-Box Metrics & CJK `1.3×` Expansion (`WordLineBox`)

Typst calculates paragraph baseline step from `OS/2.sCapHeight + par.leading` (or the font's ascender/cap box). Microsoft Word calculates single spacing (`w:line="240"`) from `hhea` / `OS/2` (`usWinAscent + usWinDescent`) and applies an additional **`1.3×` line-box expansion (`WORD_CJK_FACTOR`)** to fonts declaring East Asian codepages (`CP932`, `CP936`, `CP949`, `CP950` in `OS/2.ulCodePageRange1` bits `17..=20`, or CJK `cmap` coverage on legacy Apple AAT fonts such as `Hei.ttf`).

For example, macOS `PingFang SC` has `hhea = 1.400`, which expands under Word's CJK layout to `1.400 × 1.3 = 1.820` em (`20.02 pt` at `11 pt`). `WordLineBox::from_paragraph_metrics` reads the extracted binary `FontFaceMetricsIr` and Typst's measured `ParagraphLineMetricsIr` baseline step to solve exact `w:line` (`1/240` line units).

### Mixed Font Sizes (`PTLS7` Tallest-Run Denominator)

In Word's `<w:spacing w:line="..." w:lineRule="auto"/>` model, a line's `1.0×` height is governed by the **tallest** normal-script font box on the line—not the character-count mode. For example, a `11 pt` Chinese paragraph containing many `8.8 pt` (`0.8 em`) inline `raw` code spans still uses `11 pt` CJK as its `1.0×` single-line height in Word. `word_construct::objects::paragraph::tallest_line_box_run` selects the maximum single-line box height across all normal-script inline runs as the `w:line` denominator, preventing inflated spacing on mixed-size paragraphs.

---

## 2. Knuth-Plass to Greedy Line-Break Compensation

Typst uses Knuth-Plass global paragraph line breaking, whereas Microsoft Word breaks lines greedily (`PTLS7`) and can spill a trailing word onto an extra line. Comparing against Typst's laid-out line anchors, `word-construct` applies sub-point `<w:spacing>` character-glue adjustments on compressed justified lines so Word matches Typst's line count **without inserting hard `<w:br/>` breaks**, keeping paragraphs fully reflowable during editing.

---

## 3. Inter-Block Spacing (`compatibilityMode=15` & `flow_spacing`)

Under Word `compatibilityMode=15`:
- Adjacent paragraph margins collapse via `max(w:after, w:before)`.
- Margins do **not** collapse across `<w:tbl>` boundaries (used by `Table`, `Container`, and boxed `CodeBlock`).
- Display equations (`<m:oMathPara>`) inject default `<m:preSp>` / `<m:postSp>` padding unless zeroed in `word/settings.xml`.

`word_construct::pipeline::flow_spacing::solve_container_and_figure_flow_spacing` resolves `before_dxa` and `after_dxa` into `WordDocument::solved_nodes` directly from measured Typst `NodeReferenceLayout` vertical gaps for `Paragraph`, `Heading`, `Container`, `Table`, `Figure`, `ShapeBlock`, `EquationBlock`, and `HorizontalRule` (`#line`).

---

## 4. DrawingML Shape & Grid Calibration

- **Rounded Rectangles**: Word adds `<wps:bodyPr>` insets on top of the ECMA-376 `roundRect` inscribed text rectangle $R(1 - 1/\sqrt{2})$. `word_construct::objects::shape::solve_shape_geometry_with_baselines` subtracts this inscribed inset and half-stroke width, solves `tIns` from measured local baselines, and sets `bIns="0"` under `anchor="t"` so descenders are not clipped.
- **Centered Container Insets**: When all body text blocks inside a `#rect` / `#block` container are horizontally centered (`HAlignIr::Center`), visual `min_x` reflects centering whitespace `(width - text_w) / 2` rather than box padding. `crates/extract/src/elements/container.rs` caps inferred geometric horizontal padding to `<= 2.0 pt` for centered containers so `<wps:bodyPr lIns/rIns>` never forces premature wrapping or clipping.
- **Grid Track Widths**: `crates/extract/src/elements/table/sizing.rs` preserves Typst's `Sizing::Fr` / `Sizing::Auto` track widths whenever any column is centered or right-aligned, avoiding `col_min_x` ink-offset skewing.

---

## 5. Page Margin Calibration (`PageMarginCalibration`)

`word_construct::page_calibration::PageMarginCalibration` owns the historical page-margin adjustment:

- **`LegacyWord`** *(default)*: Subtracts `3.5 pt` from top margin (`5.0 pt` when the first block is CJK), subtracts `6.0 pt` from bottom margin, and clamps both to `18 pt`. These are empirical legacy values rather than a universal model across all fonts and Word builds.
- **`Preserve`**: Exports unadjusted section margins directly as a clean measurement baseline via `docx_writer::write_docx_to_bytes_with_options` and `DocxWriteOptions`.

### Validation Protocol for Future Calibration Changes

1. Fix the input corpus, exact font binaries, Typst version, OS, and Word build (covering Latin, CJK, and mixed text at 9/11/16/24 pt, heading/image/table first blocks, small margins, and multi-section documents).
2. Export the same `DocumentIr` under both `Preserve` and candidate policies, then render each `.docx` to PDF via Microsoft Word.
3. Run `visual-test-cli` (or `pdf_lab_inspector`) to compare page count, text completeness, baseline deltas, and ink IoU overlap.
4. Reject adjustments that improve one document while clipping content or regressing unrelated fonts or sizes. Prefer OpenType binary metrics and object-specific causes over global magic constants.
