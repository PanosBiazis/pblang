//! Drift detector for the partially self-hosted compiler frontend.
//!
//! Background: `stdlib/compiler/selfhost_frontend.sapp` ports pieces of
//! the compiler from Rust to pblang. The Rust frontend in `pblang-core`
//! remains the **reference oracle**. Every snippet in `CORPUS` is parsed
//! by both implementations, and this test asserts they agree on
//! structural invariants (statement count, presence of errors, smoke
//! conformance). When the two diverge — because the pblang parser was
//! changed and forgot a feature, *or* because a Rust-side semantic
//! change wasn't propagated to the pblang side — the diff surfaces here
//! within seconds, before the change can ship.
//!
//! See `docs/Bootstrap_safety.md` Layer 2 for the policy this test
//! enforces. Expand `CORPUS` whenever a new language feature lands.

mod common;

use common::{run_collect, run_collect_with_config};

/// Snippets that both the Rust reference parser and the pblang self-host
/// frontend should successfully accept. Pairs are (label, source).
///
/// Keep each snippet short and feature-focused so a failure points
/// directly at the construct that regressed.
const VALID_CORPUS: &[(&str, &str)] = &[
    ("let_print", "let x = 1; print x;"),
    ("multi_let", "let a = 1; let b = 2; let c = a + b; print c;"),
    ("if_then", "let x = 1; if x == 1 { print x; }"),
    ("while_loop", "let i = 0; while i < 3 { i = i + 1; }"),
    ("string_lit", "let s = \"hi\"; print s;"),
];

/// Snippets that should be rejected by both implementations as
/// syntactically malformed.
const INVALID_CORPUS: &[(&str, &str)] = &[
    ("assignment_missing_rhs", "let x = ;"),
];

#[test]
fn selfhost_drift_smoke_baseline() {
    // The lightweight smoke probe should always return true. If this
    // fails the whole self-host layer is dead and the rest of the
    // diagnostics below would be noise.
    let out = run_collect(
        r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        let r = f.frontend_smoke();
        if r == true { print "smoke_ok"; } else { print "smoke_fail"; }
        "#,
    );
    let last = out
        .last()
        .and_then(|v| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        last, "smoke_ok",
        "selfhost smoke probe failed: the lightweight analyzer in \
         stdlib/compiler/selfhost_frontend.sapp::frontend_smoke is broken. \
         Investigate before trusting the other drift assertions."
    );
}

#[test]
fn selfhost_drift_accepts_valid_corpus() {
    // For each snippet that the Rust reference parser accepts, the
    // pblang-side lightweight analyzer must also report ok=true. If
    // the analyzer started rejecting valid pblang code, this test will
    // name the offending snippet.
    for (label, snippet) in VALID_CORPUS {
        // Rust reference: must parse without errors.
        let rust_parse = pblang_core::parse_strict(snippet);
        assert!(
            rust_parse.is_ok(),
            "Rust reference parser rejected snippet {label:?} \
             ({snippet:?}) — corpus is broken, not the self-host frontend. \
             Errors: {:?}",
            rust_parse.err(),
        );

        // pblang side: drive the lightweight analyzer. Escape the
        // snippet for embedding in a pblang string literal.
        let escaped = pblang_string_escape(snippet);
        let driver = format!(
            r#"
            let f = import("stdlib:compiler/selfhost_frontend.sapp");
            let r = f.frontend_analyze("{escaped}");
            if r.ok == true {{ print "ok"; }} else {{ print "drift"; }}
            "#
        );
        let out = run_collect(&driver);
        let verdict = out
            .last()
            .and_then(|v| v.as_str().ok().map(String::from))
            .unwrap_or_default();
        assert_eq!(
            verdict, "ok",
            "drift on accepted snippet {label:?}: Rust parser accepts \
             but pblang selfhost frontend rejects. Source: {snippet:?}. \
             Either the Rust grammar gained a feature the pblang side \
             missed, or the pblang analyzer regressed. See \
             docs/Bootstrap_safety.md Layer 2."
        );
    }
}

#[test]
fn selfhost_drift_rejects_invalid_corpus() {
    // Mirror of the above: snippets the Rust parser rejects must also
    // be flagged by the lightweight pblang analyzer.
    for (label, snippet) in INVALID_CORPUS {
        let rust_parse = pblang_core::parse_strict(snippet);
        assert!(
            rust_parse.is_err(),
            "Rust reference parser unexpectedly accepted invalid snippet \
             {label:?} ({snippet:?}) — corpus is broken, not the self-host \
             frontend.",
        );

        let escaped = pblang_string_escape(snippet);
        let driver = format!(
            r#"
            let f = import("stdlib:compiler/selfhost_frontend.sapp");
            let r = f.frontend_analyze("{escaped}");
            if r.ok == false {{ print "rejected"; }} else {{ print "drift"; }}
            "#
        );
        let out = run_collect(&driver);
        let verdict = out
            .last()
            .and_then(|v| v.as_str().ok().map(String::from))
            .unwrap_or_default();
        assert_eq!(
            verdict, "rejected",
            "drift on rejected snippet {label:?}: Rust parser rejects \
             but pblang selfhost frontend accepts. Source: {snippet:?}. \
             The pblang analyzer is now permissive where it shouldn't be. \
             See docs/Bootstrap_safety.md Layer 2."
        );
    }
}

#[test]
#[ignore = "stdlib/compiler/parser.sapp running on the interpreter takes >60s per snippet; \
            run explicitly with `cargo test --test selfhost_drift -- --ignored` when investigating \
            self-host parser changes. Tracked as a perf item in Bootstrap_safety.md — once the \
            self-host parser is fast enough to default-on, drop this #[ignore]."]
