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
fn ffi_missing_policy_is_fail_closed() {
    let mut vm = VM::with_stdlib(StdlibConfig {
        ffi: true,
        ..Default::default()
    });
    vm.set_ffi_policy(None);
    let stmts = parse(r#"ffi_load("./libnope.so");"#).expect("parse");
    let chunk = compile(stmts).expect("compile");
    let err = vm
        .run(chunk)
        .expect_err("missing FFI policy must deny before attempting a load");
    assert!(
        err.message.contains("not in allowlist"),
        "expected fail-closed allowlist error, got: {}",
        err.message
    );
}

#[test]
fn import_sandbox_denied_before_path_resolution() {
    let config = StdlibConfig {
        import: false,
        ..Default::default()
    };
    let result = run_with_config(config, r#"import("./does-not-exist.sapp");"#);
    let err = result.expect_err("expected runtime error when import is disabled");
    assert!(
        err.message.contains("import access denied"),
        "expected import capability denial, got: {}",
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
        max_external_json_bytes: None,
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
        max_external_json_bytes: None,
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

fn sandbox_profile_config() -> StdlibConfig {
    StdlibConfig {
        core: true,
        fs: false,
        net: false,
        http: false,
        json: true,
        crypto: true,
        sqlite: false,
        ffi: false,
        ffi_allow_all: false,
        ffi_allowed_libs: HashSet::new(),
        ffi_allowed_symbols: HashSet::new(),
        exec: false,
        gfx: false,
        import: false,
        stdlib_import: true,
        spawn: false,
        env: false,
        process_exit: false,
    }
}

#[test]
fn sandbox_allows_trusted_stdlib_import_and_denies_traversal_and_host_io() {
    let ok_res = run_with_config(
        sandbox_profile_config(),
        r#"
        let math = import("stdlib:math.sapp");
        if math.abs(-42) != 42 { throw "unexpected math.abs result"; }
        "#,
    );
    assert!(
        ok_res.is_ok(),
        "trusted stdlib:math.sapp import must succeed in sandbox profile: {ok_res:?}"
    );

    for (label, forbidden_snippet) in [
        ("stdlib parent traversal", r#"import("stdlib:../pblang.toml");"#),
        ("relative local import", r#"import("./evil.sapp");"#),
        ("parent relative import", r#"import("../secret.sapp");"#),
        ("fs_read", r#"fs_read("Cargo.toml");"#),
        ("tcp_connect", r#"tcp_connect("127.0.0.1:80");"#),
        ("ffi_load", r#"ffi_load("./libnope.so");"#),
        ("exec", r#"exec("ls");"#),
    ] {
        let err = run_with_config(sandbox_profile_config(), forbidden_snippet)
            .unwrap_err();
        assert!(
            err.message.contains("denied")
                || err.message.contains("escape")
                || err.message.contains("traversal")
                || err.message.contains("disallowed"),
            "{label} must fail with access/traversal denial, got: {}",
            err.message
        );
    }
}

#[test]
fn cli_sandbox_flag_runs_pure_stdlib_from_temp_dir_and_blocks_escapes() {
    let bin = env!("CARGO_BIN_EXE_pblang");
    let work = std::env::temp_dir().join(format!(
        "pblang_cli_sandbox_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&work).expect("mkdir temp test dir");
    let ok_script = work.join("ok_stdlib.sapp");
    std::fs::write(
        &ok_script,
        "let m = import(\"stdlib:math.sapp\");\nprint m.abs(-99);\n",
    )
    .expect("write ok_stdlib.sapp");

    let out = std::process::Command::new(bin)
        .current_dir(&work)
        .args(["run", ok_script.to_str().unwrap(), "--sandbox"])
        .output()
        .expect("spawn pblang --sandbox");
    assert!(
        out.status.success(),
        "CLI --sandbox stdlib import failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "99");

    let bad_script = work.join("bad_escape.sapp");
    std::fs::write(&bad_script, "import(\"stdlib:../Cargo.toml\");\n").expect("write bad_escape");
    let bad_out = std::process::Command::new(bin)
        .current_dir(&work)
        .args(["run", bad_script.to_str().unwrap(), "--sandbox"])
        .output()
        .expect("spawn pblang --sandbox traversal");
    assert!(
        !bad_out.status.success(),
        "CLI --sandbox must reject stdlib:../Cargo.toml traversal"
    );

    let _ = std::fs::remove_dir_all(&work);
}
