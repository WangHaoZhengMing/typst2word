//! Custom Typst (`.typ`) syntax highlighter for GPUI Kit's `Editor` and
//! lossless OpenXML (`document.xml`) pretty-formatter.

use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui_kit::{
    HighlightStyle, SharedString, Window,
    component::input::{
        EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
        InputHighlighterFactory, Rope,
    },
};

type SpanList = Vec<(Range<usize>, &'static str)>;

/// Factory installing `TypstInputHighlighter` when an `EditorState` uses `.language("typst")`.
pub fn typst_highlighter_factory() -> InputHighlighterFactory {
    Rc::new(|lang| {
        if matches!(lang, "typst" | "typ") {
            Some(Box::new(TypstInputHighlighter::default()))
        } else {
            None
        }
    })
}

#[derive(Default)]
struct TypstInputHighlighter {
    spans: Rc<RefCell<SpanList>>,
    folds: Rc<RefCell<Vec<FoldRange>>>,
}

impl InputHighlighter for TypstInputHighlighter {
    fn language(&self) -> SharedString {
        "typst".into()
    }

    fn update(
        &mut self,
        _edit: Option<InputEdit>,
        text: &Rope,
        folding: bool,
        _window: &mut Window,
        _cx: &mut gpui_kit::Context<EditorState>,
    ) {
        let src = text.to_string();
        *self.spans.borrow_mut() = tokenize_typst(&src);
        *self.folds.borrow_mut() = if folding {
            compute_typst_folds(&src)
        } else {
            Vec::new()
        };
    }

    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let spans = self.spans.borrow();
        let first = spans.partition_point(|(r, _)| r.end <= range.start);
        let mut out = Vec::new();
        let mut cursor = range.start;

        for (span, token_kind) in &spans[first..] {
            if span.start >= range.end {
                break;
            }
            let start = span.start.max(range.start);
            let end = span.end.min(range.end);
            if start >= end || end <= cursor {
                continue;
            }
            if cursor < start {
                out.push((cursor..start, HighlightStyle::default()));
            }
            out.push((start..end, resolver.style(token_kind).unwrap_or_default()));
            cursor = end;
        }
        if cursor < range.end {
            out.push((cursor..range.end, HighlightStyle::default()));
        }
        out
    }

    fn fold_ranges(&self, _: &Rope) -> Vec<FoldRange> {
        self.folds.borrow().clone()
    }
}

