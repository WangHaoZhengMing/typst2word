//! Word CeTZ Canvas & Label Constructs (`<wpg:wgp>` wrapper paragraph and positioned `<wps:wsp>` labels).

use ir::{CetzCanvasIr, CetzLabelDrawableIr, HAlignIr, InlineNode, MathNodeIr};

use crate::units::pt_to_emu;

/// Solved Word paragraph wrapper construct for `BlockNode::CetzCanvas`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CetzCanvasConstruct {
    pub align: HAlignIr,
    pub before_dxa: i64,
    pub after_dxa: i64,
    pub line_dxa: i64,
    pub mark_sz_half_pt: u32,
}

impl CetzCanvasConstruct {
    pub fn solve(canvas: &CetzCanvasIr) -> Self {
        Self {
            align: canvas.align,
            before_dxa: 35,
            after_dxa: 295,
            line_dxa: 240,
            mark_sz_half_pt: 4,
        }
    }
}

/// Semantic AST classification of a CeTZ label's inline content for vertical baseline alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CetzLabelContentKind {
    /// Text-only label without inline OMML equations.
    TextOnly,
    /// Contains inline OMML math without subscript constructs.
    MathSimple,
    /// Contains inline OMML math with subscript constructs (`SubSup { sub: Some(_), .. }`).
    MathWithSubscript,
}

/// Solved DrawingML `<wps:wsp>` geometry and non-wrapping inline content for a [`CetzLabelDrawableIr`].
#[derive(Debug, Clone)]
pub struct CetzLabelConstruct {
    pub content_kind: CetzLabelContentKind,
    pub nobreak_content: Vec<InlineNode>,
    pub off_x_emu: i64,
    pub off_y_emu: i64,
    pub cx_emu: i64,
    pub cy_emu: i64,
    pub rot_60000: Option<i64>,
    pub line_dxa_override: Option<i64>,
}

/// Compute the extra top canvas padding (`pt`) required when a CeTZ label near `y = 0` needs an
/// upward DrawingML baseline overhang compensation (`y_shift_pt > label.y_pt`), e.g. a 2-story CJK
/// OMML fraction (`频率/组距`) placed at the top of a histogram axis.
pub fn canvas_top_pad_pt(canvas: &CetzCanvasIr) -> f64 {
    canvas
        .labels
        .iter()
        .map(|lbl| {
            let kind = classify_label_inlines(&lbl.content);
            let has_vert_frac = label_inlines_have_vertical_fraction(&lbl.content);
            let y_shift = label_y_shift_pt(kind, has_vert_frac);
            (y_shift - lbl.y_pt).max(0.0)
        })
        .fold(0.0_f64, f64::max)
}

fn label_y_shift_pt(kind: CetzLabelContentKind, has_vertical_fraction: bool) -> f64 {
    if has_vertical_fraction {
        // A 2-story OMML vertical fraction (`<m:f>`) inside a CJK/math DrawingML textbox carries
        // ~9pt of top internal leading above the numerator ink.
        return 9.5;
    }
    // Word's DrawingML textbox top-anchors the first line box (`anchor="t"`), which adds
    // slightly more top overhang for OMML math runs (and subscripted OMML runs) than plain text.
    match kind {
        CetzLabelContentKind::MathWithSubscript => 3.5,
        CetzLabelContentKind::MathSimple => 2.6,
        CetzLabelContentKind::TextOnly => 1.2,
    }
}

impl CetzLabelConstruct {
    /// Solve `<wps:wsp>` transform geometry directly from the label's semantic inline AST
    /// and canvas bounds, without inspecting serialized XML.
    pub fn solve(label: &CetzLabelDrawableIr, canvas_height_pt: f64) -> Self {
        Self::solve_with_top_pad(label, canvas_height_pt, 0.0)
    }

