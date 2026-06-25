//! pblang-lint — a clippy-style linter for pblang.
//!
//! Where `pblang-typeck` answers "is this program type-correct?", this crate
//! answers "is this program *idiomatic*?" — the same role `cargo clippy` plays
//! for Rust. It walks the parsed AST and emits style / correctness / complexity
//! lints, each with a short name, a category, and an actionable `help:`
//! suggestion.
//!
//! Lints are intentionally **conservative**: a check only fires when the pattern
//! is unambiguous, so `pblang lint` stays free of false positives. Structural
//! comparisons are span-blind and restricted to side-effect-free expressions.
//!
//! # Implemented lints
//!
//! | name                 | category    | flags                                            |
//! |----------------------|-------------|--------------------------------------------------|
//! | `bool_comparison`    | style       | `x == true`, `x != false`, …                     |
//! | `eq_op`              | correctness | identical operands: `a == a`, `a < a`, `a - a`, `a / a` |
//! | `double_not`         | complexity  | `not (not x)`                                     |
//! | `self_assignment`    | correctness | `x = x`                                           |
//! | `constant_condition` | correctness | `if true`/`if false`/`while false`               |
//! | `while_true`         | style       | `while true { … }` → suggest `loop`              |
//! | `collapsible_if`     | complexity  | `if a { if b { … } }` → `if a and b`             |

use pblang_core::{Expr, ExprKind, FStringPart, MatchArm, Span, Stmt, StmtKind};

/// Lint severity grouping, mirroring clippy's lint groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintCategory {
    /// Almost certainly a bug (always-true comparisons, dead branches, no-ops).
    Correctness,
    /// Non-idiomatic but correct code.
    Style,
    /// Code that can be simplified.
    Complexity,
    /// Avoidable runtime cost.
    Perf,
}

impl LintCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            LintCategory::Correctness => "correctness",
            LintCategory::Style => "style",
            LintCategory::Complexity => "complexity",
            LintCategory::Perf => "perf",
        }
    }
}

/// A single lint finding.
#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    /// Stable lint name (e.g. `"bool_comparison"`), shown like clippy's `note:`.
    pub name: &'static str,
    pub category: LintCategory,
    pub message: String,
    /// Actionable suggestion, rendered as a `help:` line.
    pub help: Option<String>,
    pub span: Span,
}

/// Run every lint over a parsed program and return the findings in source order.
pub fn lint(stmts: &[Stmt]) -> Vec<LintDiagnostic> {
    let mut out = Vec::new();
    for s in stmts {
        walk_stmt(s, &mut out);
    }
    out
}

/// Span-blind equality for **side-effect-free** expressions. Used by `eq_op`:
/// only pure expressions (vars, literals, and field/index chains over them) can
/// be compared, so `rand() == rand()` is never flagged.
fn pure_expr_eq(a: &Expr, b: &Expr) -> bool {
    match (&a.kind, &b.kind) {
        (ExprKind::Var(x), ExprKind::Var(y)) => x == y,
        (ExprKind::Number(x), ExprKind::Number(y)) => x == y,
        (ExprKind::Str(x), ExprKind::Str(y)) => x == y,
        (ExprKind::Bool(x), ExprKind::Bool(y)) => x == y,
        (ExprKind::Nil, ExprKind::Nil) => true,
        (ExprKind::GetAttr(oa, na), ExprKind::GetAttr(ob, nb)) => na == nb && pure_expr_eq(oa, ob),
        (ExprKind::Index(oa, ia), ExprKind::Index(ob, ib)) => {
            pure_expr_eq(oa, ob) && pure_expr_eq(ia, ib)
        }
        _ => false,
    }
}

