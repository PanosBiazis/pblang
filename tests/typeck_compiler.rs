//! Type-check patterns used by the self-hosting compiler modules.

use pblang_core::parse;
use pblang_typeck::check as type_check;

fn typecheck_source(src: &str) -> Vec<String> {
    let stmts = parse(src).expect("parse");
    type_check(&stmts)
        .into_iter()
        .map(|d| d.message)
        .collect()
}

#[test]
fn compiler_style_builtins_and_import_typecheck() {
    let diags = typecheck_source(
        r#"
        fn tokenize(source: string) -> array<any> {
            let tokens = [];
            let n = len(source);
            let i = 0;
            while i < n {
                push(tokens, source);
                i = i + 1;
            }
            return tokens;
        }
        let mod = import("stdlib/compiler/tokenizer.sapp");
        print len(mod);
        "#,
    );
    assert!(
        diags.is_empty(),
        "expected no type errors for compiler-style prelude usage: {:?}",
        diags
    );
}

#[test]
fn generic_helper_call_typechecks() {
    let diags = typecheck_source(
        r#"
        fn env_bind(name: string, ty: T) -> nil {
            return nil;
        }
        fn infer_expr(node: any) -> number {
            return 1;
        }
        let t = infer_expr(nil);
        env_bind("x", t);
        "#,
    );
    let hard_errors: Vec<_> = diags
        .iter()
        .filter(|m| m.contains("not declared") || m.contains("type mismatch"))
        .collect();
    assert!(
        hard_errors.is_empty(),
        "generic binding on call should not error: {:?}",
        hard_errors
    );
}

#[test]
fn compiler_stdlib_modules_parse() {
    for name in ["tokenizer", "typeck", "codegen", "parser"] {
        let path = format!("stdlib/compiler/{name}.sapp");
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {path}: {e}"));
        parse(&src).unwrap_or_else(|e| panic!("parse {path}: {e:?}"));
    }
}

#[test]
fn compiler_tokenizer_and_typeck_host_typecheck_clean() {
    for name in ["tokenizer", "typeck"] {
        let path = format!("stdlib/compiler/{name}.sapp");
        let src = std::fs::read_to_string(&path).expect("read");
        let diags = typecheck_source(&src);
        assert!(
            diags.is_empty(),
            "{path} should typecheck on the host: {:?}",
            diags
        );
    }
}

#[test]
fn forward_fn_reference_in_block_typechecks() {
    let diags = typecheck_source(
        r#"
        fn parse_stmt() -> array<any> {
            return [];
        }
        fn parse_program() -> number {
            let stmt = parse_stmt();
            return len(stmt);
        }
        "#,
    );
    let undeclared: Vec<_> = diags
        .iter()
        .filter(|m| m.contains("not declared"))
        .collect();
    assert!(
        undeclared.is_empty(),
        "forward fn references should resolve: {:?}",
        undeclared
    );
}
