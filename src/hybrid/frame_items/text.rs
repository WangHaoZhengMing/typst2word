// MARK: Text items
use std::cmp::Ordering;
use std::ops::Range;

use docx_rs::{
    AlignmentType, BreakType, LineSpacing, LineSpacingType, Paragraph, Pic, Run, RunFonts,
};
use typst_preview_core::typst_scene::{SceneColor, SceneItem, SceneRect, SceneText};

use super::super::{
    MathRange, SemanticParagraphRange, SourceIndex, color_hex, pt_to_emu, pt_to_twips,
    pt_to_twips_i32, rect_union,
};
use super::formula::{FormulaRegion, add_formula_region};

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

    fn append_runs_to_paragraph(self, mut paragraph: Paragraph, line_left_x: f32) -> Paragraph {
        let mut previous_right = None;

        if line_left_x > 0.0 {
            let spaces = ((self.left_x - line_left_x).max(0.0) / self.average_font_size() * 1.8)
                .round()
                .clamp(0.0, 80.0) as usize;
            if spaces > 0 {
                paragraph = paragraph.add_run(Run::new().add_text(" ".repeat(spaces)));
            }
        }

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

        paragraph
    }

    fn line_height(&self) -> f32 {
        (self.bottom_y - self.top_y)
            .max(
                self.runs
                    .iter()
                    .map(NativeRun::font_size)
                    .fold(1.0, f32::max),
            )
            .max(1.0)
    }

    fn average_font_size(&self) -> f32 {
        if self.runs.is_empty() {
            return 8.0;
        }

        self.runs.iter().map(NativeRun::font_size).sum::<f32>() / self.runs.len() as f32
    }

    fn paragraph_id(&self) -> Option<usize> {
        let mut counts: Vec<(usize, usize)> = Vec::new();
        for id in self.runs.iter().filter_map(NativeRun::paragraph_id) {
            if let Some((_, count)) = counts.iter_mut().find(|(candidate, _)| *candidate == id) {
                *count += 1;
            } else {
                counts.push((id, 1));
            }
        }

        counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(id, _)| id)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TextParagraph {
    paragraph_id: Option<usize>,
    top_y: f32,
    left_x: f32,
    bottom_y: f32,
    lines: Vec<TextLine>,
}

impl TextParagraph {
    fn new(line: TextLine) -> Self {
        Self {
            paragraph_id: line.paragraph_id(),
            top_y: line.top_y,
            left_x: line.left_x,
            bottom_y: line.bottom_y,
            lines: vec![line],
        }
    }

    fn can_accept(&self, line: &TextLine) -> bool {
        self.paragraph_id.is_some() && self.paragraph_id == line.paragraph_id()
    }

    fn push(&mut self, line: TextLine) {
        self.top_y = self.top_y.min(line.top_y);
        self.left_x = self.left_x.min(line.left_x);
        self.bottom_y = self.bottom_y.max(line.bottom_y);
        self.lines.push(line);
    }

