// @func: bibliography
// @docs: https://typst.app/docs/reference/model/bibliography/

// === @case bibliography_cite ===
// @title: Citation & Bibliography (#bibliography)
// @param: sources
// @var: @knuth84 + bibliography("refs.bib")
// @size: 340x180
// @ir: Paragraph, Bibliography
#set text(font: "Times New Roman", size: 11pt)
Literate programming was introduced by Knuth @knuth84.
#bibliography("refs.bib")
