use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, VM};

fn run_ok(src: &str) {
    let stmts = parse(src).expect("parse failed");
    let chunk = compile(stmts).expect("compile failed");
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    vm.run(chunk).expect("vm run failed");
}

#[test]
fn smoke_print_add() {
    run_ok("print 1 + 2;");
}

#[test]
fn smoke_globals_let_get() {
    run_ok("let x = 40; print x + 2;");
}

#[test]
fn smoke_fn_zero_args() {
    run_ok("fn five() { return 5; } print five();");
}

#[test]
fn smoke_string_concat() {
    run_ok(r#"fn hello() { return "Hello " + "world"; } print hello();"#);
}
