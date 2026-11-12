// @func: quote
// @docs: https://typst.app/docs/reference/model/quote/

// === @case quote_plain ===
// @title: Block Quote: Without Attribution
// @param: block: true
// @var: quote(block: true)
// @size: 340x120
// @ir: Quote
#set text(font: "Times New Roman", size: 11pt)
#quote(block: true)[Simplicity is prerequisite for reliability.]

// === @case quote_attribution ===
// @title: Block Quote: With Attribution
// @param: attribution
// @var: quote(block: true, attribution: [...])
// @size: 340x140
// @ir: Quote
// @xml: Dijkstra
#set text(font: "Times New Roman", size: 11pt)
#quote(block: true, attribution: [Edsger W. Dijkstra])[Simplicity is prerequisite for reliability.]
