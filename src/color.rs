//! Typst paint -> OpenXML hex color conversion.

use typst::visualize::Paint;

/// Convert a Typst `Paint` to a 6-character uppercase hex color string (`RRGGBB`).
pub fn paint_to_hex(paint: &Paint) -> Option<String> {
    match paint {
        Paint::Solid(c) => {
            let [r, g, b, _] = c.to_vec4_u8();
            Some(format!("{r:02X}{g:02X}{b:02X}"))
        }
        Paint::Gradient(grad) => {
            let stops = grad.stops_ref();
            if let Some((c, _)) = stops.first() {
                let [r, g, b, _] = c.to_vec4_u8();
                Some(format!("{r:02X}{g:02X}{b:02X}"))
            } else {
                None
            }
        }
        Paint::Tiling(_) => None,
    }
}
