//! Data-flow / use-def analysis helpers extracted from `main.rs` (R3-alpha).
//! Walks the AST collecting variable definitions and uses, prints the report
//! and warns about defs that are never read.

use pblang_core::{Expr, ExprKind, MatchArm, Stmt, StmtKind};

use super::diag::line_col;

#[derive(Default)]
pub(crate) struct VarDefInfo {
    pub line: usize,
    pub used: bool,
}

pub(crate) fn print_dataflow_report(ast: &[Stmt], src: &str) {
    use std::collections::BTreeMap;

    let mut defs: BTreeMap<String, VarDefInfo> = BTreeMap::new();
    let mut uses: Vec<(String, usize)> = Vec::new();

    walk_stmts_for_dataflow(ast, src, &mut defs, &mut uses);

    // Mark used flags.
    for (name, use_line) in &uses {
        if let Some(info) = defs.get_mut(name) {
            info.used = true;
        }
        println!("use {} line:{}", name, use_line);
    }

    for (name, info) in &defs {
        println!("def {} line:{}", name, info.line);
    }

    for (name, info) in &defs {
        if !info.used {
            println!("WARN {} assigned but never read", name);
        }
    }
}

fn walk_stmts_for_dataflow(
    stmts: &[Stmt],
    src: &str,
    defs: &mut std::collections::BTreeMap<String, VarDefInfo>,
    uses: &mut Vec<(String, usize)>,
) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Let(name, _ty, expr) => {
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(name.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(expr, src, defs, uses);
            }
            StmtKind::Assign(name, expr) => {
                // Treat assignment as a definition site as well.
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(name.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(expr, src, defs, uses);
            }
            StmtKind::For(var, count_expr, body) => {
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(var.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(count_expr, src, defs, uses);
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::ForIn(var, col_expr, body) => {
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(var.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(col_expr, src, defs, uses);
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::ForInKV(k, v, col_expr, body) => {
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(k.clone())
                    .or_insert(VarDefInfo { line, used: false });
                defs.entry(v.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(col_expr, src, defs, uses);
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::Expr(e)
            | StmtKind::Print(e)
            | StmtKind::PrintF(_, e)
            | StmtKind::Return(Some(e))
            | StmtKind::Defer(e) => {
                walk_expr_for_dataflow(e, src, defs, uses);
            }
            StmtKind::If(cond, then_branch, else_branch) => {
                walk_expr_for_dataflow(cond, src, defs, uses);
                walk_stmts_for_dataflow(then_branch, src, defs, uses);
                if let Some(eb) = else_branch {
                    walk_stmts_for_dataflow(eb, src, defs, uses);
                }
            }
            StmtKind::While(cond, body) => {
                walk_expr_for_dataflow(cond, src, defs, uses);
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::Fn(_, _params, _ret, body) => {
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::Block(body) | StmtKind::Seq(body) => {
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::SetIndex(target, index, value) => {
                walk_expr_for_dataflow(target, src, defs, uses);
                walk_expr_for_dataflow(index, src, defs, uses);
                walk_expr_for_dataflow(value, src, defs, uses);
            }
            StmtKind::SetAttr(target, _name, value) => {
                walk_expr_for_dataflow(target, src, defs, uses);
                walk_expr_for_dataflow(value, src, defs, uses);
            }
            StmtKind::Import(_) | StmtKind::Export(_) | StmtKind::Return(None) => {}
            // const behaves like let for dataflow analysis
            StmtKind::Const(name, _ty, expr) => {
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(name.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(expr, src, defs, uses);
            }
            // New control flow
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Throw(e) => {
                walk_expr_for_dataflow(e, src, defs, uses);
            }
            StmtKind::Loop(body) => {
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
        }
    }
}

fn walk_expr_for_dataflow(
    expr: &Expr,
    src: &str,
    defs: &mut std::collections::BTreeMap<String, VarDefInfo>,
    uses: &mut Vec<(String, usize)>,
) {
    match &expr.kind {
        ExprKind::Var(name) => {
            let (line, _) = line_col(src, expr.span.start);
            uses.push((name.clone(), line));
        }
        ExprKind::Call(callee, args) => {
            walk_expr_for_dataflow(callee, src, defs, uses);
            for a in args {
                walk_expr_for_dataflow(a, src, defs, uses);
            }
        }
        ExprKind::GetAttr(target, _) => {
            walk_expr_for_dataflow(target, src, defs, uses);
        }
        ExprKind::Index(target, index) => {
            walk_expr_for_dataflow(target, src, defs, uses);
            walk_expr_for_dataflow(index, src, defs, uses);
        }
        ExprKind::Array(items) => {
            for e in items {
                walk_expr_for_dataflow(e, src, defs, uses);
            }
        }
        ExprKind::Map(entries) => {
            for (_k, v) in entries {
                walk_expr_for_dataflow(v, src, defs, uses);
            }
        }
        ExprKind::Try(body, handler) => {
            walk_expr_for_dataflow(body, src, defs, uses);
            walk_expr_for_dataflow(handler, src, defs, uses);
        }
        ExprKind::FnLit(_name, _params, _ret, body) => {
            walk_stmts_for_dataflow(body, src, defs, uses);
        }
        ExprKind::BlockExpr(body, tail) => {
            walk_stmts_for_dataflow(body, src, defs, uses);
            if let Some(t) = tail {
                walk_expr_for_dataflow(t, src, defs, uses);
            }
        }
        ExprKind::IfExpr(cond, then_br, else_br) => {
            walk_expr_for_dataflow(cond, src, defs, uses);
            walk_expr_for_dataflow(then_br, src, defs, uses);
            walk_expr_for_dataflow(else_br, src, defs, uses);
        }
        ExprKind::Add(l, r)
        | ExprKind::Sub(l, r)
        | ExprKind::Mul(l, r)
        | ExprKind::Div(l, r)
        | ExprKind::Eq(l, r)
        | ExprKind::NotEq(l, r)
        | ExprKind::Gt(l, r)
        | ExprKind::Ge(l, r)
        | ExprKind::Lt(l, r)
        | ExprKind::Le(l, r) => {
            walk_expr_for_dataflow(l, src, defs, uses);
            walk_expr_for_dataflow(r, src, defs, uses);
        }
        ExprKind::Logical { left, right, .. } => {
            walk_expr_for_dataflow(left, src, defs, uses);
            walk_expr_for_dataflow(right, src, defs, uses);
        }
        ExprKind::Match(scrutinee, arms, else_expr) => {
            walk_expr_for_dataflow(scrutinee, src, defs, uses);
            for (pat, body) in arms {
                match pat {
                    MatchArm::Literal(e) => walk_expr_for_dataflow(e, src, defs, uses),
                    MatchArm::Bind(name) => {
                        let (line, _) = line_col(src, expr.span.start);
                        defs.entry(name.clone())
                            .or_insert(VarDefInfo { line, used: false });
                    }
                    MatchArm::Wildcard => {}
                    MatchArm::BindGuard(name, guard) => {
                        let (line, _) = line_col(src, expr.span.start);
                        defs.entry(name.clone())
                            .or_insert(VarDefInfo { line, used: false });
                        walk_expr_for_dataflow(guard, src, defs, uses);
                    }
                }
                walk_expr_for_dataflow(body, src, defs, uses);
            }
            if let Some(e) = else_expr {
                walk_expr_for_dataflow(e, src, defs, uses);
            }
        }
        ExprKind::Number(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::Nil => {}
        // Safety operators — walk their inner expressions
        ExprKind::Not(inner) => walk_expr_for_dataflow(inner, src, defs, uses),
        ExprKind::SafeGetAttr(obj, _) => walk_expr_for_dataflow(obj, src, defs, uses),
        ExprKind::SafeIndex(obj, idx) => {
            walk_expr_for_dataflow(obj, src, defs, uses);
            walk_expr_for_dataflow(idx, src, defs, uses);
        }
        ExprKind::ResultUnwrap(inner) => walk_expr_for_dataflow(inner, src, defs, uses),
        // New expression kinds
        ExprKind::NullCoalesce(a, b) | ExprKind::Pow(a, b) | ExprKind::Mod(a, b) => {
            walk_expr_for_dataflow(a, src, defs, uses);
            walk_expr_for_dataflow(b, src, defs, uses);
        }
        ExprKind::FString(parts) => {
            for part in parts {
                if let pblang_core::FStringPart::Interp(e) = part {
                    walk_expr_for_dataflow(e, src, defs, uses);
                }
            }
        }
    }
}
