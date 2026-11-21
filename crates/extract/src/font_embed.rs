//! Font collection, `.ttc` single-face SFNT extraction, and OpenType subsetting (`allsorts`)
//! for `.docx` font embedding (`FontEmbedMode::Subset` / `Full` / `None`).

use std::collections::{BTreeMap, BTreeSet};

use allsorts::binary::read::ReadScope;
use allsorts::subset::{CmapTarget, SubsetProfile};
use allsorts::tables::OpenTypeFont;
use ir::{EmbeddedFontFaceIr, EmbeddedFontSlot, FontEmbedMode};

use crate::ExtractorContext;

struct CollectedFace {
    typst_family: String,
    slot: EmbeddedFontSlot,
    font: typst::text::Font,
    used_chars: BTreeSet<char>,
    used_glyph_ids: BTreeSet<u16>,
}

pub(crate) struct ExtractedEmbeddedFonts {
    pub(crate) faces: Vec<EmbeddedFontFaceIr>,
    pub(crate) math_font_family: Option<String>,
    pub(crate) embedded_typst_families: BTreeSet<String>,
    pub(crate) notes: Vec<String>,
}

impl<'a> ExtractorContext<'a> {
    /// Collect all realized font faces across all pages and prepare standalone SFNT payloads
    /// according to `embed_mode`.
    pub(crate) fn collect_embedded_fonts(
        &self,
        embed_mode: FontEmbedMode,
    ) -> ExtractedEmbeddedFonts {
        if embed_mode == FontEmbedMode::None {
            return ExtractedEmbeddedFonts {
                faces: Vec::new(),
                math_font_family: None,
                embedded_typst_families: BTreeSet::new(),
                notes: Vec::new(),
            };
        }

        let mut collected: BTreeMap<(String, EmbeddedFontSlot), CollectedFace> = BTreeMap::new();
        for page in self.compiled.document.pages() {
            scan_frame_font_usage(&page.frame, &mut collected);
        }

        let mut faces = Vec::new();
        let math_font_family: Option<String> = None;
        let mut embedded_typst_families = BTreeSet::new();
        let mut total_bytes: usize = 0;
        let mut any_subsetted = false;

        for ((_family, _slot), item) in collected {
            let raw = item.font.data().as_slice();
            let face_index = item.font.index();
            let Ok(ttf_face) = ttf_parser::Face::parse(raw, face_index) else {
                continue;
            };

            let is_math = ttf_face
                .raw_face()
                .table(ttf_parser::Tag::from_bytes(b"MATH"))
                .is_some();

            // Skip embedding OpenType `MATH` fonts (such as Typst's CFF `NewCMMath-*.otf`):
            // Microsoft Word's `<m:oMathPara>` display-equation renderer drops equations when
            // `<m:mathFont>` is bound to an embedded `.odttf` CFF math font. Keeping Word's
            // built-in `Cambria Math` ensures 100% reliable OMML rendering and keeps `.docx`
            // files ~1 MB smaller.
            if is_math {
                continue;
            }

            let word_family =
                extract_windows_family_name(&ttf_face).unwrap_or_else(|| item.typst_family.clone());

            let (sfnt_data, is_subset) = if embed_mode == FontEmbedMode::Subset {
                let mut glyph_ids = item.used_glyph_ids;
                glyph_ids.insert(0); // `.notdef` is required as glyph 0
                // Ensure ASCII space, digits `0-9` (for dynamic Word `PAGE`/`NUMPAGES`/`PAGEREF`
                // and list-numbering fields), and every used character's cmap glyph are included.
                if let Some(gid) = ttf_face.glyph_index(' ') {
                    glyph_ids.insert(gid.0);
                }
                for digit in '0'..='9' {
                    if let Some(gid) = ttf_face.glyph_index(digit) {
                        glyph_ids.insert(gid.0);
                    }
                }
                for ch in &item.used_chars {
                    if let Some(gid) = ttf_face.glyph_index(*ch) {
                        glyph_ids.insert(gid.0);
                    }
                }
                match subset_font_face(raw, face_index, &glyph_ids) {
                    Some(bytes) => (bytes, true),
                    None => match extract_standalone_sfnt(raw, face_index) {
                        Some(bytes) => (bytes, false),
                        None => continue,
                    },
                }
            } else {
                match extract_standalone_sfnt(raw, face_index) {
                    Some(bytes) => (bytes, false),
                    None => continue,
                }
            };

            if is_subset {
                any_subsetted = true;
            }
            total_bytes += sfnt_data.len();
            embedded_typst_families.insert(item.typst_family.clone());
            faces.push(EmbeddedFontFaceIr {
                typst_family: item.typst_family,
                word_family,
                slot: item.slot,
                is_math: false,
                is_subset,
                sfnt_data,
            });
        }

        let mut notes = Vec::new();
        if !faces.is_empty() {
            let kb = (total_bytes + 512) / 1024;
            if embed_mode == FontEmbedMode::Subset && any_subsetted {
                notes.push(format!(
                    "已子集嵌入 {} 个字体（约 {} KB）；在 Word 中输入文档未出现的新字符时将回退到系统字体，如需完整可编辑请使用 --embed-fonts=full",
                    faces.len(),
                    kb
                ));
            } else {
                notes.push(format!(
                    "已完整嵌入 {} 个字体（约 {} KB）",
                    faces.len(),
                    kb
                ));
            }
        }

        ExtractedEmbeddedFonts {
            faces,
            math_font_family,
            embedded_typst_families,
            notes,
        }
    }
}

