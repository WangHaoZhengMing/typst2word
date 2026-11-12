use std::fs;
use std::path::{Path, PathBuf};

use crate::model::{
    Category, DEFAULT_MARGIN_PT, ExpectationSpec, TestCaseSpec, default_margins_pt,
};

/// Recursively discover and parse all `.typ` suite files under `suites_root`.
pub fn load_suites_from_dir(suites_root: &Path) -> Result<Vec<TestCaseSpec>, String> {
    let mut files = Vec::new();
    collect_typ_files(suites_root, &mut files)?;
    files.sort();

    let mut all_cases = Vec::with_capacity(192);
    for file_path in files {
        let rel = file_path
            .strip_prefix(suites_root)
            .map_err(|e| format!("strip prefix: {e}"))?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let top_dir = rel
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .unwrap_or_default();
        let category = Category::from_dir_slug(&top_dir)
            .ok_or_else(|| format!("unknown top-level suite directory `{top_dir}` in {rel_str}"))?;

        let content = fs::read_to_string(&file_path)
            .map_err(|e| format!("failed to read suite {}: {e}", file_path.display()))?;
        let file_cases = parse_suite_file(&content, category, &rel_str)?;
        all_cases.extend(file_cases);
    }

    Ok(all_cases)
}

fn collect_typ_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Err(format!("suites directory does not exist: {}", dir.display()));
    }
    let entries =
        fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
        let p = entry.path();
        if p.is_dir() {
            collect_typ_files(&p, out)?;
        } else if p.extension().is_some_and(|ext| ext == "typ") {
            out.push(p);
        }
    }
    Ok(())
}

fn infer_default_func(rel_path: &str) -> String {
    let parts: Vec<&str> = rel_path.split('/').collect();
    if parts.len() >= 3 {
        // e.g. `06_layout/page/margins.typ` -> `page`, `99_packages/cetz/lines.typ` -> `cetz`
        parts[1].to_string()
    } else if let Some(last) = parts.last() {
        last.trim_end_matches(".typ").to_string()
    } else {
        String::new()
    }
}

fn parse_dims(val: &str) -> Option<(f64, f64)> {
    let cleaned = val.trim().replace("pt", "");
    let (w_s, h_s) = cleaned.split_once('x').or_else(|| cleaned.split_once(','))?;
    let w = w_s.trim().parse::<f64>().ok()?;
    let h = h_s.trim().parse::<f64>().ok()?;
    Some((w, h))
}

fn parse_margins_quad(val: &str) -> Result<[f64; 4], String> {
    let trimmed = val.trim().trim_start_matches('[').trim_end_matches(']');
    let nums: Vec<f64> = trimmed
        .split(',')
        .map(|s| s.trim().trim_end_matches("pt").parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("invalid @margins `{val}`: {e}"))?;
    match nums.as_slice() {
        [m] => Ok([*m, *m, *m, *m]),
        [y, x] => Ok([*y, *x, *y, *x]),
        [t, r, b, l] => Ok([*t, *r, *b, *l]),
        _ => Err(format!(
            "@margins expects 1, 2 (y, x), or 4 (top, right, bottom, left) numbers, got `{val}`"
        )),
    }
}

pub fn parse_suite_file(
    source: &str,
    category: Category,
    suite_rel_path: &str,
) -> Result<Vec<TestCaseSpec>, String> {
    let mut file_func = infer_default_func(suite_rel_path);
    let mut cases = Vec::new();

    // Split into header (before first `@case`) and `@case` blocks.
    let mut current_id: Option<String> = None;
    let mut current_lines: Vec<&str> = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("// === @case ") {
            if let Some(prev_id) = current_id.take() {
                cases.push(build_case_from_block(
                    &prev_id,
                    &current_lines,
                    category,
                    suite_rel_path,
                    &file_func,
                )?);
                current_lines.clear();
            }
            let id = rest.trim_end_matches("===").trim().to_string();
            if id.is_empty() {
                return Err(format!("empty @case id in {suite_rel_path}"));
            }
            current_id = Some(id);
        } else if current_id.is_none() {
            if let Some(val) = trimmed.strip_prefix("// @func:") {
                let f = val.trim();
                if !f.is_empty() {
                    file_func = f.to_string();
                }
            }
        } else {
            current_lines.push(line);
        }
    }

    if let Some(prev_id) = current_id.take() {
        cases.push(build_case_from_block(
            &prev_id,
            &current_lines,
            category,
            suite_rel_path,
            &file_func,
        )?);
    }

    Ok(cases)
}

