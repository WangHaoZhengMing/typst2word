//! `list` / `enum` / `terms` -> numbered paragraphs (`<w:numPr>`) or hanging-indent terms.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{BlockNode, EnumItemIr, InlineNode, ListItemIr, TermItemIr, TextRunIr};
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    /// `BlockNode::List` -> bullet-numbered paragraphs (`numId` 1).
    pub(crate) fn render_list(
        &mut self,
        items: &[ListItemIr],
        tight: bool,
        level: usize,
        spacing_pt: Option<f64>,
        indent_pt: f64,
        hanging_pt: f64,
        out: &mut String,
    ) {
        self.render_list_with_trailing_gap(
            items,
            tight,
            level,
            spacing_pt,
            indent_pt,
            hanging_pt,
            None,
            out,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_list_with_trailing_gap(
        &mut self,
        items: &[ListItemIr],
        tight: bool,
        level: usize,
        spacing_pt: Option<f64>,
        indent_pt: f64,
        hanging_pt: f64,
        trailing_after_dxa: Option<i64>,
        out: &mut String,
    ) {
        let ilvl = (level).min(8);
        let num_items = items.len();
        let item_gap_dxa = Self::compute_list_item_gap_dxa(tight, spacing_pt);
        for (idx, item) in items.iter().enumerate() {
            let is_last_in_list = idx + 1 == num_items;
            let is_last_top = level == 0 && is_last_in_list;
            let item_end_after_dxa = if is_last_in_list {
                trailing_after_dxa.unwrap_or_else(|| {
                    if is_last_top {
                        item_gap_dxa.max(40)
                    } else {
                        item_gap_dxa
                    }
                })
            } else {
                item_gap_dxa
            };
            let prev_ended_with_table = idx > 0
                && matches!(
                    items[idx - 1].blocks.last(),
                    Some(BlockNode::Table(_))
                );
            let before_dxa = if prev_ended_with_table && item_gap_dxa > 0 {
                item_gap_dxa + 64
            } else {
                0
            };
            let bullet_hint = match item.marker.as_str() {
                "•" => Some("__bul0__"),
                "‣" => Some("__bul1__"),
                "–" => Some("__bul2__"),
                _ => None,
            };
            self.render_list_or_enum_item_blocks(
                &item.blocks,
                1, // numId 1 = bullet list
                ilvl,
                bullet_hint,
                tight,
                item_gap_dxa,
                item_end_after_dxa,
                before_dxa,
                indent_pt,
                hanging_pt,
                is_last_top,
                out,
            );
        }
    }

    /// `BlockNode::Enum` -> decimal-numbered paragraphs (`numId` allocated per list).
    pub(crate) fn render_enum(
        &mut self,
        items: &[EnumItemIr],
        tight: bool,
        level: usize,
        spacing_pt: Option<f64>,
        indent_pt: f64,
        hanging_pt: f64,
        out: &mut String,
    ) {
        self.render_enum_with_trailing_gap(
            items,
            tight,
            level,
            spacing_pt,
            indent_pt,
            hanging_pt,
            None,
            out,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_enum_with_trailing_gap(
        &mut self,
        items: &[EnumItemIr],
        tight: bool,
        level: usize,
        spacing_pt: Option<f64>,
        indent_pt: f64,
        hanging_pt: f64,
        trailing_after_dxa: Option<i64>,
        out: &mut String,
    ) {
        let ilvl = (level).min(8);
        let start_val = items.first().and_then(|it| it.number).unwrap_or(1);
        let num_id = self.alloc_enum_num_id(ilvl, start_val);
        let num_items = items.len();
        let item_gap_dxa = Self::compute_list_item_gap_dxa(tight, spacing_pt);
        for (idx, item) in items.iter().enumerate() {
            let is_last_in_list = idx + 1 == num_items;
            let is_last_top = level == 0 && is_last_in_list;
            let item_end_after_dxa = if is_last_in_list {
                trailing_after_dxa.unwrap_or_else(|| {
                    if is_last_top {
                        item_gap_dxa.max(40)
                    } else {
                        item_gap_dxa
                    }
                })
            } else {
                item_gap_dxa
            };
            let prev_ended_with_table = idx > 0
                && matches!(
                    items[idx - 1].blocks.last(),
                    Some(BlockNode::Table(_))
                );
            let before_dxa = if prev_ended_with_table && item_gap_dxa > 0 {
                item_gap_dxa + 64
            } else if idx == 0 && start_val > 1 {
                item_gap_dxa.max(180)
            } else {
                0
            };
            let custom_prefix = if Self::is_standard_decimal_marker(&item.marker_text) {
                None
            } else {
                Some(item.marker_text.as_str())
            };
            self.render_list_or_enum_item_blocks(
                &item.blocks,
                num_id,
                ilvl,
                custom_prefix,
                tight,
                item_gap_dxa,
                item_end_after_dxa,
                before_dxa,
                indent_pt,
                hanging_pt,
                is_last_top,
                out,
            );
        }
    }

    fn compute_list_item_gap_dxa(tight: bool, spacing_pt: Option<f64>) -> i64 {
        if let Some(sp_pt) = spacing_pt {
            crate::units::pt_to_dxa(sp_pt).max(0)
        } else if tight {
            0
        } else {
            90
        }
    }

    /// `BlockNode::Terms` -> hanging-indent paragraphs with bold terms.
    pub(crate) fn render_terms(&mut self, items: &[TermItemIr], tight: bool, out: &mut String) {
        let num_items = items.len();
        for (idx, item) in items.iter().enumerate() {
            let is_last = idx + 1 == num_items;
            let after = if is_last {
                110
            } else if tight {
                15
            } else {
                110
            };
            out.push_str("<w:p><w:pPr>");
            let _ = write!(
                out,
                r#"<w:spacing w:before="10" w:after="{after}" w:line="246" w:lineRule="auto"/><w:ind w:left="440" w:hanging="440"/>"#
            );
            out.push_str("</w:pPr>");
            // Render term runs in bold followed by Typst's default 0.6em space separator and description.
            for inline in &item.term {
                if let InlineNode::Text(r) = inline {
                    let mut bold_run = r.clone();
                    bold_run.bold = true;
                    self.render_text_run(&bold_run, out);
                } else {
                    self.render_inline(inline, out);
                }
            }
            let sep_run = TextRunIr {
                text: "  ".to_string(),
                font_family: Some("Libertinus Serif".to_string()),
                font_size_pt: Some(11.0),
                ..Default::default()
            };
            self.render_text_run(&sep_run, out);
            if let Some(BlockNode::Paragraph { content, .. }) = item.description.first() {
                self.render_inlines(content, out);
                out.push_str("</w:p>");
                for rest in item.description.iter().skip(1) {
                    self.render_block(rest, out);
                }
            } else {
                out.push_str("</w:p>");
                self.render_blocks(&item.description, out);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_list_or_enum_item_blocks(
        &mut self,
        blocks: &[BlockNode],
        num_id: usize,
        ilvl: usize,
        custom_prefix: Option<&str>,
        tight: bool,
        item_gap_dxa: i64,
        item_end_after_dxa: i64,
        before_dxa: i64,
        indent_pt: f64,
        hanging_pt: f64,
        is_last_top: bool,
        out: &mut String,
    ) {
        let followed_by_grid_table = matches!(
            blocks.get(1),
            Some(BlockNode::Table(t)) if !t.is_semantic_table
        );
        let followed_by_tight_list = matches!(
            blocks.get(1),
            Some(BlockNode::List { tight: true, .. } | BlockNode::Enum { tight: true, .. })
        );
        let followed_by_paragraph = matches!(blocks.get(1), Some(BlockNode::Paragraph { .. }));
        let first_line_gap_dxa = match blocks.first() {
            Some(BlockNode::Paragraph { style, .. }) => {
                let pt = style.line_spacing_pt.unwrap_or(style.space_after_pt).max(4.0);
                crate::units::pt_to_dxa(pt)
            }
            _ => 156,
        };
        let after_dxa = if followed_by_grid_table {
            first_line_gap_dxa
        } else if blocks.len() > 1 {
            if followed_by_paragraph {
                item_gap_dxa.max(132)
            } else if followed_by_tight_list || tight {
                0
            } else {
                10
            }
        } else {
            item_end_after_dxa
        };
        let hanging_dxa = if hanging_pt > 1.0 {
            crate::units::pt_to_dxa(hanging_pt).max(180)
        } else {
            380
        };
        let outer_indent_dxa = crate::units::pt_to_dxa(indent_pt).max(0);
        let indent_left = if indent_pt > 0.1 || ilvl == 0 {
            (outer_indent_dxa + hanging_dxa).max(0) as usize
        } else {
            (outer_indent_dxa + hanging_dxa * (ilvl as i64 + 1)).max(0) as usize
        };
        if blocks.is_empty() {
            let _ = write!(
                out,
                r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{ilvl}"/><w:numId w:val="{num_id}"/></w:numPr><w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="230" w:lineRule="auto"/><w:ind w:left="{indent_left}" w:hanging="{hanging_dxa}"/></w:pPr></w:p>"#
            );
            return;
        }

        for (idx, b) in blocks.iter().enumerate() {
            match b {
                BlockNode::Paragraph {
                    content,
                    style,
                    bookmark,
                } if idx == 0 => {
                    let line_dxa =
                        Self::compute_line_spacing_dxa(content, style.line_spacing_pt);
                    let tabs_xml = if Self::has_large_horizontal_space(content) {
                        format!(
                            r#"<w:tabs><w:tab w:val="num" w:pos="{indent_left}"/><w:tab w:val="right" w:pos="{}"/></w:tabs>"#,
                            self.content_width_dxa()
                        )
                    } else {
                        format!(r#"<w:tabs><w:tab w:val="num" w:pos="{indent_left}"/></w:tabs>"#)
                    };
                    let keep_next_xml = if followed_by_grid_table
                        && matches!(blocks.get(1), Some(BlockNode::Table(t)) if t.rows.len() <= 4)
                    {
                        "<w:keepNext/><w:keepLines/>"
                    } else {
                        "<w:keepLines/>"
                    };
                    let (eff_prefix, num_ilvl) = match custom_prefix {
                        Some("__bul0__") => (None, 0usize),
                        Some("__bul1__") => (None, 1usize),
                        Some("__bul2__") => (None, 2usize),
                        other => (other, ilvl),
                    };
                    out.push_str("<w:p><w:pPr>");
                    if let Some(prefix) = eff_prefix {
                        let _ = write!(
                            out,
                            r#"{keep_next_xml}<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/><w:ind w:left="{indent_left}" w:hanging="{hanging_dxa}"/>{tabs_xml}</w:pPr>"#
                        );
                        let bm_id = bookmark
                            .as_ref()
                            .map(|bm| self.emit_bookmark_start(bm, out));
                        let _ = write!(
                            out,
                            r#"<w:r><w:t xml:space="preserve">{} </w:t></w:r>"#,
                            escape_xml(prefix)
                        );
                        self.render_inlines(content, out);
                        if let Some(id) = bm_id {
                            Self::emit_bookmark_end(id, out);
                        }
                        out.push_str("</w:p>");
                    } else {
                        let _ = write!(
                            out,
                            r#"{keep_next_xml}<w:numPr><w:ilvl w:val="{num_ilvl}"/><w:numId w:val="{num_id}"/></w:numPr>{tabs_xml}<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/><w:ind w:left="{indent_left}" w:hanging="{hanging_dxa}"/></w:pPr>"#
                        );
                        let bm_id = bookmark
                            .as_ref()
                            .map(|bm| self.emit_bookmark_start(bm, out));
                        self.render_inlines(content, out);
                        if let Some(id) = bm_id {
                            Self::emit_bookmark_end(id, out);
                        }
                        out.push_str("</w:p>");
                    }
                }
                BlockNode::Paragraph {
                    content,
                    style,
                    bookmark,
                } => {
                    let mut cont_style = style.clone();
                    if cont_style.left_indent_pt < 1.0 {
                        cont_style.left_indent_pt = indent_left as f64 / 20.0;
                    }
                    if idx + 1 == blocks.len() && !tight {
                        cont_style.space_after_pt =
                            cont_style.space_after_pt.max(item_end_after_dxa as f64 / 20.0);
                    }
                    self.render_paragraph(content, &cont_style, bookmark.as_deref(), out);
                }
                BlockNode::CodeBlock {
                    lang,
                    lines,
                    fill,
                    stroke,
                } => {
                    self.render_code_block_indented(
                        lang.as_deref(),
                        lines,
                        fill.as_deref(),
                        stroke.as_ref(),
                        indent_left,
                        out,
                    );
                }
                BlockNode::ShapeBlock(shape) => {
                    self.render_shape_block_indented(shape, indent_left, out);
                }
                BlockNode::Table(tbl) => {
                    let mut adjusted = tbl.clone();
                    let target_left_pt = indent_left as f64 / 20.0;
                    if !adjusted.is_semantic_table && adjusted.align == crate::ir::HAlignIr::Left {
                        adjusted.left_indent_pt = target_left_pt;
                        let avail_w_pt =
                            ((self.content_width_dxa() as f64 / 20.0) - target_left_pt).max(100.0);
                        let total_col_w: f64 = adjusted.col_widths_pt.iter().sum();
                        if total_col_w > avail_w_pt + 6.0 {
                            let excess = total_col_w - avail_w_pt;
                            let ncols = adjusted.col_widths_pt.len();
                            if ncols > 1 {
                                let head_sum: f64 = adjusted.col_widths_pt[..ncols - 1].iter().sum();
                                if head_sum > excess + 24.0 * (ncols - 1) as f64 {
                                    for w in &mut adjusted.col_widths_pt[..ncols - 1] {
                                        *w -= excess * (*w / head_sum);
                                    }
                                } else {
                                    let scale = avail_w_pt / total_col_w;
                                    for w in &mut adjusted.col_widths_pt {
                                        *w *= scale;
                                    }
                                }
                            } else {
                                adjusted.col_widths_pt[0] = avail_w_pt;
                            }
                        }
                    }
                    if idx + 1 == blocks.len() && item_gap_dxa > 0 && is_last_top {
                        if let Some(last_row) = adjusted.rows.last_mut() {
                            for cell in &mut last_row.cells {
                                cell.inset.bottom += 4.5;
                            }
                        }
                    }
                    self.render_table(&adjusted, out);
                }
                BlockNode::List {
                    items: child_items,
                    tight: child_tight,
                    level: child_level,
                    spacing_pt: child_spacing_pt,
                    indent_pt: child_indent_pt,
                    hanging_pt: child_hanging_pt,
                } => {
                    let child_trailing = if idx + 1 == blocks.len() {
                        Some(item_end_after_dxa)
                    } else if !tight {
                        Some(item_gap_dxa.max(150))
                    } else {
                        None
                    };
                    let eff_child_indent_pt = *child_indent_pt + (indent_left as f64 / 20.0);
                    self.render_list_with_trailing_gap(
                        child_items,
                        *child_tight,
                        *child_level,
                        *child_spacing_pt,
                        eff_child_indent_pt,
                        *child_hanging_pt,
                        child_trailing,
                        out,
                    );
                }
                BlockNode::Enum {
                    items: child_items,
                    tight: child_tight,
                    level: child_level,
                    spacing_pt: child_spacing_pt,
                    indent_pt: child_indent_pt,
                    hanging_pt: child_hanging_pt,
                } => {
                    let child_trailing = if idx + 1 == blocks.len() {
                        Some(item_end_after_dxa)
                    } else if !tight {
                        Some(item_gap_dxa.max(150))
                    } else {
                        None
                    };
                    let eff_child_indent_pt = *child_indent_pt + (indent_left as f64 / 20.0);
                    self.render_enum_with_trailing_gap(
                        child_items,
                        *child_tight,
                        *child_level,
                        *child_spacing_pt,
                        eff_child_indent_pt,
                        *child_hanging_pt,
                        child_trailing,
                        out,
                    );
                }
                BlockNode::EquationBlock(eq) => {
                    let mut eq_xml = String::new();
                    self.render_equation_block(eq, &mut eq_xml);
                    eq_xml = eq_xml.replace(
                        r#"<w:spacing w:before="80" w:after="80"/>"#,
                        r#"<w:spacing w:before="110" w:after="210"/>"#,
                    );
                    if indent_left > 0 {
                        let ind_tag = format!(r#"<w:pPr><w:ind w:left="{indent_left}"/>"#);
                        eq_xml = eq_xml.replacen("<w:pPr>", &ind_tag, 1);
                    }
                    out.push_str(&eq_xml);
                }
                BlockNode::Figure {
                    body,
                    caption,
                    caption_top,
                    bookmark,
                    ..
                } => {
                    let mut fig_xml = String::new();
                    self.render_figure_indented(
                        body,
                        caption.as_deref(),
                        *caption_top,
                        bookmark.as_deref(),
                        indent_left,
                        &mut fig_xml,
                    );
                    if idx + 1 == blocks.len() && !tight {
                        fig_xml = fig_xml.replace(r#"w:after="60""#, r#"w:after="180""#);
                    }
                    out.push_str(&fig_xml);
                }
                other => {
                    self.render_block(other, out);
                }
            }
        }
    }

    pub(crate) fn is_standard_decimal_marker(marker: &str) -> bool {
        let trimmed = marker.trim();
        trimmed.ends_with('.')
            && trimmed[..trimmed.len() - 1]
                .chars()
                .all(|c| c.is_ascii_digit())
    }
}