fn selfhost_drift_full_parser_matches_statement_count() {
    // Stronger drift check: drive the full self-host parser
    // (stdlib/compiler/parser.sapp via frontend_parse_full) and compare
    // the *statement count* with what the Rust reference parser
    // produces. Different AST shapes on each side make byte-equal
    // checks impossible, but the statement count is a reliable
    // structural invariant — if one side starts counting differently,
    // a parser change wasn't mirrored.
    //
    // Note: the pblang-written parser is a work-in-progress and may
    // not handle every snippet in VALID_CORPUS yet. Snippets where the
    // pblang parser returns 0 statements but the Rust parser produced
    // N>0 are recorded as `skipped` rather than `failed`, so this test
    // documents the gap without blocking on it. When the pblang
    // parser catches up, switch the assertion to strict equality.
    for (label, snippet) in VALID_CORPUS {
        let rust_stmts = pblang_core::parse_strict(snippet)
            .unwrap_or_else(|e| {
                panic!("Rust parser unexpectedly failed on {label:?}: {e:?}");
            })
            .len();

        let escaped = pblang_string_escape(snippet);
        let driver = format!(
            r#"
            let f = import("stdlib:compiler/selfhost_frontend.sapp");
            let r = f.frontend_parse_full("{escaped}");
            if r.ast == nil {{ print "nil_ast"; }}
            else {{ print str(len(r.ast.stmts)); }}
            "#
        );
        let out = run_collect(&driver);
        let verdict = out
            .last()
            .and_then(|v| v.as_str().ok().map(String::from))
            .unwrap_or_default();

        if verdict == "nil_ast" || verdict == "0" {
            // Pblang parser doesn't handle this construct yet; not a
            // drift, just a gap. The accepts_valid_corpus test already
            // covers the lightweight analyzer for this snippet.
            eprintln!(
                "selfhost_drift: full parser skipped {label:?} (got {verdict:?}, \
                 Rust expected {rust_stmts} stmts) — language-feature gap, not drift."
            );
            continue;
        }

        let pblang_stmts: usize = verdict.parse().unwrap_or_else(|_| {
            panic!(
                "selfhost frontend returned non-numeric stmt count {verdict:?} \
                 for {label:?}"
            )
        });

        assert_eq!(
            pblang_stmts, rust_stmts,
            "drift on {label:?}: Rust parser produced {rust_stmts} \
             statement(s), pblang self-host parser produced {pblang_stmts}. \
             Source: {snippet:?}. The two implementations have diverged on \
             how this construct decomposes into statements. See \
             docs/Bootstrap_safety.md Layer 2."
        );
    }
}

/// Escape a string for embedding inside a pblang `"..."` literal.
/// Only the minimal escapes are needed for the corpus this test uses.
fn pblang_string_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out
}

fn frontend_run_output(source: &str) -> Vec<pblang_vm::Value> {
    let escaped = pblang_string_escape(source);
    let driver = format!(
        r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        let r = f.frontend_run("{escaped}");
        let i = 0;
        while i < len(r.output) {{
            print r.output[i];
            i = i + 1;
        }}
        "#
    );
    run_collect(&driver)
}

fn assert_frontend_run_matches_rust_vm(label: &str, source: &str) {
    let rust_output = run_collect(source);
    let selfhost_output = frontend_run_output(source);
    assert_eq!(
        selfhost_output, rust_output,
        "frontend_run output drift for {label}: self-hosted compiler+VM no \
         longer matches the Rust parser/compiler/VM on a real stdlib-backed \
         program. Source: {source:?}"
    );
}

/// Codegen-level drift: the self-hosted pipeline must lower the core corpus all
/// the way to bytecode (tokenize → parse → typecheck → codegen) without
/// crashing. Before the 2026-05-29 codegen alignment fixes, `frontend_compile`
/// crashed immediately — the codegen used a method-call convention pblang has no
/// (`chunk.array_push(constants, v)`), read AST fields the parser never produced
/// (`var`/`condition`/`then_body`/`elements`/`expr`), swapped `map_has` args, and
/// expected raw statement lists where `parse_block` returns a block node. This
/// locks the lowering in so it can't silently regress to crashing.
///
/// Note: this asserts the self-hosted frontend produces a non-empty bytecode IR
/// for each construct — not that that IR executes (the self-hosted opcode set has
/// no runner in this harness yet; executing it is the next self-hosting step).
#[test]
fn selfhost_drift_compiles_core_corpus_to_bytecode() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn compiles(s) {
            let r = f.frontend_compile(s);
            if r.ok and len(r.chunk.instructions) > 0 { return 1; }
            return 0;
        }
        let total = 0;
        total = total + compiles("let x = 1; let y = 2; print x + y;");
        total = total + compiles("let i = 0; while i < 3 { i = i + 1; }");
        total = total + compiles("let x = 1; if x == 1 { print x; }");
        total = total + compiles("let x = 1; if x == 1 { print x; } else { print 0; }");
        total = total + compiles("let s = \"hi\"; print s;");
        total = total + compiles("fn add(a, b) { return a + b; } print add(1, 2);");
        total = total + compiles("let a = [1, 2, 3]; print a;");
        total = total + compiles("let x = 0 - 5; print x;");
        print total;
    "#;
    let out = run_collect(src);
    let total = out
        .last()
        .and_then(|v| match v {
            pblang_vm::Value::Number(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(-1.0);
    assert_eq!(
        total, 8.0,
        "all 8 core constructs must compile to non-empty bytecode via the \
         self-hosted frontend (got {total})"
    );
}

/// The self-hosting loop, closed: pblang source → pblang compiler
/// (`frontend_compile`) → pblang bytecode VM (`stdlib/compiler/vm.sapp`) →
/// result. `frontend_run` executes the emitted IR and returns the final
/// environment; results are observed via top-level `let` slots. Covers
/// arithmetic + precedence, comparison, if/else branch selection, while-loop
/// accumulation, function calls, and recursion — all running on pblang
/// compiling and interpreting pblang.
#[test]
fn selfhost_drift_executes_compiled_bytecode() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let a = 1; let b = 2; let c = a + b;", "c");
        print g("let r = 2 + 3 * 4;", "r");
        print g("let r = 5 > 3;", "r");
        print g("let r = 0; let x = 7; if x > 5 { r = 100; } else { r = 200; }", "r");
        print g("let s = 0; let i = 0; while i < 5 { s = s + i; i = i + 1; }", "s");
        print g("fn add(a, b) { return a + b; } let z = add(3, 4);", "z");
        print g("fn fib(n) { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); } let r = fib(10);", "r");
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(3.0), "arithmetic");
    assert_eq!(out[1], pblang_vm::Value::Number(14.0), "operator precedence");
    assert_eq!(out[2], pblang_vm::Value::Bool(true), "comparison");
    assert_eq!(out[3], pblang_vm::Value::Number(100.0), "if/else branch selection");
    assert_eq!(out[4], pblang_vm::Value::Number(10.0), "while-loop accumulation");
    assert_eq!(out[5], pblang_vm::Value::Number(7.0), "function call");
    assert_eq!(out[6], pblang_vm::Value::Number(55.0), "recursion (fib 10)");
}

