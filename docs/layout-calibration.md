# Layout calibration

The DOCX generator emits paragraph/table/section properties. Microsoft Word
performs the final line breaking and pagination. `break_paragraph_with_policy`
and `paginate_blocks` are diagnostic predictors; passing their tests does not
establish that Word renders the same layout.

## Page margins

`word_construct::page_calibration::PageMarginCalibration` owns the historical
page-margin adjustment. `LegacyWord` retains the previous default: subtract
3.5 pt from the top (5 pt for a first CJK block), subtract 6 pt from the bottom,
and clamp both to 18 pt. These are empirical legacy values, not a general model
of arbitrary fonts, sizes, or Word versions. The original code did not record a
Word build or a complete calibration dataset, so none is claimed here.

Use `docx_writer::write_docx_to_bytes_with_options` and `DocxWriteOptions` with
`PageMarginCalibration::Preserve` to export an unadjusted measurement baseline.
Both policies use the active section's margins. Other historical paragraph,
equation, and table adjustments remain in their respective construct solvers;
they should be migrated only with evidence for their particular object type.

## How to validate a future adjustment

1. Fix the input documents, exact font files, Typst version, OS, and Word build.
   Include Latin/CJK/mixed text, 9/11/16/24 pt, a heading/image/table as the first
   object, explicit small margins, and multiple sections.
2. Export the same IR with both margin policies. Save each DOCX as PDF in Word.
3. Run `test/tools/pdf_inspector reference.typst.pdf candidate.word.pdf` (add
   `--overlay comparison.pdf` to inspect the differences).
4. Compare page count, text/line completeness, baseline deltas, and ink overlap.
   Reject adjustments that improve one sample while silently losing content or
   making unrelated fonts/sizes worse. Record per-case tolerances and results.
5. Keep structural regression tests in `cargo test --workspace`; these do not
   replace real Word/PDF validation. The existing Python comparison runner
   produces measurements, not a pass/fail guarantee of visual parity.

Do not fit a new constant based on one document. Prefer actual font metrics and
an object-specific cause when the collected measurements support that model.
