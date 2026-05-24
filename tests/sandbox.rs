//! Sandbox tests: VM with restricted capabilities must reject disallowed builtins.

use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{FfiPolicy, RuntimeError, StdlibConfig, VmLimits, VM};
use std::collections::HashSet;

fn run_with_config(config: StdlibConfig, script: &str) -> Result<(), RuntimeError> {
    let stmts = parse(script).expect("parse failed");
    let chunk = compile(stmts).unwrap_or_else(|e| panic!("compile failed: {e}"));
    let mut vm = VM::with_stdlib(config);
    vm.run(chunk)
}

fn run_collect_output(script: &str) -> Result<String, RuntimeError> {
    let stmts = parse(script).expect("parse failed");
    let mut chunk = compile(stmts).unwrap_or_else(|e| panic!("compile failed: {e}"));
    chunk.source_name = Some(std::rc::Rc::new("<test>".into()));
    chunk.source = Some(std::rc::Rc::new(script.into()));
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    let out = vm.run_collect(chunk)?;
    Ok(out
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n"))
}

#[test]
fn fs_sandbox_denied() {
    let config = StdlibConfig {
        fs: false,
        ..Default::default()
    };
    let result = run_with_config(config, r#"fs_write("evil.txt", "pwned");"#);
    let err = result.expect_err("expected runtime error when fs is disabled");
    assert!(
        err.message.contains("fs access denied"),
        "expected 'fs access denied', got: {}",
        err.message
    );
}

#[test]
fn exec_sandbox_denied() {
    let config = StdlibConfig {
        exec: false,
        ..Default::default()
    };
    let result = run_with_config(config, r#"exec("echo x");"#);
    let err = result.expect_err("expected runtime error when exec is disabled");
    assert!(
        err.message.contains("exec access denied"),
        "expected 'exec access denied', got: {}",
        err.message
    );
}

#[test]
fn proc_spawn_wait_sandbox_denied() {
    let config = StdlibConfig {
        exec: false,
        ..Default::default()
    };
    let result = run_with_config(config, r#"proc_spawn_wait("echo", []);"#);
    let err = result.expect_err("expected runtime error when exec is disabled");
    assert!(
        err.message.contains("proc_spawn_wait access denied")
            || err.message.contains("exec access denied"),
        "expected exec-gated access denied, got: {}",
        err.message
    );
}

#[test]
fn ffi_sandbox_denied() {
    let config = StdlibConfig {
        ffi: false,
        ..Default::default()
    };
    let result = run_with_config(config, r#"ffi_load("nonexistent.dll");"#);
    let err = result.expect_err("expected runtime error when ffi is disabled");
    assert!(
        err.message.contains("ffi access denied"),
        "expected 'ffi access denied', got: {}",
        err.message
    );
}

#[test]
fn instruction_limit_exceeded() {
    let stmts = parse("for i 99999 { print 1; }").expect("parse");
    let chunk = compile(stmts).expect("compile");
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    vm.set_limits(Some(VmLimits {
        max_instructions: Some(1000),
        max_stack_depth: None,
        max_memory_bytes: None,
        io_timeout_ms: None,
    }));
    let result = vm.run(chunk);
    let err = result.expect_err("expected instruction limit");
    assert!(
        err.message.contains("instruction limit exceeded"),
        "got: {}",
        err.message
    );
}

#[test]
fn stack_depth_limit_exceeded() {
    let stmts = parse(
        r#"
        fn recurse(n) {
            if n <= 0 { return 0; }
            return recurse(n - 1) + 1;
        }
        print recurse(5000);
        "#,
    )
    .expect("parse");
    let chunk = compile(stmts).expect("compile");
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    vm.set_limits(Some(VmLimits {
        max_instructions: None,
        max_stack_depth: Some(50),
        max_memory_bytes: None,
        io_timeout_ms: None,
    }));
    let result = vm.run(chunk);
    let err = result.expect_err("expected stack depth limit");
    assert!(
        err.message.contains("stack depth limit exceeded"),
        "got: {}",
        err.message
    );
}

#[test]
fn same_script_same_output_determinism() {
    let script = r#"
        let m = map{z: 3, a: 1, m: 2};
        print map_keys(m)[0];
        print map_keys(m)[1];
        print map_keys(m)[2];
    "#;
    let a = run_collect_output(script).expect("run 1");
    let b = run_collect_output(script).expect("run 2");
    assert_eq!(a, b, "determinism: same script must produce same output");
}

#[test]
fn ffi_allowlist_restricts_libraries() {
    // Loading any library when the policy has an empty allowed_libs must fail
    // with the explicit "not in allowlist" error — not "library load failed"
    // (which would mask a missing enforcement bug).
    let mut vm = VM::with_stdlib(StdlibConfig {
        ffi: true,
        ..Default::default()
    });
    vm.set_ffi_policy(Some(FfiPolicy {
        allow_all: false,
        allowed_libs: HashSet::new(),
        allowed_symbols: HashSet::new(),
    }));
    let stmts = parse(r#"ffi_load("./libnope.so");"#).expect("parse");
    let chunk = compile(stmts).expect("compile");
    let err = vm
        .run(chunk)
        .expect_err("ffi_load with empty allowlist must fail with allowlist error");
    assert!(
        err.message.contains("not in allowlist"),
        "expected allowlist error naming the library, got: {}",
        err.message
    );
    assert!(
        err.message.contains("libnope.so"),
        "expected error to name the rejected library, got: {}",
        err.message
    );
}

#[test]
fn ffi_allowlist_restricts_symbols() {
    // Calling a symbol that isn't in the policy's allowed_symbols set must
    // produce the explicit "not in allowlist" error naming the symbol —
    // proving the allowlist check runs *before* the handle lookup (the handle
    // is invalid here too, so a missing allowlist would be masked by a
    // generic "invalid handle" error).
    let mut vm = VM::with_stdlib(StdlibConfig {
        ffi: true,
        ..Default::default()
    });
    let mut syms = HashSet::new();
    syms.insert("allowed_sym".to_string());
    vm.set_ffi_policy(Some(FfiPolicy {
        allow_all: false,
        allowed_libs: HashSet::new(),
        allowed_symbols: syms,
    }));
    let stmts = parse(r#"ffi_call(1, "forbidden_sym", []);"#).expect("parse");
    let chunk = compile(stmts).expect("compile");
    let err = vm
        .run(chunk)
        .expect_err("ffi_call with restrictive policy must fail");
    assert!(
        err.message.contains("not in allowlist"),
        "expected allowlist error, got: {}",
        err.message
    );
    assert!(
        err.message.contains("forbidden_sym"),
        "expected error to name the rejected symbol, got: {}",
        err.message
    );
}

#[test]
fn minimal_vm_has_no_builtins() {
    let stmts = parse("print clock();").expect("parse");
    let chunk = compile(stmts).expect("compile");
    let mut vm = VM::new();
    let result = vm.run(chunk);
    let err = result.expect_err("VM::new() has no builtins, clock should be undefined");
    assert!(
        err.message.contains("undefined") || err.message.contains("clock"),
        "expected undefined global, got: {}",
        err.message
    );
}