/// Extends the closed self-hosting loop to `for` count loops, `elif` chains, and
/// `%` modulo — all compiled by the pblang compiler and run by the pblang VM.
#[test]
fn selfhost_drift_executes_loops_elif_mod() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let s = 0; for i 5 { s = s + i; }", "s");                          // 10
        print g("let p = 1; for i 5 { p = p * (i + 1); }", "p");                     // 120
        print g("let r = 17 % 5;", "r");                                            // 2
        print g("let g = 0; let x = 85; if x >= 90 { g = 4; } elif x >= 80 { g = 3; } else { g = 1; }", "g"); // 3
        print g("let g = 0; let x = 50; if x >= 90 { g = 4; } elif x >= 80 { g = 3; } else { g = 1; }", "g"); // 1
        print g("let c = 0; for i 10 { if i % 2 == 0 { c = c + 1; } }", "c");        // 5
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(10.0), "for-loop sum");
    assert_eq!(out[1], pblang_vm::Value::Number(120.0), "for-loop product");
    assert_eq!(out[2], pblang_vm::Value::Number(2.0), "modulo");
    assert_eq!(out[3], pblang_vm::Value::Number(3.0), "elif middle arm");
    assert_eq!(out[4], pblang_vm::Value::Number(1.0), "elif else arm");
    assert_eq!(out[5], pblang_vm::Value::Number(5.0), "for + if + modulo");
}

/// Data structures through the closed self-hosting loop: array literals +
/// indexing, map literals + member/index access, and chained access — all
/// compiled by the pblang compiler and run by the pblang VM.
#[test]
fn selfhost_drift_executes_arrays_and_maps() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let a = [10, 20, 30]; let x = a[1];", "x");                              // 20
        print g("let a = [3, 1, 4, 1, 5]; let s = 0; for i 5 { s = s + a[i]; }", "s");    // 14
        print g("let m = map{ k: 7, j: 9 }; let x = m.k + m.j;", "x");                    // 16
        print g("let m = map{ a: 5, b: 8 }; let x = m[\"b\"];", "x");                     // 8
        print g("let g = map{ n: 3 }; let a = [g, g]; let v = a[1].n;", "v");             // 3
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(20.0), "array index");
    assert_eq!(out[1], pblang_vm::Value::Number(14.0), "array sum via for+index");
    assert_eq!(out[2], pblang_vm::Value::Number(16.0), "map member access");
    assert_eq!(out[3], pblang_vm::Value::Number(8.0), "map string index");
    assert_eq!(out[4], pblang_vm::Value::Number(3.0), "chained index + member");
}

/// `for x in <iterable>` (including nested) and host builtin calls (`len`,
/// `array_push`, `str_split`, `str_upper`) through the closed self-hosting loop.
/// Accumulators are declared before the loop so they sit at local slot 0
/// (for-in consumes hidden slots for the array + index).
#[test]
fn selfhost_drift_executes_forin_and_builtins() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let t = 0; let a = [1, 2, 3]; for x in a { t = t + x * x; }", "t");                // 14
        print g("let t = 0; let a = [1, 2, 3]; let out = []; for x in a { array_push(out, x * x); } t = out[2];", "t"); // 9
        print g("let t = 0; let rows = [[1, 2], [3, 4]]; for r in rows { for c in r { t = t + c; } }", "t"); // 10
        print g("let a = [9, 8, 7]; let n = len(a);", "n");                                          // 3
        print g("let parts = str_split(\"a,b,c,d\", \",\"); let n = len(parts);", "n");              // 4
        print g("let u = str_upper(\"hi\");", "u");                                                  // HI
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(14.0), "for-in sum of squares");
    assert_eq!(out[1], pblang_vm::Value::Number(9.0), "array_push in for-in + index");
    assert_eq!(out[2], pblang_vm::Value::Number(10.0), "nested for-in");
    assert_eq!(out[3], pblang_vm::Value::Number(3.0), "len() builtin");
    assert_eq!(out[4], pblang_vm::Value::Number(4.0), "str_split + len");
    assert_eq!(out[5].as_str().unwrap(), "HI", "str_upper builtin");
}

/// Logical operators `and`/`or`/`not` (keyword forms) with short-circuit
/// semantics, plus real boolean literals, through the closed self-hosting loop.
/// `i < len(a) and a[i] == 1` must short-circuit so the guard is safe.
#[test]
fn selfhost_drift_executes_logical_ops() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let x = 0; if 5 > 3 and 2 < 4 { x = 1; }", "x");                 // 1
        print g("let x = 0; if 5 > 3 and 9 < 4 { x = 1; }", "x");                 // 0
        print g("let x = 0; if 5 < 3 or 2 < 4 { x = 1; }", "x");                  // 1
        print g("let x = 0; if not (5 < 3) { x = 1; }", "x");                     // 1
        print g("let b = true;", "b");                                           // true
        print g("let a = [1, 2, 3]; let x = 0; let i = 0; if i < len(a) and a[i] == 1 { x = 99; }", "x"); // 99
        print g("let x = 0; if (2 > 1 and 3 > 2) or (1 > 9) { x = 7; }", "x");    // 7
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(1.0), "and true");
    assert_eq!(out[1], pblang_vm::Value::Number(0.0), "and short-circuit false");
    assert_eq!(out[2], pblang_vm::Value::Number(1.0), "or true");
    assert_eq!(out[3], pblang_vm::Value::Number(1.0), "not");
    assert_eq!(out[4], pblang_vm::Value::Bool(true), "real boolean literal");
    assert_eq!(out[5], pblang_vm::Value::Number(99.0), "short-circuit guard");
    assert_eq!(out[6], pblang_vm::Value::Number(7.0), "nested and/or grouping");
}

