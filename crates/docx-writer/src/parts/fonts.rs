//! Embedded font obfuscation (`.odttf`, ECMA-376 §17.8.1), `word/fontTable.xml`,
//! and `word/_rels/fontTable.xml.rels`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use word_construct::model::{EmbeddedFontFaceIr, EmbeddedFontSlot};

use crate::writer::DocxWriter;
use crate::xml::escape_xml_attr;

pub(crate) struct PreparedEmbeddedFont {
    pub(crate) rel_id: String,
    pub(crate) part_filename: String,
    pub(crate) font_key_guid: String,
    pub(crate) font_table_name: String,
    pub(crate) slot: EmbeddedFontSlot,
    pub(crate) is_subset: bool,
    pub(crate) obfuscated_data: Vec<u8>,
}

/// Prepare deterministic `.odttf` obfuscated payloads for all embedded faces on `WordDocument`.
pub(crate) fn prepare_embedded_fonts(faces: &[EmbeddedFontFaceIr]) -> Vec<PreparedEmbeddedFont> {
    let mut out = Vec::with_capacity(faces.len());
    for (idx, face) in faces.iter().enumerate() {
        let num = idx + 1;
        let rel_id = format!("rId{num}");
        let part_filename = format!("font{num}.odttf");
        // For OpenType `MATH` fonts (like `NewCMMath-Regular.otf`), Word binds `<m:mathFont>`
        // and `<w:font w:name="...">` against OpenType `nameID 1` (`NewComputerModernMath`).
        // For text fonts, `<w:font w:name="...">` matches the realized `<w:rFonts>` name.
        let font_table_name = if face.is_math {
            face.word_family.clone()
        } else {
            face.typst_family.clone()
        };
        let key_bytes = derive_deterministic_guid_bytes(&font_table_name, face.slot, &face.sfnt_data);
        let font_key_guid = format_guid_braced(&key_bytes);
        let obfuscated_data = obfuscate_odttf(&face.sfnt_data, &key_bytes);
        out.push(PreparedEmbeddedFont {
            rel_id,
            part_filename,
            font_key_guid,
            font_table_name,
            slot: face.slot,
            is_subset: face.is_subset,
            obfuscated_data,
        });
    }
    out
}

/// Derive a deterministic 16-byte GUID (RFC 4122 v4 bit pattern) from font identity and payload,
/// keeping `.docx` output bit-for-bit reproducible across runs.
fn derive_deterministic_guid_bytes(
    family: &str,
    slot: EmbeddedFontSlot,
    data: &[u8],
) -> [u8; 16] {
    let mut h1: u64 = 0xcbf2_9ce4_8422_2325;
    let mut h2: u64 = 0x8422_2325_cbf2_9ce4;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            h1 ^= b as u64;
            h1 = h1.wrapping_mul(0x0000_0100_0000_01B3);
            h2 = h2.wrapping_mul(0x0000_0100_0000_01B3);
            h2 ^= (b as u64).rotate_left(3);
        }
    };
    feed(family.as_bytes());
    feed(&[slot as u8]);
    feed(&(data.len() as u64).to_le_bytes());
    // Sample head, middle, and tail of the font payload for collision resistance without slowness.
    let n = data.len();
    feed(&data[..n.min(2048)]);
    if n > 4096 {
        let mid = n / 2;
        feed(&data[mid - 512..mid + 512]);
    }
    if n > 2048 {
        feed(&data[n - 2048..]);
    }

    let mut guid = [0u8; 16];
    guid[0..8].copy_from_slice(&h1.to_be_bytes());
    guid[8..16].copy_from_slice(&h2.to_be_bytes());
    // Set RFC 4122 version 4 and variant bits.
    guid[6] = (guid[6] & 0x0F) | 0x40;
    guid[8] = (guid[8] & 0x3F) | 0x80;
    guid
}

