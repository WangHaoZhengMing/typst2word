//! Self-contained Typst 0.15 compiler backend, `World` implementation, and
//! semantic + layout query engine for `typst2word`.

pub mod archive;
pub mod compile;
pub mod semantic;
pub mod world;

pub use archive::{unpack_package_tar_gz, unpack_tar_bytes};
pub use compile::{CUSTOM_ROUTINES, CompiledTypstDocument, is_cetz_span};
pub use semantic::{
    PAGE_STYLES_LABEL, RealizedNodeRecord, RealizedSemanticStore, RealizedStyleSnapshot,
    SCOPE_STYLES_LABEL,
};
pub use world::{CustomFontRegistry, GlobalTypstResources, TypstWorld, project_file_id};

/// Compile a `TypstWorld` into a `CompiledTypstDocument`.
pub fn compile_world(world: &TypstWorld) -> Result<CompiledTypstDocument, String> {
    CompiledTypstDocument::compile(world)
}
