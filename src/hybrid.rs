use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::ops::Range;
use std::path::Path;

use docx_rs::{
    Docx, LineSpacing, LineSpacingType, PageMargin, PageSize, Paragraph, Pic, RelativeFromHType,
    RelativeFromVType, Run, SectionProperty, SectionType,
};
use typst::syntax::{Source, SpanKind, SyntaxKind, SyntaxNode};
use typst_preview_core::typst_scene::{SceneColor, SceneItem, SceneRect, SceneTextHit};
use typst_preview_core::{
    DocumentCompiler, DocumentInput, SceneDocument, SemanticParagraph, SourceLocation, SourceRange,
};
use typst_svg::SvgOptions;

use crate::docx_pack;

mod frame_items;

use frame_items::{
    FormulaRun, NativeRun, collect_image_fallbacks, crop_svg, extract_mathml_formulas,
    formula_crop, formula_regions, mathml_to_omml, page_text_runs, text_lines_from_runs,
    text_paragraphs_from_lines,
};

#[derive(Clone, Copy, Debug)]
pub struct ConvertOptions {
    pub svg_fallback: bool,
}

#[derive(Debug)]
pub struct ConversionReport {
    pub pages: usize,
    pub word_sections: usize,
    pub native_text_lines: usize,
    pub formula_omml_items: usize,
    pub formula_svg_images: usize,
    pub formula_mathml_items: usize,
    pub svg_fallback_items: usize,
    pub warnings: Vec<String>,
}

