// @func: image
// @docs: https://typst.app/docs/reference/visualize/image/

// === @case image_block_png ===
// @title: Standalone Raster Image Block (PNG)
// @param: width: 84pt
// @var: image("probe.png", width: 84pt)
// @size: 240x130
// @ir: ImageBlock
// @xml: <pic:pic
#image("probe.png", width: 84pt)

// === @case image_inline_png ===
// @title: Inline Raster Image Run (#box(image))
// @param: inline box(image)
// @var: box(image("probe.png", width: 14pt))
// @size: 280x100
// @ir: Paragraph
// @xml: <pic:pic
#set text(font: "Times New Roman", size: 12pt)
Badge #box(image("probe.png", width: 14pt)) inline.
