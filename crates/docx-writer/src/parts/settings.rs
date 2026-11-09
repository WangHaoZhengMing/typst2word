//! `word/settings.xml`: document settings and Office Math defaults.

use ir::{BlockNode, InlineNode};

use crate::writer::DocxWriter;

impl<'a> DocxWriter<'a> {
    /// Whether the document contains any East Asian (CJK) characters or CJK typeface runs.
    pub(crate) fn document_has_cjk(&self) -> bool {
        fn inlines_have_cjk(inlines: &[InlineNode]) -> bool {
            inlines.iter().any(|inl| match inl {
                InlineNode::Text(r) => r.text_script.has_cjk() || r.font_class.is_cjk_font(),
                InlineNode::Field { content, .. }
                | InlineNode::Link { content, .. }
                | InlineNode::Ref { content, .. }
                | InlineNode::Citation { content, .. } => inlines_have_cjk(content),
                InlineNode::Footnote { content, .. } => blocks_have_cjk(content),
                InlineNode::ShapeInline(s) => blocks_have_cjk(&s.body),
                InlineNode::CetzInline(c) => c.labels.iter().any(|l| inlines_have_cjk(&l.content)),
                _ => false,
            })
        }

        fn blocks_have_cjk(blocks: &[BlockNode]) -> bool {
            blocks.iter().any(|b| match b {
                BlockNode::Title { content, .. }
                | BlockNode::Heading { content, .. }
                | BlockNode::Paragraph { content, .. } => inlines_have_cjk(content),
                BlockNode::List { items, .. } => items.iter().any(|i| blocks_have_cjk(&i.blocks)),
                BlockNode::Enum { items, .. } => items.iter().any(|i| blocks_have_cjk(&i.blocks)),
                BlockNode::Terms { items, .. } => items
                    .iter()
                    .any(|i| inlines_have_cjk(&i.term) || blocks_have_cjk(&i.description)),
                BlockNode::Table(t) => t
                    .rows
                    .iter()
                    .any(|r| r.cells.iter().any(|c| blocks_have_cjk(&c.blocks))),
                BlockNode::Quote { body, attribution } => {
                    blocks_have_cjk(body)
                        || attribution.as_deref().is_some_and(inlines_have_cjk)
                }
                BlockNode::CodeBlock { lines, .. } => lines.iter().any(|line| {
                    line.iter()
                        .any(|r| r.text_script.has_cjk() || r.font_class.is_cjk_font())
                }),
                BlockNode::Figure { body, caption, .. } => {
                    blocks_have_cjk(body) || caption.as_deref().is_some_and(inlines_have_cjk)
                }
                BlockNode::ShapeBlock(s) => blocks_have_cjk(&s.body),
                BlockNode::CetzCanvas(c) => c.labels.iter().any(|l| inlines_have_cjk(&l.content)),
                BlockNode::Outline { title, entries } => {
                    title.as_deref().is_some_and(inlines_have_cjk)
                        || entries.iter().any(|e| inlines_have_cjk(&e.title))
                }
                BlockNode::Bibliography { title, entries } => {
                    title.as_deref().is_some_and(inlines_have_cjk)
                        || entries.iter().any(|e| inlines_have_cjk(&e.content))
                }
                BlockNode::Columns { body, .. } => blocks_have_cjk(body),
                _ => false,
            })
        }

        blocks_have_cjk(&self.ir.blocks)
            || blocks_have_cjk(&self.ir.page_setup.header)
            || blocks_have_cjk(&self.ir.page_setup.footer)
            || self.ir.sections.iter().any(|s| blocks_have_cjk(&s.blocks) || blocks_have_cjk(&s.page_setup.header) || blocks_have_cjk(&s.page_setup.footer))
    }

    /// Whether the document contains CeTZ canvases (`<wpg:wgp>` WordprocessingGroup),
    /// which require `compatibilityMode=15` for accurate DrawingML group layout.
    fn document_has_cetz_canvas(&self) -> bool {
        self.ir
            .blocks
            .iter()
            .any(|b| matches!(b, BlockNode::CetzCanvas(_)))
    }

    pub(crate) fn render_settings_xml(&self) -> String {
        if self.document_has_cjk() {
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
  <w:zoom w:percent="100"/>
  <w:defaultTabStop w:val="720"/>
  <w:characterSpacingControl w:val="doNotCompress"/>
  <w:doNotExpandShiftReturn/>
  <w:compat>
    <w:useFELayout/>
    <w:noColumnBalance/>
    <w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/>
  </w:compat>
  <w:themeFontLang w:val="en-US" w:eastAsia="zh-CN" w:bidi="ar-SA"/>
  <m:mathPr>
    <m:mathFont m:val="Cambria Math"/>
    <m:brkBin m:val="before"/>
    <m:brkBinSub m:val="--"/>
    <m:smallFrac m:val="0"/>
    <m:dispDef/>
    <m:lMargin m:val="0"/>
    <m:rMargin m:val="0"/>
    <m:defJc m:val="centerGroup"/>
    <m:wrapIndent m:val="1440"/>
    <m:intLim m:val="subSup"/>
    <m:naryLim m:val="undOvr"/>
  </m:mathPr>
</w:settings>"#
                .to_string()
        } else {
            let compat_mode = if self.document_has_cetz_canvas() {
                "\n    <w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/>"
            } else {
                ""
            };
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
  <w:zoom w:percent="100"/>
  <w:defaultTabStop w:val="720"/>
  <w:characterSpacingControl w:val="compressPunctuation"/>
  <w:autoSpaceDE w:val="1"/>
  <w:autoSpaceDN w:val="1"/>
  <w:doNotExpandShiftReturn/>
  <w:compat>
    <w:noColumnBalance/>{compat_mode}
  </w:compat>
  <m:mathPr>
    <m:mathFont m:val="Cambria Math"/>
    <m:brkBin m:val="before"/>
    <m:brkBinSub m:val="--"/>
    <m:smallFrac m:val="0"/>
    <m:dispDef/>
    <m:lMargin m:val="0"/>
    <m:rMargin m:val="0"/>
    <m:defJc m:val="centerGroup"/>
    <m:wrapIndent m:val="1440"/>
    <m:intLim m:val="subSup"/>
    <m:naryLim m:val="undOvr"/>
  </m:mathPr>
</w:settings>"#
            )
        }
    }
}