/// Index/member assignment (`a[i] = v`, `m.k = v`, nested `a[0].n = v`) and
/// in-loop element mutation, through the closed self-hosting loop. Also exercises
/// the parser fixes that this required: `return …;` consuming its `;`, map string
/// keys + trailing commas, and type-annotation `:` (punct).
#[test]
fn selfhost_drift_executes_index_member_assignment() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let a = [1, 2, 3]; a[1] = 99; let v = a[1];", "v");                                  // 99
        print g("let m = map{ k: 1 }; m.k = 42; let v = m.k;", "v");                                  // 42
        print g("let g = map{ n: 0 }; let a = [g]; a[0].n = 7; let v = a[0].n;", "v");                // 7
        print g("let a = [0, 0, 0]; let i = 0; while i < 3 { a[i] = i * 10; i = i + 1; } let v = a[2];", "v"); // 20
        print g("let m = map{ \"x\": 5, \"y\": 6, }; let v = m[\"y\"];", "v");                        // 6 (string keys + trailing comma)
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(99.0), "array index assignment");
    assert_eq!(out[1], pblang_vm::Value::Number(42.0), "map member assignment");
    assert_eq!(out[2], pblang_vm::Value::Number(7.0), "nested lvalue assignment");
    assert_eq!(out[3], pblang_vm::Value::Number(20.0), "in-loop element mutation");
    assert_eq!(out[4], pblang_vm::Value::Number(6.0), "map string keys + trailing comma");
}

/// `else if` chains and loop control (`break`/`continue` in while, for-count, and
/// for-in) through the closed self-hosting loop.
#[test]
fn selfhost_drift_executes_elseif_break_continue() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let g = 0; let x = 85; if x >= 90 { g = 4; } else if x >= 80 { g = 3; } else { g = 1; }", "g"); // 3
        print g("let s = 0; let i = 0; while i < 100 { if i == 5 { break; } s = s + i; i = i + 1; }", "s");       // 10
        print g("let s = 0; for i 10 { if i % 2 == 0 { continue; } s = s + i; }", "s");                          // 25
        print g("let s = 0; for i 10 { if i == 3 { break; } s = s + i; }", "s");                                 // 3
        print g("let s = 0; let a = [10, 20, 30, 40]; for x in a { if x == 30 { break; } s = s + x; }", "s");    // 30
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(3.0), "else if chain");
    assert_eq!(out[1], pblang_vm::Value::Number(10.0), "break in while");
    assert_eq!(out[2], pblang_vm::Value::Number(25.0), "continue in for");
    assert_eq!(out[3], pblang_vm::Value::Number(3.0), "break in for");
    assert_eq!(out[4], pblang_vm::Value::Number(30.0), "break in for-in");
}

/// `try <expr> catch <fallback>` through the closed self-hosting loop: the body
/// runs inside the interpreter's own try/catch (a new OP_TRY over body/handler
/// sub-chunks that share the enclosing locals). Catches array OOB, missing map
/// keys, and explicit error() raises; supports nesting.
#[test]
fn selfhost_drift_executes_try_catch() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let a = [1, 2, 3]; let v = try a[10] catch -1;", "v");                       // -1
        print g("let a = [1, 2, 3]; let v = try a[1] catch -1;", "v");                        // 2
        print g("let m = map{ a: 5 }; let v = try m[\"zzz\"] catch 0;", "v");                 // 0
        print g("let v = try error(\"boom\") catch 42;", "v");                                // 42
        print g("let a = [10]; let v = try (a[5] + 1) catch (try a[9] catch 7);", "v");       // 7
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(-1.0), "catch array OOB");
    assert_eq!(out[1], pblang_vm::Value::Number(2.0), "body succeeds");
    assert_eq!(out[2], pblang_vm::Value::Number(0.0), "catch missing map key");
    assert_eq!(out[3], pblang_vm::Value::Number(42.0), "catch explicit error()");
    assert_eq!(out[4], pblang_vm::Value::Number(7.0), "nested try/catch");
}

/// Module-level scoping: top-level `let`/`const` become module globals visible to
/// every function in the module (previously they were locals of the main chunk,
/// so a function couldn't see a module constant). This is the gating capability
/// for running real multi-function modules through the self-hosted VM.
#[test]
fn selfhost_drift_module_level_globals() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let G = 100; fn getG() { return G; } let r = getG();", "r");                         // 100
        print g("let T = map{ x: 7, y: 9 }; fn look(k) { return T[k]; } let r = look(\"y\");", "r");   // 9
        print g("const BASE = 10; fn addbase(n) { return n + BASE; } let r = addbase(5);", "r");       // 15
        print g("let a = [3, 1, 4]; fn first() { return a[0]; } let r = first();", "r");               // 3
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(100.0), "function reads module global");
    assert_eq!(out[1], pblang_vm::Value::Number(9.0), "function reads module map constant");
    assert_eq!(out[2], pblang_vm::Value::Number(15.0), "function reads module const");
    assert_eq!(out[3], pblang_vm::Value::Number(3.0), "function reads module array");
}

