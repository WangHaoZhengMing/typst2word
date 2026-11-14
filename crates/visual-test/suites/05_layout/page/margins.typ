// @func: page
// @docs: https://typst.app/docs/reference/layout/page/#parameters-margin

// === @case margin_auto_fixed_size ===
// @title: Margin: auto on Fixed 210x300pt Page (2.5/21 * min = 25pt)
// @param: margin: auto (fixed)
// @var: page(width: 210pt, height: 300pt, margin: auto) -> 25pt
// @raw_page: 210x300
// @margins: 25, 25, 25, 25
// @mirror_margins: false
// @ir: Paragraph
#set page(width: 210pt, height: 300pt, margin: auto)
#set text(font: "Times New Roman", size: 11pt)
Proportional auto margin (25pt on all four sides).

// === @case margin_auto_height_auto ===
// @title: Margin: auto with height: auto (Falls Back to A4 Width -> 70.87pt)
// @param: margin: auto + height: auto
// @var: page(width: 300pt, height: auto, margin: auto) -> 70.87pt
// @raw_page: 300x160
// @margins: 70.87, 70.87, 70.87, 70.87
// @mirror_margins: false
// @ir: Paragraph
#set page(width: 300pt, height: auto, margin: auto)
#set text(font: "Times New Roman", size: 11pt)
Auto height page uses A4 width fallback (70.87pt).

// === @case margin_auto_width_auto ===
// @title: Margin: auto with width: auto (Falls Back to A4 Width -> 70.87pt)
// @param: margin: auto + width: auto
// @var: page(width: auto, height: 220pt, margin: auto) -> 70.87pt
// @raw_page: 260x220
// @margins: 70.87, 70.87, 70.87, 70.87
// @mirror_margins: false
// @ir: Paragraph
#set page(width: auto, height: 220pt, margin: auto)
#set text(font: "Times New Roman", size: 11pt)
Auto width page fallback.

// === @case margin_zero_0pt ===
// @title: Margin: Exact Zero (0pt -> 0 dxa)
// @param: margin: 0pt
// @var: page(margin: 0pt)
// @raw_page: 280x100
// @margins: 0, 0, 0, 0
// @mirror_margins: false
// @ir: Paragraph
// @xml: w:top="0" w:right="0" w:bottom="0" w:left="0"
#set page(width: 280pt, height: 100pt, margin: 0pt)
#set text(font: "Times New Roman", size: 11pt)
Flush-edge zero margin content.

// === @case margin_uniform_10pt ===
// @title: Margin: Uniform 10pt (200 dxa)
// @param: margin: 10pt
// @var: page(margin: 10pt)
// @raw_page: 280x100
// @margins: 10, 10, 10, 10
// @mirror_margins: false
// @ir: Paragraph
// @xml: w:top="200" w:right="200" w:bottom="200" w:left="200"
#set page(width: 280pt, height: 100pt, margin: 10pt)
#set text(font: "Times New Roman", size: 11pt)
Standard 10pt visual-test margin.

// === @case margin_unit_cm ===
// @title: Margin: Metric Centimeters (1cm -> 28.35pt / 567 dxa)
// @param: margin: 1cm
// @var: page(margin: 1cm)
// @raw_page: 300x130
// @margins: 28.35, 28.35, 28.35, 28.35
// @ir: Paragraph
#set page(width: 300pt, height: 130pt, margin: 1cm)
#set text(font: "Times New Roman", size: 11pt)
One centimeter margin on all sides.

// === @case margin_unit_mm ===
// @title: Margin: Metric Millimeters (15mm -> 42.52pt / 850 dxa)
// @param: margin: 15mm
// @var: page(margin: 15mm)
// @raw_page: 300x150
// @margins: 42.52, 42.52, 42.52, 42.52
// @ir: Paragraph
#set page(width: 300pt, height: 150pt, margin: 15mm)
#set text(font: "Times New Roman", size: 11pt)
Fifteen millimeter margin.

