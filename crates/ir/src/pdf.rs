//! Backwards-compatible aliases to [`crate::reference_layout`].
//!
//! All Typst Frame layout measurements now live in [`crate::reference_layout`]
//! (`ReferenceLayout`), strictly separated from semantic source intent.

pub use crate::reference_layout::{
    BlockLayoutSpanIr as BlockPdfSpanIr, EquationLayoutMetricsIr as EquationPdfMetricsIr,
    FlowReferenceIr as FlowMeasureIr, LayoutFragmentIr, NextBlockKindIr,
    NextBlockLayoutMeasureIr as NextBlockMeasureIr, NodeReferenceLayout,
};
