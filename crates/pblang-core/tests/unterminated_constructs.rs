//! Unterminated strings and polyglot blocks must produce clear, located errors
//! (not chumsky's vague "unexpected end of input at byte 0").

use pblang_core::parse_strict;

fn err_msgs(src: &str) -> Vec<String> {
    parse_strict(src)
        .err()
        .map(|es| es.iter().map(|e| e.message.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn unterminated_string_has_a_clear_message() {
    let msgs = err_msgs("let x = \"no closing quote");
    assert!(
        msgs.iter().any(|m| m.contains("unterminated string")),
        "expected a clear unterminated-string error, got: {msgs:?}"
    );
}

#[test]
fn terminated_strings_still_parse() {
    assert!(parse_strict("let x = \"ok\"").is_ok());
    assert!(parse_strict("let x = \"a \\\" b\"").is_ok()); // escaped quote inside
    assert!(parse_strict("let x = \"http://x\"").is_ok()); // // inside string
}

#[test]
fn unterminated_polyglot_block_has_a_clear_message() {
    let msgs = err_msgs("@python {\n  print(1)\n");
    assert!(
        msgs.iter().any(|m| m.contains("unterminated polyglot block")),
        "expected a clear unterminated-polyglot error, got: {msgs:?}"
    );
}

#[test]
fn closed_polyglot_block_still_parses() {
    assert!(parse_strict("@python { print(1) }\nlet x = 1;").is_ok());
    // nested braces inside the block must still close correctly
    assert!(parse_strict("@js { if (x) { y() } }\nlet z = 2;").is_ok());
}

#[test]
fn unterminated_errors_span_from_opener_to_eof() {
    // The error span should cover the whole dangling region (opener..EOF), not a
    // single point, so an editor can underline it.
    let src = "let x = 1 /* dangling";
    let errs = parse_strict(src).expect_err("must error");
    let e = &errs[0];
    assert_eq!(e.span.start, 10, "span starts at the /* opener");
    assert_eq!(e.span.end, src.len(), "span runs to EOF");
    assert!(e.span.end > e.span.start, "span is a range, not a point");

    let s = "let x = \"dangling";
    let errs = parse_strict(s).expect_err("must error");
    assert_eq!(errs[0].span.start, 8, "string span starts at the opening quote");
    assert_eq!(errs[0].span.end, s.len(), "string span runs to EOF");
}

#[test]
fn brace_inside_polyglot_string_does_not_break_closing() {
    // A '}' inside a string within the block must not be counted as the closer.
    assert!(
        parse_strict("@python { s = \"}\" }\nlet x = 1;").is_ok(),
        "a closing brace inside a polyglot string must not end the block early"
    );
}
