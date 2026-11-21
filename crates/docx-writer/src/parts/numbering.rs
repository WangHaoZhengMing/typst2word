//! `word/numbering.xml`: bullet (`numId` 1) and decimal (`numId` 2) list definitions.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::writer::DocxWriter;
use crate::xml::escape_xml;
use word_construct::model::{BlockNode, InlineNode};

impl<'a> DocxWriter<'a> {
    fn collect_body_font_counts(blocks: &[BlockNode], counts: &mut BTreeMap<String, usize>) {
        fn visit_inlines(inlines: &[InlineNode], counts: &mut BTreeMap<String, usize>) {
            for inl in inlines {
                match inl {
                    InlineNode::Text(r) if !r.is_code => {
                        if let Some(fam) = &r.font_family {
                            *counts.entry(fam.clone()).or_insert(0) += r.text.chars().count().max(1);
                        }
                    }
                    InlineNode::Link { content, .. }
                    | InlineNode::Ref { content, .. }
                    | InlineNode::Citation { content, .. }
                    | InlineNode::Field { content, .. } => visit_inlines(content, counts),
                    _ => {}
                }
            }
        }

        for block in blocks {
            match block {
                BlockNode::Paragraph { content, .. } => visit_inlines(content, counts),
                BlockNode::List { items, .. } => {
                    for item in items {
                        Self::collect_body_font_counts(&item.blocks, counts);
                    }
                }
                BlockNode::Enum { items, .. } => {
                    for item in items {
                        Self::collect_body_font_counts(&item.blocks, counts);
                    }
                }
                BlockNode::Quote { body, .. }
                | BlockNode::Figure { body, .. }
                | BlockNode::Columns { body, .. } => {
                    Self::collect_body_font_counts(body, counts);
                }
                BlockNode::Container(c) => {
                    Self::collect_body_font_counts(&c.body, counts);
                }
                BlockNode::ShapeBlock(s) => {
                    Self::collect_body_font_counts(&s.body, counts);
                }
                BlockNode::Table(tbl) => {
                    for row in &tbl.rows {
                        for cell in &row.cells {
                            Self::collect_body_font_counts(&cell.blocks, counts);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn primary_body_font_family(&self) -> Option<String> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        Self::collect_body_font_counts(&self.doc.blocks, &mut counts);
        for sec in &self.doc.sections {
            Self::collect_body_font_counts(&sec.blocks, &mut counts);
        }
        counts
            .into_iter()
            .max_by_key(|(_, c)| *c)
            .map(|(fam, _)| fam)
            .or_else(|| {
                self.doc
                    .font_metrics
                    .keys()
                    .find(|k| !k.to_ascii_lowercase().contains("math"))
                    .cloned()
            })
    }

    pub(crate) fn render_numbering_xml(&self) -> String {
        let lvl_rpr_xml = self
            .primary_body_font_family()
            .map(|fam| {
                let fam_esc = escape_xml(&fam);
                format!(
                    r#"<w:rPr><w:rFonts w:ascii="{fam_esc}" w:hAnsi="{fam_esc}" w:eastAsia="{fam_esc}" w:cs="{fam_esc}"/></w:rPr>"#
                )
            })
            .unwrap_or_default();

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
                r#"<w:lvl w:ilvl="{lvl}"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="{b}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{left}" w:hanging="260"/></w:pPr>{lvl_rpr_xml}</w:lvl>"#
            );
        }
        out.push_str("</w:abstractNum><w:abstractNum w:abstractNumId=\"1\">");
        for lvl in 0..9 {
            let left = (lvl + 1) * 380;
            let fmt = "decimal";
            let lvl_txt = format!("%{}.", lvl + 1);
            let _ = write!(
                out,
                r#"<w:lvl w:ilvl="{lvl}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/><w:suff w:val="space"/><w:lvlText w:val="{lvl_txt}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{left}" w:hanging="380"/></w:pPr>{lvl_rpr_xml}</w:lvl>"#
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