    /// Solve `<wps:wsp>` transform geometry with an explicit `top_pad_pt` canvas top shift
    /// computed by [`canvas_top_pad_pt`].
    pub fn solve_with_top_pad(
        label: &CetzLabelDrawableIr,
        canvas_height_pt: f64,
        top_pad_pt: f64,
    ) -> Self {
        let content_kind = classify_label_inlines(&label.content);
        let nobreak_content = replace_spaces_with_nbsp(&label.content);
        let has_vertical_fraction = label_inlines_have_vertical_fraction(&nobreak_content);
        let y_shift_pt = label_y_shift_pt(content_kind, has_vertical_fraction);

        // Give the label textbox generous width so Word never wraps single-line labels,
        // while clamping height so `off_y + cy` never overflows `canvas_height_pt + top_pad_pt`.
        let w_pt = (label.width_pt * 2.0 + 48.0).max(72.0);
        let top_pt = (label.y_pt + top_pad_pt - y_shift_pt).max(0.0);
        let max_avail_h_pt = (canvas_height_pt + top_pad_pt - top_pt).max(11.0);
        let is_rotated = label.rotation_deg.abs() > 0.01;
        let h_pt = if is_rotated {
            (label.height_pt + 14.0).max(28.0)
        } else if has_vertical_fraction {
            // OMML vertical fractions (`<m:f>`) inside CJK/math DrawingML textboxes need extra
            // bottom clearance (CJK fonts like `Songti SC` carry a `1.82x` line-box factor per
            // story) so Word never clips the denominator.
            let dom_sz = ir::dominant_inline_font_size_pt(&label.content, 10.0).max(8.0);
            (label.height_pt + 22.0)
                .max(dom_sz * 4.5)
                .clamp(24.0, max_avail_h_pt)
        } else {
            (label.height_pt + 2.0).clamp(11.0, max_avail_h_pt)
        };

        Self {
            content_kind,
            nobreak_content,
            off_x_emu: pt_to_emu((label.x_pt - 0.5).max(0.0)),
            off_y_emu: pt_to_emu(top_pt),
            cx_emu: pt_to_emu(w_pt),
            cy_emu: pt_to_emu(h_pt),
            rot_60000: is_rotated.then(|| (label.rotation_deg * 60000.0).round() as i64),
            line_dxa_override: has_vertical_fraction.then_some(240),
        }
    }
}

fn classify_label_inlines(inlines: &[InlineNode]) -> CetzLabelContentKind {
    let mut kind = CetzLabelContentKind::TextOnly;
    for node in inlines {
        let child_kind = match node {
            InlineNode::EquationInline(eq) => {
                if eq.body.iter().any(math_node_has_subscript) {
                    CetzLabelContentKind::MathWithSubscript
                } else {
                    CetzLabelContentKind::MathSimple
                }
            }
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. }
            | InlineNode::Field { content, .. } => classify_label_inlines(content),
            _ => CetzLabelContentKind::TextOnly,
        };
        kind = match (kind, child_kind) {
            (CetzLabelContentKind::MathWithSubscript, _)
            | (_, CetzLabelContentKind::MathWithSubscript) => {
                CetzLabelContentKind::MathWithSubscript
            }
            (CetzLabelContentKind::MathSimple, _) | (_, CetzLabelContentKind::MathSimple) => {
                CetzLabelContentKind::MathSimple
            }
            _ => CetzLabelContentKind::TextOnly,
        };
    }
    kind
}

fn label_inlines_have_vertical_fraction(inlines: &[InlineNode]) -> bool {
    inlines.iter().any(|node| match node {
        InlineNode::EquationInline(eq) => {
            crate::objects::equation::analyze_math_slice(&eq.body, eq.is_block).max_fraction_depth
                > 0
        }
        InlineNode::Link { content, .. }
        | InlineNode::Ref { content, .. }
        | InlineNode::Citation { content, .. }
        | InlineNode::Field { content, .. } => label_inlines_have_vertical_fraction(content),
        _ => false,
    })
}

fn math_node_has_subscript(node: &MathNodeIr) -> bool {
    match node {
        MathNodeIr::SubSup { base, sub, sup } => {
            sub.is_some()
                || math_node_has_subscript(base)
                || sup.as_deref().is_some_and(math_node_has_subscript)
        }
        MathNodeIr::PreSubSup { base, sub, sup } => {
            math_node_has_subscript(base)
                || sub.as_deref().is_some_and(math_node_has_subscript)
                || sup.as_deref().is_some_and(math_node_has_subscript)
        }
        MathNodeIr::Sequence(children) => children.iter().any(math_node_has_subscript),
        MathNodeIr::Fraction { num, denom, .. } => {
            math_node_has_subscript(num) || math_node_has_subscript(denom)
        }
        MathNodeIr::Radical { degree, radicand } => {
            degree.as_deref().is_some_and(math_node_has_subscript)
                || math_node_has_subscript(radicand)
        }
        MathNodeIr::Limit { base, lower, upper } => {
            math_node_has_subscript(base)
                || lower.as_deref().is_some_and(math_node_has_subscript)
                || upper.as_deref().is_some_and(math_node_has_subscript)
        }
        MathNodeIr::Nary {
            sub, sup, operand, ..
        } => {
            sub.as_deref().is_some_and(math_node_has_subscript)
                || sup.as_deref().is_some_and(math_node_has_subscript)
                || math_node_has_subscript(operand)
        }
        MathNodeIr::Delimited { elements, .. } => elements.iter().any(math_node_has_subscript),
        MathNodeIr::Matrix { rows, .. } => rows
            .iter()
            .any(|row| row.iter().any(math_node_has_subscript)),
        MathNodeIr::EquationArray { rows } => rows.iter().any(math_node_has_subscript),
        MathNodeIr::Accent { base, .. } => math_node_has_subscript(base),
        MathNodeIr::Func { name, arg } => {
            math_node_has_subscript(name) || math_node_has_subscript(arg)
        }
        MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
            math_node_has_subscript(body)
        }
        MathNodeIr::GroupChar {
            body, annotation, ..
        } => {
            math_node_has_subscript(body)
                || annotation.as_deref().is_some_and(math_node_has_subscript)
        }
        MathNodeIr::AlignPoint | MathNodeIr::Run(_) => false,
    }
}

