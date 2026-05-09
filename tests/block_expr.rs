mod common;
use common::run_collect;
use pblang_vm::Value;

#[test]
fn block_expr_desugars_via_iife_and_returns_value() {
    let src = r#"
        let x = { let y = 1; y + 2 };
        print x;
    "#;

    let out = run_collect(src);
    assert_eq!(out, vec![Value::Number(3.0)]);
}
