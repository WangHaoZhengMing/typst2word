use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use typst2word::convert_typst_file_to_docx_with_options;
use typst2word::ir::FontEmbedMode;

fn print_usage() {
    eprintln!(
        "Usage: typst2word <input.typ> [output.docx] [--dump-ir <ir.json>] [--input key=value] [--embed-fonts=<subset|full|none>]\n\
         \n\
         Options:\n\
           --embed-fonts=<mode>  Font embedding mode: 'subset' (default), 'full', or 'none'\n\
           --no-embed-fonts      Shorthand for --embed-fonts=none\n\
           --dump-ir <ir.json>   Write extracted DocumentIr JSON for debugging\n\
           --input key=value     Pass sys.inputs key-value pair to Typst\n\
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
    let mut sys_inputs: Vec<(String, String)> = Vec::new();
    let mut embed_mode = FontEmbedMode::default();

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
            "--input" => {
                if i + 1 < args.len() {
                    let kv = &args[i + 1];
                    let Some((k, v)) = kv.split_once('=') else {
                        eprintln!("Error: --input requires key=value (got '{kv}')");
                        return ExitCode::FAILURE;
                    };
                    sys_inputs.push((k.to_string(), v.to_string()));
                    i += 2;
                } else {
                    eprintln!("Error: --input requires key=value");
                    return ExitCode::FAILURE;
                }
            }
            "--no-embed-fonts" => {
                embed_mode = FontEmbedMode::None;
                i += 1;
            }
            "--embed-fonts" => {
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    match FontEmbedMode::parse_cli(&args[i + 1]) {
                        Ok(mode) => {
                            embed_mode = mode;
                            i += 2;
                        }
                        Err(_) => {
                            embed_mode = FontEmbedMode::Subset;
                            i += 1;
                        }
                    }
                } else {
                    embed_mode = FontEmbedMode::Subset;
                    i += 1;
                }
            }
            arg if arg.starts_with("--embed-fonts=") => {
                let val = &arg["--embed-fonts=".len()..];
                match FontEmbedMode::parse_cli(val) {
                    Ok(mode) => {
                        embed_mode = mode;
                        i += 1;
                    }
                    Err(e) => {
                        eprintln!("Error: {e}");
                        return ExitCode::FAILURE;
                    }
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

    match convert_typst_file_to_docx_with_options(&input, &output, sys_inputs, embed_mode) {
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
            let mut seen_compiler_warnings = std::collections::BTreeSet::new();
            for w in &r.compiler_warnings {
                if seen_compiler_warnings.insert(w.as_str()) {
                    eprintln!("  ⚠️  {w}");
                }
            }
            for w in &r.font_fallback_warnings {
                eprintln!("  ⚠️  {w}");
            }
            for note in &r.font_embed_notes {
                println!("  ℹ️  {note}");
            }
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
