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
    /// Map(key_type, value_type)
    Map(Box<Ty>, Box<Ty>),
    Fn(Vec<Ty>, Box<Ty>),
    /// Option(inner) — T? / T | nil: optional / nullable value.
    Option(Box<Ty>),
    /// Result(ok, err) — result<T, E>: success (ok) or error (err).
    Result(Box<Ty>, Box<Ty>),
    /// Generic type parameter name (e.g. T, K, V).
    Generic(String),
    /// NonNull(inner) — T! type: the inner type but nil assignment is a compile error.
    NonNull(Box<Ty>),
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Any => write!(f, "any"),
            Ty::Number => write!(f, "number"),
            Ty::Bool => write!(f, "bool"),
            Ty::String => write!(f, "string"),
            Ty::Nil => write!(f, "nil"),
            Ty::Array(inner) => write!(f, "array<{}>", inner),
            Ty::Map(k, v) => write!(f, "map<{}, {}>", k, v),
            Ty::Fn(params, ret) => {
                let ps: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                write!(f, "fn({}) -> {}", ps.join(", "), ret)
            }
            Ty::Option(inner) => write!(f, "{}?", inner),
            Ty::Result(ok, err) => write!(f, "result<{}, {}>", ok, err),
            Ty::Generic(name) => write!(f, "{}", name),
            Ty::NonNull(inner) => write!(f, "{}!", inner),
        }
    }
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
            TypeKind::Map(k, v) => Ty::Map(
                Box::new(Ty::from_type_expr(k)),
                Box::new(Ty::from_type_expr(v)),
            ),
            TypeKind::Option(inner) => Ty::Option(Box::new(Ty::from_type_expr(inner))),
            TypeKind::Result(ok, err) => Ty::Result(
                Box::new(Ty::from_type_expr(ok)),
                Box::new(Ty::from_type_expr(err)),
            ),
            TypeKind::Generic(name) => Ty::Generic(name.clone()),
            TypeKind::NonNull(inner) => Ty::NonNull(Box::new(Ty::from_type_expr(inner))),
        }
    }

    /// Find the most specific common type that encompasses both `a` and `b`.
    ///
    /// Widening rules:
    ///   - T and T → T
    ///   - T and U (incompatible) → any
    ///   - T and nil → T?
    ///   - array<T> and array<U> → array<widen(T,U)>
    ///   - map<K1,V1> and map<K2,V2> → map<widen(K1,K2), widen(V1,V2)>
    ///   - option<T> and option<U> → option<widen(T,U)>
    ///   - option<T> and U → option<widen(T,U)>
    ///   - result<T1,E1> and result<T2,E2> → result<widen(T1,T2), widen(E1,E2)>
    fn widen(a: &Ty, b: &Ty) -> Ty {
        match (a, b) {
            // Identical types → that type
            (a, b) if a == b => a.clone(),
            // Either is Any → Any
            (Ty::Any, _) | (_, Ty::Any) => Ty::Any,
            // One is Nil → Option of the other
            (Ty::Nil, other) | (other, Ty::Nil) => Ty::Option(Box::new(other.clone())),
            // Both NonNull → widen inner, preserve NonNull if result isn't Any
            (Ty::NonNull(inner_a), Ty::NonNull(inner_b)) => {
                let w = Ty::widen(inner_a, inner_b);
                if w == Ty::Any {
                    Ty::Any
                } else {
                    Ty::NonNull(Box::new(w))
                }
            }
            // One NonNull, one not → strip NonNull, widen (result may be nil)
            (Ty::NonNull(inner), other) | (other, Ty::NonNull(inner)) => Ty::widen(inner, other),
            // Option types
            (Ty::Option(inner_a), Ty::Option(inner_b)) => {
                let w = Ty::widen(inner_a, inner_b);
                if w == Ty::Any {
                    Ty::Any
                } else {
                    Ty::Option(Box::new(w))
                }
            }
            // Option<T> and concrete U → Option<widen(T,U)>
            (Ty::Option(inner), other) | (other, Ty::Option(inner)) => {
                let w = Ty::widen(inner, other);
                if w == Ty::Any {
                    Ty::Any
                } else {
                    Ty::Option(Box::new(w))
                }
            }
            // Array covariance
            (Ty::Array(a_inner), Ty::Array(b_inner)) => {
                Ty::Array(Box::new(Ty::widen(a_inner, b_inner)))
            }
            // Map covariance (both key and value)
            (Ty::Map(ak, av), Ty::Map(bk, bv)) => {
                Ty::Map(Box::new(Ty::widen(ak, bk)), Box::new(Ty::widen(av, bv)))
            }
            // Fn types: widen param and return types
            (Ty::Fn(params_a, ret_a), Ty::Fn(params_b, ret_b)) => {
                if params_a.len() != params_b.len() {
                    Ty::Any
                } else {
                    let params: Vec<Ty> = params_a
                        .iter()
                        .zip(params_b.iter())
                        .map(|(a, b)| Ty::widen(a, b))
                        .collect();
                    let ret = Ty::widen(ret_a, ret_b);
                    Ty::Fn(params, Box::new(ret))
                }
            }
            // Result covariance
            (Ty::Result(ok_a, err_a), Ty::Result(ok_b, err_b)) => Ty::Result(
                Box::new(Ty::widen(ok_a, ok_b)),
                Box::new(Ty::widen(err_a, err_b)),
            ),
            // Incompatible concrete types → Any
            _ => Ty::Any,
        }
    }

    fn is_assignable_to(&self, dst: &Ty) -> bool {
        match (self, dst) {
            // Everything is assignable to Any
            (_, Ty::Any) => true,
            // Any is assignable to everything (gradual typing)
            (Ty::Any, _) => true,
            // Identical types
            (a, b) if a == b => true,
            // Nil cannot be assigned to a NonNull type (T!)
            (Ty::Nil, Ty::NonNull(_)) => false,
            // Nil is assignable to Option<T>
            (Ty::Nil, Ty::Option(_)) => true,
            // Option<T> is assignable to Option<U> if T is assignable to U
            (Ty::Option(inner_self), Ty::Option(inner_dst)) => {
                inner_self.is_assignable_to(inner_dst)
            }
            // Any T is assignable to Option<T> (implicit lift)
            (src, Ty::Option(inner_dst)) => src.is_assignable_to(inner_dst),
            // NonNull(T) is assignable wherever T is assignable
            (Ty::NonNull(inner_self), dst) => inner_self.is_assignable_to(dst),
            // Anything assignable to T is also assignable to T!
            (src, Ty::NonNull(inner_dst)) => src.is_assignable_to(inner_dst),
            // covariance for containers
            (Ty::Array(a), Ty::Array(b)) => a.is_assignable_to(b),
            (Ty::Map(ak, av), Ty::Map(bk, bv)) => {
                ak.is_assignable_to(bk) && av.is_assignable_to(bv)
            }
            // Fn types: covariant in return, contravariant in params (simplified)
            (Ty::Fn(params_s, ret_s), Ty::Fn(params_d, ret_d)) => {
                params_s.len() == params_d.len()
                    && params_s
                        .iter()
                        .zip(params_d.iter())
                        .all(|(s, d)| s.is_assignable_to(d))
                    && ret_s.is_assignable_to(ret_d)
            }
            // Result types: covariant in both type params
            (Ty::Result(ok_s, err_s), Ty::Result(ok_d, err_d)) => {
                ok_s.is_assignable_to(ok_d) && err_s.is_assignable_to(err_d)
            }
            // Generic type parameters: same name is assignable, anything goes to/from Any
            (Ty::Generic(a), Ty::Generic(b)) => a == b,
            (Ty::Generic(_), _) => true,
            (_, Ty::Generic(_)) => true,
            _ => false,
        }
    }

    fn substitute_generics(&self, subst: &std::collections::HashMap<String, Ty>) -> Ty {
        match self {
            Ty::Generic(name) => subst.get(name).cloned().unwrap_or_else(|| self.clone()),
            Ty::Array(inner) => Ty::Array(Box::new(inner.substitute_generics(subst))),
            Ty::Map(k, v) => Ty::Map(
                Box::new(k.substitute_generics(subst)),
                Box::new(v.substitute_generics(subst)),
            ),
            Ty::Option(inner) => Ty::Option(Box::new(inner.substitute_generics(subst))),
            Ty::Result(ok, err) => Ty::Result(
                Box::new(ok.substitute_generics(subst)),
                Box::new(err.substitute_generics(subst)),
            ),
            Ty::NonNull(inner) => Ty::NonNull(Box::new(inner.substitute_generics(subst))),
            Ty::Fn(params, ret) => Ty::Fn(
                params.iter().map(|p| p.substitute_generics(subst)).collect(),
                Box::new(ret.substitute_generics(subst)),
            ),
            _ => self.clone(),
        }
    }
}

