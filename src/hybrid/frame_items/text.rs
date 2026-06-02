// MARK: Text items
use std::cmp::Ordering;

use docx_rs::{
    AlignmentType, LineSpacing, LineSpacingType, Paragraph, Pic, Run, RunFonts,
};
use typst_preview_core::typst_scene::{SceneColor, SceneItem, SceneRect, SceneText};

use super::formula::{add_formula_region, FormulaRegion};
use super::super::{
    color_hex, pt_to_emu, pt_to_twips, pt_to_twips_i32, rect_union, MathRange, SourceIndex,
};

// MARK: Run types
#[derive(Clone, Debug)]
pub(crate) struct TextLine {
    baseline_y: f32,
    top_y: f32,
    left_x: f32,
    bottom_y: f32,
    runs: Vec<NativeRun>,
}

impl TextLine {
    fn new(run: NativeRun) -> Self {
        Self {
            baseline_y: run.baseline_y(),
            top_y: run.top_y(),
            left_x: run.x(),
            bottom_y: run.bottom_y(),
            runs: vec![run],
        }
    }

    fn can_accept(&self, run: &NativeRun) -> bool {
        let tolerance = self
            .runs
            .iter()
            .map(NativeRun::font_size)
            .fold(run.font_size(), f32::max)
            .mul_add(0.45, 2.0)
            .max(3.0);
        (self.baseline_y - run.baseline_y()).abs() <= tolerance
    }

    fn push(&mut self, run: NativeRun) {
        let len = self.runs.len() as f32;
        self.baseline_y = (self.baseline_y * len + run.baseline_y()) / (len + 1.0);
        self.top_y = self.top_y.min(run.top_y());
        self.left_x = self.left_x.min(run.x());
        self.bottom_y = self.bottom_y.max(run.bottom_y());
        self.runs.push(run);
    }

