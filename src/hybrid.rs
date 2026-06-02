use std::cmp::Ordering;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::ops::Range;
use std::path::Path;

use docx_rs::{
    AlignmentType, Docx, LineSpacing, LineSpacingType, PageMargin, PageSize, Paragraph, Pic,
    RelativeFromHType, RelativeFromVType, Run, RunFonts, SectionProperty, SectionType,
};
use typst::syntax::{Source, SyntaxKind, SyntaxNode};
use typst_preview_core::typst_scene::{SceneColor, SceneItem, SceneRect, SceneText, SceneTextHit};
use typst_preview_core::{DocumentCompiler, DocumentInput, SceneDocument, SourceLocation};

use crate::docx_pack;

#[derive(Clone, Copy, Debug)]
pub struct ConvertOptions {
    pub svg_fallback: bool,
}

#[derive(Debug)]
pub struct ConversionReport {
    pub pages: usize,
    pub word_sections: usize,
    pub native_text_lines: usize,
    pub formula_svg_images: usize,
    pub svg_fallback_pages: usize,
    pub warnings: Vec<String>,
}

pub fn convert_to_docx(
    input: &DocumentInput,
    output: &Path,
    options: ConvertOptions,
) -> Result<ConversionReport, Box<dyn Error>> {
    let source = input.source_text();
    let source_index = SourceIndex::new(source.clone());
    let math_ranges = source_index.math_ranges();
    let compiler = DocumentCompiler::new(input, source);
    let compiled = compiler.compile_current()?;
    let scene = SceneDocument::from_paged_document(
        &compiled.document,
        compiled.warnings.clone(),
        &compiled.source_map,
    );

    let mut docx = Docx::new();
    if let Some(last_page) = scene.pages.last() {
        docx = docx
            .page_size(pt_to_twips(last_page.width), pt_to_twips(last_page.height))
            .page_margin(zero_page_margin());
    }

    let mut svg_media = HashMap::new();
    let mut native_text_lines = 0;
    let mut formula_svg_images = 0;
    let mut svg_fallback_pages = 0;
    let mut warnings = compiled.warnings.clone();

    for (page_index, page) in scene.pages.iter().enumerate() {
        let mut page_svg = None;
        let mut runs = page_text_runs(
            &page.items,
            &source_index,
            &math_ranges,
            options.svg_fallback,
            &mut warnings,
        );

        if options.svg_fallback {
            let formula_regions = formula_regions(&page.items, &source_index, &math_ranges);
            if !formula_regions.is_empty() {
                let svg = page_svg
                    .get_or_insert_with(|| typst_svg::svg(&compiled.document.pages[page_index]));

                for (formula_index, region) in formula_regions.into_iter().enumerate() {
                    formula_svg_images += 1;
                    let media_id =
                        format!("rIdTypstFormulaP{}N{}", page_index + 1, formula_index + 1);
                    let crop = formula_crop(region.bounds, page.width, page.height);
                    let formula_svg = crop_svg(svg, crop);
                    svg_media.insert(media_id.clone(), formula_svg.into_bytes());
                    runs.push(NativeRun::Formula(FormulaRun {
                        x: crop.x,
                        width: crop.width,
                        top_y: crop.y,
                        baseline_y: region.baseline_y(),
                        height: crop.height,
                        media_id,
                    }));
                }
            }
        }

        let mut page_paragraphs = Vec::new();
        let needs_page_svg = options.svg_fallback
            && page_has_non_formula_non_text(&page.items, &source_index, &math_ranges);

        if needs_page_svg {
            svg_fallback_pages += 1;
            let media_id = format!("rIdTypstSvgPage{}", page_index + 1);
            let svg = page_svg
                .take()
                .unwrap_or_else(|| typst_svg::svg(&compiled.document.pages[page_index]));
            svg_media.insert(media_id.clone(), svg.into_bytes());

            page_paragraphs.push(floating_page_svg_paragraph(
                media_id,
                page.width,
                page.height,
            ));
        }

        let lines = text_lines_from_runs(runs);
        native_text_lines += lines.len();

        let mut previous_bottom = 0.0;
        for line in lines {
            let (paragraph, bottom) = line.into_positioned_paragraph(previous_bottom);
            previous_bottom = bottom;
            page_paragraphs.push(paragraph);
        }

        if page_index + 1 < scene.pages.len() {
            page_paragraphs.push(Paragraph::new().section_property(page_section_property(
                page.width,
                page.height,
                Some(SectionType::NextPage),
            )));
        }

        if page_paragraphs.is_empty() {
            page_paragraphs.push(Paragraph::new());
        }

        for paragraph in page_paragraphs {
            docx = docx.add_paragraph(paragraph);
        }
    }

    let file = File::create(output)?;
    docx_pack::pack_with_svg(docx.build(), file, &svg_media)?;

    Ok(ConversionReport {
        pages: scene.pages.len(),
        word_sections: scene.pages.len(),
        native_text_lines,
        formula_svg_images,
        svg_fallback_pages,
        warnings,
    })
}

