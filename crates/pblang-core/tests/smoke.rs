use pblang_core::parse;
use pblang_compiler::compile;
use pblang_vm::{StdlibConfig, VM};

fn run_ok(src: &str) {
    let stmts = parse(src).expect("parse failed");
    let chunk = compile(stmts);
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    vm.run(chunk).expect("vm run failed");
}

fn run_err(src: &str) -> String {
    let stmts = parse(src).expect("parse failed");
    let chunk = compile(stmts);
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    vm.run(chunk).expect_err("expected runtime error").message
}

#[test]
fn print_addition_smoke() {
    run_ok("print 1 + 2;");
}

#[test]
fn let_and_get_global() {
    run_ok("let x = 40; print x + 2;");
}

#[test]
fn zero_arg_function_call() {
    run_ok("fn five() { return 5; } print five();");
}

#[test]
fn string_concat() {
    run_ok(r#"fn hello() { return "Hello " + "world"; } print hello();"#);
}

#[test]
fn arity_mismatch_errors() {
    run_ok("fn f(a) { return 1; }"); // defining is fine
    let err = run_err("fn f(a) { return 1; } print f();");
    assert!(err.contains("arity mismatch"), "unexpected error: {err}");
}

#[test]
fn params_not_implemented_yet_should_error() {
    // This should fail until you implement locals (OpGetLocal/OpSetLocal).
    let err = run_err("fn add(a, b) { return a + b; } print add(2, 3);");
    assert!(err.contains("undefined global"), "unexpected error: {err}");
}
