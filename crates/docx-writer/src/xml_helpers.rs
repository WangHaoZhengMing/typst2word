//! Typed WordprocessingML (`w:*`) element & minimal spacer emitters (`xml_helpers`).
//!
//! Centralizes recurring low-level OOXML idioms (`1pt` paragraph marks, `<w:spacing .../>`,
//! exact-height spacer paragraphs, section-break carriers, and `<w:tcMar>`) so block/part
//! serializers stay concise and typo-free while emitting byte-identical XML.

use std::fmt::Write as _;

use word_construct::WordCellMarginsDxa;

/// `1pt` (`2` half-pt) `<w:rPr>` paragraph/run mark preventing Word's `11pt` Normal mark from
/// inflating exact-height spacers, section-break carriers, and DrawingML wrapper paragraphs.
pub(crate) const TINY_MARK_RPR_XML: &str = r#"<w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr>"#;

/// Format `<w:rPr><w:sz w:val="..."/><w:szCs w:val="..."/></w:rPr>` for a paragraph mark size in half-points.
#[inline]
pub(crate) fn format_mark_rpr_half_pt(half_pt: u32) -> String {
    format!(r#"<w:rPr><w:sz w:val="{half_pt}"/><w:szCs w:val="{half_pt}"/></w:rPr>"#)
}

/// Emit `<w:spacing w:before="..." w:after="..." w:line="..." w:lineRule="auto|exact"/>`.
#[inline]
pub(crate) fn write_w_spacing(
    before_dxa: i64,
    after_dxa: i64,
    line_dxa: i64,
    is_exact: bool,
    out: &mut String,
) {
    let rule = if is_exact { "exact" } else { "auto" };
    let _ = write!(
        out,
        r#"<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="{rule}"/>"#,
    );
}

/// Emit an empty exact-height spacer paragraph (`w:before="0"`, `w:lineRule="exact"`, `1pt` mark).
#[inline]
pub(crate) fn write_exact_spacer_p(
    after_dxa: i64,
    line_dxa: i64,
    keep_next: bool,
    out: &mut String,
) {
    out.push_str("<w:p><w:pPr>");
    if keep_next {
        out.push_str("<w:keepNext/>");
    }
    write_w_spacing(0, after_dxa, line_dxa, true, out);
    out.push_str(TINY_MARK_RPR_XML);
    out.push_str("</w:pPr></w:p>");
}

/// Open a `1pt` (`20 dxa` exact) section-break carrier `<w:p><w:pPr>...` ready for `<w:sectPr>` + `</w:pPr></w:p>`.
#[inline]
pub(crate) fn open_tiny_sect_carrier_p(out: &mut String) {
    out.push_str("<w:p><w:pPr>");
    write_w_spacing(0, 0, 20, true, out);
    out.push_str(TINY_MARK_RPR_XML);
}

/// Emit a standalone `1pt` page-break or column-break paragraph (`break_type` = `"page"` or `"column"`).
#[inline]
pub(crate) fn write_tiny_break_p(break_type: &str, out: &mut String) {
    let _ = write!(
        out,
        r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/>{TINY_MARK_RPR_XML}</w:pPr><w:r>{TINY_MARK_RPR_XML}<w:br w:type="{break_type}"/></w:r></w:p>"#,
    );
}

/// Emit `<w:tcMar>` from a solved [`WordCellMarginsDxa`].
#[inline]
pub(crate) fn write_tc_mar(margins: WordCellMarginsDxa, out: &mut String) {
    let _ = write!(
        out,
        r#"<w:tcMar><w:top w:w="{}" w:type="dxa"/><w:left w:w="{}" w:type="dxa"/><w:bottom w:w="{}" w:type="dxa"/><w:right w:w="{}" w:type="dxa"/></w:tcMar>"#,
        margins.top_dxa, margins.left_dxa, margins.bottom_dxa, margins.right_dxa,
    );
}