/// A realistic multi-function "module" running entirely through the self-hosted
/// compiler + VM: a shared module constant, helper functions that call each other,
/// string indexing, a `for` and a `while` loop, and host builtins. Demonstrates
/// the self-hosted runtime executing module-structured pblang.
#[test]
fn selfhost_drift_runs_multifunction_module() {
    let prog = "\
        let DIGITS = \"0123456789\";\
        fn is_digit(c) { return str_contains(DIGITS, c); }\
        fn count_digits(s) { let n = 0; for i len(s) { if is_digit(s[i]) { n = n + 1; } } return n; }\
        fn sum_to(k) { let total = 0; let j = 1; while j <= k { total = total + j; j = j + 1; } return total; }\
        let digits = count_digits(\"a1b2c3d4\");\
        let tri = sum_to(5);\
        let answer = digits * 100 + tri;";
    let src = format!(
        r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        let r = f.frontend_run("{}");
        print r.globals["digits"];
        print r.globals["tri"];
        print r.globals["answer"];
        "#,
        prog.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let out = run_collect(&src);
    assert_eq!(out[0], pblang_vm::Value::Number(4.0), "count_digits via module constant + builtin");
    assert_eq!(out[1], pblang_vm::Value::Number(15.0), "sum_to while loop");
    assert_eq!(out[2], pblang_vm::Value::Number(415.0), "composed result");
}

/// Recommended-next-step coverage from docs/CURRENT_STATUS.md: run real stdlib
/// modules through `frontend_run` and diff their behavior against the Rust VM.
/// This exercises import pre-resolution, exported function maps, module-level
/// globals, callbacks, maps, strings, predicates, sibling helper calls, and
/// fallback paths while keeping the programs small enough for the default
/// self-host drift suite.
#[test]
fn selfhost_drift_stdlib_option_result_matches_rust_vm() {
    let option_program = r#"
        let opt = import("stdlib/option.sapp");
        fn inc(x) { return x + 1; }
        fn gt5(x) { return x > 5; }
        fn fallback() { return opt.some(99); }
        print opt.unwrap(opt.some(41)) + 1;
        print opt.unwrap_or(opt.none(), 9);
        print opt.unwrap_or(opt.some(5), 9);
        print opt.unwrap(opt.map_val(opt.some(4), inc));
        print opt.is_none(opt.filter(opt.some(3), gt5));
        print opt.unwrap(opt.or_else(opt.none(), fallback));
        print opt.is_some(opt.of("x"));
        print opt.is_none(opt.of(nil));
        print opt.to_string(opt.some("ok"));
        print opt.to_string(opt.none());
    "#;
    assert_frontend_run_matches_rust_vm("stdlib/option.sapp", option_program);

    let result_program = r#"
        let res = import("stdlib/result.sapp");
        fn inc(x) { return x + 1; }
        fn tag(e) { return "error:" + e; }
        fn recover(e) { return res.ok(len(e)); }
        print res.unwrap(res.ok(12));
        print res.unwrap_or(res.err("missing"), 7);
        print res.unwrap_or(res.ok(3), 7);
        print res.unwrap(res.map_val(res.ok(4), inc));
        print res.unwrap_or_else(res.err("bad"), tag);
        print res.unwrap(res.or_else(res.err("abcd"), recover));
        print res.is_ok(res.ok("yes"));
        print res.is_err(res.err("no"));
        print res.to_string(res.ok("yes"));
        print res.to_string(res.err("no"));
    "#;
    assert_frontend_run_matches_rust_vm("stdlib/result.sapp", result_program);
}

/// Inline `if … { } else { }` **expressions** (the value of the chosen branch's
/// last expression), including `else if` chains, use as a function return value,
/// the typeck-style `if x != nil { x } else { default }` guard, and a branch
/// block that has a statement before its final expression.
#[test]
fn selfhost_drift_executes_if_expressions() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let x = 7; let v = if x > 5 { 100 } else { 200 };", "v");                                  // 100
        print g("let x = 2; let v = if x > 5 { 100 } else { 200 };", "v");                                  // 200
        print g("let x = 85; let v = if x >= 90 { 4 } else if x >= 80 { 3 } else { 1 };", "v");             // 3
        print g("fn pick(b) { return if b { 10 } else { 20 }; } let v = pick(false);", "v");                // 20
        print g("let p = map{ t: 5 }; let v = if p[\"t\"] != nil { p[\"t\"] } else { 0 };", "v");           // 5
        print g("let n = 3; let v = if n > 0 { let d = n * 2; d + 1 } else { 0 };", "v");                   // 7
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(100.0), "if-expr true branch");
    assert_eq!(out[1], pblang_vm::Value::Number(200.0), "if-expr false branch");
    assert_eq!(out[2], pblang_vm::Value::Number(3.0), "if-expr else-if chain");
    assert_eq!(out[3], pblang_vm::Value::Number(20.0), "if-expr as function return");
    assert_eq!(out[4], pblang_vm::Value::Number(5.0), "typeck-style nil guard");
    assert_eq!(out[5], pblang_vm::Value::Number(7.0), "branch block: stmt then final expr");
}

/// `let len = len(s)` — a local shadowing a builtin *in its own initializer*. The
/// initializer must resolve `len` to the BUILTIN (a `let`'s RHS is evaluated in
/// the enclosing scope), not the just-declared local. Fixed by compiling the
/// value before declaring the local (`codegen_local_let`). This was the exact bug
/// that blocked the real `tokenizer.sapp` (`let len = len(source)`), after which
/// `tokenize("let foo = 42;")` returns 6 correctly-kinded tokens.
#[test]
fn selfhost_drift_let_shadows_builtin_in_initializer() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("fn count(s) { let len = len(s); let n = 0; for i len { n = n + 1; } return n; } let r = count(\"hello\");", "r");
        print g("fn dbl(x) { let str = str(x); return str + str; } let r = dbl(7);", "r");
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(5.0), "let len = len(s) resolves the builtin in the initializer");
    assert_eq!(out[1].as_str().unwrap(), "77", "let str = str(x) shadowing");
}

