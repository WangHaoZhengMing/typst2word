//! Associate semantic math text with the font of its laid-out glyph clusters.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::rc::Rc;

use ir::{MathNodeIr, MathRunIr};
use typst::syntax::Span;
use typst::text::TextItem;

#[derive(Clone, PartialEq)]
struct Face {
    family: Rc<str>,
    math: bool,
}

#[derive(Default)]
pub(crate) struct EquationFonts {
    // A function or a loop can realize the same source span more than once.
    clusters: RefCell<HashMap<Span, BTreeMap<u16, VecDeque<Face>>>>,
}

impl EquationFonts {
    pub(crate) fn record(&mut self, text: &TextItem, math_family: Option<&str>) {
        let face = Face {
            family: Rc::from(math_family.unwrap_or(&text.font.info().family)),
            math: math_family.is_some(),
        };
        let clusters = self.clusters.get_mut();
        let mut previous = None;
        for glyph in &text.glyphs {
            let (span, offset) = glyph.span;
            if span.is_detached() || previous == Some(glyph.span) {
                continue;
            }
            previous = Some(glyph.span);
            clusters
                .entry(span)
                .or_default()
                .entry(offset)
                .or_default()
                .push_back(face.clone());
        }
    }

    pub(super) fn emit(&self, span: Span, mut run: MathRunIr, out: &mut Vec<MathNodeIr>) {
        let mut clusters = self.clusters.borrow_mut();
        let Some(offsets) = clusters.get_mut(&span) else {
            out.push(MathNodeIr::Run(run));
            return;
        };

        let source_text = std::mem::take(&mut run.text);
        let mut start = 0;
        let mut current = None;
        let mut emit = |text: &str, face: Option<&Face>| {
            let mut piece = run.clone();
            piece.text = text.to_owned();
            if let Some(face) = face {
                piece.font_family = Some(face.family.to_string());
                piece.normal_text = !face.math;
            }
            out.push(MathNodeIr::Run(piece));
        };
        for (&offset, faces) in offsets.iter_mut() {
            let pos = usize::from(offset);
            if pos >= source_text.len() || !source_text.is_char_boundary(pos) {
                continue;
            }
            let Some(face) = faces.pop_front() else {
                continue;
            };
            if current.as_ref() != Some(&face) {
                if pos > start {
                    emit(&source_text[start..pos], current.as_ref());
                }
                start = pos;
                current = Some(face);
            }
        }
        emit(&source_text[start..], current.as_ref());
    }
}
