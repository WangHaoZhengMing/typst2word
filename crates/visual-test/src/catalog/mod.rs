pub mod parser;
pub mod schema;

use std::path::PathBuf;

use crate::model::TestCaseSpec;

pub fn suites_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("suites")
}

/// Build the complete Typst Standard-Library-Mirrored visual test catalog from `suites/**/*.typ`.
pub fn build_catalog() -> Vec<TestCaseSpec> {
    parser::load_suites_from_dir(&suites_dir())
        .unwrap_or_else(|err| panic!("failed to load visual-test suites: {err}"))
}
