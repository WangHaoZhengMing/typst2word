//! Self-contained Typst 0.15 compiler backend, `World` implementation, and
//! semantic + layout query engine for `typst2word`.

pub mod compile;
pub mod world;

pub use compile::{CUSTOM_ROUTINES, CompiledTypstDocument, is_cetz_span};
pub use world::{GlobalTypstResources, TypstWorld};

/// Compile a `TypstWorld` into a `CompiledTypstDocument`.
pub fn compile_world(world: &TypstWorld) -> Result<CompiledTypstDocument, String> {
    CompiledTypstDocument::compile(world)
}