/// Simple Levenshtein edit distance between two strings.
fn lev_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let alen = a_chars.len();
    let blen = b_chars.len();
    if alen == 0 {
        return blen;
    }
    if blen == 0 {
        return alen;
    }

    let mut prev: Vec<usize> = (0..=blen).collect();
    let mut curr = vec![0usize; blen + 1];

    for i in 1..=alen {
        curr[0] = i;
        for j in 1..=blen {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[blen]
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

    /// Find the closest matching variable name using a simple edit-distance heuristic.
    fn closest_match(&self, name: &str) -> Option<String> {
        let mut best: Option<String> = None;
        let mut best_dist = usize::MAX;
        for s in self.scopes.iter().rev() {
            for key in s.keys() {
                let dist = lev_distance(name, key);
                if dist < best_dist && dist <= 3 {
                    best_dist = dist;
                    best = Some(key.clone());
                }
            }
        }
        best
    }
}

/// Seed the global scope with common VM/stdlib builtin signatures.
fn seed_builtin_prelude(env: &mut Env) {
    let any = Ty::Any;
    let nil = Ty::Nil;
    let num = Ty::Number;
    let str_ty = Ty::String;
    let bool_ty = Ty::Bool;

    let fn1 = |ret: Ty| Ty::Fn(vec![any.clone()], Box::new(ret));
    let fn2 = |ret: Ty| Ty::Fn(vec![any.clone(), any.clone()], Box::new(ret));
    let fn2s_n = |ret: Ty| Ty::Fn(vec![str_ty.clone(), num.clone()], Box::new(ret));

    for (name, ty) in [
        ("print", fn1(nil.clone())),
        ("len", fn1(num.clone())),
        ("str", fn1(str_ty.clone())),
        ("type", fn1(str_ty.clone())),
        ("import", Ty::Fn(vec![str_ty.clone()], Box::new(any.clone()))),
        ("array_push", fn2(nil.clone())),
        ("push", fn2(any.clone())),
        ("array_pop", fn1(any.clone())),
        ("strcharcodeat", fn2s_n(num.clone())),
        ("str_contains", fn2(bool_ty.clone())),
        ("map_keys", fn1(Ty::Array(Box::new(str_ty.clone())))),
        ("map_has", fn2(bool_ty.clone())),
        ("map_remove", fn2(nil.clone())),
        ("clock", fn1(num.clone())),
        ("parse", fn1(Ty::Fn(vec![], Box::new(any.clone())))),
        ("compile", fn1(Ty::Fn(vec![], Box::new(any.clone())))),
    ] {
        env.insert(name.to_string(), ty);
    }
}

/// Run the pblang type checker over a parsed statement list.
///
/// The type checker performs best-effort inference and reports diagnostics
/// for type mismatches, nil-assignment errors, and undeclared variables.
///
/// # Supported features
///
/// - **Function type inference**: call expressions infer the return type from
///   the callee's known function type.
/// - **Nullable / option types** (`T?`): values that may be nil are tracked as
///   `Option(T)`. Safe access operators (`?.`, `?[]`) produce option types.
/// - **Result types** (`result<T, E>`): success-or-error types, with `expr?`
///   (ResultUnwrap) extracting the `T`.
/// - **Type widening**: combining incompatible types (e.g. in arrays, if-else
///   branches, match arms) finds the most specific common supertype.
/// - **Map type inference**: map literals infer both key and value types.
/// - **Generic type placeholders**: `T`, `K`, `V` etc. act as unresolved type
///   parameters during generic function / class checking.
pub fn check(stmts: &[Stmt]) -> Vec<Diagnostic> {
    let mut diags = vec![];
    let mut env = Env::new();
    seed_builtin_prelude(&mut env);

    // Hoist function signatures so forward references type-check (compiler modules).
    for s in stmts {
        if let StmtKind::Fn(name, params, ret, _) = &s.kind {
            let ps = params
                .iter()
                .map(|p| p.ty.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any))
                .collect::<Vec<_>>();
            let r = ret.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any);
            env.insert(name.clone(), Ty::Fn(ps, Box::new(r)));
        }
    }

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
                        let _ = infer_expr(lit_expr, env, diags);
                    }
                    pblang_core::MatchArm::Bind(name) => {
                        // Best-effort: bind gets scrutinee type, otherwise Any
                        let st = infer_expr(scrutinee, env, diags);
                        env.insert(name.clone(), st);
                    }
                    pblang_core::MatchArm::Wildcard => {}
                    pblang_core::MatchArm::BindGuard(name, guard_expr) => {
                        let st = infer_expr(scrutinee, env, diags);
                        env.insert(name.clone(), st);
                        let _ = infer_expr(guard_expr, env, diags);
                    }
                }

                let t = infer_expr(body, env, diags);
                out_ty = match out_ty {
                    None => Some(t),
                    Some(prev) => Some(Ty::widen(&prev, &t)),
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
                Some(prev) => Ty::widen(&prev, &else_ty),
            }
        }
        ExprKind::Number(_) => Ty::Number,
        ExprKind::Str(_) => Ty::String,
        ExprKind::Bool(_) => Ty::Bool,
        ExprKind::Nil => Ty::Nil,
        ExprKind::Var(name) => {
            if let Some(t) = env.get(name) {
                t
            } else {
                // Build "did you mean?" suggestion
                let suggestion = env.closest_match(name);
                let mut msg = format!("variable '{}' is not declared", name);
                if let Some(s) = suggestion {
                    msg.push_str(&format!(". Did you mean '{}'?", s));
                }
                diags.push(Diagnostic {
                    message: msg,
                    span: Some(e.span),
                });
                Ty::Any
            }
        }
        ExprKind::Array(items) => {
            // Infer element type using widening across all elements
            let mut item_ty: Option<Ty> = None;
            for it in items {
                let t = infer_expr(it, env, diags);
                item_ty = match item_ty {
                    None => Some(t),
                    Some(prev) => Some(Ty::widen(&prev, &t)),
                };
            }
            Ty::Array(Box::new(item_ty.unwrap_or(Ty::Any)))
        }
        ExprKind::Map(pairs) => {
            // Infer key and value types using widening across all pairs
            let mut key_ty: Option<Ty> = None;
            let mut val_ty: Option<Ty> = None;
            for (_k, v) in pairs {
                // Map literal keys are always strings
                let kt = Ty::String;
                let vt = infer_expr(v, env, diags);
                key_ty = match key_ty {
                    None => Some(kt),
                    Some(prev) => Some(Ty::widen(&prev, &kt)),
                };
                val_ty = match val_ty {
                    None => Some(vt),
                    Some(prev) => Some(Ty::widen(&prev, &vt)),
                };
            }
            Ty::Map(
                Box::new(key_ty.unwrap_or(Ty::Any)),
                Box::new(val_ty.unwrap_or(Ty::Any)),
            )
        }
        ExprKind::Index(obj, _ix) => match infer_expr(obj, env, diags) {
            Ty::Array(inner) => *inner,
            Ty::Map(_k, v) => *v,
            _ => Ty::Any,
        },
        ExprKind::GetAttr(_obj, _name) => Ty::Any,
        ExprKind::Call(callee, args) => {
            // Infer callee type to determine return type
            let callee_ty = infer_expr(callee, env, diags);
            let arg_tys: Vec<Ty> = args.iter().map(|a| infer_expr(a, env, diags)).collect();

            match &callee_ty {
                Ty::Fn(params, ret) => {
                    let mut subst = std::collections::HashMap::<String, Ty>::new();
                    for (i, (arg_ty, param_ty)) in arg_tys.iter().zip(params.iter()).enumerate() {
                        if let Ty::Generic(name) = param_ty {
                            subst.insert(name.clone(), arg_ty.clone());
                        } else if !arg_ty.is_assignable_to(param_ty) {
                            diags.push(Diagnostic {
                                message: format!(
                                    "argument {} type mismatch: expected {}, got {}",
                                    i + 1,
                                    param_ty,
                                    arg_ty
                                ),
                                span: Some(args.get(i).map_or(callee.span, |a| a.span)),
                            });
                        }
                    }
                    ret.substitute_generics(&subst)
                }
                _ => Ty::Any,
            }
        }
        ExprKind::FnLit(_name, params, ret, body) => {
            let ps = params
                .iter()
                .map(|p| p.ty.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any))
                .collect::<Vec<_>>();
            let declared_ret = ret.as_ref().map(Ty::from_type_expr);

            env.push();
            for p in params {
                env.insert(
                    p.name.clone(),
                    p.ty.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any),
                );
            }
            let mut fn_ret_stack = vec![declared_ret.clone()];
            for st in body {
                check_stmt(st, env, &mut fn_ret_stack, diags);
            }
            let _ = fn_ret_stack.pop();
            env.pop();

            let r = declared_ret.unwrap_or(Ty::Any);
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
            Ty::widen(&ta, &tb)
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
                // Nil in addition is always wrong
                (Ty::Nil, _) => {
                    diags.push(Diagnostic {
                        message: "addition: left operand is nil".into(),
                        span: Some(a.span),
                    });
                    Ty::Any
                }
                (_, Ty::Nil) => {
                    diags.push(Diagnostic {
                        message: "addition: right operand is nil".into(),
                        span: Some(b.span),
                    });
                    Ty::Any
                }
                // Mixed types (e.g. String + Number) — warn only if both concrete
                (ta, tb) if *ta != Ty::Any && *tb != Ty::Any && ta != tb => {
                    diags.push(Diagnostic {
                        message: format!("type mismatch in +: {} + {}", ta, tb),
                        span: Some(e.span),
                    });
                    Ty::Any
                }
                _ => Ty::Any,
            }
        }
        ExprKind::Sub(a, b) | ExprKind::Mul(a, b) | ExprKind::Div(a, b) => {
            let ta = infer_expr(a, env, diags);
            let tb = infer_expr(b, env, diags);
            // Warn if either operand is a known nil
            if matches!(ta, Ty::Nil) {
                diags.push(Diagnostic {
                    message: "arithmetic on nil (left operand is nil)".into(),
                    span: Some(a.span),
                });
            } else if ta != Ty::Any && ta != Ty::Number {
                diags.push(Diagnostic {
                    message: format!("expected number operands, got {}", ta),
                    span: Some(e.span),
                });
            }
            if matches!(tb, Ty::Nil) {
                diags.push(Diagnostic {
                    message: "arithmetic on nil (right operand is nil)".into(),
                    span: Some(b.span),
                });
            } else if tb != Ty::Any && tb != Ty::Number {
                diags.push(Diagnostic {
                    message: format!("expected number operands, got {}", tb),
                    span: Some(e.span),
                });
            }
            // Detect constant division by zero
            if matches!(&e.kind, ExprKind::Div(_, _)) {
                if let ExprKind::Number(n) = &b.kind {
                    if *n == 0.0 {
                        diags.push(Diagnostic {
                            message: "division by zero (constant denominator 0)".into(),
                            span: Some(b.span),
                        });
                    }
                }
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
            let _obj_ty = infer_expr(obj, env, diags);
            // nil-safe attribute access: result is the attribute type wrapped in Option
            // Since we don't know attribute types, return Option(Any)
            Ty::Option(Box::new(Ty::Any))
        }
        ExprKind::SafeIndex(obj, idx) => {
            let obj_ty = infer_expr(obj, env, diags);
            let _ = infer_expr(idx, env, diags);
            // nil-safe index: result is the element type wrapped in Option
            match obj_ty {
                Ty::Array(inner) => Ty::Option(inner),
                Ty::Map(_k, v) => Ty::Option(v),
                _ => Ty::Option(Box::new(Ty::Any)),
            }
        }
        ExprKind::ResultUnwrap(inner) => {
            let t = infer_expr(inner, env, diags);
            match t {
                Ty::Result(ok, _err) => *ok,
                _ => Ty::Any,
            }
        }
        // Null coalescing: a ?? b returns a if non-nil, otherwise b.
        // If a is Option(inner), result is widen(inner, b).
        // Otherwise result is widen(a, b).
        ExprKind::NullCoalesce(a, b) => {
            let ta = infer_expr(a, env, diags);
            let tb = infer_expr(b, env, diags);
            match &ta {
                Ty::Option(inner) => Ty::widen(inner, &tb),
                _ => Ty::widen(&ta, &tb),
            }
        }
        ExprKind::FString(parts) => {
            use pblang_core::FStringPart;
            for part in parts {
                if let FStringPart::Interp(e) = part {
                    let _ = infer_expr(e, env, diags);
                }
            }
            Ty::String
        }
        ExprKind::Pow(a, b) | ExprKind::Mod(a, b) => {
            let ta = infer_expr(a, env, diags);
            let tb = infer_expr(b, env, diags);
            if matches!(ta, Ty::Nil) || matches!(tb, Ty::Nil) {
                diags.push(Diagnostic {
                    message: "arithmetic on nil operand".into(),
                    span: Some(e.span),
                });
            }
            Ty::Number
        }
        ExprKind::IfExpr(cond, then_br, else_br) => {
            let _ = infer_expr(cond, env, diags);
            let t1 = infer_expr(then_br, env, diags);
            let t2 = infer_expr(else_br, env, diags);
            Ty::widen(&t1, &t2)
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
                // Check NonNull (T!) constraint explicitly
                if matches!(&ann, Ty::NonNull(_)) && matches!(&init_ty, Ty::Nil) {
                    diags.push(Diagnostic {
                        message: format!(
                            "non-null constraint violated: '{}' is declared T! but initializer is nil",
                            name
                        ),
                        span: Some(s.span),
                    });
                } else if !init_ty.is_assignable_to(&ann) {
                    diags.push(Diagnostic {
                        message: format!("type mismatch: cannot assign {} to {}", init_ty, ann),
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
                // Check NonNull constraint: cannot re-assign nil to a T! variable
                if matches!(&old, Ty::NonNull(_)) && matches!(&rhs, Ty::Nil) {
                    diags.push(Diagnostic {
                        message: format!(
                            "non-null constraint violated: cannot assign nil to '{}' (declared T!)",
                            name
                        ),
                        span: Some(s.span),
                    });
                } else if !rhs.is_assignable_to(&old) {
                    diags.push(Diagnostic {
                        message: format!("type mismatch: cannot assign {} to {}", rhs, old),
                        span: Some(s.span),
                    });
                }
            }
            // Preserve NonNull type annotation; don't widen to the rhs type
            if env
                .get(name)
                .map(|t| matches!(t, Ty::NonNull(_)))
                .unwrap_or(false)
            {
                // keep the NonNull annotation
            } else {
                env.insert(name.clone(), rhs);
            }
        }
        StmtKind::Fn(name, params, ret, body) => {
            let ps = params
                .iter()
                .map(|p| p.ty.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any))
                .collect::<Vec<_>>();
            let r = ret.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any);
            env.insert(name.clone(), Ty::Fn(ps.clone(), Box::new(r.clone())));

            env.push();
            for p in params {
                env.insert(
                    p.name.clone(),
                    p.ty.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any),
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
                        message: format!("return type mismatch: got {}, expected {}", got, exp),
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
        // Seq: flat sequence at same scope — no env push/pop
        StmtKind::Seq(stmts) => {
            for st in stmts {
                check_stmt(st, env, ret_stack, diags);
            }
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
        StmtKind::ForIn(name, col, body) => {
            let col_ty = infer_expr(col, env, diags);
            env.push();
            // Infer element type from the collection type
            let elem_ty = match &col_ty {
                Ty::Array(inner) => *inner.clone(),
                Ty::Map(k, _v) => *k.clone(),
                _ => Ty::Any,
            };
            env.insert(name.clone(), elem_ty);
            for st in body {
                check_stmt(st, env, ret_stack, diags);
            }
            env.pop();
        }
        StmtKind::ForInKV(k, v, col, body) => {
            let col_ty = infer_expr(col, env, diags);
            env.push();
            // Infer key and value types from the map type
            let (k_ty, v_ty) = match &col_ty {
                Ty::Map(k, v) => (*k.clone(), *v.clone()),
                _ => (Ty::Any, Ty::Any),
            };
            env.insert(k.clone(), k_ty);
            env.insert(v.clone(), v_ty);
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

        // New control flow statements
        StmtKind::Break | StmtKind::Continue => {}
        StmtKind::Throw(e) => {
            let _ = infer_expr(e, env, diags);
        }
        StmtKind::Loop(body) => {
            env.push();
            for st in body {
                check_stmt(st, env, ret_stack, diags);
            }
            env.pop();
        }

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
                        message: format!(
                            "type mismatch in const '{}': cannot assign {} to {}",
                            name, init_ty, ann
                        ),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: parse a pblang source string and run the type checker.
    fn typecheck_source(src: &str) -> Vec<Diagnostic> {
        let stmts = match pblang_core::parse(src) {
            Ok(s) => s,
            Err(e) => panic!("parse error: {:?}", e),
        };
        check(&stmts)
    }

    #[test]
    fn test_widen_same_type() {
        assert_eq!(Ty::widen(&Ty::Number, &Ty::Number), Ty::Number);
        assert_eq!(Ty::widen(&Ty::String, &Ty::String), Ty::String);
        assert_eq!(Ty::widen(&Ty::Bool, &Ty::Bool), Ty::Bool);
    }

    #[test]
    fn test_widen_incompatible() {
        assert_eq!(Ty::widen(&Ty::Number, &Ty::String), Ty::Any);
        assert_eq!(Ty::widen(&Ty::Bool, &Ty::String), Ty::Any);
    }

    #[test]
    fn test_widen_with_nil() {
        assert_eq!(
            Ty::widen(&Ty::Number, &Ty::Nil),
            Ty::Option(Box::new(Ty::Number))
        );
        assert_eq!(
            Ty::widen(&Ty::Nil, &Ty::String),
            Ty::Option(Box::new(Ty::String))
        );
    }

    #[test]
    fn test_widen_option_types() {
        assert_eq!(
            Ty::widen(&Ty::Option(Box::new(Ty::Number)), &Ty::Number),
            Ty::Option(Box::new(Ty::Number))
        );
        assert_eq!(
            Ty::widen(
                &Ty::Option(Box::new(Ty::Number)),
                &Ty::Option(Box::new(Ty::Number))
            ),
            Ty::Option(Box::new(Ty::Number))
        );
        // widen(Option(Number), Option(String)) → Option(Any) simplified to Any
        assert_eq!(
            Ty::widen(
                &Ty::Option(Box::new(Ty::Number)),
                &Ty::Option(Box::new(Ty::String))
            ),
            Ty::Any
        );
    }

    #[test]
    fn test_widen_array_types() {
        assert_eq!(
            Ty::widen(
                &Ty::Array(Box::new(Ty::Number)),
                &Ty::Array(Box::new(Ty::Number))
            ),
            Ty::Array(Box::new(Ty::Number))
        );
        assert_eq!(
            Ty::widen(
                &Ty::Array(Box::new(Ty::Number)),
                &Ty::Array(Box::new(Ty::String))
            ),
            Ty::Array(Box::new(Ty::Any))
        );
    }

    #[test]
    fn test_widen_map_types() {
        assert_eq!(
            Ty::widen(
                &Ty::Map(Box::new(Ty::String), Box::new(Ty::Number)),
                &Ty::Map(Box::new(Ty::String), Box::new(Ty::Number))
            ),
            Ty::Map(Box::new(Ty::String), Box::new(Ty::Number))
        );
        assert_eq!(
            Ty::widen(
                &Ty::Map(Box::new(Ty::String), Box::new(Ty::Number)),
                &Ty::Map(Box::new(Ty::String), Box::new(Ty::String))
            ),
            Ty::Map(Box::new(Ty::String), Box::new(Ty::Any))
        );
    }

    #[test]
    fn test_widen_any() {
        assert_eq!(Ty::widen(&Ty::Any, &Ty::Number), Ty::Any);
        assert_eq!(Ty::widen(&Ty::Number, &Ty::Any), Ty::Any);
    }

    #[test]
    fn test_widen_nonnull() {
        assert_eq!(
            Ty::widen(&Ty::NonNull(Box::new(Ty::Number)), &Ty::Number),
            Ty::Number
        );
        assert_eq!(
            Ty::widen(
                &Ty::NonNull(Box::new(Ty::Number)),
                &Ty::NonNull(Box::new(Ty::Number))
            ),
            Ty::NonNull(Box::new(Ty::Number))
        );
    }

    #[test]
    fn test_is_assignable_option() {
        assert!(Ty::Number.is_assignable_to(&Ty::Option(Box::new(Ty::Number))));
        assert!(Ty::Nil.is_assignable_to(&Ty::Option(Box::new(Ty::Number))));
        assert!(!Ty::Option(Box::new(Ty::Number)).is_assignable_to(&Ty::Number));
        assert!(
            Ty::Option(Box::new(Ty::Number)).is_assignable_to(&Ty::Option(Box::new(Ty::Number)))
        );
    }

    #[test]
    fn test_is_assignable_result() {
        assert!(Ty::Result(Box::new(Ty::Number), Box::new(Ty::String))
            .is_assignable_to(&Ty::Result(Box::new(Ty::Number), Box::new(Ty::String))));
        assert!(
            Ty::Number.is_assignable_to(&Ty::Result(Box::new(Ty::Number), Box::new(Ty::String)))
                == false
        );
    }

    #[test]
    fn test_is_assignable_generic() {
        assert!(Ty::Generic("T".into()).is_assignable_to(&Ty::Generic("T".into())));
        assert!(!Ty::Generic("T".into()).is_assignable_to(&Ty::Generic("K".into())));
        assert!(Ty::Number.is_assignable_to(&Ty::Generic("T".into())));
        assert!(Ty::Generic("T".into()).is_assignable_to(&Ty::Number));
    }

    #[test]
    fn test_is_assignable_map() {
        assert!(Ty::Map(Box::new(Ty::String), Box::new(Ty::Number))
            .is_assignable_to(&Ty::Map(Box::new(Ty::String), Box::new(Ty::Number))));
        assert!(Ty::Map(Box::new(Ty::Number), Box::new(Ty::Number))
            .is_assignable_to(&Ty::Map(Box::new(Ty::Any), Box::new(Ty::Number))));
    }

    #[test]
    fn test_function_call_inference() {
        // fn add(a: number, b: number) -> number { return a + b }
        let diags = typecheck_source(
            "fn add(a: number, b: number) -> number { return a + b }\nlet x = add(1, 2)",
        );
        // Should have no type errors (only the + operands may generate warnings about Any)
        let nil_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("type mismatch") || d.message.contains("cannot assign"))
            .collect();
        assert!(
            nil_diags.is_empty(),
            "unexpected type errors: {:?}",
            nil_diags
        );
    }

    #[test]
    fn test_function_call_arg_type_mismatch() {
        // Passing a string where number is expected
        let diags = typecheck_source(
            "fn add(a: number, b: number) -> number { return a + b }\nlet x = add(\"hello\", 2)",
        );
        let mismatch_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("argument") && d.message.contains("type mismatch"))
            .collect();
        assert!(
            !mismatch_diags.is_empty(),
            "expected argument type mismatch diagnostic"
        );
    }

    #[test]
    fn test_map_literal_inference() {
        // map literal should infer map<string, number>
        let diags = typecheck_source("let m = map{\"a\": 1, \"b\": 2}");
        // No type errors expected
        assert!(diags.is_empty(), "unexpected diagnostics: {:?}", diags);
    }

    #[test]
    fn test_safe_index_returns_option() {
        // Safe index on array should produce option type
        // We can't directly test return types, but we can verify no errors
        let diags = typecheck_source("let arr = [1, 2, 3]\nlet x = arr?[0]");
        assert!(diags.is_empty(), "unexpected diagnostics: {:?}", diags);
    }

    #[test]
    fn test_result_unwrap() {
        // Result unwrap should extract the ok type from a Result
        // Test that the typechecker handles result<T,E> type annotations
        let diags = typecheck_source("let x: result<number, string> = 1");
        // Should not panic; result<T,E> is a valid type annotation
        let _ = diags;
    }

    #[test]
    fn test_display_ty() {
        assert_eq!(Ty::Number.to_string(), "number");
        assert_eq!(Ty::Option(Box::new(Ty::Number)).to_string(), "number?");
        assert_eq!(Ty::Array(Box::new(Ty::Number)).to_string(), "array<number>");
        assert_eq!(
            Ty::Map(Box::new(Ty::String), Box::new(Ty::Number)).to_string(),
            "map<string, number>"
        );
        assert_eq!(
            Ty::Result(Box::new(Ty::Number), Box::new(Ty::String)).to_string(),
            "result<number, string>"
        );
        assert_eq!(Ty::Generic("T".into()).to_string(), "T");
        assert_eq!(Ty::NonNull(Box::new(Ty::Number)).to_string(), "number!");
    }

    #[test]
    fn test_from_type_expr_option() {
        // Test that TypeKind::Option is properly converted
        // We test this indirectly through parsing
        let diags = typecheck_source("let x: number? = nil");
        // nil should be assignable to number?
        let assign_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("cannot assign"))
            .collect();
        assert!(
            assign_diags.is_empty(),
            "nil should be assignable to number?: {:?}",
            assign_diags
        );
    }

    #[test]
    fn test_widen_fn_types() {
        let fn1 = Ty::Fn(vec![Ty::Number], Box::new(Ty::Number));
        let fn2 = Ty::Fn(vec![Ty::Number], Box::new(Ty::Number));
        assert_eq!(Ty::widen(&fn1, &fn2), fn1);

        let fn3 = Ty::Fn(vec![Ty::Number], Box::new(Ty::String));
        assert_eq!(
            Ty::widen(&fn1, &fn3),
            Ty::Fn(vec![Ty::Number], Box::new(Ty::Any))
        );
    }

    #[test]
    fn test_widen_result_types() {
        let r1 = Ty::Result(Box::new(Ty::Number), Box::new(Ty::String));
        let r2 = Ty::Result(Box::new(Ty::Number), Box::new(Ty::String));
        assert_eq!(Ty::widen(&r1, &r2), r1);

        let r3 = Ty::Result(Box::new(Ty::String), Box::new(Ty::Number));
        assert_eq!(
            Ty::widen(&r1, &r3),
            Ty::Result(Box::new(Ty::Any), Box::new(Ty::Any))
        );
    }

    #[test]
    fn test_builtin_prelude_import_and_print() {
        let diags = typecheck_source(
            "let m = import(\"stdlib/compiler/tokenizer.sapp\");\nprint len(m);",
        );
        let undeclared: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("not declared"))
            .collect();
        assert!(
            undeclared.is_empty(),
            "prelude builtins should be known: {:?}",
            undeclared
        );
    }

    #[test]
    fn test_generic_call_substitution() {
        let diags = typecheck_source(
            r#"
            fn id(x: T) -> T { return x; }
            let n: number = id(1);
            "#,
        );
        let mismatches: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("type mismatch") || d.message.contains("not declared"))
            .collect();
        assert!(
            mismatches.is_empty(),
            "generic call should bind T to number: {:?}",
            mismatches
        );
    }

    #[test]
    fn test_fn_lit_body_checked() {
        let diags = typecheck_source(
            "let f = fn(x: number) -> number { return x + \"oops\"; };",
        );
        assert!(
            diags.iter().any(|d| d.message.contains("type mismatch")),
            "fn literal body should be type-checked: {:?}",
            diags
        );
    }
}
