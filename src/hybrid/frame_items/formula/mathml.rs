// MARK: Formula MathML extraction
#[derive(Clone, Debug)]
pub(crate) struct MathMlFormula {
    source: String,
    display: bool,
}

impl MathMlFormula {
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn display(&self) -> bool {
        self.display
    }
}

pub(crate) fn extract_mathml_formulas(html: &str) -> Vec<MathMlFormula> {
    let mut formulas = Vec::new();
    let mut cursor = 0;

    while let Some(start) = html[cursor..].find("<math") {
        let start = cursor + start;
        let Some(open_end) = html[start..].find('>') else {
            break;
        };
        let open_end = start + open_end + 1;
        let Some(close_start) = html[open_end..].find("</math>") else {
            break;
        };
        let close_end = open_end + close_start + "</math>".len();
        let source = &html[start..close_end];

        if roxmltree::Document::parse(&format!("<root>{source}</root>")).is_ok() {
            formulas.push(MathMlFormula {
                source: source.to_string(),
                display: has_display_block(source),
            });
        }

        cursor = close_end;
    }

    formulas
}

fn has_display_block(mathml: &str) -> bool {
    mathml
        .get(..mathml.find('>').unwrap_or(mathml.len()))
        .is_some_and(|open_tag| open_tag.contains("display=\"block\""))
}