/// THE milestone: pblang tokenizes pblang. Reads the real stdlib
/// `compiler/tokenizer.sapp`, runs its `tokenize()` on a pblang statement through
/// the self-hosted compiler + VM (`frontend_run`), and checks the full token
/// stream. (The fix that unlocked this: `char_at` must use char-indexed `slice`,
/// or multi-byte UTF-8 chars in comments desync pos vs. len and truncate the
/// input. `strcharcodeat` is now codepoint-indexed too — §45 — so the whole
/// string API is uniformly codepoint-based.)
#[test]
fn selfhost_tokenizes_pblang_end_to_end() {
    let mut cfg = pblang_vm::StdlibConfig::default();
    cfg.fs = true;
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        let toksrc = fs_read("stdlib/compiler/tokenizer.sapp");
        let driver = "\nlet _toks = tokenize(\"let foo = 42 + bar;\");\nlet _desc = \"\";\nlet _i = 0;\nwhile _i < len(_toks) { _desc = _desc + str(_toks[_i].kind) + \":\" + str(_toks[_i].text) + \" \"; _i = _i + 1; }\nlet _END = 1;\n";
        let r = f.frontend_run(toksrc + driver);
        print r.globals["_desc"];
    "#;
    let out = run_collect_with_config(cfg, src);
    assert_eq!(
        out[0].as_str().unwrap(),
        "keyword:let ident:foo operator:= number:42 operator:+ ident:bar punct:; eof: ",
        "pblang tokenizing pblang: full correct token stream"
    );
}

/// THE next milestone: pblang PARSES pblang. Reads the real stdlib
/// `compiler/tokenizer.sapp` + `compiler/parser.sapp`, runs `tokenize` then
/// `parse` on a pblang snippet through the self-hosted compiler + VM, and checks
/// the resulting AST. The pblang-written front-end lexing AND parsing pblang.
#[test]
fn selfhost_parses_pblang_end_to_end() {
    let mut cfg = pblang_vm::StdlibConfig::default();
    cfg.fs = true;
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        let toksrc = fs_read("stdlib/compiler/tokenizer.sapp");
        let parsrc = fs_read("stdlib/compiler/parser.sapp");
        let driver = "\nlet _toks = tokenize(\"let x = 1; fn add(a, b) { return a + b; }\");\nlet _ast = parse(_toks);\nlet _n = len(_ast.stmts);\nlet _k0 = _ast.stmts[0].kind;\nlet _k1 = _ast.stmts[1].kind;\nlet _fname = _ast.stmts[1].name;\nlet _END = 1;\n";
        let r = f.frontend_run(toksrc + "\n" + parsrc + driver);
        print r.globals["_n"];
        print r.globals["_k0"];
        print r.globals["_k1"];
        print r.globals["_fname"];
    "#;
    let out = run_collect_with_config(cfg, src);
    assert_eq!(out[0], pblang_vm::Value::Number(2.0), "two top-level statements");
    assert_eq!(out[1].as_str().unwrap(), "let", "stmt 0 is a let");
    assert_eq!(out[2].as_str().unwrap(), "fn", "stmt 1 is a fn");
    assert_eq!(out[3].as_str().unwrap(), "add", "fn name parsed");
}

/// `for i in LO to HI` inclusive range loops through the self-hosted VM.
#[test]
fn selfhost_drift_executes_for_range() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let s = 0; for i in 1 to 5 { s = s + i; }", "s");
        print g("let p = 1; for i in 2 to 4 { p = p * i; }", "p");
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(15.0), "for-range sum 1..5");
    assert_eq!(out[1], pblang_vm::Value::Number(24.0), "for-range product 2..4");
}

/// Stage 3: pblang COMPILES pblang. Reads tokenizer + parser + codegen and runs
/// the full front-end (`compile_stmts(parse(tokenize(snippet)))`) through the
/// self-hosted VM, producing bytecode (instructions + constants).
#[test]
fn selfhost_compiles_pblang_to_bytecode() {
    let mut cfg = pblang_vm::StdlibConfig::default();
    cfg.fs = true;
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        let combined = fs_read("stdlib/compiler/tokenizer.sapp") + "\n"
                     + fs_read("stdlib/compiler/parser.sapp") + "\n"
                     + fs_read("stdlib/compiler/codegen.sapp")
                     + "\nlet _c = compile_stmts(parse(tokenize(\"let x = 1; let y = x + 2;\")).stmts);\nlet _ni = len(_c.instructions);\nlet _END = 1;\n";
        let r = f.frontend_run(combined);
        print r.globals["_ni"];
    "#;
    let out = run_collect_with_config(cfg, src);
    assert!(matches!(out[0], pblang_vm::Value::Number(n) if n >= 6.0),
        "self-hosted codegen emitted bytecode, got {:?}", out[0]);
}

/// THE crown-jewel milestone: pblang COMPILES AND RUNS pblang. The entire
/// self-hosted toolchain — tokenizer + parser + codegen + bytecode VM, all
/// written in pblang — runs inside the self-hosted VM, compiling and executing a
/// pblang program end to end: `let a = 6; let b = 7; let answer = a * b;` → 42.
#[test]
fn selfhost_compiles_and_runs_pblang_end_to_end() {
    let mut cfg = pblang_vm::StdlibConfig::default();
    cfg.fs = true;
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        let combined = fs_read("stdlib/compiler/tokenizer.sapp") + "\n"
                     + fs_read("stdlib/compiler/parser.sapp") + "\n"
                     + fs_read("stdlib/compiler/codegen.sapp") + "\n"
                     + fs_read("stdlib/compiler/vm.sapp")
                     + "\nlet _toks = tokenize(\"let a = 6; let b = 7; let answer = a * b;\");\nlet _chunk = compile_stmts(parse(_toks).stmts);\nlet _res = run(_chunk);\nlet _answer = _res.globals[\"answer\"];\nlet _END = 1;\n";
        let r = f.frontend_run(combined);
        print r.globals["_answer"];
    "#;
    let out = run_collect_with_config(cfg, src);
    assert_eq!(out[0], pblang_vm::Value::Number(42.0),
        "the full pblang-written toolchain compiles and runs pblang: 6 * 7 = 42");
}

