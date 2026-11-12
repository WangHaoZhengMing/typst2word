// @func: page
// @docs: https://typst.app/docs/reference/layout/page/#parameters-header

// === @case page_header_footer_fields ===
// @title: Page Header & Dynamic Page-Number Footer
// @param: header + footer
// @var: page(header, footer, counter(page))
// @raw_page: 340x160
// @margins: 28, 10, 28, 10
// @ir: Paragraph
// @xml: headerReference | footerReference
// @tol: 4.0, 12.0, 16.0, 20.0
#set page(
  width: 340pt,
  height: 160pt,
  margin: (x: 10pt, y: 28pt),
  header: align(right)[#text(font: "Times New Roman", size: 9pt)[Lab Header]],
  footer: align(center)[#text(font: "Times New Roman", size: 9pt)[#context counter(page).display("1 / 1", both: true)]],
)
#set text(font: "Times New Roman", size: 11pt)
Body content between header and footer.
