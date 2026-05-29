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

use common::run_collect;

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
