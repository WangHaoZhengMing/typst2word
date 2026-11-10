//! Inline `image` -> `<w:drawing><wp:inline>` picture (PNG/JPEG/GIF/SVG).

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use word_construct::model::{ImageFormatIr, ImageIr};
use word_construct::units::pt_to_emu;
use crate::xml::escape_xml;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_image_run(&mut self, img: &ImageIr, out: &mut String) {
        let (rel_id, drawing_id) = self.register_image(img);
        let cx = pt_to_emu(img.width_pt).max(9525);
        let cy = pt_to_emu(img.height_pt).max(9525);
        let descr = escape_xml(img.alt.as_deref().unwrap_or("Image"));

        let blip_xml = if img.format == ImageFormatIr::Svg {
            format!(
                r#"<a:blip><a:extLst><a:ext uri="{{96DAC541-7B7A-43D3-8B79-37D633B846F1}}"><asvg:svgBlip r:embed="{rel_id}"/></a:ext></a:extLst></a:blip>"#
            )
        } else {
            format!(r#"<a:blip r:embed="{rel_id}"/>"#)
        };

        if let Some((dx_pt, dy_pt)) = img.placed_offset_pt {
            let dx_emu = pt_to_emu(dx_pt);
            let dy_emu = pt_to_emu(dy_pt);
            let _ = write!(
                out,
                concat!(
                    r#"<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="25165824" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1">"#,
                    r#"<wp:simplePos x="0" y="0"/>"#,
                    r#"<wp:positionH relativeFrom="column"><wp:posOffset>{dx_emu}</wp:posOffset></wp:positionH>"#,
                    r#"<wp:positionV relativeFrom="paragraph"><wp:posOffset>{dy_emu}</wp:posOffset></wp:positionV>"#,
                    r#"<wp:extent cx="{cx}" cy="{cy}"/>"#,
                    r#"<wp:effectExtent l="0" t="0" r="0" b="0"/>"#,
                    r#"<wp:wrapNone/>"#,
                    r#"<wp:docPr id="{drawing_id}" name="Picture {drawing_id}" descr="{descr}"/>"#,
                    r#"<wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr>"#,
                    r#"<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
                    r#"<pic:pic>"#,
                    r#"<pic:nvPicPr><pic:cNvPr id="{drawing_id}" name="Picture {drawing_id}" descr="{descr}"/><pic:cNvPicPr/></pic:nvPicPr>"#,
                    r#"<pic:blipFill>{blip_xml}<a:stretch><a:fillRect/></a:stretch></pic:blipFill>"#,
                    r#"<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr>"#,
                    r#"</pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#
                ),
                dx_emu = dx_emu,
                dy_emu = dy_emu,
                cx = cx,
                cy = cy,
                drawing_id = drawing_id,
                descr = descr,
                blip_xml = blip_xml,
            );
            return;
        }

        let _ = write!(
            out,
            concat!(
                r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">"#,
                r#"<wp:extent cx="{cx}" cy="{cy}"/>"#,
                r#"<wp:effectExtent l="0" t="0" r="0" b="0"/>"#,
                r#"<wp:docPr id="{drawing_id}" name="Picture {drawing_id}" descr="{descr}"/>"#,
                r#"<wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr>"#,
                r#"<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
                r#"<pic:pic>"#,
                r#"<pic:nvPicPr><pic:cNvPr id="{drawing_id}" name="Picture {drawing_id}" descr="{descr}"/><pic:cNvPicPr/></pic:nvPicPr>"#,
                r#"<pic:blipFill>{blip_xml}<a:stretch><a:fillRect/></a:stretch></pic:blipFill>"#,
                r#"<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr>"#,
                r#"</pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
            ),
            cx = cx,
            cy = cy,
            drawing_id = drawing_id,
            descr = descr,
            blip_xml = blip_xml,
        );
    }
}
