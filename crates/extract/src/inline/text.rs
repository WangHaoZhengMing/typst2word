//! Laid-out `TextItem`s -> `TextRunIr` (pure Typst glyph & font extraction).

use typst::text::{FontWeight, TextItem};

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::inline::ActiveInlineState;
use ir::{InlineNode, TextRunIr, is_cjk_char};

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Conversion & Geometry Helpers
    // =========================================================================
    pub(crate) fn text_item_to_run(item: &TextItem, state: &ActiveInlineState) -> TextRunIr {
        let info = item.font.info();
        let family_lower = info.family.to_lowercase();
        let bold = state.bold
            || info.variant.weight >= FontWeight::SEMIBOLD
            || family_lower.contains("bold")
            || item.stroke.is_some();
        let italic = state.italic
            || matches!(
                info.variant.style,
                typst::text::FontStyle::Italic | typst::text::FontStyle::Oblique
            )
            || family_lower.contains("italic")
            || family_lower.contains("oblique");

        let color = paint_to_hex(&item.fill).filter(|hex| hex != "000000" || state.is_code);

        TextRunIr {
            text: item.text.to_string(),
            font_family: Some(info.family.to_string()),
            font_size_pt: Some(item.size.to_pt()),
            bold,
            italic,
            underline: state.underline.clone(),
            strike: state.strike.clone(),
            overline: state.overline.clone(),
            color,
            highlight: state.highlight.clone(),
            border: state.border.clone(),
            script: state.script,
            smallcaps: state.smallcaps,
            is_code: state.is_code,
            ..Default::default()
        }
    }

    pub(crate) fn strip_trailing_soft_hyphen_on_line_wrap(existing: &mut [InlineNode]) -> bool {
        if let Some(InlineNode::Text(last_run)) = existing.last_mut()
            && last_run.text.ends_with('\u{ad}')
        {
            while last_run.text.ends_with('\u{ad}') {
                last_run.text.pop();
            }
            return true;
        }
        false
    }

    pub(crate) fn should_insert_line_join_space(existing: &[InlineNode], next_text: &str) -> bool {
        let Some(InlineNode::Text(last_run)) = existing.last() else {
            return false;
        };
        let Some(last_ch) = last_run.text.chars().last() else {
            return false;
        };
        let Some(next_ch) = next_text.chars().next() else {
            return false;
        };
        if last_ch.is_whitespace() || next_ch.is_whitespace() {
            return false;
        }
        // Do not insert spaces after hyphens/slashes when wrapping compound words across lines.
        if matches!(last_ch, '\u{ad}' | '-' | '\u{2010}' | '\u{2013}' | '\u{2014}' | '/') {
            return false;
        }
        // Do not insert spaces between CJK characters when wrapping lines.
        if is_cjk_char(last_ch) || is_cjk_char(next_ch) {
            return false;
        }
        true
    }
}
