mod docx_pack;
mod hybrid;

use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;

use typst_preview_core::DocumentInput;

use crate::hybrid::ConvertOptions;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(env::args_os().skip(1).collect())?;
    if args.help {
        print_usage();
        return Ok(());
    }

    let input = DocumentInput::from_env();
    let output = args.output.unwrap_or_else(|| default_output_path(&input));
    let report = hybrid::convert_to_docx(
        &input,
        &output,
        ConvertOptions {
            svg_fallback: args.svg_fallback,
        },
    )?;

    eprintln!(
        "wrote {} ({} Typst page(s), {} Word section(s), {} native text line(s), {} formula SVG image(s), {} SVG fallback page(s))",
        output.display(),
        report.pages,
        report.word_sections,
        report.native_text_lines,
        report.formula_svg_images,
        report.svg_fallback_pages
    );

    for warning in report.warnings {
        eprintln!("warning: {warning}");
    }

    Ok(())
}

#[derive(Debug)]
struct Args {
    output: Option<PathBuf>,
    svg_fallback: bool,
    help: bool,
}

impl Args {
    fn parse(raw: Vec<OsString>) -> Result<Self, Box<dyn Error>> {
        if raw.iter().any(|arg| arg == "--help" || arg == "-h") {
            return Ok(Self {
                output: None,
                svg_fallback: true,
                help: true,
            });
        }

        if raw.is_empty() {
            return Err("missing input .typ path; run with --help for usage".into());
        }

        if raw[0].to_string_lossy().starts_with('-') {
            return Err("the input .typ path must be the first argument".into());
        }

        let mut output = None;
        let mut svg_fallback = true;
        let mut index = 1;

        while index < raw.len() {
            match raw[index].to_string_lossy().as_ref() {
                "-o" | "--output" => {
                    index += 1;
                    let Some(path) = raw.get(index) else {
                        return Err("missing path after -o/--output".into());
                    };
                    output = Some(PathBuf::from(path));
                }
                "--no-svg-fallback" => svg_fallback = false,
                other => return Err(format!("unrecognized argument: {other}").into()),
            }

            index += 1;
        }

        Ok(Self {
            output,
            svg_fallback,
            help: false,
        })
    }
}

fn default_output_path(input: &DocumentInput) -> PathBuf {
    let main_path = input.editor_main_path();
    if main_path.file_name().and_then(|name| name.to_str()) == Some("__preview__.typ") {
        return PathBuf::from("typst-output.docx");
    }

    main_path.with_extension("docx")
}

fn print_usage() {
    eprintln!(
        "usage: typst2word <input.typ> [-o output.docx] [--no-svg-fallback]\n\
         \n\
         Converts Typst into a hybrid DOCX: native editable text plus SVG formulas and page fallbacks for other non-text content."
    );
}