// === @case margin_unit_inch ===
// @title: Margin: Imperial Inches (0.5in -> 36pt / 720 dxa)
// @param: margin: 0.5in
// @var: page(margin: 0.5in)
// @raw_page: 300x140
// @margins: 36, 36, 36, 36
// @ir: Paragraph
// @xml: w:top="720" w:right="720" w:bottom="720" w:left="720"
#set page(width: 300pt, height: 140pt, margin: 0.5in)
#set text(font: "Times New Roman", size: 11pt)
Half-inch margin (720 dxa).

// === @case margin_em_root_font ===
// @title: Margin: Font-Relative 2em with 15pt Root Text (30pt)
// @param: margin: 2em
// @var: text(15pt) + page(margin: 2em) -> 30pt
// @raw_page: 320x140
// @margins: 30, 30, 30, 30
// @ir: Paragraph
#set text(font: "Times New Roman", size: 15pt)
#set page(width: 320pt, height: 140pt, margin: 2em)
Root font 15pt times 2em equals 30pt margin.

// === @case margin_em_uncorrupted_by_inner_scope ===
// @title: Margin: 2em Uncorrupted by Inner Scoped #set text(24pt)
// @param: margin: 2em (scoped text)
// @var: text(12pt) + page(margin: 2em) + #[set text(24pt)] -> 24pt
// @raw_page: 320x160
// @margins: 24, 24, 24, 24
// @ir: Paragraph
#set text(font: "Times New Roman", size: 12pt)
#set page(width: 320pt, height: 160pt, margin: 2em)
Root 12pt line keeps page margin at 24pt.

#[
  #set text(size: 24pt)
  Inner 24pt block
]

// === @case margin_ratio_asymmetric_page ===
// @title: Margin: Ratio 10% on 300x200pt Page (X=30pt, Y=20pt)
// @param: margin: 10%
// @var: page(width: 300pt, height: 200pt, margin: 10%)
// @raw_page: 300x200
// @margins: 20, 30, 20, 30
// @ir: Paragraph
#set page(width: 300pt, height: 200pt, margin: 10%)
#set text(font: "Times New Roman", size: 11pt)
Percentage resolves per axis (L/R=30pt, T/B=20pt).

// === @case margin_relative_length_plus_ratio ===
// @title: Margin: Composite Relative Length (10pt + 5%)
// @param: margin: 10pt + 5%
// @var: page(width: 300pt, height: 200pt, margin: 10pt + 5%)
// @raw_page: 300x200
// @margins: 20, 25, 20, 25
// @ir: Paragraph
#set page(width: 300pt, height: 200pt, margin: 10pt + 5%)
#set text(font: "Times New Roman", size: 11pt)
Relative margin (T/B=20pt, L/R=25pt).

// === @case margin_ratio_on_auto_axis ===
// @title: Margin: Ratio on Infinite Axis (height: auto + margin: 10pt + 10%)
// @param: margin: 10pt + 10% (height: auto)
// @var: page(width: 300pt, height: auto, margin: 10pt + 10%)
// @raw_page: 300x100
// @margins: 10, 40, 10, 40
// @ir: Paragraph
#set page(width: 300pt, height: auto, margin: 10pt + 10%)
#set text(font: "Times New Roman", size: 11pt)
Ratio component vanishes on infinite height axis (T/B=10pt, L/R=40pt).

// === @case margin_ratio_with_flipped_true ===
// @title: Margin: Ratio 10% with flipped: true (Swaps Axes Before Ratio)
// @param: margin: 10% + flipped: true
// @var: page(width: 200pt, height: 300pt, flipped: true, margin: 10%)
// @raw_page: 300x200
// @margins: 20, 30, 20, 30
// @ir: Paragraph
// @xml: w:orient="landscape"
#set page(width: 200pt, height: 300pt, flipped: true, margin: 10%)
#set text(font: "Times New Roman", size: 11pt)
Flipped 200x300 -> 300x200, so L/R=30pt and T/B=20pt.