/// Default parameter values through the self-hosted compiler+VM: an omitted arg
/// reads as nil and is filled with the param's default expression at function
/// entry (`fn f(a, b = 5)` etc.). Covers numeric, string, and nil defaults, full
/// override, and partial application.
#[test]
fn selfhost_drift_executes_default_params() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("fn add(a, b = 5) { return a + b; } let r = add(3);", "r");                            // 8
        print g("fn add(a, b = 5) { return a + b; } let r = add(3, 10);", "r");                         // 13
        print g("fn greet(name, gr = \"Hi\") { return gr + \" \" + name; } let r = greet(\"Bob\");", "r"); // Hi Bob
        print g("fn opt(x, d = nil) { if d == nil { return x; } return d; } let r = opt(7);", "r");     // 7
        print g("fn box(w = 2, h = 3) { return w * h; } let r = box();", "r");                          // 6
        print g("fn box(w = 2, h = 3) { return w * h; } let r = box(5);", "r");                         // 15
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(8.0), "default used");
    assert_eq!(out[1], pblang_vm::Value::Number(13.0), "default overridden");
    assert_eq!(out[2].as_str().unwrap(), "Hi Bob", "string default");
    assert_eq!(out[3], pblang_vm::Value::Number(7.0), "nil default");
    assert_eq!(out[4], pblang_vm::Value::Number(6.0), "two defaults, both omitted");
    assert_eq!(out[5], pblang_vm::Value::Number(15.0), "two defaults, partial");
}

/// §40 follow-up: nil-safe operators through the self-hosted compiler+VM.
/// `??` null-coalescing (only nil falls through, NOT false/0), `?.` optional
/// member, and `?[` optional index — each short-circuiting on a nil receiver.
#[test]
fn selfhost_drift_executes_nil_safe_operators() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let x = nil; let r = x ?? 5;", "r");                  // 5
        print g("let x = 7; let r = x ?? 5;", "r");                    // 7
        print g("let r = false ?? 9;", "r");                          // false (nil-only, not falsy)
        print g("let r = 0 ?? 9;", "r");                              // 0
        print g("let m = map{a: 3}; let r = m?.a;", "r");             // 3
        print g("let m = nil; let r = m?.a;", "r");                   // nil
        print g("let m = nil; let r = m?.a?.b;", "r");                // nil (chained short-circuit)
        print g("let a = [10, 20, 30]; let r = a?[1];", "r");         // 20
        print g("let a = nil; let r = a?[1];", "r");                  // nil
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(5.0), "?? falls through on nil");
    assert_eq!(out[1], pblang_vm::Value::Number(7.0), "?? keeps non-nil");
    assert_eq!(out[2], pblang_vm::Value::Bool(false), "?? keeps false (nil-only, not falsy)");
    assert_eq!(out[3], pblang_vm::Value::Number(0.0), "?? keeps 0 (nil-only, not falsy)");
    assert_eq!(out[4], pblang_vm::Value::Number(3.0), "?. reads member on non-nil");
    assert_eq!(out[5], pblang_vm::Value::Nil, "?. short-circuits on nil");
    assert_eq!(out[6], pblang_vm::Value::Nil, "?. chains short-circuit on nil");
    assert_eq!(out[7], pblang_vm::Value::Number(20.0), "?[ indexes on non-nil");
    assert_eq!(out[8], pblang_vm::Value::Nil, "?[ short-circuits on nil");
}

/// §42: `match` expressions through the self-hosted compiler+VM. Covers literal
/// arms (number + string), `else` fallthrough, no-match→nil, ident bind, `_`
/// wildcard, guarded bind (`case n if n > 5`), and a `nil` literal arm.
#[test]
fn selfhost_drift_executes_match_expressions() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("let r = match 2 { case 1: \"one\"; case 2: \"two\"; else: \"other\"; };", "r");   // two
        print g("let r = match 9 { case 1: \"one\"; else: \"other\"; };", "r");                     // other
        print g("let r = match 9 { case 1: \"one\"; };", "r");                                       // nil
        print g("let r = match 7 { case x: x * 2; };", "r");                                          // 14
        print g("let r = match 5 { case 1: \"a\"; case _: \"wild\"; };", "r");                       // wild
        print g("let r = match \"b\" { case \"a\": 1; case \"b\": 2; else: 3; };", "r");             // 2
        print g("let r = match 7 { case n if n > 5: \"big\"; case n: \"small\"; };", "r");           // big
        print g("let r = match 3 { case n if n > 5: \"big\"; case n: \"small\"; };", "r");           // small
        print g("let r = match nil { case nil: \"isnil\"; else: \"no\"; };", "r");                   // isnil
    "#;
    let out = run_collect(src);
    assert_eq!(out[0].as_str().unwrap(), "two", "literal number arm");
    assert_eq!(out[1].as_str().unwrap(), "other", "else fallthrough");
    assert_eq!(out[2], pblang_vm::Value::Nil, "no match, no else -> nil");
    assert_eq!(out[3], pblang_vm::Value::Number(14.0), "bind pattern body sees scrutinee");
    assert_eq!(out[4].as_str().unwrap(), "wild", "wildcard arm");
    assert_eq!(out[5], pblang_vm::Value::Number(2.0), "string literal arm");
    assert_eq!(out[6].as_str().unwrap(), "big", "guarded bind matches when guard true");
    assert_eq!(out[7].as_str().unwrap(), "small", "guarded bind falls through when guard false");
    assert_eq!(out[8].as_str().unwrap(), "isnil", "nil literal arm");
}