    pub(crate) fn into_positioned_paragraph(
        mut self,
        previous_bottom: f32,
    ) -> (Paragraph, f32) {
        self.runs
            .sort_by(|a, b| a.x().partial_cmp(&b.x()).unwrap_or(Ordering::Equal));

        let before = (self.top_y - previous_bottom).max(0.0);
        let line_height = (self.bottom_y - self.top_y)
            .max(
                self.runs
                    .iter()
                    .map(NativeRun::font_size)
                    .fold(1.0, f32::max),
            )
            .max(1.0);

        let mut paragraph = Paragraph::new()
            .align(AlignmentType::Left)
            .indent(Some(pt_to_twips_i32(self.left_x)), None, None, None)
            .line_spacing(
                LineSpacing::new()
                    .before(pt_to_twips(before))
                    .after(0)
                    .line(pt_to_twips_i32(line_height))
                    .line_rule(LineSpacingType::Exact),
            );
        let mut previous_right = None;

        for run in self.runs {
            if let Some(right) = previous_right {
                let gap = run.x() - right;
                if gap > run.font_size() * 0.35 {
                    paragraph = paragraph.add_run(Run::new().add_text(" "));
                }
            }

            previous_right = Some(run.x() + run.width());
            paragraph = paragraph.add_run(run.into_docx_run());
        }

        (paragraph, self.bottom_y)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum NativeRun {
    Text(TextRun),
    Formula(FormulaRun),
}

impl NativeRun {
    fn x(&self) -> f32 {
        match self {
            Self::Text(run) => run.x,
            Self::Formula(run) => run.x,
        }
    }

    fn width(&self) -> f32 {
        match self {
            Self::Text(run) => run.width,
            Self::Formula(run) => run.width,
        }
    }

    fn baseline_y(&self) -> f32 {
        match self {
            Self::Text(run) => run.baseline_y,
            Self::Formula(run) => run.baseline_y,
        }
    }

    fn top_y(&self) -> f32 {
        match self {
            Self::Text(run) => run.top_y,
            Self::Formula(run) => run.top_y,
        }
    }

    fn bottom_y(&self) -> f32 {
        self.top_y() + self.height()
    }

    fn height(&self) -> f32 {
        match self {
            Self::Text(run) => run.height,
            Self::Formula(run) => run.height,
        }
    }

    fn font_size(&self) -> f32 {
        match self {
            Self::Text(run) => run.font_size,
            Self::Formula(run) => run.height.max(8.0),
        }
    }

    fn into_docx_run(self) -> Run {
        match self {
            Self::Text(run) => run.into_docx_run(),
            Self::Formula(run) => run.into_docx_run(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TextRun {
    x: f32,
    width: f32,
    top_y: f32,
    height: f32,
    baseline_y: f32,
    text: String,
    font_family: String,
    font_size: f32,
    font_weight: f32,
    italic: bool,
    fill: SceneColor,
}

impl TextRun {
    fn into_docx_run(self) -> Run {
        let size = (self.font_size * 2.0).round().clamp(1.0, 400.0) as usize;
        let fonts = RunFonts::new()
            .ascii(&self.font_family)
            .hi_ansi(&self.font_family)
            .east_asia(&self.font_family)
            .cs(&self.font_family);

        let mut run = Run::new()
            .add_text(self.text)
            .size(size)
            .fonts(fonts)
            .color(color_hex(self.fill));

        if self.font_weight >= 600.0 {
            run = run.bold();
        }

        if self.italic {
            run = run.italic();
        }

        run
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FormulaRun {
    x: f32,
    width: f32,
    top_y: f32,
    baseline_y: f32,
    height: f32,
    media_id: String,
}

impl FormulaRun {
    pub(crate) fn new(
        x: f32,
        width: f32,
        top_y: f32,
        baseline_y: f32,
        height: f32,
        media_id: String,
    ) -> Self {
        Self {
            x,
            width,
            top_y,
            baseline_y,
            height,
            media_id,
        }
    }

    fn into_docx_run(self) -> Run {
        let placeholder = format!("svg-placeholder-{}", self.media_id).into_bytes();
        let pic = Pic::new_with_dimensions(placeholder, 1, 1)
            .id(self.media_id)
            .size(
                pt_to_emu(self.width.max(1.0)),
                pt_to_emu(self.height.max(1.0)),
            );
        Run::new().add_image(pic)
    }
}

// MARK: Text item extraction
pub(crate) fn page_text_runs(
    items: &[SceneItem],
    source_index: &SourceIndex,
    math_ranges: &[MathRange],
    svg_fallback: bool,
    warnings: &mut Vec<String>,
) -> Vec<NativeRun> {
    const MIXED_WARNING: &str =
        "split a text run that mixes formula and non-formula glyphs using source ranges";

    let mut runs = Vec::new();

    for item in items {
        let SceneItem::Text(text) = item else {
            continue;
        };

        if text.text.trim().is_empty() {
            continue;
        }

        if svg_fallback {
            match text_math_state(text, source_index, math_ranges) {
                TextMathState::AllMath => continue,
                TextMathState::Mixed => {
                    if !warnings.iter().any(|warning| warning == MIXED_WARNING) {
                        warnings.push(MIXED_WARNING.to_string());
                    }
                    runs.extend(split_non_math_text_runs(text, source_index, math_ranges));
                    continue;
                }
                TextMathState::NoMath => {}
            }
        }

        runs.push(text_run_from_bounds(text, text.bounds, text.text.clone()));
    }

    runs
}

pub(crate) fn collect_formula_regions_from_text(
    items: &[SceneItem],
    source_index: &SourceIndex,
    math_ranges: &[MathRange],
    regions: &mut Vec<FormulaRegion>,
) {
    for item in items {
        let SceneItem::Text(text) = item else {
            continue;
        };

        for glyph in &text.glyphs {
            if let Some(math_id) = source_index.glyph_math_id(glyph, math_ranges) {
                add_formula_region(regions, math_id, glyph.bounds, Some(text.origin.y));
            }
        }
    }
}

fn split_non_math_text_runs(
    text: &SceneText,
    source_index: &SourceIndex,
    math_ranges: &[MathRange],
) -> Vec<NativeRun> {
    let mut runs = Vec::new();
    let mut current_text = String::new();
    let mut current_bounds = None;

    for glyph in &text.glyphs {
        if source_index.glyph_math_id(glyph, math_ranges).is_some() {
            push_pending_text_run(text, &mut runs, &mut current_text, &mut current_bounds);
            continue;
        }

        let Some(piece) = source_index.glyph_source_text(glyph) else {
            push_pending_text_run(text, &mut runs, &mut current_text, &mut current_bounds);
            continue;
        };

        if piece.is_empty() {
            continue;
        }

        current_text.push_str(piece);
        current_bounds = Some(match current_bounds {
            Some(bounds) => rect_union(bounds, glyph.bounds),
            None => glyph.bounds,
        });
    }

    push_pending_text_run(text, &mut runs, &mut current_text, &mut current_bounds);
    runs
}

fn push_pending_text_run(
    text: &SceneText,
    runs: &mut Vec<NativeRun>,
    current_text: &mut String,
    current_bounds: &mut Option<SceneRect>,
) {
    if current_text.is_empty() {
        *current_bounds = None;
        return;
    }

    if let Some(bounds) = current_bounds.take() {
        runs.push(text_run_from_bounds(
            text,
            bounds,
            std::mem::take(current_text),
        ));
    } else {
        current_text.clear();
    }
}

fn text_run_from_bounds(text: &SceneText, bounds: SceneRect, value: String) -> NativeRun {
    NativeRun::Text(TextRun {
        x: bounds.x,
        width: bounds.width,
        top_y: bounds.y,
        height: bounds.height,
        baseline_y: text.origin.y,
        text: value,
        font_family: text.font_family.clone(),
        font_size: text.font_size,
        font_weight: text.font_weight,
        italic: text.italic,
        fill: text.fill,
    })
}

// MARK: Text line layout
pub(crate) fn text_lines_from_runs(mut runs: Vec<NativeRun>) -> Vec<TextLine> {
    runs.sort_by(|a, b| {
        a.baseline_y()
            .partial_cmp(&b.baseline_y())
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.x().partial_cmp(&b.x()).unwrap_or(Ordering::Equal))
    });

    let mut lines: Vec<TextLine> = Vec::new();
    for run in runs {
        if let Some(line) = lines.iter_mut().find(|line| line.can_accept(&run)) {
            line.push(run);
        } else {
            lines.push(TextLine::new(run));
        }
    }

    lines.sort_by(|a, b| {
        a.baseline_y
            .partial_cmp(&b.baseline_y)
            .unwrap_or(Ordering::Equal)
    });
    lines
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextMathState {
    NoMath,
    AllMath,
    Mixed,
}

fn text_math_state(
    text: &SceneText,
    source_index: &SourceIndex,
    math_ranges: &[MathRange],
) -> TextMathState {
    let mut math = 0;
    let mut non_math = 0;

    for glyph in &text.glyphs {
        if source_index.glyph_math_id(glyph, math_ranges).is_some() {
            math += 1;
        } else {
            non_math += 1;
        }
    }

    match (math, non_math) {
        (0, _) => TextMathState::NoMath,
        (_, 0) => TextMathState::AllMath,
        _ => TextMathState::Mixed,
    }
}
