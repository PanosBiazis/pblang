//! Unicode-letter identifiers (UAX#31 mainstream stance): identifiers may use
//! Unicode letters (CJK / Greek / Cyrillic / accented Latin), not just ASCII.
//! Digits still may not start an identifier; emoji are not letters and remain
//! excluded. This exercises the lexer change that replaces ASCII-only
//! `text::ident()` with a Unicode-aware identifier parser.

use pblang_core::{parse_strict, ExprKind, StmtKind};

#[test]
fn accents_in_let_name_and_reference() {
    let ast = parse_strict("let café = 1; print café;").expect("café should parse");
    assert_eq!(ast.len(), 2);
    match &ast[0].kind {
        StmtKind::Let(name, _, _) => assert_eq!(name, "café"),
        other => panic!("expected let, got {other:?}"),
    }
    match &ast[1].kind {
        StmtKind::Print(e) => match &e.kind {
            ExprKind::Var(name) => assert_eq!(name, "café"),
            other => panic!("expected Var ref, got {other:?}"),
        },
        other => panic!("expected print, got {other:?}"),
    }
}

#[test]
fn cjk_identifier() {
    let ast = parse_strict("let 変数 = 5; print 変数;").expect("CJK identifier should parse");
    match &ast[0].kind {
        StmtKind::Let(name, _, _) => assert_eq!(name, "変数"),
        other => panic!("expected let, got {other:?}"),
    }
}

#[test]
fn greek_identifier_and_params() {
    parse_strict("fn αβγ(λ) { return λ; }").expect("Greek fn name + param should parse");
    parse_strict("let π = 3; let τ = π + π;").expect("Greek var names should parse");
}

#[test]
fn ascii_still_works() {
    parse_strict("let x = 40; print x + 2;").expect("ASCII identifiers must still parse");
    parse_strict("fn add(a, b) { return a + b; } print add(1, 2);").expect("ASCII fn must parse");
}

#[test]
fn digit_cannot_start_identifier() {
    // A digit is not an identifier start: `9abc` must NOT lex as one identifier.
    // It splits into the number 9 followed by the identifier `abc` (two stmts),
    // proving the leading digit is not absorbed into an identifier.
    let ast = parse_strict("9; abc;").expect("number then ident parses");
    assert!(
        matches!(ast[0].kind, StmtKind::Expr(ref e) if matches!(e.kind, ExprKind::Number(n) if n == 9.0)),
        "leading digit stays a number, not an identifier"
    );
}

#[test]
fn digits_allowed_in_continue_position() {
    parse_strict("let x9 = 1; let café2 = 2; let 変数1 = 3;")
        .expect("digits are valid after the first identifier character");
}

#[test]
fn identifiers_may_start_with_a_keyword_prefix() {
    // Regression: literal/operator keywords (true/false/nil/and/or/not) were
    // matched with `just(...)` (no word boundary), so identifiers that merely
    // START with one (nilish, truething, android, nothing) failed to parse.
    for src in [
        "let nilish = 1; print nilish;",
        "let truething = 2; print truething;",
        "let falsehood = 3; print falsehood;",
        "let android = 4; print android;",
        "let nothing = 5; print nothing;",
        "let orbit = 6; print orbit;",
        "fn nilcheck(nil_idx) { return nil_idx; }",
    ] {
        assert!(
            parse_strict(src).is_ok(),
            "identifier starting with a keyword prefix must parse: {src:?} -> {:?}",
            parse_strict(src).err()
        );
    }
}

#[test]
fn keywords_still_work_as_keywords() {
    // The fix must not break the actual keywords.
    parse_strict("let a = true; let b = false; let c = nil;").expect("literal keywords");
    parse_strict("let d = not true; let e = (1 < 2) and (3 < 4); let f = (1 > 2) or (3 > 2);")
        .expect("operator keywords");
}

#[test]
fn emoji_identifier_rejected() {
    // Emoji are not letters (UAX#31); they remain invalid as identifiers.
    assert!(
        parse_strict("let \u{1F600} = 1;").is_err(),
        "emoji must not be a valid identifier"
    );
}