    pub(crate) fn into_positioned_paragraph(mut self, previous_bottom: f32) -> (Paragraph, f32) {
        self.lines.sort_by(|a, b| {
            a.baseline_y
                .partial_cmp(&b.baseline_y)
                .unwrap_or(Ordering::Equal)
        });

        let before = (self.top_y - previous_bottom).max(0.0);
        let line_height = self
            .lines
            .iter()
            .map(TextLine::line_height)
            .fold(1.0, f32::max);

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

        let mut first = true;
        for line in self.lines {
            if first {
                first = false;
            } else {
                paragraph = paragraph.add_run(Run::new().add_break(BreakType::TextWrapping));
            }

            paragraph = line.append_runs_to_paragraph(paragraph, self.left_x);
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

    fn paragraph_id(&self) -> Option<usize> {
        match self {
            Self::Text(run) => run.paragraph_id,
            Self::Formula(run) => run.paragraph_id,
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
    paragraph_id: Option<usize>,
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
    paragraph_id: Option<usize>,
    payload: FormulaRunPayload,
}

#[derive(Clone, Debug)]
enum FormulaRunPayload {
    Svg { media_id: String },
    Omml { placeholder: String },
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
            paragraph_id: None,
            payload: FormulaRunPayload::Svg { media_id },
        }
    }

    pub(crate) fn new_omml(
        x: f32,
        width: f32,
        top_y: f32,
        baseline_y: f32,
        height: f32,
        placeholder: String,
    ) -> Self {
        Self {
            x,
            width,
            top_y,
            baseline_y,
            height,
            paragraph_id: None,
            payload: FormulaRunPayload::Omml { placeholder },
        }
    }

    pub(crate) fn with_paragraph_id(mut self, paragraph_id: Option<usize>) -> Self {
        self.paragraph_id = paragraph_id;
        self
    }

    fn into_docx_run(self) -> Run {
        match self.payload {
            FormulaRunPayload::Svg { media_id } => {
                let placeholder = format!("svg-placeholder-{media_id}").into_bytes();
                let pic = Pic::new_with_dimensions(placeholder, 1, 1)
                    .id(media_id)
                    .size(
                        pt_to_emu(self.width.max(1.0)),
                        pt_to_emu(self.height.max(1.0)),
                    );
                Run::new().add_image(pic)
            }
            FormulaRunPayload::Omml { placeholder } => Run::new().add_text(placeholder),
        }
    }
}

// MARK: Text item extraction
pub(crate) fn page_text_runs(
    items: &[SceneItem],
    source_index: &SourceIndex,
    math_ranges: &[MathRange],
    paragraphs: &[SemanticParagraphRange],
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
                    runs.extend(split_non_math_text_runs(
                        text,
                        source_index,
                        math_ranges,
                        paragraphs,
                    ));
                    continue;
                }
                TextMathState::NoMath => {}
            }
        }

        runs.push(text_run_from_bounds(
            text,
            text.bounds,
            text.text.clone(),
            text_source_range(text, source_index),
            paragraphs,
        ));
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
    paragraphs: &[SemanticParagraphRange],
) -> Vec<NativeRun> {
    let mut runs = Vec::new();
    let mut current_text = String::new();
    let mut current_bounds = None;
    let mut current_source_range = None;

    for glyph in &text.glyphs {
        if source_index.glyph_math_id(glyph, math_ranges).is_some() {
            push_pending_text_run(
                text,
                &mut runs,
                &mut current_text,
                &mut current_bounds,
                &mut current_source_range,
                paragraphs,
            );
            continue;
        }

        let Some(piece) = source_index.glyph_source_text(glyph) else {
            push_pending_text_run(
                text,
                &mut runs,
                &mut current_text,
                &mut current_bounds,
                &mut current_source_range,
                paragraphs,
            );
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
        if let Some(range) = source_index.glyph_source_range(glyph) {
            current_source_range = Some(merge_ranges(current_source_range.take(), range));
        }
    }

    push_pending_text_run(
        text,
        &mut runs,
        &mut current_text,
        &mut current_bounds,
        &mut current_source_range,
        paragraphs,
    );
    runs
}

fn push_pending_text_run(
    text: &SceneText,
    runs: &mut Vec<NativeRun>,
    current_text: &mut String,
    current_bounds: &mut Option<SceneRect>,
    current_source_range: &mut Option<Range<usize>>,
    paragraphs: &[SemanticParagraphRange],
) {
    if current_text.is_empty() {
        *current_bounds = None;
        *current_source_range = None;
        return;
    }

    if let Some(bounds) = current_bounds.take() {
        runs.push(text_run_from_bounds(
            text,
            bounds,
            std::mem::take(current_text),
            current_source_range.take(),
            paragraphs,
        ));
    } else {
        current_text.clear();
        *current_source_range = None;
    }
}

fn text_run_from_bounds(
    text: &SceneText,
    bounds: SceneRect,
    value: String,
    source_range: Option<Range<usize>>,
    paragraphs: &[SemanticParagraphRange],
) -> NativeRun {
    let paragraph_id = source_range
        .as_ref()
        .and_then(|range| SourceIndex::paragraph_id_for_source_range(range, paragraphs));

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
        paragraph_id,
    })
}

fn text_source_range(text: &SceneText, source_index: &SourceIndex) -> Option<Range<usize>> {
    text.glyphs
        .iter()
        .filter_map(|glyph| source_index.glyph_source_range(glyph))
        .fold(None, |current, range| Some(merge_ranges(current, range)))
}

fn merge_ranges(current: Option<Range<usize>>, range: Range<usize>) -> Range<usize> {
    match current {
        Some(current) => current.start.min(range.start)..current.end.max(range.end),
        None => range,
    }
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

pub(crate) fn text_paragraphs_from_lines(lines: Vec<TextLine>) -> Vec<TextParagraph> {
    let mut paragraphs: Vec<TextParagraph> = Vec::new();

    for line in lines {
        if let Some(paragraph) = paragraphs.last_mut()
            && paragraph.can_accept(&line)
        {
            paragraph.push(line);
            continue;
        }

        paragraphs.push(TextParagraph::new(line));
    }

    paragraphs
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