#[derive(Clone, Debug)]
struct TextLine {
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

    fn into_positioned_paragraph(mut self, previous_bottom: f32) -> (Paragraph, f32) {
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
enum NativeRun {
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
struct TextRun {
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
struct FormulaRun {
    x: f32,
    width: f32,
    top_y: f32,
    baseline_y: f32,
    height: f32,
    media_id: String,
}

impl FormulaRun {
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

fn page_text_runs(
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

fn text_lines_from_runs(mut runs: Vec<NativeRun>) -> Vec<TextLine> {
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

fn page_has_non_formula_non_text(
    items: &[SceneItem],
    source_index: &SourceIndex,
    math_ranges: &[MathRange],
) -> bool {
    items.iter().any(|item| match item {
        SceneItem::Shape(shape) => source_index
            .location_math_id(shape.source.as_ref(), math_ranges)
            .is_none(),
        SceneItem::Image(image) => source_index
            .location_math_id(image.source.as_ref(), math_ranges)
            .is_none(),
        SceneItem::Text(_) => false,
    })
}

fn page_section_property(
    width: f32,
    height: f32,
    section_type: Option<SectionType>,
) -> SectionProperty {
    let mut property = SectionProperty::new()
        .page_size(PageSize::new().size(pt_to_twips(width), pt_to_twips(height)))
        .page_margin(zero_page_margin());
    property.section_type = section_type;
    property
}

fn zero_page_margin() -> PageMargin {
    PageMargin {
        top: 0,
        left: 0,
        bottom: 0,
        right: 0,
        header: 0,
        footer: 0,
        gutter: 0,
    }
}

fn floating_page_svg_paragraph(media_id: String, width: f32, height: f32) -> Paragraph {
    let placeholder = format!("svg-placeholder-{media_id}").into_bytes();
    let pic = Pic::new_with_dimensions(placeholder, 1, 1)
        .id(media_id)
        .size(pt_to_emu(width), pt_to_emu(height))
        .floating()
        .overlapping()
        .relative_from_h(RelativeFromHType::Page)
        .relative_from_v(RelativeFromVType::Page)
        .offset_x(0)
        .offset_y(0);

    Paragraph::new()
        .line_spacing(
            LineSpacing::new()
                .before(0)
                .after(0)
                .line(1)
                .line_rule(LineSpacingType::Exact),
        )
        .add_run(Run::new().add_image(pic))
}

#[derive(Clone, Debug)]
struct MathRange {
    id: usize,
    bytes: Range<usize>,
}

impl MathRange {
    fn contains(&self, byte: usize) -> bool {
        self.bytes.start <= byte && byte < self.bytes.end
    }
}

#[derive(Debug)]
struct SourceIndex {
    source: Source,
}

impl SourceIndex {
    fn new(text: String) -> Self {
        Self {
            source: Source::detached(text),
        }
    }

    fn math_ranges(&self) -> Vec<MathRange> {
        let mut ranges = Vec::new();
        self.collect_math_ranges(self.source.root(), &mut ranges);
        ranges
    }

    fn collect_math_ranges(&self, node: &SyntaxNode, ranges: &mut Vec<MathRange>) {
        if node.kind() == SyntaxKind::Equation {
            if let Some(bytes) = self.source.range(node.span()) {
                ranges.push(MathRange {
                    id: ranges.len(),
                    bytes,
                });
            }
            return;
        }

        for child in node.children() {
            self.collect_math_ranges(child, ranges);
        }
    }

    fn glyph_math_id(&self, glyph: &SceneTextHit, ranges: &[MathRange]) -> Option<usize> {
        self.location_math_id(glyph.leading.as_ref(), ranges)
            .or_else(|| self.location_math_id(glyph.trailing.as_ref(), ranges))
    }

    fn glyph_source_text(&self, glyph: &SceneTextHit) -> Option<&str> {
        let start = self.location_to_byte(glyph.leading.as_ref()?)?;
        let end = self.location_to_byte(glyph.trailing.as_ref()?)?;
        self.source.text().get(start.min(end)..end.max(start))
    }

    fn location_math_id(
        &self,
        location: Option<&SourceLocation>,
        ranges: &[MathRange],
    ) -> Option<usize> {
        let byte = self.location_to_byte(location?)?;

        ranges
            .iter()
            .find(|range| range.contains(byte))
            .map(|range| range.id)
    }

    fn location_to_byte(&self, location: &SourceLocation) -> Option<usize> {
        self.source
            .lines()
            .line_column_to_byte(location.line, location.column)
    }
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

#[derive(Clone, Debug)]
struct FormulaRegion {
    math_id: usize,
    bounds: SceneRect,
    baseline_sum: f32,
    baseline_count: usize,
}

impl FormulaRegion {
    fn new(math_id: usize, bounds: SceneRect, baseline: Option<f32>) -> Self {
        let (baseline_sum, baseline_count) = baseline.map(|value| (value, 1)).unwrap_or((0.0, 0));

        Self {
            math_id,
            bounds,
            baseline_sum,
            baseline_count,
        }
    }

    fn add(&mut self, bounds: SceneRect, baseline: Option<f32>) {
        self.bounds = rect_union(self.bounds, bounds);
        if let Some(value) = baseline {
            self.baseline_sum += value;
            self.baseline_count += 1;
        }
    }

    fn baseline_y(&self) -> f32 {
        if self.baseline_count > 0 {
            self.baseline_sum / self.baseline_count as f32
        } else {
            self.bounds.y + self.bounds.height
        }
    }
}

fn formula_regions(
    items: &[SceneItem],
    source_index: &SourceIndex,
    math_ranges: &[MathRange],
) -> Vec<FormulaRegion> {
    let mut regions = Vec::new();

    for item in items {
        match item {
            SceneItem::Text(text) => {
                for glyph in &text.glyphs {
                    if let Some(math_id) = source_index.glyph_math_id(glyph, math_ranges) {
                        add_formula_region(
                            &mut regions,
                            math_id,
                            glyph.bounds,
                            Some(text.origin.y),
                        );
                    }
                }
            }
            SceneItem::Shape(shape) => {
                if let Some(math_id) =
                    source_index.location_math_id(shape.source.as_ref(), math_ranges)
                {
                    add_formula_region(&mut regions, math_id, shape.bounds, None);
                }
            }
            SceneItem::Image(image) => {
                if let Some(math_id) =
                    source_index.location_math_id(image.source.as_ref(), math_ranges)
                {
                    add_formula_region(&mut regions, math_id, image.bounds, None);
                }
            }
        }
    }

    regions.sort_by(|a, b| {
        a.bounds
            .y
            .partial_cmp(&b.bounds.y)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                a.bounds
                    .x
                    .partial_cmp(&b.bounds.x)
                    .unwrap_or(Ordering::Equal)
            })
    });
    regions
}

fn add_formula_region(
    regions: &mut Vec<FormulaRegion>,
    math_id: usize,
    bounds: SceneRect,
    baseline: Option<f32>,
) {
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }

    if let Some(region) = regions.iter_mut().find(|region| region.math_id == math_id) {
        region.add(bounds, baseline);
    } else {
        regions.push(FormulaRegion::new(math_id, bounds, baseline));
    }
}

fn formula_crop(bounds: SceneRect, page_width: f32, page_height: f32) -> SceneRect {
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

fn crop_svg(svg: &str, crop: SceneRect) -> String {
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

fn rect_union(a: SceneRect, b: SceneRect) -> SceneRect {
    let min_x = a.x.min(b.x);
    let min_y = a.y.min(b.y);
    let max_x = (a.x + a.width).max(b.x + b.width);
    let max_y = (a.y + a.height).max(b.y + b.height);

    SceneRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

fn color_hex(color: SceneColor) -> String {
    let channel = |value: f32| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };
    format!(
        "{:02X}{:02X}{:02X}",
        channel(color.r),
        channel(color.g),
        channel(color.b)
    )
}

fn pt_to_twips(pt: f32) -> u32 {
    (pt.max(0.0) * 20.0).round() as u32
}

fn pt_to_twips_i32(pt: f32) -> i32 {
    (pt.max(0.0) * 20.0).round() as i32
}

fn pt_to_emu(pt: f32) -> u32 {
    (pt.max(0.0) * 12_700.0).round() as u32
}
