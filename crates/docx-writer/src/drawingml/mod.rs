//! Native Microsoft Word DrawingML (`<wps:wsp>` and `<wpg:wgp>`) generator for:
//! 1. Typst shapes (`rect`, `square`, `circle`, `ellipse`, `line`, `polygon`, `path`, `curve`),
//!    including shapes containing rich text/formulas inside `<wps:txbx><w:txbxContent>`.
//! 2. CeTZ vector canvases (`cetz.canvas`) converted into native WordprocessingGroup
//!    (`<wpg:wgp>`) vector graphics with custom Bézier/line paths (`<a:custGeom>`) and
//!    positioned text/formula labels.

mod cetz;
mod geometry;
mod line;
mod shape;
mod textbox;

pub(crate) use cetz::cetz_canvas_to_drawingml_run;
pub(crate) use shape::shape_to_drawingml_run;

use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_DOC_PR_ID: AtomicU32 = AtomicU32::new(1000);

pub(crate) fn next_drawing_id() -> u32 {
    NEXT_DOC_PR_ID.fetch_add(1, Ordering::Relaxed)
}
