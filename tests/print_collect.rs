use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, Value, VM};

#[test]
fn prints_true_and_one() {
    let program = r#"
        print 1 == 1;
        if true { print 1; }
    "#;

    let stmts = parse(program).unwrap();
    let chunk = compile(stmts).expect("compile failed");

    let mut vm = VM::with_stdlib(StdlibConfig::default());
    let out = vm.run_collect(chunk).unwrap();

    assert_eq!(out, vec![Value::Bool(true), Value::Number(1.0)]);
}