/// Expr-level lints applied to a single node (recursion happens in `walk_expr`).
fn check_expr_lints(e: &Expr, out: &mut Vec<LintDiagnostic>) {
    match &e.kind {
        ExprKind::Eq(a, b) | ExprKind::NotEq(a, b) => {
            let is_eq = matches!(e.kind, ExprKind::Eq(..));
            // bool_comparison: comparing against a boolean literal.
            let mut bool_flagged = false;
            for lit_side in [a, b] {
                if let ExprKind::Bool(bv) = &lit_side.kind {
                    let simpler = match (is_eq, *bv) {
                        (true, true) | (false, false) => "the expression itself",
                        (true, false) | (false, true) => "`not <expr>`",
                    };
                    out.push(LintDiagnostic {
                        name: "bool_comparison",
                        category: LintCategory::Style,
                        message: format!("comparison with boolean literal `{}`", bv),
                        help: Some(format!("use {} instead of comparing to `{}`", simpler, bv)),
                        span: e.span,
                    });
                    bool_flagged = true;
                    break;
                }
            }
            // eq_op: identical operands → constant result, usually a typo.
            if !bool_flagged && pure_expr_eq(a, b) {
                out.push(LintDiagnostic {
                    name: "eq_op",
                    category: LintCategory::Correctness,
                    message: "both operands of this comparison are identical".into(),
                    help: Some(
                        if is_eq {
                            "this comparison is always true"
                        } else {
                            "this comparison is always false"
                        }
                        .into(),
                    ),
                    span: e.span,
                });
            }
        }
        // Only the STRICT relational ops are flagged: `a < a` / `a > a` are
        // always false, even under IEEE-754 (NaN compares false to everything).
        // `a <= a` / `a >= a` are deliberately NOT flagged — like `a == a` /
        // `a != a`, they are the idiomatic NaN test (`NaN <= NaN` is false), so
        // flagging them would be a false positive.
        ExprKind::Lt(a, b) | ExprKind::Gt(a, b) => {
            if pure_expr_eq(a, b) {
                out.push(LintDiagnostic {
                    name: "eq_op",
                    category: LintCategory::Correctness,
                    message: "both operands of this comparison are identical".into(),
                    help: Some("this comparison is always false".into()),
                    span: e.span,
                });
            }
        }
        ExprKind::Sub(a, b) | ExprKind::Div(a, b) => {
            if pure_expr_eq(a, b) {
                let is_div = matches!(e.kind, ExprKind::Div(..));
                out.push(LintDiagnostic {
                    name: "eq_op",
                    category: LintCategory::Correctness,
                    message: "both operands of this arithmetic operation are identical".into(),
                    help: Some(
                        if is_div {
                            "`x / x` is always 1 (or a divide-by-zero); did you mean different operands?"
                        } else {
                            "`x - x` is always 0; did you mean different operands?"
                        }
                        .into(),
                    ),
                    span: e.span,
                });
            }
        }
        ExprKind::Not(inner) => {
            if matches!(inner.kind, ExprKind::Not(_)) {
                out.push(LintDiagnostic {
                    name: "double_not",
                    category: LintCategory::Complexity,
                    message: "double negation".into(),
                    help: Some("`not (not x)` is equivalent to `x`".into()),
                    span: e.span,
                });
            }
        }
        _ => {}
    }
}

