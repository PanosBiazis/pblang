//! Diagnostic / error-formatting helpers extracted from `main.rs`. These are
//! pure utilities (no `clap` / `vm` dependencies) used to render parse-error
//! source contexts with carets and line/column information.

use pblang_core::ParseError;

pub(crate) fn line_col(src: &str, byte_idx: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in src.char_indices() {
        if i >= byte_idx {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub(crate) fn get_line(src: &str, line_no: usize) -> &str {
    src.lines().nth(line_no.saturating_sub(1)).unwrap_or("")
}

pub(crate) fn underline_span(
    src: &str,
    sp: std::ops::Range<usize>,
) -> (usize, usize, String, String) {
    let start = sp.start.min(src.len());
    let end = sp.end.min(src.len()).max(start.saturating_add(1));

    let (ln1, col1) = line_col(src, start);
    let (ln2, col2) = line_col(src, end);

    let line = get_line(src, ln1).to_string();

    // If the span is on one line, draw ^^^ under the range.
    // If it spans multiple lines, fall back to a single caret at the start.
    let len = if ln1 == ln2 {
        col2.saturating_sub(col1).max(1)
    } else {
        1
    };

    let marker = format!("{}{}", " ".repeat(col1.saturating_sub(1)), "^".repeat(len));

    (ln1, col1, line, marker)
}

pub(crate) fn format_parse_error(err: &ParseError) -> String {
    // Chumsky 1.0's `Simple` no longer exposes `expected`/`reason` (those moved
    // to `Rich`), so the message here is the short "unexpected char" line built
    // by `errs_to_owned`; span/found are surfaced via `print_parse_errors`.
    err.message.clone()
}

pub(crate) fn print_parse_errors(src: &str, errors: Vec<ParseError>, file: Option<&str>) {
    let max = 8usize;
    for (i, err) in errors.iter().take(max).enumerate() {
        let msg = format_parse_error(err);
        let (ln, col, line, marker) = underline_span(src, err.span.clone());
        let prefix = match file {
            Some(path) => format!("error[{}:{}:{}]:", path, ln, col),
            None => format!("error[{}:{}]:", ln, col),
        };
        eprintln!("{} {}", prefix, msg);
        eprintln!("  | {}", line);
        eprintln!("  | {}", marker);

        if i + 1 < errors.len().min(max) {
            eprintln!();
        }
    }

    if errors.len() > max {
        eprintln!("(and {} more parse errors)", errors.len() - max);
    }
}