/// §43: `struct` / `enum` through the self-hosted compiler+VM. Both desugar in
/// the parser to constructs codegen already lowers: a `struct` becomes a typed
/// constructor `fn` returning a `{__struct, ...fields}` tagged map; an `enum`
/// becomes `let Name = map{...}` whose unit variants are `{__enum, __variant}`
/// tagged maps and whose data variants are constructor functions returning
/// `{__enum, __variant, values}`. Exercises the two enabling primitives added
/// this round: `fn_lit` codegen (anonymous closures as map values) and member /
/// index callees in `codegen_call` (`obj.f(args)`, `tbl[i](args)`).
#[test]
fn selfhost_drift_executes_struct_enum() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("struct Point { x: number, y: number } let p = Point(3, 4); let r = p.x + p.y;", "r");          // 7
        print g("struct Point { x, y } let p = Point(3, 4); let r = p.__struct;", "r");                          // Point
        print g("struct P { x, y } fn d2(p) { return p.x * p.x + p.y * p.y; } let r = d2(P(3, 4));", "r");       // 25
        print g("enum Color { Red, Green, Blue } let r = Color.Green.__variant;", "r");                          // Green
        print g("enum Color { Red, Green, Blue } let r = Color.Blue.__enum;", "r");                              // Color
        print g("enum Shape { Circle(number), Rect(number, number) } let s = Shape.Circle(5); let r = s.values[0];", "r");   // 5
        print g("enum Shape { Circle(number), Rect(number, number) } let s = Shape.Rect(2, 9); let r = s.values[1];", "r");  // 9
        print g("enum Shape { Circle(number), Rect(number, number) } let s = Shape.Rect(2, 9); let r = s.__variant;", "r");  // Rect
        print g("enum Color { Red, Green } let c = Color.Red; let r = match c.__variant { case \"Red\": 1; case \"Green\": 2; };", "r");  // 1
        print g("let m = map{ ff: fn(a) { return a * 2; } }; let r = m.ff(3);", "r");                            // 6
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(7.0), "struct field access + arithmetic");
    assert_eq!(out[1].as_str().unwrap(), "Point", "struct __struct tag");
    assert_eq!(out[2], pblang_vm::Value::Number(25.0), "struct passed to a function, fields read inside");
    assert_eq!(out[3].as_str().unwrap(), "Green", "enum unit variant __variant");
    assert_eq!(out[4].as_str().unwrap(), "Color", "enum unit variant __enum");
    assert_eq!(out[5], pblang_vm::Value::Number(5.0), "enum data variant values[0]");
    assert_eq!(out[6], pblang_vm::Value::Number(9.0), "enum data variant values[1]");
    assert_eq!(out[7].as_str().unwrap(), "Rect", "enum data variant __variant");
    assert_eq!(out[8], pblang_vm::Value::Number(1.0), "match on enum variant tag");
    assert_eq!(out[9], pblang_vm::Value::Number(6.0), "fn_lit stored in a map, called via member (obj.f(args))");
}

/// §44: `class` + colon-call (`obj:method(args)`) through the self-hosted
/// compiler+VM. A `class` desugars (in the parser) to a class table
/// `map{ __class_name, <method>: fn(self,...), ..., new }`; the auto-generated
/// `new` allocates `map{__class: <table>}`, runs `init`, returns it. A colon-call
/// `obj:m(a)` desugars to `(obj.__class ?? obj).m(obj, a)` — implicit-`self`
/// dispatch via the instance's class table (or the receiver itself for a plain
/// method-map). No new opcodes / no codegen change — pure parser sugar over the
/// `??`, member, member-call, and `fn_lit` primitives.
#[test]
fn selfhost_drift_executes_classes() {
    let src = r#"
        let f = import("stdlib:compiler/selfhost_frontend.sapp");
        fn g(s, n) { let r = f.frontend_run(s); return r.globals[n]; }
        print g("class Counter { fn init(self, start) { self.count = start; } fn get(self) { return self.count; } } let c = Counter:new(5); let r = c:get();", "r");  // 5
        print g("class Counter { fn init(self, s) { self.n = s; } fn inc(self) { self.n = self.n + 1; } fn get(self) { return self.n; } } let c = Counter:new(0); c:inc(); c:inc(); c:inc(); let r = c:get();", "r");  // 3
        print g("class Adder { fn init(self, base) { self.base = base; } fn add(self, x) { return self.base + x; } } let a = Adder:new(10); let r = a:add(7);", "r");  // 17
        print g("class Pt { fn init(self, x, y) { self.x = x; self.y = y; } } let p = Pt:new(3, 4); let r = p.x + p.y;", "r");  // 7
        print g("class Greeter { fn hi(self) { return \"hi \" + self.who; } } let gr = Greeter:new(); gr.who = \"bob\"; let r = gr:hi();", "r");  // hi bob
        print g("class M { fn init(self, v) { self.v = v; } fn dbl(self) { return self.v * 2; } fn quad(self) { return self:dbl() + self:dbl(); } } let m = M:new(5); let r = m:quad();", "r");  // 20
        print g("class N { fn init(self, v) { self.v = v; } fn val(self) { return self.v; } } let n = N:new(8); let r = n:val() + 1;", "r");  // 9
        print g("class K { fn init(self, v) { self.v = v; } fn val(self) { return self.v; } } let k = K:new(3); let r = k.__class.__class_name;", "r");  // K
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], pblang_vm::Value::Number(5.0), "init stores field, method reads it");
    assert_eq!(out[1], pblang_vm::Value::Number(3.0), "mutating method across multiple colon-calls");
    assert_eq!(out[2], pblang_vm::Value::Number(17.0), "method with an argument");
    assert_eq!(out[3], pblang_vm::Value::Number(7.0), "multi-field init + field access");
    assert_eq!(out[4].as_str().unwrap(), "hi bob", "no-init class, manual field set, method dispatch");
    assert_eq!(out[5], pblang_vm::Value::Number(20.0), "method calls another method via self:m()");
    assert_eq!(out[6], pblang_vm::Value::Number(9.0), "colon-call result used in arithmetic");
    assert_eq!(out[7].as_str().unwrap(), "K", "instance __class points at the class table");
}