fn replace_spaces_with_nbsp(inlines: &[InlineNode]) -> Vec<InlineNode> {
    inlines
        .iter()
        .map(|node| match node {
            InlineNode::Text(run) => {
                let mut r = run.clone();
                r.text = r.text.replace(' ', "\u{00A0}");
                InlineNode::Text(r)
            }
            InlineNode::Link { target, content } => InlineNode::Link {
                target: target.clone(),
                content: replace_spaces_with_nbsp(content),
            },
            InlineNode::Ref { target, content } => InlineNode::Ref {
                target: target.clone(),
                content: replace_spaces_with_nbsp(content),
            },
            InlineNode::Citation { keys, content } => InlineNode::Citation {
                keys: keys.clone(),
                content: replace_spaces_with_nbsp(content),
            },
            InlineNode::Field {
                kind,
                format,
                content,
            } => InlineNode::Field {
                kind: kind.clone(),
                format: format.clone(),
                content: replace_spaces_with_nbsp(content),
            },
            other => other.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::{EquationIr, MathNodeIr, MathRunIr, TextRunIr};

    fn make_label(content: Vec<InlineNode>) -> CetzLabelDrawableIr {
        CetzLabelDrawableIr {
            x_pt: 20.0,
            y_pt: 30.0,
            width_pt: 12.0,
            height_pt: 10.0,
            rotation_deg: 0.0,
            content,
        }
    }

    #[test]
    fn cetz_label_construct_classifies_text_and_replaces_spaces() {
        let label = make_label(vec![InlineNode::Text(TextRunIr {
            text: "a b".to_string(),
            ..Default::default()
        })]);
        let solved = CetzLabelConstruct::solve(&label, 120.0);
        assert_eq!(solved.content_kind, CetzLabelContentKind::TextOnly);
        assert_eq!(solved.off_y_emu, pt_to_emu(30.0 - 1.2));
        assert_eq!(solved.off_x_emu, pt_to_emu(19.5));
        assert_eq!(solved.rot_60000, None);
        match &solved.nobreak_content[0] {
            InlineNode::Text(r) => assert_eq!(r.text, "a\u{00A0}b"),
            other => panic!("expected Text run, got {other:?}"),
        }
    }

    fn make_inline_eq(body: Vec<MathNodeIr>) -> EquationIr {
        EquationIr {
            id: ir::NodeId::UNSET,
            math_font_family: None,
            is_block: false,
            body,
            source_text: String::new(),
            numbering_text: None,
            numbering_runs: Vec::new(),
            bookmark: None,
            font_size_pt: Some(11.0),
            layout_metrics: Default::default(),
        }
    }

    #[test]
    fn cetz_label_construct_classifies_math_and_subscripts_from_ast() {
        let simple_eq = make_inline_eq(vec![MathNodeIr::Run(MathRunIr {
            text: "x".to_string(),
            ..Default::default()
        })]);
        let simple_solved =
            CetzLabelConstruct::solve(&make_label(vec![InlineNode::EquationInline(simple_eq)]), 120.0);
        assert_eq!(simple_solved.content_kind, CetzLabelContentKind::MathSimple);
        assert_eq!(simple_solved.off_y_emu, pt_to_emu(30.0 - 2.6));

        let sub_eq = make_inline_eq(vec![MathNodeIr::SubSup {
            base: Box::new(MathNodeIr::Run(MathRunIr {
                text: "x".to_string(),
                ..Default::default()
            })),
            sub: Some(Box::new(MathNodeIr::Run(MathRunIr {
                text: "i".to_string(),
                ..Default::default()
            }))),
            sup: None,
        }]);
        let sub_solved =
            CetzLabelConstruct::solve(&make_label(vec![InlineNode::EquationInline(sub_eq)]), 120.0);
        assert_eq!(
            sub_solved.content_kind,
            CetzLabelContentKind::MathWithSubscript
        );
        assert_eq!(sub_solved.off_y_emu, pt_to_emu(30.0 - 3.5));
    }
}
