//! Per-item block rendering (`render_list_or_enum_item_blocks`, first paragraph, child tables) for lists/enums.

use std::fmt::Write as _;

use crate::construct::list::{ListItemConstruct, ListMarkerConstruct};
use crate::construct::table::solve_list_item_child_table;
use crate::docx::builder::DocxBuilder;
use crate::ir::{BlockNode, InlineNode, ParagraphStyleIr, TableIr};
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    pub(super) fn render_list_or_enum_item_blocks(
        &mut self,
        blocks: &[BlockNode],
        ctx: &ListItemConstruct<'_>,
        out: &mut String,
    ) {
        let followed_by_grid_table = matches!(
            blocks.get(1),
            Some(BlockNode::Table(t)) if !t.is_semantic()
        );
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

        let keep_first_with_next = followed_by_grid_table
            && matches!(blocks.get(1), Some(BlockNode::Table(t)) if t.rows.len() <= 4);

        for (idx, b) in blocks.iter().enumerate() {
            match b {
                BlockNode::Paragraph {
                    content,
                    style,
                    bookmark,
                } if idx == 0 => {
                    self.render_list_item_first_paragraph(
                        content,
                        style,
                        bookmark.as_deref(),
                        ctx,
                        indent_left,
                        hanging_dxa,
                        after_dxa,
                        keep_first_with_next,
                        out,
                    );
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
                        cont_style.space_after_pt = cont_style
                            .space_after_pt
                            .max(ctx.item_end_after_dxa as f64 / 20.0);
                    }
                    self.render_paragraph_with_keep_next(
                        content,
                        &cont_style,
                        bookmark.as_deref(),
                        keep_with_next,
                        out,
                    );
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
                    let is_last_block = idx + 1 == blocks.len();
                    let bottom_pad_pt = if is_last_block && ctx.item_gap_dxa > 0 && ctx.is_last_top
                    {
                        4.5
                    } else if !is_last_block && !tbl.is_semantic() {
                        5.5
                    } else {
                        0.0
                    };
                    self.render_list_item_child_table(tbl, indent_left, bottom_pad_pt, out);
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
                        180
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
        content: &[InlineNode],
        style: &ParagraphStyleIr,
        bookmark: Option<&str>,
        ctx: &ListItemConstruct<'_>,
        indent_left: usize,
        hanging_dxa: i64,
        after_dxa: i64,
        keep_with_next: bool,
        out: &mut String,
    ) {
        let raw_line_dxa = Self::compute_line_spacing_dxa(content, style.line_spacing_pt);
        let line_dxa = match (ctx.line_dxa_override, raw_line_dxa) {
            (Some(override_dxa), 276) => override_dxa,
            _ => raw_line_dxa,
        };
        let tabs_xml = if Self::has_large_horizontal_space(content) {
            format!(
                r#"<w:tabs><w:tab w:val="num" w:pos="{indent_left}"/><w:tab w:val="right" w:pos="{}"/></w:tabs>"#,
                self.content_width_dxa()
            )
        } else {
            format!(r#"<w:tabs><w:tab w:val="num" w:pos="{indent_left}"/></w:tabs>"#)
        };
        let keep_next_xml = if keep_with_next {
            "<w:keepNext/><w:keepLines/>"
        } else {
            "<w:keepLines/>"
        };
        let before_dxa = ctx.before_dxa;
        let num_id = ctx.num_id;

        out.push_str("<w:p><w:pPr>");
        match ctx.marker {
            ListMarkerConstruct::CustomPrefix(prefix) => {
                let _ = write!(
                    out,
                    r#"{keep_next_xml}<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/><w:ind w:left="{indent_left}" w:hanging="{hanging_dxa}"/>{tabs_xml}"#
                );
                Self::render_paragraph_mark_rpr(content, out);
                out.push_str("</w:pPr>");
                let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
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
            }
            ListMarkerConstruct::NumberedLevel(num_ilvl) => {
                let _ = write!(
                    out,
                    r#"{keep_next_xml}<w:numPr><w:ilvl w:val="{num_ilvl}"/><w:numId w:val="{num_id}"/></w:numPr>{tabs_xml}<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/><w:ind w:left="{indent_left}" w:hanging="{hanging_dxa}"/>"#
                );
                Self::render_paragraph_mark_rpr(content, out);
                out.push_str("</w:pPr>");
                let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
                self.render_inlines(content, out);
                if Self::is_only_inline_equation(content) {
                    out.push_str("<w:r><w:t>&#8203;</w:t></w:r>");
                }
                if let Some(id) = bm_id {
                    Self::emit_bookmark_end(id, out);
                }
                out.push_str("</w:p>");
            }
        }
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
