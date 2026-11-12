//! `word/settings.xml`: document settings and Office Math defaults.

use crate::writer::DocxWriter;

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
        if self.doc.has_cjk {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
  <w:zoom w:percent="100"/>
  <w:defaultTabStop w:val="720"/>
  <w:characterSpacingControl w:val="doNotCompress"/>
  <w:doNotExpandShiftReturn/>{mirror_pr}{fn_pr}
  <w:compat>
    <w:useFELayout/>
    <w:doNotExpandShiftReturn/>
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
            )
        } else {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
  <w:zoom w:percent="100"/>
  <w:defaultTabStop w:val="720"/>
  <w:characterSpacingControl w:val="doNotCompress"/>
  <w:autoSpaceDE w:val="1"/>
  <w:autoSpaceDN w:val="1"/>
  <w:doNotExpandShiftReturn/>{mirror_pr}{fn_pr}
  <w:compat>
    <w:doNotExpandShiftReturn/>
    <w:noColumnBalance/>
    <w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/>
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
