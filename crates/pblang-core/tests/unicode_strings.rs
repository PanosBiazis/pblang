use pblang_core::{parse, ExprKind, StmtKind};

#[test]
fn parses_braced_unicode_scalar_escape() {
    let ast = parse(r#"print "\u{1F600}";"#).unwrap();
    assert_eq!(ast.len(), 1);
    match &ast[0].kind {
        StmtKind::Print(expr) => {
            assert!(matches!(&expr.kind, ExprKind::Str(s) if s == "😀"));
        }
        _ => panic!("expected print statement"),
    }
}

#[test]
fn parses_direct_unicode_string_text() {
    let ast = parse("print \"pblang π ∏ 😀\";").unwrap();
    assert_eq!(ast.len(), 1);
    match &ast[0].kind {
        StmtKind::Print(expr) => {
            assert!(matches!(&expr.kind, ExprKind::Str(s) if s == "pblang π ∏ 😀"));
        }
        _ => panic!("expected print statement"),
    }
}
