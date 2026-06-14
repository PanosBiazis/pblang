//! `pblang lint` command handler.
//!
//! This keeps the legacy lint behavior intact while moving the command-specific
//! AST walk and file loop out of `src/main.rs`.

use pblang_core::{parse_strict, Expr, ExprKind, Span, Stmt, StmtKind};
use pblang_typeck::check as type_check;

use super::diag::print_parse_errors;

fn lint_unused_warnings(stmts: &[Stmt]) -> Vec<(Span, String)> {
    let mut warnings = vec![];
    // (span, is_import, read)
    let mut scope: Vec<std::collections::HashMap<String, (Span, bool, bool)>> = vec![];
    scope.push(std::collections::HashMap::new());

    fn mark_read(name: &str, scope: &mut [std::collections::HashMap<String, (Span, bool, bool)>]) {
        for frame in scope.iter_mut().rev() {
            if let Some((_, _, read)) = frame.get_mut(name) {
                *read = true;
                return;
            }
        }
    }

    fn is_import_call(e: &Expr) -> bool {
        if let ExprKind::Call(callee, _) = &e.kind {
            if let ExprKind::Var(s) = &callee.kind {
                return s == "import";
            }
        }
        false
    }

    fn pop_scope_and_report(
        scope: &mut Vec<std::collections::HashMap<String, (Span, bool, bool)>>,
        warnings: &mut Vec<(Span, String)>,
    ) {
        if let Some(top) = scope.pop() {
            for (name, (sp, is_import, read)) in top {
                if !read {
                    let msg = if is_import {
                        format!("unused import: `{}`", name)
                    } else {
                        format!("unused variable: `{}`", name)
                    };
                    warnings.push((sp, msg));
                }
            }
        }
    }

    fn walk_expr(
        e: &Expr,
        scope: &mut Vec<std::collections::HashMap<String, (Span, bool, bool)>>,
        warnings: &mut Vec<(Span, String)>,
    ) {
        match &e.kind {
            ExprKind::Var(name) => mark_read(name, scope),
            ExprKind::Call(callee, args) => {
                walk_expr(callee, scope, warnings);
                for a in args {
                    walk_expr(a, scope, warnings);
                }
            }
            ExprKind::GetAttr(obj, _) => walk_expr(obj, scope, warnings),
            ExprKind::Index(obj, ix) => {
                walk_expr(obj, scope, warnings);
                walk_expr(ix, scope, warnings);
            }
            ExprKind::Add(a, b)
            | ExprKind::Sub(a, b)
            | ExprKind::Mul(a, b)
            | ExprKind::Div(a, b) => {
                walk_expr(a, scope, warnings);
                walk_expr(b, scope, warnings);
            }
            ExprKind::Eq(a, b)
            | ExprKind::NotEq(a, b)
            | ExprKind::Gt(a, b)
            | ExprKind::Ge(a, b)
            | ExprKind::Lt(a, b)
            | ExprKind::Le(a, b) => {
                walk_expr(a, scope, warnings);
                walk_expr(b, scope, warnings);
            }
            ExprKind::Logical { left, right, .. } => {
                walk_expr(left, scope, warnings);
                walk_expr(right, scope, warnings);
            }
            ExprKind::Try(try_e, catch_e) => {
                walk_expr(try_e, scope, warnings);
                walk_expr(catch_e, scope, warnings);
            }
            ExprKind::Match(scrut, arms, else_e) => {
                walk_expr(scrut, scope, warnings);
                for (_arm, e) in arms {
                    walk_expr(e, scope, warnings);
                }
                if let Some(ee) = else_e {
                    walk_expr(ee, scope, warnings);
                }
            }
            ExprKind::Array(arr) => {
                for a in arr {
                    walk_expr(a, scope, warnings);
                }
            }
            ExprKind::Map(entries) => {
                for (_, v) in entries {
                    walk_expr(v, scope, warnings);
                }
            }
            ExprKind::FnLit(_, _, _, body) => {
                scope.push(std::collections::HashMap::new());
                for s in body {
                    walk_stmt(s, scope, warnings);
                }
                pop_scope_and_report(scope, warnings);
            }
            ExprKind::BlockExpr(stmts_in, opt_final) => {
                scope.push(std::collections::HashMap::new());
                for s in stmts_in {
                    walk_stmt(s, scope, warnings);
                }
                if let Some(e) = opt_final {
                    walk_expr(e, scope, warnings);
                }
                pop_scope_and_report(scope, warnings);
            }
            _ => {}
        }
    }

    fn walk_stmt(
        s: &Stmt,
        scope: &mut Vec<std::collections::HashMap<String, (Span, bool, bool)>>,
        warnings: &mut Vec<(Span, String)>,
    ) {
        match &s.kind {
            StmtKind::Let(name, _, init) => {
                if scope.len() > 1 {
                    for frame in scope[..scope.len() - 1].iter() {
                        if frame.contains_key(name) {
                            warnings.push((
                                s.span,
                                format!("variable `{}` shadows outer binding", name),
                            ));
                            break;
                        }
                    }
                }
                let is_import = is_import_call(init);
                if let Some(top) = scope.last_mut() {
                    top.insert(name.clone(), (s.span, is_import, false));
                }
                walk_expr(init, scope, warnings);
            }
            StmtKind::Assign(name, rhs) => {
                mark_read(name, scope);
                walk_expr(rhs, scope, warnings);
            }
            StmtKind::SetIndex(t, ix, val) => {
                walk_expr(t, scope, warnings);
                walk_expr(ix, scope, warnings);
                walk_expr(val, scope, warnings);
            }
            StmtKind::SetAttr(obj, _, val) => {
                walk_expr(obj, scope, warnings);
                walk_expr(val, scope, warnings);
            }
            StmtKind::Expr(e) => walk_expr(e, scope, warnings),
            StmtKind::Print(e)
            | StmtKind::PrintF(_, e)
            | StmtKind::Return(Some(e))
            | StmtKind::Defer(e) => {
                walk_expr(e, scope, warnings);
            }
            StmtKind::If(cond, then_b, else_b) => {
                walk_expr(cond, scope, warnings);
                scope.push(std::collections::HashMap::new());
                for s in then_b {
                    walk_stmt(s, scope, warnings);
                }
                pop_scope_and_report(scope, warnings);
                if let Some(else_stmts) = else_b {
                    scope.push(std::collections::HashMap::new());
                    for s in else_stmts {
                        walk_stmt(s, scope, warnings);
                    }
                    pop_scope_and_report(scope, warnings);
                }
            }
            StmtKind::While(cond, body) => {
                walk_expr(cond, scope, warnings);
                scope.push(std::collections::HashMap::new());
                for s in body {
                    walk_stmt(s, scope, warnings);
                }
                pop_scope_and_report(scope, warnings);
            }
            StmtKind::For(name, count, body) => {
                walk_expr(count, scope, warnings);
                if scope.len() > 1 {
                    for frame in scope[..scope.len() - 1].iter() {
                        if frame.contains_key(name) {
                            warnings.push((
                                s.span,
                                format!("variable `{}` shadows outer binding", name),
                            ));
                            break;
                        }
                    }
                }
                if let Some(top) = scope.last_mut() {
                    top.insert(name.clone(), (s.span, false, false));
                }
                scope.push(std::collections::HashMap::new());
                for s in body {
                    walk_stmt(s, scope, warnings);
                }
                pop_scope_and_report(scope, warnings);
            }
            StmtKind::ForIn(name, col, body) => {
                walk_expr(col, scope, warnings);
                scope.push(std::collections::HashMap::new());
                if let Some(top) = scope.last_mut() {
                    top.insert(name.clone(), (s.span, false, false));
                }
                for st in body {
                    walk_stmt(st, scope, warnings);
                }
                pop_scope_and_report(scope, warnings);
            }
            StmtKind::ForInKV(k, v, col, body) => {
                walk_expr(col, scope, warnings);
                scope.push(std::collections::HashMap::new());
                if let Some(top) = scope.last_mut() {
                    top.insert(k.clone(), (s.span, false, false));
                    top.insert(v.clone(), (s.span, false, false));
                }
                for st in body {
                    walk_stmt(st, scope, warnings);
                }
                pop_scope_and_report(scope, warnings);
            }
            StmtKind::Fn(_name, _params, _ret, body) => {
                scope.push(std::collections::HashMap::new());
                for s in body {
                    walk_stmt(s, scope, warnings);
                }
                pop_scope_and_report(scope, warnings);
            }
            StmtKind::Block(body) => {
                scope.push(std::collections::HashMap::new());
                for s in body {
                    walk_stmt(s, scope, warnings);
                }
                pop_scope_and_report(scope, warnings);
            }
            _ => {}
        }
    }

    for s in stmts {
        walk_stmt(s, &mut scope, &mut warnings);
    }
    pop_scope_and_report(&mut scope, &mut warnings);
    warnings
}