/// Fast single-pass UTF-8 scanner producing strictly ordered, non-overlapping
/// semantic highlight spans for Typst markup and code expressions.
fn tokenize_typst(src: &str) -> SpanList {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut spans: SpanList = Vec::new();
    let mut i = 0;
    let mut at_line_start = true;

    while i < len {
        let b = bytes[i];

        if b == b'\n' {
            at_line_start = true;
            i += 1;
            continue;
        }
        if b == b' ' || b == b'\t' || b == b'\r' {
            i += 1;
            continue;
        }

        // Heading at start of line: `= Title`, `== Section`
        if at_line_start && b == b'=' {
            let start = i;
            while i < len && bytes[i] == b'=' {
                i += 1;
            }
            if i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                spans.push((start..i, "title"));
                continue;
            }
            at_line_start = false;
            continue;
        }
        at_line_start = false;

        // Line comment `// ...`
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            let start = i;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            spans.push((start..i, "comment"));
            continue;
        }

        // Block comment `/* ... */`
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(len);
            spans.push((start..i, "comment"));
            continue;
        }

        // Quoted string `"..."`
        if b == b'"' {
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(len);
                    continue;
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            // Snap to UTF-8 boundary if truncated
            while i < len && !src.is_char_boundary(i) {
                i += 1;
            }
            spans.push((start..i, "string"));
            continue;
        }

        // Inline raw `` `...` ``
        if b == b'`' {
            let start = i;
            let mut ticks = 0;
            while i < len && bytes[i] == b'`' {
                ticks += 1;
                i += 1;
            }
            while i < len {
                if bytes[i] == b'`' {
                    let mut close = 0;
                    while i < len && bytes[i] == b'`' {
                        close += 1;
                        i += 1;
                    }
                    if close >= ticks {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            while i < len && !src.is_char_boundary(i) {
                i += 1;
            }
            spans.push((start..i, "text.literal"));
            continue;
        }

        // Math `$...$`
        if b == b'$' {
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(len);
                    continue;
                }
                if bytes[i] == b'$' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            while i < len && !src.is_char_boundary(i) {
                i += 1;
            }
            spans.push((start..i, "string.special"));
            continue;
        }

        // Typst `#directive` or `#function`
        if b == b'#' {
            let start = i;
            i += 1;
            let ident_start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            if i > ident_start {
                let word = &src[ident_start..i];
                let kind = if is_typst_keyword(word) {
                    "keyword"
                } else {
                    "function"
                };
                spans.push((start..i, kind));
            } else {
                spans.push((start..i, "operator"));
            }
            continue;
        }

        // Numeric literal + optional Typst unit (`12pt`, `0.5em`, `100%`, `1fr`, `2cm`)
        if b.is_ascii_digit()
            || (b == b'-' && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            if b == b'-' {
                i += 1;
            }
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            // Optional unit suffix
            let unit_start = i;
            if i < len && bytes[i] == b'%' {
                i += 1;
            } else {
                while i < len && bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let unit = &src[unit_start..i];
                if !matches!(
                    unit,
                    "" | "pt" | "mm" | "cm" | "in" | "em" | "fr" | "deg" | "rad"
                ) {
                    i = unit_start;
                }
            }
            spans.push((start..i, "number"));
            continue;
        }

        // ASCII identifier: keyword, boolean/constant, function call `foo(`, or named param `foo:`
        if is_ident_start(b) {
            let start = i;
            i += 1;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let word = &src[start..i];
            if is_typst_keyword(word) {
                spans.push((start..i, "keyword"));
            } else if matches!(word, "true" | "false") {
                spans.push((start..i, "boolean"));
            } else if matches!(word, "none" | "auto") {
                spans.push((start..i, "constant"));
            } else if i < len && bytes[i] == b':' {
                spans.push((start..i, "property"));
            } else if i < len && (bytes[i] == b'(' || bytes[i] == b'[') {
                spans.push((start..i, "function"));
            }
            continue;
        }

        // Brackets & delimiters
        if matches!(b, b'(' | b')' | b'[' | b']' | b'{' | b'}') {
            spans.push((i..i + 1, "punctuation.bracket"));
            i += 1;
            continue;
        }

        // Advance one full UTF-8 character
        if let Some(ch) = src[i..].chars().next() {
            i += ch.len_utf8();
        } else {
            i += 1;
        }
    }

    spans
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn is_typst_keyword(word: &str) -> bool {
    matches!(
        word,
        "set"
            | "show"
            | "let"
            | "import"
            | "include"
            | "if"
            | "else"
            | "for"
            | "while"
            | "in"
            | "not"
            | "and"
            | "or"
            | "return"
            | "break"
            | "continue"
            | "context"
            | "as"
    )
}

fn compute_typst_folds(src: &str) -> Vec<FoldRange> {
    let mut stack: Vec<(u8, usize)> = Vec::new();
    let mut folds = Vec::new();

    for (line_idx, line) in src.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut j = 0;
        let mut in_str = false;
        while j < bytes.len() {
            let c = bytes[j];
            if in_str {
                if c == b'\\' {
                    j += 2;
                    continue;
                }
                if c == b'"' {
                    in_str = false;
                }
                j += 1;
                continue;
            }
            if c == b'/' && j + 1 < bytes.len() && bytes[j + 1] == b'/' {
                break;
            }
            if c == b'"' {
                in_str = true;
            } else if matches!(c, b'{' | b'(' | b'[') {
                stack.push((c, line_idx));
            } else if matches!(c, b'}' | b')' | b']') {
                let expected = match c {
                    b'}' => b'{',
                    b')' => b'(',
                    _ => b'[',
                };
                if let Some(&(open, start_line)) = stack.last()
                    && open == expected
                {
                    stack.pop();
                    if line_idx.saturating_sub(start_line) >= 2 {
                        folds.push(FoldRange::new(start_line, line_idx));
                    }
                }
            }
            j += 1;
        }
    }
    folds
}