// === @case margin_dict_four_sides ===
// @title: Margin: Explicit 4-Side Dictionary (top, right, bottom, left)
// @param: margin: (top, right, bottom, left)
// @var: page(margin: (top: 12pt, right: 18pt, bottom: 24pt, left: 30pt))
// @raw_page: 320x150
// @margins: 12, 18, 24, 30
// @ir: Paragraph
#set page(width: 320pt, height: 150pt, margin: (top: 12pt, right: 18pt, bottom: 24pt, left: 30pt))
#set text(font: "Times New Roman", size: 11pt)
Distinct top=12, right=18, bottom=24, left=30.

// === @case margin_dict_x_y ===
// @title: Margin: Axis Shorthand Dictionary (x: 22pt, y: 14pt)
// @param: margin: (x, y)
// @var: page(margin: (x: 22pt, y: 14pt))
// @raw_page: 300x140
// @margins: 14, 22, 14, 22
// @ir: Paragraph
#set page(width: 300pt, height: 140pt, margin: (x: 22pt, y: 14pt))
#set text(font: "Times New Roman", size: 11pt)
Symmetric horizontal x=22pt and vertical y=14pt.

// === @case margin_dict_precedence_rest_xy_sides ===
// @title: Margin: Dictionary Key Precedence (side > x/y > rest)
// @param: margin: (rest, x, y, top, left)
// @var: page(margin: (rest: 10pt, x: 20pt, y: 16pt, top: 28pt, left: 34pt))
// @raw_page: 320x150
// @margins: 28, 20, 16, 34
// @ir: Paragraph
#set page(width: 320pt, height: 150pt, margin: (rest: 10pt, x: 20pt, y: 16pt, top: 28pt, left: 34pt))
#set text(font: "Times New Roman", size: 11pt)
Precedence yields top=28, right=20, bottom=16, left=34.

// === @case margin_dict_partial_auto_fallback ===
// @title: Margin: Omitted Dictionary Side Falls Back to Proportional auto
// @param: margin: (x: 15pt, top: 20pt)
// @var: page(width: 210pt, height: 300pt, margin: (x: 15pt, top: 20pt))
// @raw_page: 210x300
// @margins: 20, 15, 25, 15
// @ir: Paragraph
#set page(width: 210pt, height: 300pt, margin: (x: 15pt, top: 20pt))
#set text(font: "Times New Roman", size: 11pt)
Omitted bottom resolves proportionally to 25pt.

// === @case margin_nested_set_page_folding ===
// @title: Margin: Nested #set page Rule Folding Preserves Unchanged Sides
// @param: nested set page(margin)
// @var: set page(x: 26pt, y: 18pt) + set page(top: 32pt)
// @raw_page: 300x150
// @margins: 32, 26, 18, 26
// @ir: Paragraph
#set page(width: 300pt, height: 150pt, margin: (x: 26pt, y: 18pt))
#set page(margin: (top: 32pt))
#set text(font: "Times New Roman", size: 11pt)
Folded margin: top=32, right=26, bottom=18, left=26.

// === @case margin_two_sided_binding_left ===
// @title: Margin: Two-Sided inside/outside with binding: left (Mirror Margins)
// @param: margin: (inside, outside) + binding: left
// @var: page(margin: (inside: 36pt, outside: 14pt, y: 12pt), binding: left)
// @raw_page: 300x120
// @margins: 12, 14, 12, 36
// @section_margins: [12, 14, 12, 36]
// @mirror_margins: true
// @ir: Paragraph, PageBreak
// @xml: <w:mirrorMargins/>
#set page(width: 300pt, height: 120pt, margin: (inside: 36pt, outside: 14pt, y: 12pt), binding: left)
#set text(font: "Times New Roman", size: 11pt)
Odd page one (left=36pt, right=14pt).
#pagebreak()
Even page two (left=14pt, right=36pt via w:mirrorMargins).

