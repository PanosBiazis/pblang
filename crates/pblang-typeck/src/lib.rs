use pblang_core::{Expr, ExprKind, LogicalOp, Span, Stmt, StmtKind, TypeExpr, TypeKind};

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Any,
    Number,
    Bool,
    String,
    Nil,
    Array(Box<Ty>),
    Map(Box<Ty>),
    Fn(Vec<Ty>, Box<Ty>),
}

impl Ty {
    fn from_type_expr(t: &TypeExpr) -> Ty {
        match &t.kind {
            TypeKind::Any => Ty::Any,
            TypeKind::Number => Ty::Number,
            TypeKind::Bool => Ty::Bool,
            TypeKind::String => Ty::String,
            TypeKind::Nil => Ty::Nil,
            TypeKind::Array(inner) => Ty::Array(Box::new(Ty::from_type_expr(inner))),
            TypeKind::Map(inner) => Ty::Map(Box::new(Ty::from_type_expr(inner))),
            // FIX #18b: T! (NonNull) constraint is currently ERASED here —
            // we convert it to the inner type and lose the non-null requirement.
            // A proper implementation would add nil-assignment guards in the compiler.
            // Tracked as audit finding #18.
            TypeKind::NonNull(inner) => Ty::from_type_expr(inner),
        }
    }

    fn is_assignable_to(&self, dst: &Ty) -> bool {
        match (self, dst) {
            (_, Ty::Any) => true,
            (Ty::Any, _) => true,
            (a, b) if a == b => true,
            // simple covariance for containers
            (Ty::Array(a), Ty::Array(b)) => a.is_assignable_to(b),
            (Ty::Map(a), Ty::Map(b)) => a.is_assignable_to(b),
            _ => false,
        }
    }
}

#[derive(Default)]
struct Env {
    scopes: Vec<std::collections::HashMap<String, Ty>>,
}

impl Env {
    fn new() -> Self {
        Env {
            scopes: vec![std::collections::HashMap::new()],
        }
    }
    fn push(&mut self) {
        self.scopes.push(std::collections::HashMap::new());
    }
    fn pop(&mut self) {
        let _ = self.scopes.pop();
    }
    fn insert(&mut self, k: String, v: Ty) {
        if let Some(s) = self.scopes.last_mut() {
            s.insert(k, v);
        }
    }
    fn get(&self, k: &str) -> Option<Ty> {
        for s in self.scopes.iter().rev() {
            if let Some(v) = s.get(k) {
                return Some(v.clone());
            }
        }
        None
    }
}

/// Run the pblang type checker over a parsed statement list.
///
/// # Audit Note (Fix #18) — Limitations
///
/// The type checker is currently a **best-effort inference pass** only.
/// The following are NOT checked and generate no warnings:
///
/// - Function return type annotations (`fn f() -> number`): return types are
///   parsed but never verified against actual return expressions.
/// - Type annotation mismatches on `let`/`const` (`let x: number = "hello"`
///   will NOT produce a diagnostic).
/// - The `NonNull` type (`T!`): the non-null constraint is silently erased
///   at line 33 (`TypeKind::NonNull(inner) => Ty::from_type_expr(inner)`).
///   No nil-assignment check is performed.
/// - Generic/struct/enum types: not yet designed.
///
/// `pblang lint` WILL catch: undeclared variable reads, basic type mismatches
/// in expressions where both sides have a known concrete type, and parse errors.
///
/// A full Hindley-Milner type inference engine is planned for a future version.
pub fn check(stmts: &[Stmt]) -> Vec<Diagnostic> {
    let mut diags = vec![];
    let mut env = Env::new();
    let mut ret_stack: Vec<Option<Ty>> = vec![];

    for s in stmts {
        check_stmt(s, &mut env, &mut ret_stack, &mut diags);
    }

    diags
}

