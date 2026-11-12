// @func: text
// @docs: https://typst.app/docs/reference/text/text/#parameters-cjk-latin-spacing

// === @case par_cjk_kinsoku ===
// @title: CJK Kinsoku (避头尾 Punctuation Rules)
// @param: lang: zh (kinsoku)
// @var: CJK forbidden line-start/line-end punctuation
// @size: 240x140
// @ir: Paragraph
#set text(font: "Songti SC", size: 12pt, lang: "zh")
#set par(justify: true)
测试中文排版避头尾规则，确保句号、逗号、感叹号不会出现在行首，并且左括号（如“引号”与《书名号》）不会孤立在行尾。

// === @case par_cjk_latin_mixed ===
// @title: CJK + Latin Mixed Kerning & Wrapping
// @param: cjk-latin-spacing
// @var: CJK + Western mixed tokens
// @size: 300x140
// @ir: Paragraph
#set text(font: "Songti SC", size: 11pt, lang: "zh")
#set par(justify: true)
本项目通过 Spring Boot 集成 MyBatis-Plus 与 Redis 缓存组件，在 macOS 平台上验证 OpenXML 排版一致性。
