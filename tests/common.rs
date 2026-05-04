//! Shared test helpers for integration tests.

use pblang_compiler::compile;
use pblang_core::parse_with_polyglot;
use pblang_vm::{RuntimeError, StdlibConfig, Value, VM};

/// Parse, compile, and run `src`; collect all `print`-ed values. Panics on error.
pub fn run_collect(src: &str) -> Vec<Value> {
    run_collect_with_config(StdlibConfig::default(), src)
}

/// Like `run_collect` but with a custom stdlib config (e.g. ffi=true).
pub fn run_collect_with_config(config: StdlibConfig, src: &str) -> Vec<Value> {
    let (stmts, _) = parse_with_polyglot(src).unwrap_or_else(|errs| {
        panic!("parse error: {:?}", errs);
    });
    let chunk = compile(stmts);
    let mut vm = VM::with_stdlib(config);
    vm.run_collect(chunk).unwrap_or_else(|e| {
        panic!("runtime error: {}", e.message);
    })
}

/// Run `src` and expect a runtime error; panic if it succeeds. Returns the error.
pub fn run_err(src: &str) -> RuntimeError {
    run_err_with_config(StdlibConfig::default(), src)
}

/// Like `run_err` but with a custom stdlib config.
pub fn run_err_with_config(config: StdlibConfig, src: &str) -> RuntimeError {
    let (stmts, _) = parse_with_polyglot(src).unwrap_or_else(|errs| {
        panic!("parse error: {:?}", errs);
    });
    let chunk = compile(stmts);
    let mut vm = VM::with_stdlib(config);
    match vm.run_collect(chunk) {
        Err(e) => e,
        Ok(vals) => panic!("expected runtime error, but got values: {:?}", vals),
    }
}