fn build_case_from_block(
    id: &str,
    lines: &[&str],
    category: Category,
    suite_rel_path: &str,
    default_func: &str,
) -> Result<TestCaseSpec, String> {
    let mut title = id.to_string();
    let mut target_func = default_func.to_string();
    let mut target_param = String::new();
    let mut isolated_var: Option<String> = None;
    let mut content_dims = (340.0, 110.0);
    let mut raw_page_dims: Option<(f64, f64)> = None;
    let mut exp = ExpectationSpec::default_margin();
    exp.expected_margins_pt = default_margins_pt();

    let mut body_lines: Vec<&str> = Vec::new();
    let mut in_meta = true;

    for &line in lines {
        let trimmed = line.trim();
        if in_meta && trimmed.is_empty() {
            continue;
        }
        if in_meta
            && let Some(meta) = trimmed.strip_prefix("// @")
            && let Some((key, raw_val)) = meta.split_once(':')
        {
            let val = raw_val.trim();
            match key.trim() {
                "title" => title = val.to_string(),
                "func" => target_func = val.to_string(),
                "param" => target_param = val.to_string(),
                "var" => isolated_var = Some(val.to_string()),
                "size" => {
                    content_dims = parse_dims(val).ok_or_else(|| {
                        format!("invalid @size `{val}` in case `{id}` ({suite_rel_path})")
                    })?;
                }
                "raw_page" => {
                    if val == "true" {
                        raw_page_dims = Some((340.0, 160.0));
                    } else {
                        raw_page_dims = Some(parse_dims(val).ok_or_else(|| {
                            format!("invalid @raw_page `{val}` in case `{id}` ({suite_rel_path})")
                        })?);
                    }
                }
                "margins" => {
                    exp.expected_margins_pt = parse_margins_quad(val)?;
                }
                "section_margins" => {
                    let mut secs = Vec::new();
                    for part in val.split(';') {
                        let p = part.trim();
                        if !p.is_empty() {
                            secs.push(parse_margins_quad(p)?);
                        }
                    }
                    exp.expected_section_margins_pt = secs;
                }
                "mirror_margins" => {
                    exp.expected_mirror_margins = Some(val == "true");
                }
                "ir" => {
                    exp.required_ir_kinds = val
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                "xml" => {
                    exp.required_xml_snippets = val
                        .split('|')
                        .map(|s| {
                            let t = s.trim();
                            // Allow optional surrounding backticks when trailing spaces matter (e.g. `<w:u `).
                            if t.len() >= 2 && t.starts_with('`') && t.ends_with('`') {
                                t[1..t.len() - 1].to_string()
                            } else {
                                t.to_string()
                            }
                        })
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                "tol" => {
                    let nums: Vec<f64> = val
                        .split(',')
                        .map(|s| s.trim().trim_end_matches("pt").parse::<f64>())
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| format!("invalid @tol `{val}` in `{id}`: {e}"))?;
                    if nums.len() != 4 {
                        return Err(format!("@tol requires 4 numbers in `{id}`"));
                    }
                    exp.max_delta_left_pt = nums[0];
                    exp.max_delta_top_pt = nums[1];
                    exp.max_delta_width_pt = nums[2];
                    exp.max_delta_height_pt = nums[3];
                }
                "allow_unsupported" => {
                    exp.allow_unsupported = val == "true";
                }
                other => {
                    return Err(format!(
                        "unknown metadata key `@{other}` in case `{id}` ({suite_rel_path})"
                    ));
                }
            }
            continue;
        }
        in_meta = false;
        body_lines.push(line);
    }

    // Trim trailing blank lines while preserving one trailing newline.
    while body_lines.last().is_some_and(|l| l.trim().is_empty()) {
        body_lines.pop();
    }
    let raw_body = format!("{}\n", body_lines.join("\n"));

    let (page_width_pt, page_height_pt, typst_source) = if let Some((pw, ph)) = raw_page_dims {
        (pw, ph, raw_body)
    } else {
        let pw = content_dims.0 + 2.0 * DEFAULT_MARGIN_PT;
        let ph = content_dims.1 + 2.0 * DEFAULT_MARGIN_PT;
        let src = format!(
            "#set page(width: {pw}pt, height: {ph}pt, margin: {DEFAULT_MARGIN_PT}pt)\n{raw_body}"
        );
        (pw, ph, src)
    };

    let isolated_variable = isolated_var.unwrap_or_else(|| {
        if !target_param.is_empty() {
            format!("{target_func}({target_param})")
        } else {
            target_func.clone()
        }
    });

    Ok(TestCaseSpec {
        id: id.to_string(),
        category,
        suite_rel_path: suite_rel_path.to_string(),
        target_func,
        target_param,
        title,
        isolated_variable,
        page_width_pt,
        page_height_pt,
        typst_source,
        expectation: exp,
    })
}
