// MARK: MathML to OMML
pub(crate) fn mathml_to_omml(mathml: &str) -> Option<String> {
    let wrapped = format!("<root>{mathml}</root>");
    let document = roxmltree::Document::parse(&wrapped).ok()?;
    let math = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "math")?;

    let mut body = String::new();
    for child in math.children().filter(|child| child.is_element()) {
        body.push_str(&node_to_omml(child)?);
    }

    Some(format!("<m:oMath>{body}</m:oMath>"))
}

fn node_to_omml(node: roxmltree::Node<'_, '_>) -> Option<String> {
    match node.tag_name().name() {
        "mrow" | "math" | "semantics" => children_to_omml(node),
        "mi" | "mn" | "mo" | "mtext" => Some(text_run(node.text().unwrap_or_default())),
        "msup" => script_omml(node, "m:sSup", &[("m:e", 0), ("m:sup", 1)]),
        "msub" => script_omml(node, "m:sSub", &[("m:e", 0), ("m:sub", 1)]),
        "msubsup" => script_omml(node, "m:sSubSup", &[("m:e", 0), ("m:sub", 1), ("m:sup", 2)]),
        "mfrac" => fraction_omml(node),
        "msqrt" => radical_omml(node, None),
        "mroot" => radical_omml(node, Some(1)),
        "mstyle" | "mpadded" | "mphantom" => children_to_omml(node),
        "mspace" => Some(text_run(" ")),
        _ => None,
    }
}

fn children_to_omml(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let mut out = String::new();
    for child in node.children().filter(|child| child.is_element()) {
        out.push_str(&node_to_omml(child)?);
    }
    Some(out)
}

fn script_omml(
    node: roxmltree::Node<'_, '_>,
    tag: &str,
    parts: &[(&str, usize)],
) -> Option<String> {
    let children = element_children(node);
    let mut out = format!("<{tag}>");
    for (part_tag, index) in parts {
        let child = *children.get(*index)?;
        out.push_str(&format!("<{part_tag}>"));
        out.push_str(&node_to_omml(child)?);
        out.push_str(&format!("</{part_tag}>"));
    }
    out.push_str(&format!("</{tag}>"));
    Some(out)
}

fn fraction_omml(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let children = element_children(node);
    let numerator = *children.first()?;
    let denominator = *children.get(1)?;

    Some(format!(
        "<m:f><m:num>{}</m:num><m:den>{}</m:den></m:f>",
        node_to_omml(numerator)?,
        node_to_omml(denominator)?
    ))
}

fn radical_omml(node: roxmltree::Node<'_, '_>, degree_index: Option<usize>) -> Option<String> {
    let children = element_children(node);
    let base_index = if degree_index.is_some() { 0 } else { 0 };
    let base = *children.get(base_index)?;
    let degree = degree_index
        .and_then(|index| children.get(index).copied())
        .and_then(node_to_omml)
        .unwrap_or_default();

    Some(format!(
        "<m:rad><m:deg>{degree}</m:deg><m:e>{}</m:e></m:rad>",
        node_to_omml(base)?
    ))
}

fn element_children<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    node.children()
        .filter(|child| child.is_element())
        .collect::<Vec<_>>()
}

fn text_run(text: &str) -> String {
    format!("<m:r><m:t>{}</m:t></m:r>", escape_xml(text))
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
