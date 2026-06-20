// MARK: Formula SVG crop helpers
use typst_preview_core::typst_scene::SceneRect;

pub(crate) fn formula_crop(bounds: SceneRect, page_width: f32, page_height: f32) -> SceneRect {
    let padding = 2.0;
    let x = (bounds.x - padding).max(0.0);
    let y = (bounds.y - padding).max(0.0);
    let right = (bounds.x + bounds.width + padding).min(page_width);
    let bottom = (bounds.y + bounds.height + padding).min(page_height);

    SceneRect {
        x,
        y,
        width: (right - x).max(1.0),
        height: (bottom - y).max(1.0),
    }
}

pub(crate) fn crop_svg(svg: &str, crop: SceneRect) -> String {
    let mut output = svg.to_string();
    set_svg_attr(
        &mut output,
        "viewBox",
        &format!(
            "{} {} {} {}",
            svg_number(crop.x),
            svg_number(crop.y),
            svg_number(crop.width),
            svg_number(crop.height)
        ),
    );
    set_svg_attr(
        &mut output,
        "width",
        &format!("{}pt", svg_number(crop.width)),
    );
    set_svg_attr(
        &mut output,
        "height",
        &format!("{}pt", svg_number(crop.height)),
    );
    set_svg_attr(&mut output, "overflow", "hidden");
    output
}

fn set_svg_attr(svg: &mut String, name: &str, value: &str) {
    let Some(tag_start) = svg.find("<svg") else {
        return;
    };
    let Some(relative_tag_end) = svg[tag_start..].find('>') else {
        return;
    };
    let tag_end = tag_start + relative_tag_end;
    let pattern = format!("{name}=\"");

    if let Some(relative_attr_start) = svg[tag_start..tag_end].find(&pattern) {
        let value_start = tag_start + relative_attr_start + pattern.len();
        if let Some(relative_value_end) = svg[value_start..].find('"') {
            svg.replace_range(value_start..value_start + relative_value_end, value);
        }
    } else {
        svg.insert_str(tag_end, &format!(" {name}=\"{value}\""));
    }
}

fn svg_number(value: f32) -> String {
    format!("{value:.3}")
}
