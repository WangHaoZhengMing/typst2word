//! Per-item block rendering (`render_list_or_enum_item_blocks`, first paragraph, child tables) for lists/enums.

use std::fmt::Write as _;

use super::{ListItemRenderCtx, ListMarkerKind};
use crate::docx::builder::DocxBuilder;
use crate::ir::{BlockNode, HAlignIr, InlineNode, ParagraphStyleIr, TableIr};
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    pub(super) fn render_list_or_enum_item_blocks(
        &mut self,
        blocks: &[BlockNode],
        ctx: &ListItemRenderCtx<'_>,
        out: &mut String,
    ) {
        let followed_by_grid_table = matches!(
            blocks.get(1),
            Some(BlockNode::Table(t)) if !t.is_semantic()
        );
        let followed_by_tight_list = matches!(
            blocks.get(1),
            Some(BlockNode::List { tight: true, .. } | BlockNode::Enum { tight: true, .. })
        );
        let followed_by_paragraph = matches!(blocks.get(1), Some(BlockNode::Paragraph { .. }));
        let (first_para_space_after_pt, first_para_line_spacing_pt) = match blocks.first() {
            Some(BlockNode::Paragraph { style, .. }) => {
                (Some(style.space_after_pt), style.line_spacing_pt)
            }
            _ => (None, None),
        };
        let following_block = word_construct::FollowingBlockKind::from_flags(
            blocks.len(),
            followed_by_grid_table,
            followed_by_tight_list,
            followed_by_paragraph,
        );
        let item_layout =
            word_construct::WordListItemLayout::solve_from_input(word_construct::ListItemLayoutInput {
                ilvl: ctx.ilvl,
                tight: ctx.tight,
                indent_pt: ctx.indent_pt,
                hanging_pt: ctx.hanging_pt,
                marker_width: word_construct::EnumMarkerWidthKind::from_bool(
                    ctx.is_single_digit_enum_item,
                ),
                item_gap_dxa: ctx.item_gap_dxa,
                item_end_after_dxa: ctx.item_end_after_dxa,
                before_dxa: ctx.before_dxa,
                num_blocks: blocks.len(),
                first_para_space_after_pt,
                first_para_line_spacing_pt,
                following_block,
            });
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
        ctx: &ListItemRenderCtx<'_>,
        indent_left: usize,
        hanging_dxa: i64,
        after_dxa: i64,
        keep_with_next: bool,
        out: &mut String,
    ) {
        let line_dxa = Self::compute_line_spacing_dxa(content, style.line_spacing_pt);
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
            ListMarkerKind::CustomPrefix(prefix) => {
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
            ListMarkerKind::NumberedLevel(num_ilvl) => {
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
        let mut adjusted = tbl.clone();
        let target_left_pt = indent_left as f64 / 20.0;
        if !adjusted.is_semantic() && adjusted.align == HAlignIr::Center {
            let avail_w_pt = ((self.content_width_dxa() as f64 / 20.0) - target_left_pt).max(100.0);
            let total_col_w: f64 = adjusted.col_widths_pt.iter().sum();
            adjusted.align = HAlignIr::Left;
            adjusted.left_indent_pt = target_left_pt + ((avail_w_pt - total_col_w).max(0.0) * 0.5);
        } else if !adjusted.is_semantic() && adjusted.align == HAlignIr::Left {
            adjusted.left_indent_pt = target_left_pt;
            let avail_w_pt = ((self.content_width_dxa() as f64 / 20.0) - target_left_pt).max(100.0);
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
        if bottom_pad_pt > 0.0
            && let Some(last_row) = adjusted.rows.last_mut()
        {
            for cell in &mut last_row.cells {
                cell.inset.bottom += bottom_pad_pt;
            }
        }
        self.render_table(&adjusted, out);
    }
}
