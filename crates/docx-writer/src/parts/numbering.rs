//! `word/numbering.xml`: bullet (`numId` 1) and decimal (`numId` 2) list definitions.

use std::fmt::Write as _;

use crate::writer::DocxWriter;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_numbering_xml(&self) -> String {
        let mut out = String::new();
        out.push_str(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">"#,
        );
        let bullets = ["•", "‣", "–", "•", "‣", "–", "•", "‣", "–"];
        for (lvl, b) in bullets.iter().enumerate() {
            let left = (lvl + 1) * 260;
            let _ = write!(
                out,
                r#"<w:lvl w:ilvl="{lvl}"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="{b}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{left}" w:hanging="260"/></w:pPr></w:lvl>"#
            );
        }
        out.push_str("</w:abstractNum><w:abstractNum w:abstractNumId=\"1\">");
        for lvl in 0..9 {
            let left = (lvl + 1) * 380;
            let fmt = "decimal";
            let lvl_txt = format!("%{}.", lvl + 1);
            let _ = write!(
                out,
                r#"<w:lvl w:ilvl="{lvl}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/><w:lvlText w:val="{lvl_txt}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{left}" w:hanging="380"/></w:pPr></w:lvl>"#
            );
        }
        out.push_str(
            r#"</w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
        );
        if self.enum_instances.is_empty() {
            out.push_str(r#"<w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>"#);
        } else {
            for &(num_id, ilvl, start_val) in &self.enum_instances {
                let _ = write!(
                    out,
                    r#"<w:num w:numId="{num_id}"><w:abstractNumId w:val="1"/><w:lvlOverride w:ilvl="{ilvl}"><w:startOverride w:val="{start_val}"/></w:lvlOverride></w:num>"#
                );
            }
        }
        out.push_str("</w:numbering>");
        out
    }
}
