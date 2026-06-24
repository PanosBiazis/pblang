//! An unterminated block comment must be a hard parse error — not silently
//! masked to EOF (which would make a stray `/*` delete the rest of the file).

use pblang_core::parse_strict;

fn err_msgs(src: &str) -> Vec<String> {
    parse_strict(src)
        .err()
        .map(|es| es.iter().map(|e| e.message.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn unterminated_block_comment_is_an_error() {
    let msgs = err_msgs("let x = 1 /* oops, never closed");
    assert!(
        msgs.iter().any(|m| m.contains("unterminated block comment")),
        "expected an unterminated-comment error, got: {msgs:?}"
    );
}

#[test]
fn unterminated_nested_block_comment_is_an_error() {
    let msgs = err_msgs("let x = 1 /* a /* b */ still open");
    assert!(
        msgs.iter().any(|m| m.contains("unterminated block comment")),
        "nested unterminated comment must error, got: {msgs:?}"
    );
}

#[test]
fn terminated_block_comments_still_parse() {
    assert!(parse_strict("let x = 1 /* fine */").is_ok());
    assert!(parse_strict("let x = 1 /* a /* nested */ b */").is_ok());
    assert!(parse_strict("let x = 1 // line comment\nlet y = 2").is_ok());
}

#[test]
fn slash_star_inside_a_string_is_not_a_comment() {
    // A "/*" living inside a string literal must NOT count as an open comment.
    assert!(
        parse_strict("let s = \"/* not a comment\"").is_ok(),
        "/* inside a string must not be treated as an unterminated comment"
    );
}

#[test]
fn slash_star_inside_a_line_comment_is_not_an_open_comment() {
    // A "/*" that appears AFTER `//` is part of the line comment, not a block open.
    assert!(
        parse_strict("let x = 1 // /* not opened\nlet y = 2").is_ok(),
        "/* inside a // line comment must not open a block comment"
    );
}
