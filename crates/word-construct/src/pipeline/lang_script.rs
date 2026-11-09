//! Pass 2 (`WordPipelinePassKind::LanguageAndScriptRecognition`):
//! Populates `TextScriptClassIr` across `TextRunIr` and `MathNodeIr` (`MathRunIr::text_script`).

use ir::{
    BlockNode, DocumentIr, InlineNode, MathNodeIr, TextScriptClassIr,
    visit_document_block_lists_mut, visit_document_inline_lists_mut,
};

/// Run Pass 2 (`LanguageAndScriptRecognition`) across all text runs, code blocks, and equations in `doc`.
pub fn run_language_and_script_pass(doc: &mut DocumentIr) {
    visit_document_inline_lists_mut(doc, &mut |inlines| {
        for inline in inlines.iter_mut() {
            match inline {
                InlineNode::Text(run) => {
                    run.text_script = TextScriptClassIr::classify(&run.text);
                }
                InlineNode::EquationInline(eq) => {
                    classify_math_nodes_script(&mut eq.body);
                }
                _ => {}
            }
        }
    });

    visit_document_block_lists_mut(doc, &mut |blocks| {
        for block in blocks.iter_mut() {
            match block {
                BlockNode::CodeBlock { lines, .. } => {
                    for line in lines.iter_mut() {
                        for run in line.iter_mut() {
                            run.text_script = TextScriptClassIr::classify(&run.text);
                        }
                    }
                }
                BlockNode::EquationBlock(eq) => {
                    classify_math_nodes_script(&mut eq.body);
                }
                _ => {}
            }
        }
    });
}

fn classify_math_nodes_script(nodes: &mut [MathNodeIr]) {
    for node in nodes {
        classify_math_node_script(node);
    }
}

fn classify_math_node_script(node: &mut MathNodeIr) {
    match node {
        MathNodeIr::Sequence(children) => {
            classify_math_nodes_script(children);
        }
        MathNodeIr::Fraction { num, denom, .. } => {
            classify_math_node_script(num);
            classify_math_node_script(denom);
        }
        MathNodeIr::Radical { degree, radicand } => {
            if let Some(d) = degree {
                classify_math_node_script(d);
            }
            classify_math_node_script(radicand);
        }
        MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
            classify_math_node_script(base);
            if let Some(s) = sub {
                classify_math_node_script(s);
            }
            if let Some(s) = sup {
                classify_math_node_script(s);
            }
        }
        MathNodeIr::Limit { base, lower, upper } => {
            classify_math_node_script(base);
            if let Some(l) = lower {
                classify_math_node_script(l);
            }
            if let Some(u) = upper {
                classify_math_node_script(u);
            }
        }
        MathNodeIr::Nary {
            sub, sup, operand, ..
        } => {
            if let Some(s) = sub {
                classify_math_node_script(s);
            }
            if let Some(s) = sup {
                classify_math_node_script(s);
            }
            classify_math_node_script(operand);
        }
        MathNodeIr::Delimited { elements, .. } => {
            classify_math_nodes_script(elements);
        }
        MathNodeIr::Matrix { rows } => {
            for row in rows {
                classify_math_nodes_script(row);
            }
        }
        MathNodeIr::EquationArray { rows } => {
            classify_math_nodes_script(rows);
        }
        MathNodeIr::Accent { base, .. } => {
            classify_math_node_script(base);
        }
        MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
            classify_math_node_script(body);
        }
        MathNodeIr::GroupChar {
            body, annotation, ..
        } => {
            classify_math_node_script(body);
            if let Some(ann) = annotation {
                classify_math_node_script(ann);
            }
        }
        MathNodeIr::AlignPoint => {}
        MathNodeIr::Run(r) => {
            r.text_script = TextScriptClassIr::classify(&r.text);
        }
    }
}
