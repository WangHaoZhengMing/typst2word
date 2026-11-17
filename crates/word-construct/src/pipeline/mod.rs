//! Stateless Word-oriented recognition and flow-spacing helpers used during [`crate::build_word_document`]:
//! - [`font_recognize`]: Font family classification ([`font_recognize::FontFamilyClassIr`]) and
//!   read-only CJK font alias resolution for internal metric lookups (never mutating `font_family`).
//! - [`lang_script`]: CJK / East Asian symbol / Latin script classification ([`lang_script::TextScriptClassIr`]).
//! - [`flow_spacing`]: Resolving `NodeReferenceLayout` spans and inter-block transitions into Word spacing.

pub mod flow_spacing;
pub mod font_recognize;
pub mod lang_script;
