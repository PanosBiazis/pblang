//! Compiler / optimizer correctness + robustness. The optimizer runs on every
//! compile (script, functions, thunks), so miscompiles or hangs here are global.

use pblang_compiler::compile;
use pblang_core::parse;
use pblang_ir::Value;

fn constants_of(src: &str) -> Vec<Value> {
    let stmts = parse(src).expect("parse");
    compile(stmts).expect("compile").constants
}
fn has_number(src: &str, n: f64) -> bool {
    constants_of(src)
        .iter()
        .any(|c| matches!(c, Value::Number(x) if *x == n))
}
fn has_bool(src: &str, b: bool) -> bool {
    constants_of(src)
        .iter()
        .any(|c| matches!(c, Value::Bool(x) if *x == b))
}

#[test]
fn folds_a_single_constant_pair() {
    assert!(has_number("let x = 2 + 3;", 5.0), "2+3 should fold to 5");
    assert!(has_number("let x = 10 - 4;", 6.0), "10-4 should fold to 6");
    assert!(has_number("let x = 6 * 7;", 42.0), "6*7 should fold to 42");
    assert!(has_number("let x = 8 / 2;", 4.0), "8/2 should fold to 4");
}

#[test]
fn folds_comparisons() {
    assert!(has_bool("let x = 3 < 5;", true), "3<5 should fold to true");
    assert!(has_bool("let x = 9 == 2;", false), "9==2 should fold to false");
}

#[test]
fn does_not_fold_division_by_zero() {
    // 1/0 must be left for the runtime (folding it would bake in inf), so no
    // folded constant should appear — the literals 1 and 0 remain unfolded.
    let cs = constants_of("let x = 1 / 0;");
    let folded = cs
        .iter()
        .filter(|c| matches!(c, Value::Number(_)))
        .count();
    // only the two literals 1.0 and 0.0 — no third "folded" number
    assert!(folded <= 2, "1/0 must not be folded, got constants {cs:?}");
}

#[test]
fn folds_a_constant_chain_to_a_fixpoint() {
    // (((2+3)+4)+5) = 14. A single fold only reaches 5; the optimizer must
    // iterate so the whole chain collapses to one constant.
    assert!(
        has_number("let x = 2 + 3 + 4 + 5;", 14.0),
        "a constant chain should fully fold to 14"
    );
    assert!(
        has_number("let x = 2 * 3 * 4;", 24.0),
        "2*3*4 should fully fold to 24"
    );
}

#[test]
fn folds_constant_string_concatenation() {
    // pblang strings are immutable, so "a" + "b" can fold to "ab" at compile time.
    let has_ab = constants_of("let s = \"a\" + \"b\";")
        .iter()
        .any(|c| matches!(c, Value::Obj(o) if format!("{o:?}").contains("ab")));
    assert!(has_ab, "\"a\"+\"b\" should fold to the string \"ab\"");
    // a chain folds fully too: "a"+"b"+"c" -> "abc"
    let has_abc = constants_of("let s = \"a\" + \"b\" + \"c\";")
        .iter()
        .any(|c| matches!(c, Value::Obj(o) if format!("{o:?}").contains("abc")));
    assert!(has_abc, "a constant string chain should fold to \"abc\"");
}

#[test]
fn compiles_large_branchy_program_without_hanging() {
    // Many conditionals/loops exercise the jump-threading pass; this must always
    // terminate (the test completing is the assertion — a jump cycle would hang).
    let mut src = String::from("let acc = 0;\n");
    for i in 0..400 {
        src.push_str(&format!(
            "if {i} < 200 {{ acc = acc + 1; }} else {{ acc = acc - 1; }}\n"
        ));
    }
    src.push_str("print acc;\n");
    let stmts = parse(&src).expect("parse");
    assert!(compile(stmts).is_ok(), "large branchy program must compile");
}
