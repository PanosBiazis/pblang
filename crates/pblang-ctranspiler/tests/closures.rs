//! Closure / first-class-function support in the C transpiler.
//!
//! These tests transpile a pblang program to C, compile it with the host C
//! compiler, run it, and assert on stdout — proving real native behavior, not
//! just generated text. They build only pblang-core + pblang-ir + this crate
//! (no graphics), so `cargo test -p pblang-ctranspiler` is safe.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Transpile `src`, compile it, run it, and return trimmed stdout.
/// Panics with a clear message if any stage fails (so failures are legible).
fn run(src: &str) -> String {
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir();
    let c_path = dir.join(format!("pbclo_{}_{}.c", std::process::id(), id));
    let bin_path = dir.join(format!("pbclo_{}_{}", std::process::id(), id));
    std::fs::write(&c_path, &out.c_source).expect("write c");

    let cc = ["cc", "gcc", "clang"]
        .iter()
        .find(|c| Command::new(c).arg("--version").output().map(|o| o.status.success()).unwrap_or(false))
        .expect("a C compiler");

    let status = Command::new(cc)
        .arg(&c_path)
        .arg("-o")
        .arg(&bin_path)
        .arg("-lm")
        .output()
        .expect("invoke cc");
    assert!(
        status.status.success(),
        "C compile failed:\n{}\n--- C source ---\n{}",
        String::from_utf8_lossy(&status.stderr),
        out.c_source
    );

    let run = Command::new(&bin_path).output().expect("run binary");
    let _ = std::fs::remove_file(&c_path);
    let _ = std::fs::remove_file(&bin_path);
    assert!(run.status.success(), "binary exited non-zero");
    String::from_utf8_lossy(&run.stdout).trim().to_string()
}

#[test]
fn closure_captures_outer_variable() {
    // The canonical adder: the inner fn captures `n` from the enclosing call.
    let out = run("fn adder(n){ return fn(x){ return x + n; }; } let add5 = adder(5); print add5(100);");
    assert_eq!(out, "105");
}

#[test]
fn first_class_function_passed_and_called() {
    // A top-level function used as a value, stored in a variable, then called.
    let out = run("fn sq(x){ return x * x; } let f = sq; print f(9);");
    assert_eq!(out, "81");
}

#[test]
fn higher_order_function_takes_closure_param() {
    // Pass a closure into a function and call it indirectly inside.
    let out = run(
        "fn apply(f, x){ return f(x); } let g = fn(n){ return n * 3; }; print apply(g, 7);",
    );
    assert_eq!(out, "21");
}

// ── Optimization: captureless function values are static (no per-use malloc) ───

#[test]
fn captureless_function_value_is_static_no_malloc() {
    // A top-level function used as a value has no captured environment, so it
    // should compile to a STATIC closure singleton referenced by address — no
    // pb_make_closure() heap allocation each time it's evaluated.
    let src = "fn sq(x){ return x*x; } let f = sq; print f(6);";
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    assert!(
        out.c_source.contains("pb_fn_as_val"),
        "captureless fn value should use the static-closure helper, got:\n{}",
        out.c_source
    );
    assert_eq!(run(src), "36"); // still correct
}

#[test]
fn captureless_fn_literal_is_static() {
    // An anonymous function that captures nothing is also a static singleton.
    let src = "let inc = fn(x){ return x + 1; }; print inc(41);";
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    assert!(
        out.c_source.contains("pb_fn_as_val"),
        "captureless fn-literal should be a static closure"
    );
    assert_eq!(run(src), "42");
}

// ── Nested function definitions (fn inside fn) ─────────────────────────────────

#[test]
fn nested_function_definition() {
    let out = run("fn outer(n){ fn helper(x){ return x * 2; } return helper(n) + 1; } print outer(10);");
    assert_eq!(out, "21");
}

#[test]
fn nested_function_captures_enclosing_param() {
    let out = run("fn outer(n){ fn add(x){ return x + n; } return add(5); } print outer(100);");
    assert_eq!(out, "105");
}

// ── By-reference-style capture: mutation persists across a closure's own calls ──

#[test]
fn closure_mutates_captured_state_across_calls() {
    let out = run(
        "fn counter(){ let c = 0; return fn(){ c = c + 1; return c; }; } \
         let nx = counter(); print nx() + nx() + nx();",
    );
    assert_eq!(out, "6"); // 1 + 2 + 3
}

#[test]
fn independent_counters_do_not_share_state() {
    let out = run(
        "fn counter(){ let c = 0; return fn(){ c = c + 1; return c; }; } \
         let a = counter(); let b = counter(); print a() + a() + b();",
    );
    assert_eq!(out, "4"); // a:1, a:2, b:1
}

// ── Heap-cell upvalues: two closures SHARE one mutable captured variable ───────

#[test]
fn closures_share_mutable_captured_variable() {
    // inc mutates n; get reads it; both must see the same heap cell.
    let out = run(
        "fn pair(){ let n = 0; \
           let inc = fn(){ n = n + 1; return n; }; \
           let get = fn(){ return n; }; \
           inc(); inc(); return get(); } \
         print pair();",
    );
    assert_eq!(out, "2");
}

#[test]
fn self_recursive_nested_function() {
    let out = run(
        "fn outer(){ fn fact(k){ if k < 2 { return 1; } return k * fact(k - 1); } return fact(5); } \
         print outer();",
    );
    assert_eq!(out, "120");
}

// ── Regression: the common (non-closure) paths must still work unchanged ───────

#[test]
fn regression_direct_recursive_call() {
    let out = run("fn fib(n){ if n < 2 { return n; } return fib(n-1) + fib(n-2); } print fib(15);");
    assert_eq!(out, "610");
}

#[test]
fn regression_builtins_and_map_fields() {
    let out = run("let m = map{a: 7, b: 9}; print m.a + m.b + len([1,2,3]);");
    assert_eq!(out, "19");
}

#[test]
fn closure_independent_environments() {
    // Two closures from the same factory capture independent values.
    let out = run(
        "fn adder(n){ return fn(x){ return x + n; }; } \
         let a = adder(10); let b = adder(20); print a(1) + b(1);",
    );
    assert_eq!(out, "32"); // (1+10) + (1+20)
}
