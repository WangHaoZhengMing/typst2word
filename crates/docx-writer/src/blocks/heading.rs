//! `title` / `heading` -> `HeadingConstruct` -> `Title` / `HeadingN` paragraphs.

use std::fmt::Write as _;

use word_construct::heading::HeadingConstruct;
use crate::writer::DocxWriter;
use word_construct::model::{ContainerStyleIr, HAlignIr, InlineNode, StrokeIr};

impl<'a> DocxWriter<'a> {
    /// `BlockNode::Title` -> `Title` styled paragraph.
    pub(crate) fn render_title(
        &mut self,
        content: &[InlineNode],
        align: HAlignIr,
        out: &mut String,
    ) {
        out.push_str("<w:p><w:pPr><w:pStyle w:val=\"Title\"/><w:spacing w:before=\"0\" w:after=\"120\" w:line=\"216\" w:lineRule=\"auto\"/>");
        Self::render_jc(align, out);
        out.push_str("</w:pPr>");
        self.render_inlines(content, out);
        out.push_str("</w:p>");
    }

    /// `BlockNode::Heading` -> `HeadingN` styled paragraph with optional numbering prefix and box style.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_heading(
        &mut self,
        level: usize,
        numbering: Option<&str>,
        content: &[InlineNode],
        bookmark: Option<&str>,
        align: HAlignIr,
        box_style: Option<&ContainerStyleIr>,
        out: &mut String,
    ) {
        self.render_heading_inner(
            level, numbering, content, bookmark, align, box_style, false, false, out,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_heading_inner(
        &mut self,
        level: usize,
        numbering: Option<&str>,
        content: &[InlineNode],
        bookmark: Option<&str>,
        align: HAlignIr,
        box_style: Option<&ContainerStyleIr>,
        collapse_before: bool,
        compact_after: bool,
        out: &mut String,
    ) {
        let construct = HeadingConstruct::solve(
            level,
            numbering,
            content,
            align,
            collapse_before,
            compact_after,
        );

        out.push_str("<w:p><w:pPr>");
        let _ = write!(out, r#"<w:pStyle w:val="{}"/>"#, construct.style_id);
        if let Some(bs) = box_style {
            if bs.borders.has_any() || bs.fill.is_some() {
                out.push_str("<w:pBdr>");
                Self::render_p_border("top", bs.borders.top.as_ref(), bs.inset.top, bs.fill.as_deref(), out);
                Self::render_p_border("left", bs.borders.left.as_ref(), bs.inset.left, bs.fill.as_deref(), out);
                Self::render_p_border("bottom", bs.borders.bottom.as_ref(), bs.inset.bottom, bs.fill.as_deref(), out);
                Self::render_p_border("right", bs.borders.right.as_ref(), bs.inset.right, bs.fill.as_deref(), out);
                out.push_str("</w:pBdr>");
            }
            if let Some(fill_hex) = &bs.fill {
                let _ = write!(
                    out,
                    r#"<w:shd w:val="clear" w:color="auto" w:fill="{fill_hex}"/>"#
                );
            }
        }
        let _ = write!(
            out,
            r#"<w:spacing w:before="{}" w:after="{}" w:line="{}" w:lineRule="auto"/>"#,
            construct.before_dxa, construct.after_dxa, construct.line_dxa
        );
        if construct.align != HAlignIr::Left {
            Self::render_jc(construct.align, out);
        }
        out.push_str("</w:pPr>");

        let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
        if let Some(prefix_run) = &construct.prefix_run {
            self.render_text_run(prefix_run, out);
        }
        self.render_inlines(content, out);
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }

    fn render_p_border(
        side: &str,
        stroke: Option<&StrokeIr>,
        inset_pt: f64,
        fill_hex: Option<&str>,
        out: &mut String,
    ) {
        let space = (inset_pt.round() as u32).clamp(0, 31);
        if let Some(s) = stroke {
            let sz = ((s.thickness_pt * 8.0).round() as u32).clamp(2, 96);
            let val = Self::dash_to_w_border_val(&s.dash);
            let _ = write!(
                out,
                r#"<w:{side} w:val="{val}" w:sz="{sz}" w:space="{space}" w:color="{}"/>"#,
                s.color
            );
        } else if let Some(bg) = fill_hex
            && space > 0
        {
            // Invisible matching border preserves `w:space` padding around shaded `<w:shd>` headings.
            let _ = write!(
                out,
                r#"<w:{side} w:val="single" w:sz="2" w:space="{space}" w:color="{bg}"/>"#
            );
        }
    }
}
