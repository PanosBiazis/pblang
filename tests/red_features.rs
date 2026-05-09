use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, VM};

fn run(src: &str) -> Result<(), String> {
    let stmts = parse(src).map_err(|e| format!("parse error: {e:?}"))?;
    let chunk = compile(stmts).map_err(|e| format!("compile error: {e}"))?;
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    vm.run(chunk).map_err(|e| e.message)
}

#[test]
//#[ignore]  remove ignore after you implement comparison opcodes in compiler+vm
fn eq_expression_works() {
    run("print 1 == 1;").unwrap();
}

#[test]
//#[ignore]  remove ignore after you implement if compilation
fn if_works() {
    run("if true { print 1; }").unwrap();
}

#[test]
//#[ignore]  remove ignore after you implement while compilation
fn while_works() {
    run("let x = 0; while x < 3 { x = x + 1; } print x;").unwrap();
}

#[test]
//#[ignore]  remove ignore after you implement for compilation
fn for_works() {
    run("let sum = 0; for i 3 { sum = sum + 1; } print sum;").unwrap();
}