fn scan_frame_font_usage(
    frame: &typst::layout::Frame,
    collected: &mut BTreeMap<(String, EmbeddedFontSlot), CollectedFace>,
) {
    for (_, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Text(text) => {
                let info = text.font.info();
                let family = info.family.clone();
                let is_bold = info.variant.weight.to_number() >= 600;
                let is_italic = info.variant.style != typst::text::FontStyle::Normal;
                let slot = match (is_bold, is_italic) {
                    (false, false) => EmbeddedFontSlot::Regular,
                    (true, false) => EmbeddedFontSlot::Bold,
                    (false, true) => EmbeddedFontSlot::Italic,
                    (true, true) => EmbeddedFontSlot::BoldItalic,
                };
                let entry = collected
                    .entry((family.clone(), slot))
                    .or_insert_with(|| CollectedFace {
                        typst_family: family,
                        slot,
                        font: text.font.font().clone(),
                        used_chars: BTreeSet::new(),
                        used_glyph_ids: BTreeSet::new(),
                    });
                for ch in text.text.chars() {
                    entry.used_chars.insert(ch);
                }
                for glyph in &text.glyphs {
                    entry.used_glyph_ids.insert(glyph.id);
                }
            }
            typst::layout::FrameItem::Group(grp) => {
                scan_frame_font_usage(&grp.frame, collected);
            }
            _ => {}
        }
    }
}

/// Extract OpenType `nameID 1` (Windows Font Family Name), preferring Platform 3 (Windows)
/// English US (`0x0409`), which Microsoft Word uses to match `<w:font w:name="...">` and `<m:mathFont>`.
fn extract_windows_family_name(face: &ttf_parser::Face<'_>) -> Option<String> {
    let mut fallback_win: Option<String> = None;
    let mut fallback_any: Option<String> = None;
    for name in face.names() {
        if name.name_id != ttf_parser::name_id::FAMILY {
            continue;
        }
        let Some(s) = name.to_string() else {
            continue;
        };
        let trimmed = s.trim();
        if trimmed.is_empty() {
            continue;
        }
        if name.platform_id == ttf_parser::PlatformId::Windows {
            if name.language_id == 0x0409 {
                return Some(trimmed.to_string());
            }
            if fallback_win.is_none() {
                fallback_win = Some(trimmed.to_string());
            }
        } else if fallback_any.is_none() {
            fallback_any = Some(trimmed.to_string());
        }
    }
    fallback_win.or(fallback_any)
}

