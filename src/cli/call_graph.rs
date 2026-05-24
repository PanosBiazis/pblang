//! Call-graph analysis helpers extracted from `main.rs` (R2-alpha).
//! Pure AST walkers that build a caller → callees map and pretty-print it.

use pblang_core::{Expr, ExprKind, Stmt, StmtKind};

/// Caller → set of callees (function names called inside that function).
pub(crate) type CallGraph = std::collections::BTreeMap<String, std::collections::BTreeSet<String>>;

pub(crate) fn build_call_graph(ast: &[Stmt], _file: &str) -> CallGraph {
    let mut graph: CallGraph = std::collections::BTreeMap::new();
    let mut current_fn = "<main>".to_string();
    graph.entry(current_fn.clone()).or_default();
    walk_stmts_for_cg(ast, &mut graph, &mut current_fn);
    graph
}

fn walk_stmts_for_cg(stmts: &[Stmt], graph: &mut CallGraph, current_fn: &mut String) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Fn(name, _params, _ret, body) => {
                let prev = current_fn.clone();
                *current_fn = name.clone();
                graph.entry(name.clone()).or_default();
                walk_stmts_for_cg(body, graph, current_fn);
                *current_fn = prev;
            }
            StmtKind::Expr(e) | StmtKind::Return(Some(e)) => {
                collect_calls_from_expr(e, graph, current_fn);
            }
            StmtKind::Let(_, _, e) => {
                collect_calls_from_expr(e, graph, current_fn);
            }
            StmtKind::If(cond, then_branch, else_branch) => {
                collect_calls_from_expr(cond, graph, current_fn);
                walk_stmts_for_cg(then_branch, graph, current_fn);
                if let Some(eb) = else_branch {
                    walk_stmts_for_cg(eb, graph, current_fn);
                }
            }
            StmtKind::While(cond, body) => {
                collect_calls_from_expr(cond, graph, current_fn);
                walk_stmts_for_cg(body, graph, current_fn);
            }
            StmtKind::For(_, count, body) => {
                collect_calls_from_expr(count, graph, current_fn);
                walk_stmts_for_cg(body, graph, current_fn);
            }
            StmtKind::ForIn(_, col, body) => {
                collect_calls_from_expr(col, graph, current_fn);
                walk_stmts_for_cg(body, graph, current_fn);
            }
            StmtKind::ForInKV(_, _, col, body) => {
                collect_calls_from_expr(col, graph, current_fn);
                walk_stmts_for_cg(body, graph, current_fn);
            }
            StmtKind::Print(e) | StmtKind::PrintF(_, e) => {
                graph
                    .entry(current_fn.clone())
                    .or_default()
                    .insert("print".to_string());
                collect_calls_from_expr(e, graph, current_fn);
            }
            StmtKind::Defer(e) => {
                collect_calls_from_expr(e, graph, current_fn);
            }
            _ => {}
        }
    }
}

fn collect_calls_from_expr(expr: &Expr, graph: &mut CallGraph, current_fn: &str) {
    match &expr.kind {
        ExprKind::Call(callee, args) => {
            if let ExprKind::Var(name) = &callee.kind {
                graph
                    .entry(current_fn.to_string())
                    .or_default()
                    .insert(name.clone());
            }
            collect_calls_from_expr(callee, graph, current_fn);
            for a in args {
                collect_calls_from_expr(a, graph, current_fn);
            }
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
            collect_calls_from_expr(l, graph, current_fn);
            collect_calls_from_expr(r, graph, current_fn);
        }
        ExprKind::Logical { left, right, .. } => {
            collect_calls_from_expr(left, graph, current_fn);
            collect_calls_from_expr(right, graph, current_fn);
        }
        ExprKind::Index(obj, ix) => {
            collect_calls_from_expr(obj, graph, current_fn);
            collect_calls_from_expr(ix, graph, current_fn);
        }
        ExprKind::GetAttr(obj, _) => {
            collect_calls_from_expr(obj, graph, current_fn);
        }
        ExprKind::Try(try_e, catch_e) => {
            collect_calls_from_expr(try_e, graph, current_fn);
            collect_calls_from_expr(catch_e, graph, current_fn);
        }
        ExprKind::FnLit(opt_name, _, _, body) => {
            let mut inner = opt_name
                .clone()
                .unwrap_or_else(|| "<anonymous>".to_string());
            graph.entry(inner.clone()).or_default();
            walk_stmts_for_cg(body, graph, &mut inner);
        }
        ExprKind::BlockExpr(stmts, opt_e) => {
            let mut block_ctx = current_fn.to_string();
            walk_stmts_for_cg(stmts, graph, &mut block_ctx);
            if let Some(e) = opt_e {
                collect_calls_from_expr(e, graph, current_fn);
            }
        }
        ExprKind::Match(scrut, arms, else_e) => {
            collect_calls_from_expr(scrut, graph, current_fn);
            for (_arm, e) in arms {
                collect_calls_from_expr(e, graph, current_fn);
            }
            if let Some(e) = else_e {
                collect_calls_from_expr(e, graph, current_fn);
            }
        }
        ExprKind::Array(exprs) => {
            for e in exprs {
                collect_calls_from_expr(e, graph, current_fn);
            }
        }
        ExprKind::Map(pairs) => {
            for (_, v) in pairs {
                collect_calls_from_expr(v, graph, current_fn);
            }
        }
        _ => {}
    }
}

pub(crate) fn print_call_graph(graph: &CallGraph) {
    for (caller, callees) in graph {
        if callees.is_empty() {
            println!("{caller} -> (no calls)");
        } else {
            for callee in callees {
                println!("{caller} -> {callee}");
            }
        }
    }
}
