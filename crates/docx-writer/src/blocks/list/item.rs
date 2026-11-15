//! Per-item block rendering (`render_list_or_enum_item_blocks`, first paragraph, child tables) for lists/enums.

use std::fmt::Write as _;

use word_construct::list::{ListItemConstruct, ListMarkerConstruct};
use word_construct::paragraph::{is_only_inline_equation, paragraph_mark_rpr_sz_half_pt};
use word_construct::table::solve_list_item_child_table;
use crate::writer::DocxWriter;
use word_construct::model::{BlockNode, InlineNode, NodeId, ParagraphStyleIr, TableIr};
use crate::xml::escape_xml;

impl<'a> DocxWriter<'a> {
    pub(super) fn render_list_or_enum_item_blocks(
        &mut self,
        blocks: &[BlockNode],
        ctx: &ListItemConstruct<'_>,
        out: &mut String,
    ) {
        let item_layout = ctx.solve_item_layout();
        let after_dxa = item_layout.first_after_dxa;
        let hanging_dxa = item_layout.hanging_dxa;
        let indent_left = item_layout.indent_left_dxa;

        if blocks.is_empty() {
            let _ = write!(
                out,
                r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{}"/><w:numId w:val="{}"/></w:numPr><w:spacing w:before="{}" w:after="{after_dxa}" w:line="230" w:lineRule="auto"/><w:ind w:left="{indent_left}" w:hanging="{hanging_dxa}"/></w:pPr></w:p>"#,
                ctx.ilvl, ctx.num_id, ctx.before_dxa
            );
            return;
        }

        for (idx, b) in blocks.iter().enumerate() {
            match b {
                BlockNode::Paragraph {
                    id,
                    content,
                    style,
                    bookmark,
                } if idx == 0 => {
                    self.render_list_item_first_paragraph(
                        *id,
                        content,
                        style,
                        bookmark.as_deref(),
                        ctx,
                        indent_left,
                        hanging_dxa,
                        after_dxa,
                        out,
                    );
                }
                BlockNode::Paragraph {
                    id,
                    content,
                    style,
                    bookmark,
                } => {
                    let mut cont_style = style.clone();
                    if cont_style.left_indent_pt < 1.0 {
                        cont_style.left_indent_pt = indent_left as f64 / 20.0;
                    }
                    let char_len: usize = content
                        .iter()
                        .map(|inl| match inl {
                            InlineNode::Text(r) => r.text.chars().count(),
                            _ => 2,
                        })
                        .sum();
                    let keep_with_next = char_len <= 12
                        && matches!(
                            blocks.get(idx + 1),
                            Some(BlockNode::List { .. } | BlockNode::Enum { .. })
                        );
                    if idx + 1 == blocks.len() && !ctx.tight {
                        let item_end_pt = ctx.item_end_after_dxa as f64 / 20.0;
                        cont_style.space_after_pt = cont_style.space_after_pt.max(item_end_pt);
                    }
                    self.render_paragraph_with_keep_next(
                        *id,
                        content,
                        &cont_style,
                        bookmark.as_deref(),
                        keep_with_next,
                        out,
                    );
                }
                BlockNode::CodeBlock {
                    id,
                    lang,
                    lines,
                    fill,
                    stroke,
                    ..
                } => {
                    if idx == 0 {
                        let mut body_xml = String::new();
                        self.render_code_block(
                            *id,
                            lang.as_deref(),
                            lines,
                            fill.as_deref(),
                            stroke.as_ref(),
                            &mut body_xml,
                        );
                        self.render_list_item_marker_beside_block(
                            ctx,
                            indent_left,
                            hanging_dxa,
                            0,
                            &body_xml,
                            out,
                        );
                    } else {
                        self.render_code_block_indented(
                            *id,
                            lang.as_deref(),
                            lines,
                            fill.as_deref(),
                            stroke.as_ref(),
                            indent_left,
                            out,
                        );
                    }
                }
                BlockNode::ShapeBlock(shape) => {
                    if idx == 0 {
                        let top_pad_dxa =
                            word_construct::units::pt_to_dxa(shape.inset.top * 0.5).clamp(0, 160);
                        let mut body_xml = String::new();
                        self.render_shape_block(shape, &mut body_xml);
                        self.render_list_item_marker_beside_block(
                            ctx,
                            indent_left,
                            hanging_dxa,
                            top_pad_dxa,
                            &body_xml,
                            out,
                        );
                    } else {
                        let shape_after_dxa = if idx + 1 == blocks.len() {
                            (!ctx.tight).then_some(ctx.item_end_after_dxa)
                        } else {
                            Some(ctx.item_gap_dxa.max(180))
                        };
                        self.render_shape_block_indented_with_after(
                            shape,
                            indent_left,
                            shape_after_dxa,
                            out,
                        );
                    }
                }
                BlockNode::Table(tbl) => {
                    let bottom_pad_pt = ctx.solve_child_table_bottom_pad_pt(idx, tbl);
                    self.render_list_item_child_table(tbl, indent_left, bottom_pad_pt, out);
                }
                BlockNode::List {
                    items: child_items,
                    tight: child_tight,
                    level: child_level,
                    spacing_pt: child_spacing_pt,
                    indent_pt: child_indent_pt,
                    hanging_pt: child_hanging_pt,
                    ..
                } => {
                    let child_trailing = if idx + 1 == blocks.len() {
                        Some(ctx.item_end_after_dxa)
                    } else {
                        Some(ctx.item_gap_dxa.max(140))
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
                        ctx.line_dxa_override,
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
                    ..
                } => {
                    let child_trailing = if idx + 1 == blocks.len() {
                        Some(ctx.item_end_after_dxa)
                    } else {
                        Some(ctx.item_gap_dxa.max(140))
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
                        ctx.line_dxa_override,
                        out,
                    );
                }
                BlockNode::EquationBlock(eq) => {
                    let (before, after) = if !ctx.tight || idx + 1 < blocks.len() {
                        (110, 210)
                    } else {
                        (80, 80)
                    };
                    self.render_equation_block_with_layout(eq, before, after, indent_left, out);
                }
                BlockNode::Figure {
                    body,
                    caption,
                    caption_pos,
                    bookmark,
                    ..
                } => {
                    let caption_after_dxa = if idx + 1 == blocks.len() && !ctx.tight {
                        ctx.item_end_after_dxa
                    } else {
                        60
                    };
                    self.render_figure_with_layout(
                        body,
                        caption.as_deref(),
                        *caption_pos,
                        bookmark.as_deref(),
                        indent_left,
                        caption_after_dxa,
                        out,
                    );
                }
                other => {
                    self.render_block(other, out);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_list_item_first_paragraph(
        &mut self,
        id: NodeId,
        content: &[InlineNode],
        style: &ParagraphStyleIr,
        bookmark: Option<&str>,
        ctx: &ListItemConstruct<'_>,
        indent_left: usize,
        hanging_dxa: i64,
        after_dxa: i64,
        out: &mut String,
    ) {
        // Resolve calibrated `w:line` directly from `WordDocument` (`FontFaceMetricsIr` +
        // `ParagraphLineMetricsIr`), falling back to `ctx.line_dxa_override` only when the
        // paragraph carried neither pre-solved metrics nor an explicit `line_spacing_pt`.
        let line_dxa = ctx
            .line_dxa_override
            .filter(|_| style.line_spacing_pt.is_none() && !self.doc.solved_line_dxa.contains_key(&id))
            .unwrap_or_else(|| {
                self.doc
                    .resolve_paragraph_line_dxa(id, content, style.line_spacing_pt)
            });
        let has_wide_space = word_construct::paragraph::has_flush_right_horizontal_space(
            content,
            self.content_width_dxa(),
        );
        let tabs_xml = if has_wide_space {
            format!(
                r#"<w:tabs><w:tab w:val="num" w:pos="{indent_left}"/><w:tab w:val="right" w:pos="{}"/></w:tabs>"#,
                self.content_width_dxa()
            )
        } else {
            format!(r#"<w:tabs><w:tab w:val="num" w:pos="{indent_left}"/></w:tabs>"#)
        };
        // Honor semantic `sticky` (`<w:keepNext/>`) and `!breakable` (`<w:keepLines/>`) instead of
        // forcing multi-line items or parent stems of breakable child grids onto the next page.
        let keep_next_xml = match (style.sticky, !style.breakable) {
            (true, true) => "<w:keepNext/><w:keepLines/>",
            (true, false) => "<w:keepNext/>",
            (false, true) => "<w:keepLines/>",
            (false, false) => "",
        };
        let before_dxa = ctx.before_dxa;
        let num_id = ctx.num_id;
        let mark_rpr_sz = paragraph_mark_rpr_sz_half_pt(content);

        out.push_str("<w:p><w:pPr>");
        match ctx.marker {
            ListMarkerConstruct::CustomPrefix(prefix) => {
                let _ = write!(
                    out,
                    r#"{keep_next_xml}<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/><w:ind w:left="{indent_left}" w:hanging="{hanging_dxa}"/>{tabs_xml}"#
                );
                if let Some(sz_val) = mark_rpr_sz {
                    let _ = write!(
                        out,
                        r#"<w:rPr><w:sz w:val="{sz_val}"/><w:szCs w:val="{sz_val}"/></w:rPr>"#
                    );
                }
                out.push_str("</w:pPr>");
                let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
                let _ = write!(
                    out,
                    r#"<w:r><w:t xml:space="preserve">{} </w:t></w:r>"#,
                    escape_xml(prefix)
                );
                let prev_right_tab = self.in_right_tab_paragraph;
                self.in_right_tab_paragraph = has_wide_space;
                self.render_inlines(content, out);
                self.in_right_tab_paragraph = prev_right_tab;
                if let Some(id) = bm_id {
                    Self::emit_bookmark_end(id, out);
                }
                out.push_str("</w:p>");
            }
            ListMarkerConstruct::NumberedLevel(num_ilvl) => {
                let _ = write!(
                    out,
                    r#"{keep_next_xml}<w:numPr><w:ilvl w:val="{num_ilvl}"/><w:numId w:val="{num_id}"/></w:numPr>{tabs_xml}<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/><w:ind w:left="{indent_left}" w:hanging="{hanging_dxa}"/>"#
                );
                if let Some(sz_val) = mark_rpr_sz {
                    let _ = write!(
                        out,
                        r#"<w:rPr><w:sz w:val="{sz_val}"/><w:szCs w:val="{sz_val}"/></w:rPr>"#
                    );
                }
                out.push_str("</w:pPr>");
                let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
                let prev_right_tab = self.in_right_tab_paragraph;
                self.in_right_tab_paragraph = has_wide_space;
                self.render_inlines(content, out);
                self.in_right_tab_paragraph = prev_right_tab;
                if is_only_inline_equation(content) {
                    out.push_str("<w:r><w:t>&#8203;</w:t></w:r>");
                }
                if let Some(id) = bm_id {
                    Self::emit_bookmark_end(id, out);
                }
                out.push_str("</w:p>");
            }
        }
    }

    /// When a `List` / `Enum` item begins directly with a block object (`+ #coder(...)` or
    /// `+ ```java ... ````) rather than a `Paragraph`, place the list marker (`<w:numPr>` or
    /// custom prefix) in a top-aligned left column beside the block so Word renders `1.` / `•`
    /// at the top of the block instead of dropping it or baseline-aligning it to the bottom.
    fn render_list_item_marker_beside_block(
        &self,
        ctx: &ListItemConstruct<'_>,
        indent_left: usize,
        hanging_dxa: i64,
        marker_top_pad_dxa: i64,
        body_xml: &str,
        out: &mut String,
    ) {
        let marker_col_dxa = (indent_left as i64).max(260);
        let body_col_dxa = (self.content_width_dxa() - marker_col_dxa).max(400);
        let total_w_dxa = marker_col_dxa + body_col_dxa;
        let marker_left_dxa = (indent_left as i64 - hanging_dxa).max(0);
        let tab_pos = (marker_col_dxa - 20).max(marker_left_dxa + 120);
        let cell_hang = (tab_pos - marker_left_dxa).max(0);
        let before_dxa = ctx.before_dxa;

        let _ = write!(
            out,
            concat!(
                r#"<w:tbl><w:tblPr><w:tblW w:w="{total_w_dxa}" w:type="dxa"/>"#,
                r#"<w:tblBorders><w:top w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#,
                r#"<w:left w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#,
                r#"<w:bottom w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#,
                r#"<w:right w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#,
                r#"<w:insideH w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#,
                r#"<w:insideV w:val="none" w:sz="0" w:space="0" w:color="auto"/></w:tblBorders>"#,
                r#"<w:tblLayout w:type="fixed"/>"#,
                r#"<w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="0" w:type="dxa"/>"#,
                r#"<w:bottom w:w="0" w:type="dxa"/><w:right w:w="0" w:type="dxa"/></w:tblCellMar></w:tblPr>"#,
                r#"<w:tblGrid><w:gridCol w:w="{marker_col_dxa}"/><w:gridCol w:w="{body_col_dxa}"/></w:tblGrid>"#,
                r#"<w:tr><w:tc><w:tcPr><w:tcW w:w="{marker_col_dxa}" w:type="dxa"/>"#,
                r#"<w:tcMar><w:top w:w="{marker_top_pad_dxa}" w:type="dxa"/><w:left w:w="0" w:type="dxa"/>"#,
                r#"<w:bottom w:w="0" w:type="dxa"/><w:right w:w="0" w:type="dxa"/></w:tcMar>"#,
                r#"<w:vAlign w:val="top"/></w:tcPr><w:p><w:pPr>"#
            ),
            total_w_dxa = total_w_dxa,
            marker_col_dxa = marker_col_dxa,
            body_col_dxa = body_col_dxa,
            marker_top_pad_dxa = marker_top_pad_dxa,
        );
        match ctx.marker {
            ListMarkerConstruct::CustomPrefix(prefix) => {
                let _ = write!(
                    out,
                    r#"<w:spacing w:before="{before_dxa}" w:after="0" w:line="220" w:lineRule="auto"/><w:ind w:left="{marker_left_dxa}"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>"#,
                    escape_xml(prefix)
                );
            }
            ListMarkerConstruct::NumberedLevel(num_ilvl) => {
                let num_id = ctx.num_id;
                let _ = write!(
                    out,
                    r#"<w:numPr><w:ilvl w:val="{num_ilvl}"/><w:numId w:val="{num_id}"/></w:numPr><w:tabs><w:tab w:val="num" w:pos="{tab_pos}"/></w:tabs><w:spacing w:before="{before_dxa}" w:after="0" w:line="220" w:lineRule="auto"/><w:ind w:left="{tab_pos}" w:hanging="{cell_hang}"/></w:pPr></w:p>"#,
                );
            }
        }
        let trail_p = if !body_xml.contains("<w:p") || !body_xml.ends_with("</w:p>") {
            r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr></w:pPr></w:p>"#
        } else {
            ""
        };
        let _ = write!(
            out,
            r#"</w:tc><w:tc><w:tcPr><w:tcW w:w="{body_col_dxa}" w:type="dxa"/><w:vAlign w:val="top"/></w:tcPr>{body_xml}{trail_p}</w:tc></w:tr></w:tbl>"#
        );
    }

    fn render_list_item_child_table(
        &mut self,
        tbl: &TableIr,
        indent_left: usize,
        bottom_pad_pt: f64,
        out: &mut String,
    ) {
        let adjusted = solve_list_item_child_table(
            tbl,
            indent_left,
            self.content_width_dxa(),
            bottom_pad_pt,
        );
        self.render_table(&adjusted, out);
    }
}