// MARK: Conversion pipeline
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
    let mut warnings = compiled.warnings.clone();
    let semantic_paragraphs = match compiler.compile_semantic_current() {
        Ok(compiled_semantic) => {
            source_index.semantic_paragraph_ranges(&compiled_semantic.paragraphs)
        }
        Err(error) => {
            warnings.push(format!("Semantic paragraph extraction skipped: {error}"));
            Vec::new()
        }
    };
    let mathml_formulas = match compiler.compile_html_current() {
        Ok(compiled_html) => {
            let formulas = extract_mathml_formulas(&compiled_html.html);
            let _mathml_bytes: usize = formulas.iter().map(|formula| formula.source().len()).sum();
            let _display_formulas = formulas.iter().filter(|formula| formula.display()).count();
            formulas
        }
        Err(error) => {
            warnings.push(format!("MathML extraction skipped: {error}"));
            Vec::new()
        }
    };
    let formula_mathml_items = mathml_formulas.len();
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
    let mut raw_xml_replacements = HashMap::new();
    let mut native_text_lines = 0;
    let mut formula_omml_items = 0;
    let mut formula_svg_images = 0;
    let mut svg_fallback_items = 0;
    let svg_options = SvgOptions::default();
    let mut mathml_formulas = mathml_formulas.into_iter();

    for (page_index, page) in scene.pages.iter().enumerate() {
        let mut page_svg = None;
        let mut page_paragraphs = Vec::new();
        let mut runs = page_text_runs(
            &page.items,
            &source_index,
            &math_ranges,
            &semantic_paragraphs,
            options.svg_fallback,
            &mut warnings,
        );

        if options.svg_fallback {
            let formula_regions = formula_regions(&page.items, &source_index, &math_ranges);
            if !formula_regions.is_empty() {
                let svg = page_svg.get_or_insert_with(|| {
                    typst_svg::svg(&compiled.document.pages()[page_index], &svg_options)
                });

                for (formula_index, region) in formula_regions.into_iter().enumerate() {
                    let crop = formula_crop(region.bounds(), page.width, page.height);

                    if let Some(formula) = mathml_formulas.next()
                        && let Some(omml) = mathml_to_omml(formula.source())
                    {
                        formula_omml_items += 1;
                        let placeholder =
                            format!("TYPSTOMMLP{}N{}", page_index + 1, formula_index + 1);
                        raw_xml_replacements.insert(placeholder.clone(), omml);
                        let paragraph_id = math_ranges.get(region.math_id()).and_then(|range| {
                            SourceIndex::paragraph_id_for_source_range(
                                &range.bytes,
                                &semantic_paragraphs,
                            )
                        });
                        runs.push(NativeRun::Formula(
                            FormulaRun::new_omml(
                                crop.x,
                                crop.width,
                                crop.y,
                                region.baseline_y(),
                                crop.height,
                                placeholder,
                            )
                            .with_paragraph_id(paragraph_id),
                        ));
                        continue;
                    }

                    formula_svg_images += 1;
                    let media_id =
                        format!("rIdTypstFormulaP{}N{}", page_index + 1, formula_index + 1);
                    let formula_svg = crop_svg(svg, crop);
                    svg_media.insert(media_id.clone(), formula_svg.into_bytes());
                    let paragraph_id = math_ranges.get(region.math_id()).and_then(|range| {
                        SourceIndex::paragraph_id_for_source_range(
                            &range.bytes,
                            &semantic_paragraphs,
                        )
                    });
                    runs.push(NativeRun::Formula(
                        FormulaRun::new(
                            crop.x,
                            crop.width,
                            crop.y,
                            region.baseline_y(),
                            crop.height,
                            media_id,
                        )
                        .with_paragraph_id(paragraph_id),
                    ));
                }
            }
        }

        if options.svg_fallback {
            svg_fallback_items += collect_image_fallbacks(
                &page.items,
                page_index,
                &source_index,
                &math_ranges,
                &mut svg_media,
                &mut page_paragraphs,
            );

            for (item_index, item) in page.items.iter().enumerate() {
                match item {
                    SceneItem::Shape(shape) => {
                        if source_index
                            .location_math_id(shape.source.as_ref(), &math_ranges)
                            .is_some()
                        {
                            continue;
                        }

                        let svg = page_svg.get_or_insert_with(|| {
                            typst_svg::svg(&compiled.document.pages()[page_index], &svg_options)
                        });
                        let crop = formula_crop(shape.bounds, page.width, page.height);
                        let media_id =
                            format!("rIdTypstShapeP{}N{}", page_index + 1, item_index + 1);
                        svg_media.insert(media_id.clone(), crop_svg(svg, crop).into_bytes());
                        page_paragraphs.push(floating_svg_paragraph(media_id, crop));
                        svg_fallback_items += 1;
                    }
                    SceneItem::Image(_) | SceneItem::Text(_) => {}
                }
            }
        }

        let lines = text_lines_from_runs(runs);
        native_text_lines += lines.len();
        let text_paragraphs = text_paragraphs_from_lines(lines);

        let mut previous_bottom = 0.0;
        for text_paragraph in text_paragraphs {
            let (paragraph, bottom) = text_paragraph.into_positioned_paragraph(previous_bottom);
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
    docx_pack::pack_with_svg(docx.build(), file, &svg_media, &raw_xml_replacements)?;

    Ok(ConversionReport {
        pages: scene.pages.len(),
        word_sections: scene.pages.len(),
        native_text_lines,
        formula_omml_items,
        formula_svg_images,
        formula_mathml_items,
        svg_fallback_items,
        warnings,
    })
}

// MARK: DOCX layout helpers
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

pub(crate) fn floating_svg_paragraph(media_id: String, bounds: SceneRect) -> Paragraph {
    let placeholder = format!("svg-placeholder-{media_id}").into_bytes();
    let pic = Pic::new_with_dimensions(placeholder, 1, 1)
        .id(media_id)
        .size(pt_to_emu(bounds.width), pt_to_emu(bounds.height));

    floating_positioned_pic_paragraph(pic, bounds)
}

pub(crate) fn floating_raster_paragraph(pic: Pic, bounds: SceneRect) -> Paragraph {
    let pic = pic.size(pt_to_emu(bounds.width), pt_to_emu(bounds.height));
    floating_positioned_pic_paragraph(pic, bounds)
}

fn floating_positioned_pic_paragraph(pic: Pic, bounds: SceneRect) -> Paragraph {
    let pic = pic
        .floating()
        .overlapping()
        .relative_from_h(RelativeFromHType::Page)
        .relative_from_v(RelativeFromVType::Page)
        .offset_x(pt_to_emu(bounds.x) as i32)
        .offset_y(pt_to_emu(bounds.y) as i32);
    floating_pic_paragraph(pic)
}

pub(crate) fn floating_pic_paragraph(pic: Pic) -> Paragraph {
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

// MARK: Source indexing
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

#[derive(Clone, Debug)]
pub(crate) struct SemanticParagraphRange {
    id: usize,
    bytes: Range<usize>,
}

impl SemanticParagraphRange {
    fn overlaps(&self, range: &Range<usize>) -> bool {
        self.bytes.start < range.end && range.start < self.bytes.end
    }

    fn overlap_len(&self, range: &Range<usize>) -> usize {
        if !self.overlaps(range) {
            return 0;
        }

        self.bytes.end.min(range.end) - self.bytes.start.max(range.start)
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
            if let Some(bytes) = self.span_range(node.span()) {
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

    fn glyph_source_range(&self, glyph: &SceneTextHit) -> Option<Range<usize>> {
        let start = self.location_to_byte(glyph.leading.as_ref()?)?;
        let end = self.location_to_byte(glyph.trailing.as_ref()?)?;
        Some(start.min(end)..end.max(start))
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

    fn semantic_paragraph_ranges(
        &self,
        paragraphs: &[SemanticParagraph],
    ) -> Vec<SemanticParagraphRange> {
        paragraphs
            .iter()
            .filter_map(|paragraph| {
                let bytes = self.source_range_to_bytes(paragraph.range.as_ref()?)?;
                if bytes.start >= bytes.end {
                    return None;
                }

                Some(SemanticParagraphRange {
                    id: paragraph.id,
                    bytes,
                })
            })
            .collect()
    }

    fn source_range_to_bytes(&self, range: &SourceRange) -> Option<Range<usize>> {
        let start = self.location_to_byte(&range.start)?;
        let end = self.location_to_byte(&range.end)?;
        Some(start.min(end)..end.max(start))
    }

    fn paragraph_id_for_source_range(
        range: &Range<usize>,
        paragraphs: &[SemanticParagraphRange],
    ) -> Option<usize> {
        paragraphs
            .iter()
            .filter(|paragraph| paragraph.overlaps(range))
            .max_by_key(|paragraph| paragraph.overlap_len(range))
            .map(|paragraph| paragraph.id)
    }

    fn span_range(&self, span: typst::syntax::Span) -> Option<Range<usize>> {
        match span.get() {
            SpanKind::Detached => None,
            SpanKind::Number { id, num } => {
                if id != self.source.id() {
                    return None;
                }
                self.source.range(num, None)
            }
            SpanKind::Range { id, range } => {
                if id != self.source.id() {
                    return None;
                }
                Some(range)
            }
        }
    }
}
// MARK: Geometry & unit helpers
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

pub(crate) fn pt_to_emu(pt: f32) -> u32 {
    (pt.max(0.0) * 12_700.0).round() as u32
}