/// Subset a TrueType (`.ttf`), OpenType CFF (`.otf`), or `.ttc` face using `allsorts`,
/// preserving `cmap` (Unicode), `name`, `OS/2`, `head`, `hhea`, `hmtx`, `maxp`, `post`.
fn subset_font_face(raw: &[u8], face_index: u32, used_glyphs: &BTreeSet<u16>) -> Option<Vec<u8>> {
    let scope = ReadScope::new(raw);
    let otf = scope.read::<OpenTypeFont<'_>>().ok()?;
    let provider = otf.table_provider(face_index as usize).ok()?;
    let glyph_ids: Vec<u16> = used_glyphs.iter().copied().collect();
    if glyph_ids.is_empty() || glyph_ids[0] != 0 {
        return None;
    }
    let subset_bytes = allsorts::subset::subset(
        &provider,
        &glyph_ids,
        &SubsetProfile::Minimal,
        CmapTarget::Unicode,
    )
    .ok()?;
    if subset_bytes.len() < 12 {
        return None;
    }
    Some(ensure_windows_cmap_subtable(subset_bytes))
}

/// Microsoft Word requires a Platform `3` (Windows) `cmap` subtable (`(3, 1)` for BMP format 4
/// and/or `(3, 10)` for full Unicode format 12). `allsorts`'s `CmapTarget::Unicode` only emits
/// Platform `0` (`(0, 3)` / `(0, 4)`), which shares the exact same subtable binary format.
/// This helper aliases Platform `0` subtables to Platform `3` and updates SFNT offsets & checksums.
fn ensure_windows_cmap_subtable(sfnt: Vec<u8>) -> Vec<u8> {
    if sfnt.len() < 12 {
        return sfnt;
    }
    let num_tables = u16::from_be_bytes([sfnt[4], sfnt[5]]) as usize;
    let header_size = 12 + num_tables * 16;
    if header_size > sfnt.len() {
        return sfnt;
    }

    let mut tables: Vec<([u8; 4], u32, Vec<u8>)> = Vec::with_capacity(num_tables);
    let mut cmap_idx: Option<usize> = None;
    for i in 0..num_tables {
        let rec = 12 + i * 16;
        let tag = [sfnt[rec], sfnt[rec + 1], sfnt[rec + 2], sfnt[rec + 3]];
        let checksum = u32::from_be_bytes([sfnt[rec + 4], sfnt[rec + 5], sfnt[rec + 6], sfnt[rec + 7]]);
        let offset = u32::from_be_bytes([sfnt[rec + 8], sfnt[rec + 9], sfnt[rec + 10], sfnt[rec + 11]]) as usize;
        let length = u32::from_be_bytes([sfnt[rec + 12], sfnt[rec + 13], sfnt[rec + 14], sfnt[rec + 15]]) as usize;
        if offset.checked_add(length).is_none_or(|end| end > sfnt.len()) {
            return sfnt;
        }
        if &tag == b"cmap" {
            cmap_idx = Some(i);
        }
        tables.push((tag, checksum, sfnt[offset..offset + length].to_vec()));
    }

    let Some(ci) = cmap_idx else {
        return sfnt;
    };
    let cmap = &tables[ci].2;
    if cmap.len() < 4 {
        return sfnt;
    }
    let version = u16::from_be_bytes([cmap[0], cmap[1]]);
    let nsub = u16::from_be_bytes([cmap[2], cmap[3]]) as usize;
    if 4 + nsub * 8 > cmap.len() {
        return sfnt;
    }

    let mut records: Vec<(u16, u16, u32)> = Vec::with_capacity(nsub + 2);
    let mut has_win = false;
    for j in 0..nsub {
        let r = 4 + j * 8;
        let pid = u16::from_be_bytes([cmap[r], cmap[r + 1]]);
        let eid = u16::from_be_bytes([cmap[r + 2], cmap[r + 3]]);
        let off = u32::from_be_bytes([cmap[r + 4], cmap[r + 5], cmap[r + 6], cmap[r + 7]]);
        if pid == 3 {
            has_win = true;
        }
        records.push((pid, eid, off));
    }
    if has_win {
        return sfnt;
    }

    let mut added: Vec<(u16, u16, u32)> = Vec::new();
    for &(pid, eid, off) in &records {
        if pid == 0 && eid == 3 {
            added.push((3, 1, off));
        } else if pid == 0 && eid == 4 {
            added.push((3, 10, off));
        }
    }
    if added.is_empty() {
        // Fallback: alias the first Unicode subtable to Windows Unicode BMP `(3, 1)`.
        if let Some(&(_, _, off)) = records.first() {
            added.push((3, 1, off));
        } else {
            return sfnt;
        }
    }

    let delta = (added.len() * 8) as u32;
    let mut new_records: Vec<(u16, u16, u32)> = records
        .into_iter()
        .chain(added)
        .map(|(p, e, o)| (p, e, o + delta))
        .collect();
    new_records.sort_by_key(|&(p, e, _)| (p, e));

    let subtable_payload = &cmap[4 + nsub * 8..];
    let mut new_cmap = Vec::with_capacity(4 + new_records.len() * 8 + subtable_payload.len());
    new_cmap.extend_from_slice(&version.to_be_bytes());
    new_cmap.extend_from_slice(&(new_records.len() as u16).to_be_bytes());
    for (p, e, o) in new_records {
        new_cmap.extend_from_slice(&p.to_be_bytes());
        new_cmap.extend_from_slice(&e.to_be_bytes());
        new_cmap.extend_from_slice(&o.to_be_bytes());
    }
    new_cmap.extend_from_slice(subtable_payload);

    let new_cmap_checksum = sfnt_table_checksum(&new_cmap);
    tables[ci].1 = new_cmap_checksum;
    tables[ci].2 = new_cmap;

    // Rebuild SFNT container with updated offsets and `head.checkSumAdjustment`.
    let mut out = Vec::with_capacity(
        header_size
            + tables
                .iter()
                .map(|(_, _, d)| (d.len() + 3) & !3)
                .sum::<usize>(),
    );
    out.extend_from_slice(&sfnt[0..12]);
    let mut cur_off = header_size;
    let mut head_off_in_out: Option<usize> = None;
    for (tag, csum, data) in &tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&csum.to_be_bytes());
        out.extend_from_slice(&(cur_off as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        if tag == b"head" {
            head_off_in_out = Some(cur_off);
        }
        cur_off += (data.len() + 3) & !3;
    }
    for (_, _, data) in &tables {
        out.extend_from_slice(data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    if let Some(head_off) = head_off_in_out
        && head_off + 12 <= out.len()
    {
        out[head_off + 8..head_off + 12].copy_from_slice(&[0, 0, 0, 0]);
        let adj = 0xB1B0_AFBAu32.wrapping_sub(sfnt_table_checksum(&out));
        out[head_off + 8..head_off + 12].copy_from_slice(&adj.to_be_bytes());
    }
    out
}

fn sfnt_table_checksum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    let mut chunks = data.chunks_exact(4);
    for chunk in &mut chunks {
        sum = sum.wrapping_add(u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    let rem = chunks.remainder();
    if !rem.is_empty() {
        let mut last = [0u8; 4];
        last[..rem.len()].copy_from_slice(rem);
        sum = sum.wrapping_add(u32::from_be_bytes(last));
    }
    sum
}

/// Extract a standalone SFNT (`.ttf` or `.otf`) binary from `raw`.
/// - If `raw` is already a standalone SFNT (`0x00010000`, `OTTO`, `true`), returns it directly.
/// - If `raw` is a TrueType Collection (`ttcf`), extracts the face at `face_index` into a valid
///   standalone SFNT with recalculated table offsets and `head.checkSumAdjustment`.
pub(crate) fn extract_standalone_sfnt(raw: &[u8], face_index: u32) -> Option<Vec<u8>> {
    if raw.len() < 12 {
        return None;
    }
    let magic = &raw[0..4];
    if magic == b"\x00\x01\x00\x00" || magic == b"OTTO" || magic == b"true" {
        return Some(raw.to_vec());
    }
    if magic != b"ttcf" {
        return None;
    }

    let num_fonts = u32::from_be_bytes([raw[8], raw[9], raw[10], raw[11]]);
    if face_index >= num_fonts {
        return None;
    }
    let idx_off = 12usize + (face_index as usize) * 4;
    if idx_off + 4 > raw.len() {
        return None;
    }
    let face_off = u32::from_be_bytes([
        raw[idx_off],
        raw[idx_off + 1],
        raw[idx_off + 2],
        raw[idx_off + 3],
    ]) as usize;
    if face_off + 12 > raw.len() {
        return None;
    }

    let sfnt_version = &raw[face_off..face_off + 4];
    let num_tables = u16::from_be_bytes([raw[face_off + 4], raw[face_off + 5]]) as usize;
    let header_size = 12 + num_tables * 16;
    if face_off + header_size > raw.len() {
        return None;
    }

    struct TableEntry<'a> {
        tag: [u8; 4],
        checksum: u32,
        data: &'a [u8],
    }

    let mut tables = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let rec = face_off + 12 + i * 16;
        let tag = [raw[rec], raw[rec + 1], raw[rec + 2], raw[rec + 3]];
        let checksum = u32::from_be_bytes([raw[rec + 4], raw[rec + 5], raw[rec + 6], raw[rec + 7]]);
        let offset = u32::from_be_bytes([raw[rec + 8], raw[rec + 9], raw[rec + 10], raw[rec + 11]])
            as usize;
        let length =
            u32::from_be_bytes([raw[rec + 12], raw[rec + 13], raw[rec + 14], raw[rec + 15]])
                as usize;
        if offset.checked_add(length).is_none_or(|end| end > raw.len()) {
            return None;
        }
        tables.push(TableEntry {
            tag,
            checksum,
            data: &raw[offset..offset + length],
        });
    }

    let mut out = Vec::with_capacity(
        header_size
            + tables
                .iter()
                .map(|t| (t.data.len() + 3) & !3)
                .sum::<usize>(),
    );
    out.extend_from_slice(sfnt_version);
    out.extend_from_slice(&(num_tables as u16).to_be_bytes());
    out.extend_from_slice(&raw[face_off + 6..face_off + 12]); // searchRange, entrySelector, rangeShift

    let mut cur_offset = header_size;
    let mut head_offset_in_out: Option<usize> = None;
    for t in &tables {
        out.extend_from_slice(&t.tag);
        out.extend_from_slice(&t.checksum.to_be_bytes());
        out.extend_from_slice(&(cur_offset as u32).to_be_bytes());
        out.extend_from_slice(&(t.data.len() as u32).to_be_bytes());
        if &t.tag == b"head" {
            head_offset_in_out = Some(cur_offset);
        }
        cur_offset += (t.data.len() + 3) & !3;
    }

    for t in &tables {
        out.extend_from_slice(t.data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }

    // Recalculate `head.checkSumAdjustment` = 0xB1B0AFBA - sfnt_checksum.
    if let Some(head_off) = head_offset_in_out
        && head_off + 12 <= out.len()
    {
        out[head_off + 8..head_off + 12].copy_from_slice(&[0, 0, 0, 0]);
        let mut sum: u32 = 0;
        for chunk in out.chunks_exact(4) {
            sum = sum.wrapping_add(u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        let adj = 0xB1B0_AFBAu32.wrapping_sub(sum);
        out[head_off + 8..head_off + 12].copy_from_slice(&adj.to_be_bytes());
    }

    Some(out)
}
