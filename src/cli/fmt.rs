//! Formatter helpers extracted from `main.rs`. These functions implement the
//! `pblang fmt` (and IDE-side) source normalization / re-indentation that is
//! also used by the HTTP serve endpoints. They have no `clap` / `vm`
//! dependencies — pure string transforms.

/// IDE ergonomics: users often write JS/Python-style `print("x");`. Normalize
/// simple line-based forms to pblang's `print` statement.
pub(crate) fn normalize_ide_source(src: &str) -> String {
    let mut normalized = src
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("print(") && trimmed.ends_with(");") {
                let indent_len = line.chars().take_while(|c| c.is_whitespace()).count();
                let indent = &line[..indent_len];
                let inner = &trimmed["print(".len()..trimmed.len() - 2];
                format!("{indent}print {inner};")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if src.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}

pub(crate) fn format_ide_source(src: &str, max_line_length: Option<usize>) -> String {
    const INDENT: &str = "    ";
    let lines: Vec<&str> = src.lines().collect();
    let mut out = String::new();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut in_single_string = false;
    let mut block_comment_depth = 0;

    for line in &lines {
        let mut trimmed = line.trim().to_string();
        trimmed = fmt_normalize_operator_spacing(&trimmed);
        trimmed = fmt_normalize_comma_spacing(&trimmed);
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }
        // Separate each top-level function declaration with a blank line.
        if depth == 0
            && trimmed.starts_with("fn ")
            && !out.is_empty()
            && !out.ends_with("\n\n")
        {
            out.push('\n');
        }
        // A line that opens with '}' closes a block, so render the brace one
        // level out. depth itself is advanced only by scan_line_for_braces
        // below — decrementing it here as well would count the brace twice.
        let render_depth = if trimmed.starts_with('}') {
            (depth - 1).max(0)
        } else {
            depth.max(0)
        };
        let indent_str: String = (0..render_depth as usize).map(|_| INDENT).collect();
        let full_line = format!("{indent_str}{trimmed}");
        if let Some(max_len) = max_line_length {
            let mut line_to_output = full_line;
            while !line_to_output.is_empty() {
                if line_to_output.len() <= max_len {
                    out.push_str(&line_to_output);
                    out.push('\n');
                    scan_line_for_braces(
                        &line_to_output,
                        &mut in_string,
                        &mut in_single_string,
                        &mut block_comment_depth,
                        &mut depth,
                    );
                    break;
                }
                let break_at = line_to_output[..max_len].rfind(' ').unwrap_or(max_len);
                let break_at = if break_at == 0 { max_len } else { break_at };
                let rest = line_to_output.split_off(break_at);
                let emitted = line_to_output.trim_end().to_string();
                out.push_str(&emitted);
                out.push('\n');
                scan_line_for_braces(
                    &emitted,
                    &mut in_string,
                    &mut in_single_string,
                    &mut block_comment_depth,
                    &mut depth,
                );
                line_to_output = format!("{}{}", INDENT, rest.trim_start());
            }
        } else {
            out.push_str(&full_line);
            out.push('\n');
            scan_line_for_braces(
                &full_line,
                &mut in_string,
                &mut in_single_string,
                &mut block_comment_depth,
                &mut depth,
            );
        }
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub(crate) fn scan_line_for_braces(
    line: &str,
    in_double_string: &mut bool,
    in_single_string: &mut bool,
    block_comment_depth: &mut i32,
    brace_depth: &mut i32,
) {
    let mut chars = line.chars().peekable();
    let mut prev_was_backslash = false;

    while let Some(ch) = chars.next() {
        let next = chars.peek().copied();

        // If we're in a string, only care about escapes and matching close quote.
        if *in_double_string || *in_single_string {
            if *in_double_string && ch == '"' && !prev_was_backslash {
                *in_double_string = false;
            } else if *in_single_string && ch == '\'' && !prev_was_backslash {
                *in_single_string = false;
            }
            prev_was_backslash = ch == '\\' && !prev_was_backslash;
            continue;
        }

        // If we're in a block comment, look for nested /* and closing */
        if *block_comment_depth > 0 {
            if ch == '/' && next == Some('*') {
                chars.next(); // consume '*'
                *block_comment_depth += 1;
                continue;
            }
            if ch == '*' && next == Some('/') {
                chars.next(); // consume '/'
                *block_comment_depth -= 1;
                continue;
            }
            continue;
        }

        // Not in string/comment: detect start of line comment //
        if ch == '/' && next == Some('/') {
            break; // ignore rest of line
        }

        // Not in string/comment: detect start of block comment /*
        if ch == '/' && next == Some('*') {
            chars.next(); // consume '*'
            *block_comment_depth += 1;
            continue;
        }

        // Not in string/comment: entering string
        if ch == '"' {
            *in_double_string = true;
            prev_was_backslash = false;
            continue;
        }
        if ch == '\'' {
            *in_single_string = true;
            prev_was_backslash = false;
            continue;
        }

        // Count braces only in normal code
        match ch {
            '{' => *brace_depth += 1,
            '}' => *brace_depth -= 1,
            _ => {}
        }
    }
}

/// Normalize spacing around binary operators (outside string literals). Ensures one space
/// before and after + - * / == != < > <= >=. Does not change content inside "...".
pub(crate) fn fmt_normalize_operator_spacing(line: &str) -> String {
    let mut in_string = false;
    let mut escape_next = false;
    let mut i = 0;
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len() + 16);
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            if escape_next {
                escape_next = false;
                out.push(c);
            } else if c == '\\' {
                escape_next = true;
                out.push(c);
            } else if c == '"' {
                in_string = false;
                out.push(c);
            } else {
                out.push(c);
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        let need_space_before = !out.is_empty()
            && !out.ends_with(' ')
            && !out.ends_with('(')
            && !out.ends_with('[')
            && !out.ends_with('=')
            && !out.ends_with('!')
            && !out.ends_with('<')
            && !out.ends_with('>')
            && !out.ends_with(',')
            && !out.ends_with(';');
        // Multi-char operators (longest first)
        let (op_len, need_space_after) = if i + 2 <= bytes.len() {
            let two = std::str::from_utf8(&bytes[i..i + 2]).unwrap_or("");
            if two == "==" || two == "!=" || two == "<=" || two == ">=" {
                (2, true)
            } else {
                (0, false)
            }
        } else {
            (0, false)
        };
        let (op_len, need_space_after) = if op_len > 0 {
            (op_len, need_space_after)
        } else if matches!(c, '+' | '*' | '/' | '<' | '>') {
            (1, true)
        } else if c == '-' {
            // Unary minus (-1): no space after if next is digit and we didn't add space before
            let next_is_digit =
                i + 1 < bytes.len() && matches!(bytes[i + 1] as char, '0'..='9' | '.');
            let space_after = !next_is_digit || need_space_before;
            (1, space_after)
        } else {
            out.push(c);
            i += 1;
            continue;
        };
        let prev_ok = out.ends_with(' ')
            || out.is_empty()
            || out.ends_with('(')
            || out.ends_with('[')
            || out.ends_with(',')
            || out.ends_with(';');
        if need_space_before && !prev_ok {
            out.push(' ');
        }
        for j in 0..op_len {
            out.push(bytes[i + j] as char);
        }
        i += op_len;
        if need_space_after && i < bytes.len() {
            let next = bytes[i] as char;
            if next != ' '
                && next != ')'
                && next != ']'
                && next != ';'
                && next != ','
                && next != '\n'
            {
                out.push(' ');
            }
        }
    }
    out
}

/// Ensure comma is followed by a single space when not inside string literals.
pub(crate) fn fmt_normalize_comma_spacing(line: &str) -> String {
    let mut in_string = false;
    let mut escape_next = false;
    let mut i = 0;
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len() + 8);
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            if escape_next {
                escape_next = false;
                out.push(c);
            } else if c == '\\' {
                escape_next = true;
                out.push(c);
            } else if c == '"' {
                in_string = false;
                out.push(c);
            } else {
                out.push(c);
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == ',' {
            out.push(c);
            i += 1;
            if i < bytes.len() {
                let next = bytes[i] as char;
                if next != ' ' && next != '\n' && next != '\r' {
                    out.push(' ');
                }
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod ide_source_tests {
    use super::{format_ide_source, normalize_ide_source};

    #[test]
    fn rewrites_print_call_style_lines() {
        let src = "print(\"Hello\");\nprint(1 + 2);\n";
        let got = normalize_ide_source(src);
        assert_eq!(got, "print \"Hello\";\nprint 1 + 2;\n");
    }

    #[test]
    fn keeps_statement_style_unchanged() {
        let src = "print \"Hello\";\nlet x = 1;\n";
        let got = normalize_ide_source(src);
        assert_eq!(got, "print \"Hello\";\nlet x = 1;\n");
    }

    #[test]
    fn formats_simple_block_indentation() {
        let src = "fn x(){\nprint 1;\n}\n";
        let got = format_ide_source(src, None);
        assert!(got.contains("fn x("));
        assert!(got.contains("\n    print 1;\n"));
    }

    #[test]
    fn keeps_braces_inside_single_quoted_strings_from_affecting_indent() {
        let src = "fn x(){\nlet s = '{';\nprint 1;\n}\n";
        let got = format_ide_source(src, None);
        assert!(got.contains("\n    print 1;\n"));
        assert!(!got.contains("\n        print 1;\n"));
    }
}
