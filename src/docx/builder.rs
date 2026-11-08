//! `DocxBuilder`: shared rendering state (relationship ids, media, hyperlinks,
//! footnotes, bookmarks) plus small helpers used by every renderer.

use std::fmt::Write as _;

use crate::ir::{BlockNode, DocumentIr, HAlignIr, ImageFormatIr, ImageIr, InlineNode};
use crate::units::pt_to_dxa;

pub(crate) struct MediaAsset {
    pub(crate) rel_id: String,
    pub(crate) filename: String,
    #[allow(dead_code)]
    pub(crate) format: ImageFormatIr,
    pub(crate) data: Vec<u8>,
}

pub(crate) struct HyperlinkRel {
    pub(crate) rel_id: String,
    pub(crate) url: String,
}

pub(crate) struct FootnoteRecord {
    pub(crate) id: usize,
    pub(crate) blocks: Vec<BlockNode>,
}

pub(crate) struct DocxBuilder<'a> {
    pub(crate) ir: &'a DocumentIr,
    pub(crate) next_rel_id: usize,
    pub(crate) next_drawing_id: u32,
    pub(crate) next_bookmark_id: u32,
    pub(crate) media_assets: Vec<MediaAsset>,
    pub(crate) hyperlinks: Vec<HyperlinkRel>,
    pub(crate) footnotes: Vec<FootnoteRecord>,
    pub(crate) enum_instances: Vec<(usize, usize, u64)>,
    pub(crate) next_enum_num_id: usize,
    pub(crate) header_rel_id: Option<String>,
    pub(crate) footer_rel_id: Option<String>,
    pub(crate) active_columns: (usize, f64),
    pub(crate) has_scoped_columns: bool,
}

impl<'a> DocxBuilder<'a> {
    pub(crate) fn new(ir: &'a DocumentIr) -> Self {
        Self {
            ir,
            // Reserve rId1..rId4 for styles, numbering, settings, footnotes
            next_rel_id: 5,
            next_drawing_id: 1,
            next_bookmark_id: 1,
            media_assets: Vec::new(),
            hyperlinks: Vec::new(),
            footnotes: Vec::new(),
            enum_instances: Vec::new(),
            next_enum_num_id: 2,
            header_rel_id: None,
            footer_rel_id: None,
            active_columns: (1, 0.0),
            has_scoped_columns: false,
        }
    }

    pub(crate) fn alloc_enum_num_id(&mut self, ilvl: usize, start_val: u64) -> usize {
        let num_id = self.next_enum_num_id;
        self.next_enum_num_id += 1;
        self.enum_instances.push((num_id, ilvl, start_val));
        num_id
    }

    pub(crate) fn content_width_dxa(&self) -> i64 {
        let ps = &self.ir.page_setup;
        let full_w_pt = (ps.width_pt - ps.margin_left_pt - ps.margin_right_pt).max(200.0);
        let (cols, gutter_pt) = if self.active_columns.0 > 1 {
            self.active_columns
        } else if ps.columns > 1 {
            (ps.columns, 18.0)
        } else {
            (1, 0.0)
        };
        let w_pt = if cols > 1 {
            ((full_w_pt - (cols - 1) as f64 * gutter_pt) / cols as f64).max(100.0)
        } else {
            full_w_pt
        };
        pt_to_dxa(w_pt)
    }

    pub(crate) fn has_large_horizontal_space(inlines: &[InlineNode]) -> bool {
        inlines
            .iter()
            .any(|inl| matches!(inl, InlineNode::HorizontalSpace { width_pt } if *width_pt >= 36.0))
    }

    pub(crate) fn alloc_rel_id(&mut self) -> String {
        let id = self.next_rel_id;
        self.next_rel_id += 1;
        format!("rId{id}")
    }

    pub(crate) fn register_hyperlink(&mut self, url: &str) -> String {
        if let Some(existing) = self.hyperlinks.iter().find(|h| h.url == url) {
            return existing.rel_id.clone();
        }
        let rel_id = self.alloc_rel_id();
        self.hyperlinks.push(HyperlinkRel {
            rel_id: rel_id.clone(),
            url: url.to_string(),
        });
        rel_id
    }

    pub(crate) fn register_image(&mut self, img: &ImageIr) -> (String, u32) {
        let rel_id = self.alloc_rel_id();
        let idx = self.media_assets.len() + 1;
        let filename = format!("image{}.{}", idx, img.format.extension());
        self.media_assets.push(MediaAsset {
            rel_id: rel_id.clone(),
            filename,
            format: img.format,
            data: img.data.clone(),
        });
        let drawing_id = self.next_drawing_id;
        self.next_drawing_id += 1;
        (rel_id, drawing_id)
    }

    pub(crate) fn render_jc(align: HAlignIr, out: &mut String) {
        let val = match align {
            HAlignIr::Left => "left",
            HAlignIr::Center => "center",
            HAlignIr::Right => "right",
            HAlignIr::Justify => "both",
        };
        let _ = write!(out, r#"<w:jc w:val="{val}"/>"#);
    }

    pub(crate) fn render_table_jc(align: HAlignIr, out: &mut String) {
        let val = match align {
            HAlignIr::Left | HAlignIr::Justify => "left",
            HAlignIr::Center => "center",
            HAlignIr::Right => "right",
        };
        let _ = write!(out, r#"<w:jc w:val="{val}"/>"#);
    }
}
