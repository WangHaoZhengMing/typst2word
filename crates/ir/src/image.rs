//! Raster / vector images.

use serde::{Deserialize, Serialize};

use super::HAlignIr;

/// Raster or vector image node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageIr {
    pub data: Vec<u8>,
    pub format: ImageFormatIr,
    pub width_pt: f64,
    pub height_pt: f64,
    pub alt: Option<String>,
    pub align: HAlignIr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placed_offset_pt: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormatIr {
    Png,
    Jpeg,
    Gif,
    Svg,
}

impl ImageFormatIr {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
            Self::Svg => "svg",
        }
    }
}
