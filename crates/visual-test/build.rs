use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let swift_src = manifest_dir.join("tools/pdf_lab_inspector.swift");
    println!("cargo:rerun-if-changed={}", swift_src.display());

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        println!("cargo:rustc-env=VISUAL_TEST_INSPECTOR_BIN=");
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let bin_path = out_dir.join("pdf_lab_inspector");

    let status = Command::new("swiftc")
        .arg("-O")
        .arg(&swift_src)
        .arg("-o")
        .arg(&bin_path)
        .status();

    match status {
        Ok(s) if s.success() => {
            println!(
                "cargo:rustc-env=VISUAL_TEST_INSPECTOR_BIN={}",
                bin_path.display()
            );
        }
        Ok(s) => {
            panic!(
                "swiftc failed to compile {} (exit status: {s})",
                swift_src.display()
            );
        }
        Err(err) => {
            println!(
                "cargo:warning=swiftc unavailable during build ({err}); runtime fallback will be used"
            );
            println!("cargo:rustc-env=VISUAL_TEST_INSPECTOR_BIN=");
        }
    }
}
