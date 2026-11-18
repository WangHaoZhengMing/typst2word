//! OpenType/TrueType binary font metric extraction (`head`, `hhea`, `OS/2`, `GSUB`, `post`).

use std::collections::BTreeMap;

use ir::FontFaceMetricsIr;

use crate::ExtractorContext;

impl<'a> ExtractorContext<'a> {
    /// Scan all laid-out `FrameItem::Text` items across the compiled document and extract exact
    /// OpenType/TrueType binary metrics (`head`, `hhea`, `OS/2`, `GSUB`, `post`) for every realized
    /// font family. When multiple faces of a family appear (e.g. Bold + Regular), prefers the
    /// Regular upright face so paragraph line-box metrics reflect body text.
    pub(crate) fn collect_document_font_metrics(&self) -> BTreeMap<String, FontFaceMetricsIr> {
        let mut chosen_fonts: BTreeMap<String, (bool, typst::text::Font)> = BTreeMap::new();
        for page in self.compiled.document.pages() {
            scan_frame_fonts(&page.frame, &mut chosen_fonts);
        }

        let mut out = BTreeMap::new();
        for (family, (_, font)) in chosen_fonts {
            if let Some(metrics) =
                parse_font_face_metrics(&font, self.world.is_system_font_family(&family))
            {
                out.insert(family, metrics);
            }
        }
        out
    }
}

fn scan_frame_fonts(
    frame: &typst::layout::Frame,
    chosen: &mut BTreeMap<String, (bool, typst::text::Font)>,
) {
    for (_, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Text(text) => {
                let info = text.font.info();
                let family = info.family.clone();
                let is_regular_upright = info.variant.weight == typst::text::FontWeight::REGULAR
                    && info.variant.style == typst::text::FontStyle::Normal;
                match chosen.get(&family) {
                    None => {
                        chosen.insert(family, (is_regular_upright, text.font.font().clone()));
                    }
                    Some((already_regular, _)) if !*already_regular && is_regular_upright => {
                        chosen.insert(family, (true, text.font.font().clone()));
                    }
                    _ => {}
                }
            }
            typst::layout::FrameItem::Group(grp) => {
                scan_frame_fonts(&grp.frame, chosen);
            }
            _ => {}
        }
    }
}

/// Parse OpenType/TrueType binary tables (`head`, `hhea`, `OS/2`, `GSUB`, `post`) from `font`.
fn parse_font_face_metrics(
    font: &typst::text::Font,
    is_system_installed: bool,
) -> Option<FontFaceMetricsIr> {
    let face = ttf_parser::Face::parse(font.data().as_slice(), font.index()).ok()?;
    let upem = face.units_per_em().max(1);
    let hhea_ascender = face.ascender();
    let hhea_descender = face.descender();
    let hhea_line_gap = face.line_gap();
    // When `OS/2` (or `sCapHeight` in `OS/2` v0/v1) is absent, fall back to `'H'`'s glyph
    // bounding box top (`y_max`), matching Typst's own `FontMetrics::from_ttf` definition.
    let s_cap_height = face.capital_height().or_else(|| {
        face.glyph_index('H')
            .and_then(|gid| face.glyph_bounding_box(gid))
            .map(|bbox| bbox.y_max)
            .filter(|&h| h > 0)
    });
    let s_typo_ascender = face.tables().os2.map(|t| t.typographic_ascender());
    let s_typo_descender = face.tables().os2.map(|t| t.typographic_descender());

    // Read unsigned `usWinAscent`, `usWinDescent`, and `ulCodePageRange1` directly from raw
    // `OS/2` bytes (`ttf-parser` 0.25's `windows_descender()` negates `usWinDescent` as `i16`
    // and does not expose `ulCodePageRange1` bits `17..=20` for CJK codepages).
    let mut us_win_ascent = None;
    let mut us_win_descent = None;
    let mut has_cjk_codepage = false;
    let mut has_os2_codepage_field = false;
    if let Some(os2) = face.raw_face().table(ttf_parser::Tag::from_bytes(b"OS/2")) {
        if os2.len() >= 78 {
            us_win_ascent = Some(u16::from_be_bytes([os2[74], os2[75]]));
            us_win_descent = Some(u16::from_be_bytes([os2[76], os2[77]]));
        }
        if os2.len() >= 86 {
            let version = u16::from_be_bytes([os2[0], os2[1]]);
            if version >= 1 {
                has_os2_codepage_field = true;
                let cp1 = u32::from_be_bytes([os2[78], os2[79], os2[80], os2[81]]);
                // Bits 17..=20: JIS/Japan (CP932), Chinese Simplified (CP936),
                // Korean Wansung (CP949), Chinese Traditional (CP950).
                has_cjk_codepage = (cp1 & 0x001E_0000) != 0;
            }
        }
    }
    // Apple AAT / legacy TrueType CJK fonts (such as macOS `Hei.ttf` / `Kai.ttf`) omit the
    // `OS/2` table (or use version 0 without `ulCodePageRange1`), yet Word's `<w:useFELayout/>`
    // still applies East Asian CJK line-box expansion (`1.3x`). Detect CJK coverage directly
    // from `cmap` when `ulCodePageRange1` is absent.
    if !has_os2_codepage_field {
        has_cjk_codepage = ['中', '文', '国', '字']
            .into_iter()
            .all(|c| face.glyph_index(c).is_some());
    }

    let has_gsub_hani = face
        .tables()
        .gsub
        .is_some_and(|g| g.scripts.find(ttf_parser::Tag::from_bytes(b"hani")).is_some());

    Some(FontFaceMetricsIr {
        upem,
        hhea_ascender,
        hhea_descender,
        hhea_line_gap,
        s_cap_height,
        s_typo_ascender,
        s_typo_descender,
        us_win_ascent,
        us_win_descent,
        has_cjk_codepage,
        has_gsub_hani,
        is_monospaced: face.is_monospaced(),
        is_system_installed,
    })
}
