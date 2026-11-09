//! Stage 1 of `word-construct`: Sequential pre-processing pipeline over [`DocumentIr`].
//!
//! Runs a deterministic sequence of Word-oriented recognition and spacing passes before
//! constructing per-object Word layout constructs:
//! 1. [`WordPipelinePassKind::FontRecognition`]: Font family classification ([`FontFamilyClassIr`])
//!    and canonical CJK font alias resolution (`STSong` -> `Songti SC`).
//! 2. [`WordPipelinePassKind::LanguageAndScriptRecognition`]: CJK / East Asian symbol / Latin script
//!    classification ([`TextScriptClassIr`]) across text runs, code blocks, and math runs.
//! 3. [`WordPipelinePassKind::InterBlockFlowAndSpacing`]: Resolving `ParagraphPdfMetricsIr` and
//!    `InterBlockFlowIr` into paragraph spacing via [`crate::WordParagraphConstruct`].

pub mod flow_spacing;
pub mod font_recognize;
pub mod lang_script;

use ir::DocumentIr;

/// Exhaustive enum of sequential Word pre-processing passes executed over `DocumentIr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordPipelinePassKind {
    /// Pass 1: Classify each run's `FontFamilyClassIr` and normalize legacy CJK shorthand names.
    FontRecognition,
    /// Pass 2: Classify script/language characteristics (`TextScriptClassIr` and math `text_script`).
    LanguageAndScriptRecognition,
    /// Pass 3: Resolve Typst + PDF physical vertical spacing (`ParagraphPdfMetricsIr` and `InterBlockFlowIr`).
    InterBlockFlowAndSpacing,
}

impl WordPipelinePassKind {
    /// Ordered pipeline pass schedule.
    pub const ALL: [Self; 3] = [
        Self::FontRecognition,
        Self::LanguageAndScriptRecognition,
        Self::InterBlockFlowAndSpacing,
    ];

    /// Execute this single pass on `doc`.
    pub fn run(self, doc: &mut DocumentIr) {
        match self {
            Self::FontRecognition => font_recognize::run_font_recognition_pass(doc),
            Self::LanguageAndScriptRecognition => lang_script::run_language_and_script_pass(doc),
            Self::InterBlockFlowAndSpacing => flow_spacing::run_flow_spacing_pass(doc),
        }
    }
}

/// Run all sequential Word pre-processing passes on `doc` in canonical order.
pub fn run_word_construct_pipeline(doc: &mut DocumentIr) {
    for pass in WordPipelinePassKind::ALL {
        pass.run(doc);
    }
}