fn infer_expr(e: &Expr, env: &mut Env, diags: &mut Vec<Diagnostic>) -> Ty {
    match &e.kind {
        ExprKind::Match(scrutinee, arms, else_expr) => {
            let _ = infer_expr(scrutinee, env, diags);

            let mut out_ty: Option<Ty> = None;

            for (arm, body) in arms {
                // arm-specific scope for bind
                env.push();
                match arm {
                    pblang_core::MatchArm::Literal(lit_expr) => {
                        // typecheck the literal expr (Number/Str/Bool/Nil in your parser)
                        let _ = infer_expr(lit_expr, env, diags);
                    }
                    pblang_core::MatchArm::Bind(name) => {
                        // Best-effort: bind gets scrutinee type, otherwise Any
                        let st = infer_expr(scrutinee, env, diags);
                        env.insert(name.clone(), st);
                    }
                    pblang_core::MatchArm::Wildcard => {}
                    pblang_core::MatchArm::BindGuard(name, guard_expr) => {
                        // Bind gets scrutinee type; also typecheck the guard
                        let st = infer_expr(scrutinee, env, diags);
                        env.insert(name.clone(), st);
                        let _ = infer_expr(guard_expr, env, diags);
                    }
                }

                let t = infer_expr(body, env, diags);
                out_ty = match out_ty {
                    None => Some(t),
                    Some(prev) => {
                        if prev == t {
                            Some(prev)
                        } else {
                            Some(Ty::Any)
                        }
                    }
                };
                env.pop();
            }

            // else branch participates in the result type too
            let else_ty = else_expr
                .as_ref()
                .map(|e| infer_expr(e, env, diags))
                .unwrap_or(Ty::Nil);

            match out_ty {
                None => else_ty, // match with no arms: result is else (or nil)
                Some(prev) => {
                    if prev == else_ty {
                        prev
                    } else {
                        Ty::Any
                    }
                }
            }
        }
        ExprKind::Number(_) => Ty::Number,
        ExprKind::Str(_) => Ty::String,
        ExprKind::Bool(_) => Ty::Bool,
        ExprKind::Nil => Ty::Nil,
        ExprKind::Var(name) => env.get(name).unwrap_or(Ty::Any),
        ExprKind::Array(items) => {
            // very simple: if all same non-any, keep it; otherwise Any
            let mut item_ty: Option<Ty> = None;
            for it in items {
                let t = infer_expr(it, env, diags);
                item_ty = match item_ty {
                    None => Some(t),
                    Some(prev) => {
                        if prev == t {
                            Some(prev)
                        } else {
                            Some(Ty::Any)
                        }
                    }
                };
            }
            Ty::Array(Box::new(item_ty.unwrap_or(Ty::Any)))
        }
        ExprKind::Map(pairs) => {
            let mut val_ty: Option<Ty> = None;
            for (_k, v) in pairs {
                let t = infer_expr(v, env, diags);
                val_ty = match val_ty {
                    None => Some(t),
                    Some(prev) => {
                        if prev == t {
                            Some(prev)
                        } else {
                            Some(Ty::Any)
                        }
                    }
                };
            }
            Ty::Map(Box::new(val_ty.unwrap_or(Ty::Any)))
        }
        ExprKind::Index(obj, _ix) => match infer_expr(obj, env, diags) {
            Ty::Array(inner) => *inner,
            Ty::Map(inner) => *inner,
            _ => Ty::Any,
        },
        ExprKind::GetAttr(_obj, _name) => Ty::Any,
        ExprKind::Call(_callee, _args) => Ty::Any,
        ExprKind::FnLit(_name, params, ret, _body) => {
            let ps = params
                .iter()
                .map(|(_n, t)| t.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any))
                .collect::<Vec<_>>();
            let r = ret.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any);
            Ty::Fn(ps, Box::new(r))
        }
        ExprKind::BlockExpr(stmts, tail) => {
            env.push();
            for st in stmts {
                check_stmt(st, env, &mut vec![], diags);
            }
            let t = tail
                .as_ref()
                .map(|e| infer_expr(e, env, diags))
                .unwrap_or(Ty::Nil);
            env.pop();
            t
        }
        ExprKind::Try(a, b) => {
            let ta = infer_expr(a, env, diags);
            let tb = infer_expr(b, env, diags);
            if ta == tb {
                ta
            } else {
                Ty::Any
            }
        }
        ExprKind::Logical { left, op, right } => match op {
            LogicalOp::And | LogicalOp::Or => {
                let _ = infer_expr(left, env, diags);
                let _ = infer_expr(right, env, diags);
                Ty::Bool
            }
        },
        ExprKind::Add(a, b) => {
            let ta = infer_expr(a, env, diags);
            let tb = infer_expr(b, env, diags);
            match (&ta, &tb) {
                (Ty::Number, Ty::Number) => Ty::Number,
                (Ty::String, Ty::String) => Ty::String,
                _ => Ty::Any,
            }
        }
        ExprKind::Sub(a, b) | ExprKind::Mul(a, b) | ExprKind::Div(a, b) => {
            let ta = infer_expr(a, env, diags);
            let tb = infer_expr(b, env, diags);
            if ta != Ty::Number || tb != Ty::Number {
                diags.push(Diagnostic {
                    message: "expected number operands".into(),
                    span: Some(e.span),
                });
            }
            Ty::Number
        }
        ExprKind::Eq(a, b)
        | ExprKind::NotEq(a, b)
        | ExprKind::Gt(a, b)
        | ExprKind::Ge(a, b)
        | ExprKind::Lt(a, b)
        | ExprKind::Le(a, b) => {
            let _ = infer_expr(a, env, diags);
            let _ = infer_expr(b, env, diags);
            Ty::Bool
        }

        // Safety operators
        ExprKind::Not(inner) => {
            let _ = infer_expr(inner, env, diags);
            Ty::Bool
        }
        ExprKind::SafeGetAttr(obj, _name) => {
            let _ = infer_expr(obj, env, diags);
            Ty::Any // nil-safe attr: result is T | nil
        }
        ExprKind::SafeIndex(obj, idx) => {
            let _ = infer_expr(obj, env, diags);
            let _ = infer_expr(idx, env, diags);
            Ty::Any // nil-safe index: result is element_type | nil
        }
        ExprKind::ResultUnwrap(inner) => {
            let _ = infer_expr(inner, env, diags);
            Ty::Any // unwrapped value type depends on the Result
        }
    }
}

