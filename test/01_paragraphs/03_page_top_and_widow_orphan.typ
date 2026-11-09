// Test 01_paragraphs/03_page_top_and_widow_orphan.typ
// Covers: multi-page flow, page-top spacing suppression, heading keep-with-next,
// and widow/orphan control across page boundaries.

#set page(
  paper: "a4",
  margin: (top: 2.54cm, bottom: 2.54cm, left: 3.18cm, right: 3.18cm),
)
#set text(font: "Times New Roman", size: 11pt, lang: "en", hyphenate: false)
#show raw: set text(font: "Courier New", size: 9pt)
#set par(
  justify: true,
  linebreaks: "simple",
  justification-limits: (spacing: (min: 100%, max: 500%)),
  leading: 0.65em,
  spacing: 1.2em,
)

= 1. Multi-Page Vertical Flow and Page Break Behavior

Paragraph 1: When a document spans multiple pages, the vertical position of the first line on Page 1 and the first line on Page 2 depends on how the layout engine handles top-of-page spacing. In Microsoft Word, `w:before` spacing at the very top of the first page is suppressed unless explicitly forced, whereas after a soft page break Word suppresses `w:before` by default in modern compatibility modes (`compatSetting` version 15).

Paragraph 2: To test multi-page transitions deterministically, we populate the first page with carefully measured paragraphs that approach the bottom margin at 29.7cm minus 2.54cm equals 27.16cm. Each paragraph contains enough lines to exercise both intra-paragraph line wrapping and inter-paragraph spacing accumulation.

Paragraph 3: In typographic layout systems, an orphan is the first line of a paragraph stranded alone at the bottom of a page, and a widow is the last line of a paragraph stranded alone at the top of the next page. Both Word (`w:widowControl`) and Typst enforce a minimum of two lines on either side of a page break when widow/orphan protection is active.

Paragraph 4: Furthermore, section headings carry the `keepWithNext` property so that a heading is never left isolated at the bottom of a page without at least the first two lines of its following body paragraph.

Paragraph 5: This fifth paragraph adds additional vertical height to ensure we approach the lower region of Page 1. Every word and punctuation mark is chosen to keep line widths deterministic under 11pt Times New Roman metrics.

Paragraph 6: Continuing down Page 1, we observe that cumulative baseline positions remain tightly synchronized when line heights and inter-paragraph gaps match to within a twentieth of a point (`1 dxa`).

#pagebreak()

= 2. Explicit Page Break Top-of-Page Alignment

Paragraph 7 (Top of Page 2): Immediately following an explicit `#pagebreak()`, this heading and paragraph begin at the exact top margin of Page 2 (y = 2.54cm = 72pt). Comparing the first baseline on Page 2 between Typst and Word verifies that explicit page breaks do not introduce spurious vertical offsets.

Paragraph 8: This final paragraph on Page 2 completes the multi-page vertical rhythm verification.
