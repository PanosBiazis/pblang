//! Native exact int128 fixed-point decimal on the C path.
//! Transpile -> cc -> run -> assert stdout (must match the interpreter exactly
//! within int128 range). Covers construct, +, -, *, compare, eq, type, promote.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn run(src: &str) -> String {
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir();
    let c_path = dir.join(format!("pbdec_{}_{}.c", std::process::id(), id));
    let bin_path = dir.join(format!("pbdec_{}_{}", std::process::id(), id));
    std::fs::write(&c_path, &out.c_source).expect("write c");
    let cc = ["cc", "gcc", "clang"]
        .iter()
        .find(|c| Command::new(c).arg("--version").output().map(|o| o.status.success()).unwrap_or(false))
        .expect("a C compiler");
    let st = Command::new(cc).arg(&c_path).arg("-o").arg(&bin_path).arg("-lm").output().expect("cc");
    assert!(st.status.success(), "compile failed:\n{}\n{}", String::from_utf8_lossy(&st.stderr), out.c_source);
    let r = Command::new(&bin_path).output().expect("run");
    let _ = std::fs::remove_file(&c_path);
    let _ = std::fs::remove_file(&bin_path);
    String::from_utf8_lossy(&r.stdout).trim().to_string()
}

#[test]
fn decimal_exact_add() {
    // the canonical IEEE-754 trap, now exact in a native binary
    assert_eq!(run("print decimal(\"0.1\") + decimal(\"0.2\");"), "0.3");
    assert_eq!(run("print (decimal(\"0.1\") + decimal(\"0.2\")) == decimal(\"0.3\");"), "true");
}

#[test]
fn decimal_exact_mul() {
    // 1.005 * 100: the classic float-rounding bug, avoided
    assert_eq!(run("print decimal(\"1.005\") * decimal(\"100\");"), "100.5");
    assert_eq!(run("print decimal(\"1000000\") * decimal(\"1000000\");"), "1000000000000");
}

#[test]
fn decimal_no_summation_drift() {
    let src = "let s = decimal(\"0\"); let i = 0; while i < 10 { s = s + decimal(\"0.1\"); i = i + 1; } print s; print s == decimal(\"1\");";
    assert_eq!(run(src), "1\ntrue");
}

#[test]
fn decimal_subtract_and_negative() {
    assert_eq!(run("print decimal(\"0\") - decimal(\"5\");"), "-5");
    assert_eq!(run("print decimal(\"1.5\") - decimal(\"2.0\");"), "-0.5");
}

#[test]
fn decimal_compare() {
    assert_eq!(run("print decimal(\"100.50\") > decimal(\"100.49\");"), "true");
    assert_eq!(run("print decimal(\"1.1\") < decimal(\"1.10001\");"), "true");
    // equality is scale-insensitive
    assert_eq!(run("print decimal(\"2\") == decimal(\"2.00\");"), "true");
}

#[test]
fn decimal_type_and_promote() {
    assert_eq!(run("print type(decimal(\"1.5\"));"), "decimal");
    // a plain number promotes to decimal in mixed arithmetic
    assert_eq!(run("print decimal(\"1\") + 2;"), "3");
    assert_eq!(run("print decimal(\"0.50\") + 1;"), "1.5");
}