pub(crate) fn handle_lint(args: Vec<String>) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let files: Vec<_> = args.iter().skip(1).map(|s| cwd.join(s)).collect();
    if files.is_empty() {
        eprintln!("pblang lint: pass one or more .sap, .sapp, or .bpx files");
        std::process::exit(1);
    }
    let mut has_error = false;
    for path in &files {
        let path = path.canonicalize().unwrap_or_else(|_| path.clone());
        let path_str = path.to_string_lossy();
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pblang lint: {}: {}", path_str, e);
                has_error = true;
                continue;
            }
        };
        match parse_strict(&src) {
            Ok(stmts) => {
                let diags = type_check(&stmts);
                for d in &diags {
                    has_error = true;
                    if let Some(sp) = d.span {
                        eprintln!("{}:{}..{}: {}", path_str, sp.start, sp.end, d.message);
                    } else {
                        eprintln!("{}: {}", path_str, d.message);
                    }
                }
                if diags.is_empty() {
                    for (sp, msg) in lint_unused_warnings(&stmts) {
                        eprintln!("{}:{}..{}: warning: {}", path_str, sp.start, sp.end, msg);
                    }
                    for d in pblang_lint::lint(&stmts) {
                        eprintln!(
                            "{}:{}..{}: warning: {} [{}: {}]",
                            path_str,
                            d.span.start,
                            d.span.end,
                            d.message,
                            d.category.as_str(),
                            d.name
                        );
                        if let Some(help) = &d.help {
                            eprintln!("    help: {}", help);
                        }
                    }
                }
            }
            Err(errs) => {
                has_error = true;
                print_parse_errors(&src, errs, Some(&path_str));
            }
        }
    }
    if has_error {
        std::process::exit(1);
    }
}
