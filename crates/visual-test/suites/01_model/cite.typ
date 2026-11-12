// @func: cite
// @docs: https://typst.app/docs/reference/model/cite/

// === @case cite_prose_supplement ===
// @title: Prose Citation with Page Supplement
// @param: form + supplement
// @var: cite(<knuth84>, form: "prose", supplement: [p. 99])
// @size: 340x150
// @ir: Paragraph, Bibliography
// @xml: Knuth
#set text(font: "Times New Roman", size: 11pt)
According to #cite(<knuth84>, form: "prose", supplement: [p. 99]), clarity comes first.
#bibliography("refs.bib", title: none)