/// Stmt-level lints applied to a single node (recursion happens in `walk_stmt`).
fn check_stmt_lints(s: &Stmt, out: &mut Vec<LintDiagnostic>) {
    match &s.kind {
        StmtKind::Assign(name, expr) => {
            if let ExprKind::Var(v) = &expr.kind {
                if v == name {
                    out.push(LintDiagnostic {
                        name: "self_assignment",
                        category: LintCategory::Correctness,
                        message: format!("`{}` is assigned to itself", name),
                        help: Some("this assignment has no effect; remove it".into()),
                        span: s.span,
                    });
                }
            }
        }
        StmtKind::If(cond, then_body, None) => {
            if let ExprKind::Bool(b) = &cond.kind {
                out.push(LintDiagnostic {
                    name: "constant_condition",
                    category: LintCategory::Correctness,
                    message: format!("`if` condition is always `{}`", b),
                    help: Some(
                        if *b {
                            "the branch always runs; remove the `if`"
                        } else {
                            "the branch is dead code; remove it"
                        }
                        .into(),
                    ),
                    span: cond.span,
                });
            }
            // collapsible_if: `if a { if b { … } }` with no else on either.
            if then_body.len() == 1 {
                if let StmtKind::If(_, _, None) = &then_body[0].kind {
                    out.push(LintDiagnostic {
                        name: "collapsible_if",
                        category: LintCategory::Complexity,
                        message: "this `if` can be collapsed into the outer `if`".into(),
                        help: Some("combine the conditions: `if outer and inner { … }`".into()),
                        span: s.span,
                    });
                }
            }
        }
        StmtKind::If(cond, _, Some(_)) => {
            if let ExprKind::Bool(b) = &cond.kind {
                out.push(LintDiagnostic {
                    name: "constant_condition",
                    category: LintCategory::Correctness,
                    message: format!("`if` condition is always `{}`", b),
                    help: Some("one branch is always dead code".into()),
                    span: cond.span,
                });
            }
        }
        StmtKind::While(cond, _) => match &cond.kind {
            ExprKind::Bool(true) => out.push(LintDiagnostic {
                name: "while_true",
                category: LintCategory::Style,
                message: "`while true` loop".into(),
                help: Some("use `loop { … }` for an unconditional loop".into()),
                span: cond.span,
            }),
            ExprKind::Bool(false) => out.push(LintDiagnostic {
                name: "constant_condition",
                category: LintCategory::Correctness,
                message: "`while false` never executes".into(),
                help: Some("this loop body is dead code; remove it".into()),
                span: cond.span,
            }),
            _ => {}
        },
        _ => {}
    }
}

/// Recurse through every expression node, applying expr-level lints.
fn walk_expr(e: &Expr, out: &mut Vec<LintDiagnostic>) {
    check_expr_lints(e, out);
    match &e.kind {
        ExprKind::Call(callee, args) => {
            walk_expr(callee, out);
            for a in args {
                walk_expr(a, out);
            }
        }
        ExprKind::GetAttr(o, _)
        | ExprKind::SafeGetAttr(o, _)
        | ExprKind::ResultUnwrap(o)
        | ExprKind::Not(o) => walk_expr(o, out),
        ExprKind::Index(o, i) | ExprKind::SafeIndex(o, i) => {
            walk_expr(o, out);
            walk_expr(i, out);
        }
        ExprKind::Array(items) => {
            for it in items {
                walk_expr(it, out);
            }
        }
        ExprKind::Map(entries) => {
            for (_, v) in entries {
                walk_expr(v, out);
            }
        }
        ExprKind::Try(a, b)
        | ExprKind::NullCoalesce(a, b)
        | ExprKind::Add(a, b)
        | ExprKind::Sub(a, b)
        | ExprKind::Mul(a, b)
        | ExprKind::Div(a, b)
        | ExprKind::Eq(a, b)
        | ExprKind::NotEq(a, b)
        | ExprKind::Gt(a, b)
        | ExprKind::Ge(a, b)
        | ExprKind::Lt(a, b)
        | ExprKind::Le(a, b)
        | ExprKind::Pow(a, b)
        | ExprKind::Mod(a, b) => {
            walk_expr(a, out);
            walk_expr(b, out);
        }
        ExprKind::Logical { left, right, .. } => {
            walk_expr(left, out);
            walk_expr(right, out);
        }
        ExprKind::Match(scrut, arms, else_e) => {
            walk_expr(scrut, out);
            for (arm, body) in arms {
                match arm {
                    MatchArm::Literal(le) => walk_expr(le, out),
                    MatchArm::BindGuard(_, ge) => walk_expr(ge, out),
                    _ => {}
                }
                walk_expr(body, out);
            }
            if let Some(ee) = else_e {
                walk_expr(ee, out);
            }
        }
        ExprKind::FnLit(_, _, _, body) => {
            for s in body {
                walk_stmt(s, out);
            }
        }
        ExprKind::BlockExpr(stmts, tail) => {
            for s in stmts {
                walk_stmt(s, out);
            }
            if let Some(t) = tail {
                walk_expr(t, out);
            }
        }
        ExprKind::IfExpr(c, t, f) => {
            walk_expr(c, out);
            walk_expr(t, out);
            walk_expr(f, out);
        }
        ExprKind::FString(parts) => {
            for p in parts {
                if let FStringPart::Interp(ie) = p {
                    walk_expr(ie, out);
                }
            }
        }
        ExprKind::Number(_)
        | ExprKind::Str(_)
        | ExprKind::Bool(_)
        | ExprKind::Nil
        | ExprKind::Var(_) => {}
    }
}