fn format_guid_braced(b: &[u8; 16]) -> String {
    format!(
        "{{{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        b[0], b[1], b[2], b[3],
        b[4], b[5],
        b[6], b[7],
        b[8], b[9],
        b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// OOXML §17.8.1 font obfuscation: XOR the first 32 bytes with the 16-byte GUID in reversed byte order.
fn obfuscate_odttf(sfnt_data: &[u8], guid_bytes: &[u8; 16]) -> Vec<u8> {
    let mut out = sfnt_data.to_vec();
    let limit = out.len().min(32);
    for (i, byte) in out.iter_mut().take(limit).enumerate() {
        *byte ^= guid_bytes[15 - (i % 16)];
    }
    out
}

/// Known cross-platform `<w:altName>` + `(charset, family, pitch)` metadata for fallback when a
/// viewer does not support embedded fonts or when `--embed-fonts=none` is selected.
fn known_font_meta(name: &str) -> (Option<&'static str>, &'static str, &'static str, &'static str) {
    match name {
        "DejaVu Sans Mono" => (Some("Consolas"), "00", "modern", "fixed"),
        "Libertinus Serif" => (Some("Times New Roman"), "00", "roman", "variable"),
        "New Computer Modern Math" | "NewComputerModernMath" => {
            (Some("Cambria Math"), "00", "roman", "variable")
        }
        "Hei" => (Some("Heiti SC"), "86", "swiss", "variable"),
        _ => (None, "00", "auto", "variable"),
    }
}

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_font_table_xml(&self, prepared: &[PreparedEmbeddedFont]) -> String {
        let mut out = String::new();
        out.push_str(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
        );

        // Group prepared embedded faces by `<w:font w:name="...">`.
        let mut by_name: BTreeMap<&str, Vec<&PreparedEmbeddedFont>> = BTreeMap::new();
        for pf in prepared {
            by_name.entry(pf.font_table_name.as_str()).or_default().push(pf);
        }

        // Preserve baseline fallback entries plus every realized/embedded font family.
        let mut all_names: BTreeSet<String> = [
            "DejaVu Sans Mono",
            "Libertinus Serif",
            "New Computer Modern Math",
            "Hei",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        for name in self.doc.font_metrics.keys() {
            all_names.insert(name.clone());
        }
        for pf in prepared {
            all_names.insert(pf.font_table_name.clone());
        }

        for name in &all_names {
            let name_esc = escape_xml_attr(name);
            let (alt_name, default_charset, default_family, default_pitch) = known_font_meta(name);
            let metrics = self.doc.font_metrics.get(name);
            let charset = if metrics.is_some_and(|m| m.has_cjk_codepage) {
                "86"
            } else {
                default_charset
            };
            let pitch = if metrics.is_some_and(|m| m.is_monospaced) {
                "fixed"
            } else {
                default_pitch
            };
            let family = if metrics.is_some_and(|m| m.is_monospaced) && default_family == "auto" {
                "modern"
            } else {
                default_family
            };

            let _ = write!(out, "\n  <w:font w:name=\"{name_esc}\">");
            if let Some(alt) = alt_name {
                let _ = write!(out, "\n    <w:altName w:val=\"{}\"/>", escape_xml_attr(alt));
            }
            let _ = write!(
                out,
                "\n    <w:charset w:val=\"{charset}\"/>\n    <w:family w:val=\"{family}\"/>\n    <w:pitch w:val=\"{pitch}\"/>"
            );

            if let Some(entries) = by_name.get(name.as_str()) {
                for entry in entries {
                    let tag = entry.slot.ooxml_tag();
                    let subset_attr = if entry.is_subset {
                        r#" w:subsetted="1""#
                    } else {
                        ""
                    };
                    let _ = write!(
                        out,
                        "\n    <w:{tag} r:id=\"{}\" w:fontKey=\"{}\"{subset_attr}/>",
                        entry.rel_id, entry.font_key_guid
                    );
                }
            }
            out.push_str("\n  </w:font>");
        }

        out.push_str("\n</w:fonts>");
        out
    }

    pub(crate) fn render_font_table_rels_xml(prepared: &[PreparedEmbeddedFont]) -> String {
        let mut out = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );
        for pf in prepared {
            let _ = write!(
                out,
                "\n  <Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/font\" Target=\"fonts/{}\"/>",
                pf.rel_id, pf.part_filename
            );
        }
        out.push_str("\n</Relationships>");
        out
    }
}