// === @case margin_two_sided_binding_right ===
// @title: Margin: Two-Sided inside/outside with binding: right
// @param: margin: (inside, outside) + binding: right
// @var: page(margin: (inside: 36pt, outside: 14pt, y: 12pt), binding: right)
// @raw_page: 300x120
// @margins: 12, 36, 12, 14
// @section_margins: [12, 36, 12, 14]
// @mirror_margins: true
// @ir: Paragraph, PageBreak
// @xml: <w:mirrorMargins/>
#set page(width: 300pt, height: 120pt, margin: (inside: 36pt, outside: 14pt, y: 12pt), binding: right)
#set text(font: "Times New Roman", size: 11pt)
Odd page one bound on right (left=14pt, right=36pt).
#pagebreak()
Even page two bound on left (left=36pt, right=14pt).

// === @case margin_two_sided_rtl_auto_binding ===
// @title: Margin: Two-Sided with RTL Text Direction & binding: auto
// @param: margin: (inside, outside) + dir: rtl
// @var: text(dir: rtl) + page(margin: (inside: 34pt, outside: 16pt), binding: auto)
// @raw_page: 300x120
// @margins: 12, 34, 12, 16
// @mirror_margins: true
// @ir: Paragraph
// @xml: <w:mirrorMargins/>
#set text(font: "Times New Roman", size: 11pt, dir: rtl)
#set page(width: 300pt, height: 120pt, margin: (inside: 34pt, outside: 16pt, y: 12pt), binding: auto)
RTL auto binding selects right edge on odd pages.

// === @case margin_two_sided_override_to_one_sided ===
// @title: Margin: Overriding inside/outside with left/right Disables Mirroring
// @param: inside/outside -> left/right override
// @var: set page(inside/outside) -> set page(left: 20pt, right: 24pt)
// @raw_page: 300x120
// @margins: 12, 24, 12, 20
// @mirror_margins: false
// @ir: Paragraph
#set page(width: 300pt, height: 120pt, margin: (inside: 36pt, outside: 14pt, y: 12pt))
#set page(margin: (left: 20pt, right: 24pt))
#set text(font: "Times New Roman", size: 11pt)
Explicit left/right clears two_sided back to false.

// === @case margin_scoped_page_three_sections ===
// @title: Margin: Scoped #page(margin: 28pt)[...] 3-Section Roundtrip
// @param: scoped #page(margin)[...]
// @var: set page(12pt) + #page(margin: 28pt)[...] + restored 12pt
// @raw_page: 280x120
// @margins: 12, 12, 12, 12
// @section_margins: [12, 12, 12, 12]; [28, 28, 28, 28]; [12, 12, 12, 12]
// @mirror_margins: false
// @ir: Paragraph
#set page(width: 280pt, height: 120pt, margin: 12pt)
#set text(font: "Times New Roman", size: 11pt)
Section 1 (12pt margin).
#page(margin: 28pt)[Section 2 scoped page (28pt margin).]
Section 3 restored (12pt margin).

// === @case margin_tight_with_header_footer ===
// @title: Margin: Tight 20pt Vertical Margin with Header & Footer Clamping
// @param: tight margin + header/footer
// @var: page(margin: (x: 12pt, y: 20pt), header, footer)
// @raw_page: 320x140
// @margins: 20, 12, 20, 12
// @mirror_margins: false
// @ir: Paragraph
// @xml: headerReference | footerReference
#set page(
  width: 320pt,
  height: 140pt,
  margin: (x: 12pt, y: 20pt),
  header: text(font: "Times New Roman", size: 8pt)[Tight Header],
  footer: text(font: "Times New Roman", size: 8pt)[Tight Footer],
)
#set text(font: "Times New Roman", size: 11pt)
Body text with w:header <= w:top and w:footer <= w:bottom.




下面例子中的灰色框是版心轮廓。（待处理
#set page(paper: "a4")
#rect(width: 100%, height: 100%, stroke: gray)