/// Recurse through every statement node, applying stmt-level lints.
fn walk_stmt(s: &Stmt, out: &mut Vec<LintDiagnostic>) {
    check_stmt_lints(s, out);
    match &s.kind {
        StmtKind::Print(e)
        | StmtKind::PrintF(_, e)
        | StmtKind::Throw(e)
        | StmtKind::Defer(e)
        | StmtKind::Expr(e)
        | StmtKind::Let(_, _, e)
        | StmtKind::Const(_, _, e)
        | StmtKind::Assign(_, e) => walk_expr(e, out),
        StmtKind::SetIndex(t, i, v) => {
            walk_expr(t, out);
            walk_expr(i, out);
            walk_expr(v, out);
        }
        StmtKind::SetAttr(t, _, v) => {
            walk_expr(t, out);
            walk_expr(v, out);
        }
        StmtKind::If(c, then_b, else_b) => {
            walk_expr(c, out);
            for st in then_b {
                walk_stmt(st, out);
            }
            if let Some(eb) = else_b {
                for st in eb {
                    walk_stmt(st, out);
                }
            }
        }
        StmtKind::While(c, body) | StmtKind::For(_, c, body) => {
            walk_expr(c, out);
            for st in body {
                walk_stmt(st, out);
            }
        }
        StmtKind::ForIn(_, e, body) => {
            walk_expr(e, out);
            for st in body {
                walk_stmt(st, out);
            }
        }
        StmtKind::ForInKV(_, _, e, body) => {
            walk_expr(e, out);
            for st in body {
                walk_stmt(st, out);
            }
        }
        StmtKind::Loop(body)
        | StmtKind::Block(body)
        | StmtKind::Seq(body)
        | StmtKind::Fn(_, _, _, body) => {
            for st in body {
                walk_stmt(st, out);
            }
        }
        StmtKind::Return(opt) => {
            if let Some(e) = opt {
                walk_expr(e, out);
            }
        }
        StmtKind::Import(_) | StmtKind::Export(_) | StmtKind::Break | StmtKind::Continue => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lints_of(src: &str) -> Vec<LintDiagnostic> {
        let stmts = pblang_core::parse(src).expect("parse failed");
        lint(&stmts)
    }

    fn has_lint(src: &str, name: &str) -> bool {
        lints_of(src).iter().any(|d| d.name == name)
    }

    #[test]
    fn bool_comparison_eq_true() {
        assert!(has_lint("let r = x == true", "bool_comparison"));
        assert!(has_lint("let r = x != false", "bool_comparison"));
        assert!(has_lint("let r = false == y", "bool_comparison"));
    }

    #[test]
    fn bool_comparison_not_fired_normally() {
        assert!(!has_lint("let r = x == y", "bool_comparison"));
        assert!(!has_lint("let r = a == 3", "bool_comparison"));
    }

    #[test]
    fn eq_op_identical_operands() {
        assert!(has_lint("let r = a == a", "eq_op"));
        assert!(has_lint("let r = a - a", "eq_op"));
        assert!(has_lint("let r = a / a", "eq_op"));
        assert!(has_lint("let r = a.b.c == a.b.c", "eq_op"));
    }

    #[test]
    fn eq_op_strict_relational_identical_operands() {
        // `a < a` / `a > a` are always false (NaN included) → flag as a likely typo.
        assert!(has_lint("let r = a < a", "eq_op"));
        assert!(has_lint("let r = a > a", "eq_op"));
        assert!(has_lint("let r = a.b < a.b", "eq_op"));
        // Different or impure operands never fire.
        assert!(!has_lint("let r = a < b", "eq_op"));
        assert!(!has_lint("let r = f() < f()", "eq_op"));
    }

    #[test]
    fn eq_op_does_not_flag_nan_idiom_relationals() {
        // `a <= a` / `a >= a` are the idiomatic NaN test (NaN <= NaN is false),
        // not constant — flagging them would be a false positive.
        assert!(!has_lint("let r = a <= a", "eq_op"));
        assert!(!has_lint("let r = a >= a", "eq_op"));
    }

    #[test]
    fn eq_op_not_fired_on_different_or_impure() {
        assert!(!has_lint("let r = a == b", "eq_op"));
        // Impure operands (calls) are never compared.
        assert!(!has_lint("let r = f() == f()", "eq_op"));
    }

    #[test]
    fn double_not() {
        assert!(has_lint("let r = not (not x)", "double_not"));
        assert!(!has_lint("let r = not x", "double_not"));
    }

    #[test]
    fn self_assignment() {
        assert!(has_lint("fn f() { x = x; }", "self_assignment"));
        assert!(!has_lint("fn f() { x = y; }", "self_assignment"));
    }

    #[test]
    fn constant_condition_if() {
        assert!(has_lint(
            "fn f() { if true { print 1; } }",
            "constant_condition"
        ));
        assert!(has_lint(
            "fn f() { if false { print 1; } }",
            "constant_condition"
        ));
        assert!(!has_lint(
            "fn f() { if x { print 1; } }",
            "constant_condition"
        ));
    }

    #[test]
    fn constant_condition_while_false() {
        assert!(has_lint(
            "fn f() { while false { print 1; } }",
            "constant_condition"
        ));
    }

    #[test]
    fn while_true_suggests_loop() {
        assert!(has_lint("fn f() { while true { print 1; } }", "while_true"));
        assert!(!has_lint("fn f() { while x { print 1; } }", "while_true"));
    }

    #[test]
    fn collapsible_if() {
        assert!(has_lint(
            "fn f() { if a { if b { print 1; } } }",
            "collapsible_if"
        ));
        // Not collapsible: outer has an else.
        assert!(!has_lint(
            "fn f() { if a { if b { print 1; } } else { print 2; } }",
            "collapsible_if"
        ));
        // Not collapsible: inner has an else.
        assert!(!has_lint(
            "fn f() { if a { if b { print 1; } else { print 2; } } }",
            "collapsible_if"
        ));
        // Not collapsible: outer block has more than the inner `if`.
        assert!(!has_lint(
            "fn f() { if a { if b { print 1; } print 3; } }",
            "collapsible_if"
        ));
    }

    #[test]
    fn lints_recurse_into_nested_scopes() {
        // A lint inside a function body inside a loop is still found.
        assert!(has_lint(
            "fn outer() { for i in items { let r = a == a; } }",
            "eq_op"
        ));
    }

    #[test]
    fn clean_code_has_no_lints() {
        let src = "fn add(a, b) { return a + b; }\nlet x = add(1, 2)\nif x > 0 { print x; }";
        assert!(
            lints_of(src).is_empty(),
            "unexpected lints: {:?}",
            lints_of(src)
        );
    }
}
