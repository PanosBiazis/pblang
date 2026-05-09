use pblang_vm::Value;

mod common;
use common::run_collect;

#[test]
fn if_else_executes_else_branch() {
    let src = r#"
        if false { print 1; } else { print 2; }
    "#;

    let out = run_collect(src);
    assert_eq!(out, vec![Value::Number(2.0)]);
}

#[test]
fn else_if_executes_middle_branch() {
    let src = r#"
        if false { print 1; }
        else if true { print 2; }
        else { print 3; }
    "#;

    let out = run_collect(src);
    assert_eq!(out, vec![Value::Number(2.0)]);
}
