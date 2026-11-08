use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use typst2word::convert_typst_file_to_docx;

fn print_usage() {
    eprintln!(
        "Usage: typst2word <input.typ> [output.docx] [--dump-ir <ir.json>]\n\
         \n\
         Converts a Typst document (.typ) into Microsoft Word (.docx) while\n\
         preserving both visual layout and semantic structure (headings, lists,\n\
         tables, native DrawingML shapes, OMML formulas, CeTZ vector graphics,\n\
         and explicit placeholders for unsupported elements)."
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        return ExitCode::SUCCESS;
    }

    let mut input_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut dump_ir_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dump-ir" => {
                if i + 1 < args.len() {
                    dump_ir_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("Error: --dump-ir requires a file path");
                    return ExitCode::FAILURE;
                }
            }
            arg if arg.starts_with('-') => {
                eprintln!("Unknown option: {arg}");
                print_usage();
                return ExitCode::FAILURE;
            }
            arg => {
                if input_path.is_none() {
                    input_path = Some(PathBuf::from(arg));
                } else if output_path.is_none() {
                    output_path = Some(PathBuf::from(arg));
                } else {
                    eprintln!("Unexpected positional argument: {arg}");
                    print_usage();
                    return ExitCode::FAILURE;
                }
                i += 1;
            }
        }
    }

    let Some(input) = input_path else {
        print_usage();
        return ExitCode::FAILURE;
    };

    let output = output_path.unwrap_or_else(|| input.with_extension("docx"));

    match convert_typst_file_to_docx(&input, &output) {
        Ok(ir) => {
            if let Some(ir_path) = dump_ir_path {
                match serde_json::to_string_pretty(&ir) {
                    Ok(json) => {
                        if let Err(e) = std::fs::write(&ir_path, json) {
                            eprintln!(
                                "Warning: failed to write IR JSON to {}: {e}",
                                ir_path.display()
                            );
                        }
                    }
                    Err(e) => eprintln!("Warning: failed to serialize IR JSON: {e}"),
                }
            }

            let r = &ir.report;
            println!(
                "Converted {} -> {}\n  Pages: {}, Headings: {}, Paragraphs: {}, Lists: {}, Tables: {}\n  Equations (OMML): {}, Shapes (DrawingML): {}, CeTZ Canvases: {}, Images: {}\n  Quotes: {}, Code Blocks: {}, Figures: {}, Footnotes: {}, Links: {}, Unsupported: {}",
                input.display(),
                output.display(),
                r.pages_count,
                r.headings_count,
                r.paragraphs_count,
                r.lists_count,
                r.tables_count,
                r.equations_count,
                r.shapes_count,
                r.cetz_canvases_count,
                r.images_count,
                r.quotes_count,
                r.code_blocks_count,
                r.figures_count,
                r.footnotes_count,
                r.links_count,
                r.unsupported_elements.len()
            );
            for unsup in &r.unsupported_elements {
                println!("  ! {}", unsup.display_badge());
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("Conversion failed: {err}");
            ExitCode::FAILURE
        }
    }
}