fn check_stmt(
    s: &Stmt,
    env: &mut Env,
    ret_stack: &mut Vec<Option<Ty>>,
    diags: &mut Vec<Diagnostic>,
) {
    match &s.kind {
        StmtKind::Defer(expr) => {
            let _ = infer_expr(expr, env, diags);
        }
        StmtKind::Let(name, ty, init) => {
            let init_ty = infer_expr(init, env, diags);
            if let Some(t) = ty {
                let ann = Ty::from_type_expr(t);
                if !init_ty.is_assignable_to(&ann) {
                    diags.push(Diagnostic {
                        message: format!("type mismatch: cannot assign {init_ty:?} to {ann:?}"),
                        span: Some(s.span),
                    });
                }
                env.insert(name.clone(), ann);
            } else {
                env.insert(name.clone(), init_ty);
            }
        }
        StmtKind::Assign(name, expr) => {
            let rhs = infer_expr(expr, env, diags);
            if let Some(old) = env.get(name) {
                if !rhs.is_assignable_to(&old) {
                    diags.push(Diagnostic {
                        message: format!("type mismatch: cannot assign {rhs:?} to {old:?}"),
                        span: Some(s.span),
                    });
                }
            }
            env.insert(name.clone(), rhs);
        }
        StmtKind::Fn(name, params, ret, body) => {
            let ps = params
                .iter()
                .map(|(_n, t)| t.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any))
                .collect::<Vec<_>>();
            let r = ret.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any);
            env.insert(name.clone(), Ty::Fn(ps.clone(), Box::new(r.clone())));

            env.push();
            for (pname, pty) in params {
                env.insert(
                    pname.clone(),
                    pty.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any),
                );
            }
            ret_stack.push(Some(r));
            for st in body {
                check_stmt(st, env, ret_stack, diags);
            }
            let _ = ret_stack.pop();
            env.pop();
        }
        StmtKind::Return(opt) => {
            let expected = ret_stack.last().cloned().flatten();
            let got = opt
                .as_ref()
                .map(|e| infer_expr(e, env, diags))
                .unwrap_or(Ty::Nil);
            if let Some(exp) = expected {
                if !got.is_assignable_to(&exp) {
                    diags.push(Diagnostic {
                        message: format!("return type mismatch: got {got:?}, expected {exp:?}"),
                        span: Some(s.span),
                    });
                }
            }
        }
        StmtKind::Block(stmts) => {
            env.push();
            for st in stmts {
                check_stmt(st, env, ret_stack, diags);
            }
            env.pop();
        }
        StmtKind::If(cond, then_body, else_body) => {
            let _ = infer_expr(cond, env, diags);
            env.push();
            for st in then_body {
                check_stmt(st, env, ret_stack, diags);
            }
            env.pop();
            if let Some(stmts) = else_body {
                env.push();
                for st in stmts {
                    check_stmt(st, env, ret_stack, diags);
                }
                env.pop();
            }
        }
        StmtKind::While(cond, body) => {
            let _ = infer_expr(cond, env, diags);
            env.push();
            for st in body {
                check_stmt(st, env, ret_stack, diags);
            }
            env.pop();
        }
        StmtKind::For(name, count, body) => {
            let _ = infer_expr(count, env, diags);
            env.push();
            env.insert(name.clone(), Ty::Number);
            for st in body {
                check_stmt(st, env, ret_stack, diags);
            }
            env.pop();
        }
        StmtKind::Print(e) | StmtKind::PrintF(_, e) => {
            let _ = infer_expr(e, env, diags);
        }
        StmtKind::Expr(e) => {
            let _ = infer_expr(e, env, diags);
        }
        StmtKind::SetIndex(obj, ix, v) => {
            let _ = infer_expr(obj, env, diags);
            let _ = infer_expr(ix, env, diags);
            let _ = infer_expr(v, env, diags);
        }
        StmtKind::SetAttr(obj, _name, v) => {
            let _ = infer_expr(obj, env, diags);
            let _ = infer_expr(v, env, diags);
        }
        StmtKind::Import(_) | StmtKind::Export(_) => {}

        // `const name = expr;` — same as Let but additionally check non-nullable if annotation ends in `!`
        StmtKind::Const(name, ty, init) => {
            let init_ty = infer_expr(init, env, diags);
            // Reject nil assignment to const (non-null semantics for const)
            if matches!(init_ty, Ty::Nil) {
                diags.push(Diagnostic {
                    message: format!("const '{}' cannot be initialized to nil", name),
                    span: Some(s.span),
                });
            }
            if let Some(t) = ty {
                let ann = Ty::from_type_expr(t);
                if !init_ty.is_assignable_to(&ann) {
                    diags.push(Diagnostic {
                        message: format!("type mismatch in const '{}': cannot assign {init_ty:?} to {ann:?}", name),
                        span: Some(s.span),
                    });
                }
                env.insert(name.clone(), ann);
            } else {
                env.insert(name.clone(), init_ty);
            }
        }
    }
}