/// Formats OpenXML (`document.xml`) so long multi-namespace opening tags and
/// compact elements wrap cleanly onto readable indented lines for inspection.
pub fn format_xml_for_display(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with("<!--") {
        return raw.to_string();
    }

    let mut out = String::with_capacity(raw.len() + raw.len() / 4);
    let mut depth: usize = 0;
    let mut idx = 0;
    let bytes = trimmed.as_bytes();

    while idx < bytes.len() {
        if bytes[idx] == b'<' {
            let Some(rel_end) = trimmed[idx..].find('>') else {
                out.push_str(&trimmed[idx..]);
                break;
            };
            let tag = &trimmed[idx..=idx + rel_end];
            idx += rel_end + 1;

            if tag.starts_with("<?") || tag.starts_with("<!") {
                push_indent(&mut out, depth);
                out.push_str(tag);
                out.push('\n');
            } else if tag.starts_with("</") {
                depth = depth.saturating_sub(1);
                push_indent(&mut out, depth);
                out.push_str(tag);
                out.push('\n');
            } else {
                let self_closing = tag.ends_with("/>");
                // Check if next token is immediate text + closing tag: `<w:t>Hello</w:t>`
                if !self_closing
                    && let Some(next_lt) = trimmed[idx..].find('<')
                {
                    let text_between = &trimmed[idx..idx + next_lt];
                    if !text_between.trim().is_empty()
                        && !text_between.contains('\n')
                        && trimmed[idx + next_lt..].starts_with("</")
                        && let Some(close_gt) = trimmed[idx + next_lt..].find('>')
                    {
                        let close_tag = &trimmed[idx + next_lt..=idx + next_lt + close_gt];
                        push_indent(&mut out, depth);
                        out.push_str(&wrap_long_xml_tag(tag, depth));
                        out.push_str(text_between);
                        out.push_str(close_tag);
                        out.push('\n');
                        idx += next_lt + close_gt + 1;
                        continue;
                    }
                }

                push_indent(&mut out, depth);
                out.push_str(&wrap_long_xml_tag(tag, depth));
                out.push('\n');
                if !self_closing {
                    depth += 1;
                }
            }
        } else {
            let next_lt = trimmed[idx..].find('<').unwrap_or(trimmed.len() - idx);
            let text = trimmed[idx..idx + next_lt].trim();
            if !text.is_empty() {
                push_indent(&mut out, depth);
                out.push_str(text);
                out.push('\n');
            }
            idx += next_lt;
        }
    }

    out
}

fn push_indent(buf: &mut String, depth: usize) {
    for _ in 0..depth {
        buf.push_str("  ");
    }
}

/// Breaks tags with many attributes (like `<w:document xmlns:wpc=... xmlns:w=...>`)
/// onto aligned attribute lines when the tag exceeds 110 characters.
fn wrap_long_xml_tag(tag: &str, depth: usize) -> String {
    if tag.len() <= 110 {
        return tag.to_string();
    }
    let self_closing = tag.ends_with("/>");
    let inner = if self_closing {
        &tag[1..tag.len() - 2]
    } else {
        &tag[1..tag.len() - 1]
    }
    .trim();

    let mut parts: Vec<&str> = Vec::new();
    let b = inner.as_bytes();
    let mut start = 0;
    let mut i = 0;
    let mut in_quote = false;
    while i < b.len() {
        if b[i] == b'"' {
            in_quote = !in_quote;
        } else if !in_quote && b[i].is_ascii_whitespace() {
            if start < i {
                parts.push(&inner[start..i]);
            }
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            start = i;
            continue;
        }
        i += 1;
    }
    if start < b.len() {
        parts.push(&inner[start..]);
    }

    if parts.len() <= 3 {
        return tag.to_string();
    }

    let mut formatted = String::with_capacity(tag.len() + parts.len() * (depth * 2 + 5));
    formatted.push('<');
    formatted.push_str(parts[0]);
    for attr in &parts[1..] {
        formatted.push('\n');
        push_indent(&mut formatted, depth + 2);
        formatted.push_str(attr);
    }
    if self_closing {
        formatted.push_str("/>");
    } else {
        formatted.push('>');
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::{compute_typst_folds, format_xml_for_display, tokenize_typst};

    #[test]
    fn typst_tokenizer_handles_cjk_directives_units_and_math_safely() {
        let src = r#"// Atomic CJK visual test case
#set page(width: 380pt, height: 140pt, margin: 10pt)
#set text(font: "Heiti SC", size: 12pt, fallback: false)
= 一级标题 Heading
天地玄黄宇宙洪荒 `raw_code` $sum_(i=1)^n x_i = 1$
"#;
        let spans = tokenize_typst(src);
        assert!(!spans.is_empty());

        let mut prev_end = 0;
        for (range, kind) in &spans {
            assert!(range.start < range.end, "empty span for {kind}");
            assert!(
                range.start >= prev_end,
                "overlapping spans: {:?} after {}",
                range,
                prev_end
            );
            assert!(src.is_char_boundary(range.start));
            assert!(src.is_char_boundary(range.end));
            prev_end = range.end;
        }
    }

    #[test]
    fn typst_fold_ranges_detect_multiline_blocks() {
        let src = "#table(\n  columns: 2,\n  [A], [B],\n)\n";
        let folds = compute_typst_folds(src);
        assert_eq!(folds.len(), 1);
    }

    #[test]
    fn xml_formatter_wraps_long_namespace_root_and_preserves_inline_text() {
        let raw = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>天地玄黄</w:t></w:r></w:p></w:body></w:document>"#;
        let pretty = format_xml_for_display(raw);
        assert!(pretty.lines().count() >= 8);
        assert!(pretty.contains("      <w:t>天地玄黄</w:t>"));
        assert!(pretty.contains("xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">"));
    }
}
