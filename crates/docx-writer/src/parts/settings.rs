//! `word/settings.xml`: document settings and Office Math defaults.

use crate::writer::DocxWriter;
use crate::xml::escape_xml_attr;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_settings_xml(&self) -> String {
        let fn_pr = if self.footnotes.is_empty() {
            ""
        } else {
            "\n  <w:footnotePr>\n    <w:footnote w:id=\"-1\"/>\n    <w:footnote w:id=\"0\"/>\n  </w:footnotePr>"
        };
        let has_mirror = self.doc.page_setup.mirror_margins
            || self.doc.sections.iter().any(|s| s.page_setup.mirror_margins);
        let mirror_pr = if has_mirror {
            "\n  <w:mirrorMargins/>"
        } else {
            ""
        };
        let has_odd_even = self.doc.page_setup.different_odd_even
            || self
                .doc
                .sections
                .iter()
                .any(|s| s.page_setup.different_odd_even);
        let even_odd_pr = if has_odd_even {
            "\n  <w:evenAndOddHeaders/>"
        } else {
            ""
        };
        let has_bg = self.doc.page_setup.fill.is_some()
            || self.doc.sections.iter().any(|s| s.page_setup.fill.is_some());
        let bg_pr = if has_bg {
            "\n  <w:displayBackgroundShape/>"
        } else {
            ""
        };
        let embed_fonts_pr = if self.doc.embedded_fonts.is_empty() {
            String::new()
        } else if self.doc.embedded_fonts.iter().any(|f| f.is_subset) {
            "\n  <w:embedTrueTypeFonts/>\n  <w:embedSystemFonts/>\n  <w:saveSubsetFonts/>".to_string()
        } else {
            "\n  <w:embedTrueTypeFonts/>\n  <w:embedSystemFonts/>".to_string()
        };
        let math_font = escape_xml_attr(
            self.doc
                .math_font_family
                .as_deref()
                .unwrap_or("Cambria Math"),
        );
        if self.doc.has_cjk {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
  <w:zoom w:percent="100"/>{bg_pr}{embed_fonts_pr}
  <w:defaultTabStop w:val="720"/>"#,
            )
            + &format!(
                r#"
  <w:characterSpacingControl w:val="doNotCompress"/>{mirror_pr}{even_odd_pr}{fn_pr}
  <w:compat>
    <w:useFELayout/>
    <w:doNotExpandShiftReturn/>
    <w:noColumnBalance/>
    <w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/>
  </w:compat>
  <w:themeFontLang w:val="en-US" w:eastAsia="zh-CN" w:bidi="ar-SA"/>
  <m:mathPr>
    <m:mathFont m:val="{math_font}"/>
    <m:brkBin m:val="before"/>
    <m:brkBinSub m:val="--"/>
    <m:smallFrac m:val="0"/>
    <m:dispDef/>
    <m:lMargin m:val="0"/>
    <m:rMargin m:val="0"/>
    <m:defJc m:val="centerGroup"/>
    <m:preSp m:val="0"/>
    <m:postSp m:val="0"/>
    <m:wrapIndent m:val="1440"/>
    <m:intLim m:val="subSup"/>
    <m:naryLim m:val="undOvr"/>
  </m:mathPr>
</w:settings>"#
            )
        } else {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
  <w:zoom w:percent="100"/>{bg_pr}{embed_fonts_pr}
  <w:defaultTabStop w:val="720"/>
  <w:characterSpacingControl w:val="doNotCompress"/>
  <w:autoSpaceDE w:val="1"/>
  <w:autoSpaceDN w:val="1"/>{mirror_pr}{even_odd_pr}{fn_pr}
  <w:compat>
    <w:doNotExpandShiftReturn/>
    <w:noColumnBalance/>
    <w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/>
  </w:compat>
  <m:mathPr>
    <m:mathFont m:val="{math_font}"/>
    <m:brkBin m:val="before"/>
    <m:brkBinSub m:val="--"/>
    <m:smallFrac m:val="0"/>
    <m:dispDef/>
    <m:lMargin m:val="0"/>
    <m:rMargin m:val="0"/>
    <m:defJc m:val="centerGroup"/>
    <m:preSp m:val="0"/>
    <m:postSp m:val="0"/>
    <m:wrapIndent m:val="1440"/>
    <m:intLim m:val="subSup"/>
    <m:naryLim m:val="undOvr"/>
  </m:mathPr>
</w:settings>"#
            )
        }
    }
}
