use pblang_core::{Expr, ExprKind, LogicalOp, Param, Span, Stmt, StmtKind, TypeExpr, TypeKind};

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Any,
    Number,
    /// Arbitrary-precision integer (`bigint(x)`).
    BigInt,
    /// Exact arbitrary-precision rational (`rational(x)`).
    Rational,
    /// Arbitrary-precision base-10 decimal (`decimal(x)`).
    Decimal,
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
    /// Struct(name, fields) — a `struct Name { f: T, ... }` instance. Fields are
    /// stored in declaration order (with the implicit `__struct` tag) so that
    /// field access (`p.x`) can be typed and unknown fields reported.
    Struct(String, Vec<(String, Ty)>),
    /// Enum(name, variants) — the namespace value bound by an `enum Name { ... }`
    /// declaration. Each variant maps to its access type: `EnumInstance(name)` for
    /// unit variants (`Color.Red`) and `Fn(params) -> EnumInstance(name)` for data
    /// variants (`Shape.Circle`), so that variant access is checked and
    /// data-variant calls are arg-checked.
    Enum(String, Vec<(String, Ty)>),
    /// EnumInstance(name) — a value of enum `name` (e.g. the result of `Color.Red`
    /// or a `c: Color` parameter). Carries only the enum name; the variant set is
    /// looked up in the checker's enum registry. Enables exhaustiveness checking
    /// on `match instance.__variant { ... }`.
    EnumInstance(String),
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Any => write!(f, "any"),
            Ty::Number => write!(f, "number"),
            Ty::BigInt => write!(f, "bigint"),
            Ty::Rational => write!(f, "rational"),
            Ty::Decimal => write!(f, "decimal"),
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
            Ty::Struct(name, _) => write!(f, "{}", name),
            Ty::Enum(name, _) => write!(f, "{}", name),
            Ty::EnumInstance(name) => write!(f, "{}", name),
        }
    }
}

impl Ty {
    /// True for any numeric type (the precision ladder: number ⊂ bigint ⊂
    /// rational ⊂ decimal, plus `Any`/`Generic` which are gradual).
    fn is_numeric(&self) -> bool {
        matches!(
            self,
            Ty::Number | Ty::BigInt | Ty::Rational | Ty::Decimal | Ty::Any | Ty::Generic(_)
        )
    }

    /// "Rank" within the precision ladder, for arithmetic result inference.
    fn numeric_rank(&self) -> u8 {
        match self {
            Ty::Number => 1,
            Ty::BigInt => 2,
            Ty::Rational => 3,
            Ty::Decimal => 4,
            _ => 0,
        }
    }

    /// Result type of an arithmetic op on two numeric operands: the higher-rank
    /// type (mirrors the VM's promotion). `Any` propagates as `Any`.
    fn numeric_widen(a: &Ty, b: &Ty) -> Ty {
        if matches!(a, Ty::Any) || matches!(b, Ty::Any) || matches!(a, Ty::Generic(_)) || matches!(b, Ty::Generic(_)) {
            return Ty::Any;
        }
        let (ra, rb) = (a.numeric_rank(), b.numeric_rank());
        match ra.max(rb) {
            1 => Ty::Number,
            2 => Ty::BigInt,
            3 => Ty::Rational,
            4 => Ty::Decimal,
            _ => Ty::Any,
        }
    }

    fn from_type_expr(t: &TypeExpr) -> Ty {
        match &t.kind {
            TypeKind::Any => Ty::Any,
            TypeKind::Number => Ty::Number,
            TypeKind::BigInt => Ty::BigInt,
            TypeKind::Rational => Ty::Rational,
            TypeKind::Decimal => Ty::Decimal,
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
            // Two distinct numeric types widen up the precision ladder
            // (e.g. an array of [bigint, number] has element type bigint).
            (x, y) if x.numeric_rank() > 0 && y.numeric_rank() > 0 => Ty::numeric_widen(x, y),
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
            // The numeric types interoperate (the VM promotes/demotes between
            // number/bigint/rational/decimal), so any numeric is assignable to
            // any numeric. This still flags number↛string / bigint↛bool, etc.
            (a, b) if a.numeric_rank() > 0 && b.numeric_rank() > 0 => true,
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
            // Structs: same-named structs are interchangeable, and a struct is
            // structurally a map (and vice-versa, for the desugared constructor
            // whose body returns a map literal).
            (Ty::Struct(a, _), Ty::Struct(b, _)) => a == b,
            (Ty::Struct(_, _), Ty::Map(_, _)) => true,
            (Ty::Map(_, _), Ty::Struct(_, _)) => true,
            // Enum namespaces are structurally maps too (same-named are equal).
            (Ty::Enum(a, _), Ty::Enum(b, _)) => a == b,
            (Ty::Enum(_, _), Ty::Map(_, _)) => true,
            (Ty::Map(_, _), Ty::Enum(_, _)) => true,
            // Enum instances: same-named are interchangeable; structurally maps.
            (Ty::EnumInstance(a), Ty::EnumInstance(b)) => a == b,
            (Ty::EnumInstance(_), Ty::Map(_, _)) => true,
            (Ty::Map(_, _), Ty::EnumInstance(_)) => true,
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
                params
                    .iter()
                    .map(|p| p.substitute_generics(subst))
                    .collect(),
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

/// Detect the desugared struct-constructor pattern the parser emits for
/// `struct Name { f: T, ... }`: a function whose body is a single
/// `return { __struct: "Name", f, ... }`. Returns the matching `Ty::Struct`
/// (fields taken from the constructor params, plus the implicit `__struct`
/// tag), so constructor calls are typed as structs and field access can be
/// checked. Returns `None` for ordinary functions.
fn struct_ctor_return_ty(params: &[Param], body: &[Stmt]) -> Option<Ty> {
    if body.len() != 1 {
        return None;
    }
    let ret_expr = match &body[0].kind {
        StmtKind::Return(Some(e)) => e,
        _ => return None,
    };
    let entries = match &ret_expr.kind {
        ExprKind::Map(entries) => entries,
        _ => return None,
    };
    let mut struct_name: Option<String> = None;
    for (k, v) in entries {
        if k == "__struct" {
            if let ExprKind::Str(s) = &v.kind {
                struct_name = Some(s.clone());
            }
        }
    }
    let name = struct_name?;
    let mut fields: Vec<(String, Ty)> = Vec::with_capacity(params.len() + 1);
    fields.push(("__struct".to_string(), Ty::String));
    for p in params {
        fields.push((
            p.name.clone(),
            p.ty.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any),
        ));
    }
    Some(Ty::Struct(name, fields))
}

/// First string-valued entry for `key` in a map literal's entry list.
fn map_str_field(entries: &[(String, Expr)], key: &str) -> Option<String> {
    for (k, v) in entries {
        if k == key {
            if let ExprKind::Str(s) = &v.kind {
                return Some(s.clone());
            }
        }
    }
    None
}

/// Enum name carried by a data-variant constructor's body, i.e. a single
/// `return { __enum: "Name", ... }`.
fn fnlit_enum_name(body: &[Stmt]) -> Option<String> {
    if body.len() != 1 {
        return None;
    }
    let ret = match &body[0].kind {
        StmtKind::Return(Some(e)) => e,
        _ => return None,
    };
    let m = match &ret.kind {
        ExprKind::Map(m) => m,
        _ => return None,
    };
    map_str_field(m, "__enum")
}

/// Detect the desugared enum declaration the parser emits for
/// `enum Name { Unit, Data(T), ... }`: a map whose every value is either a
/// unit-variant map (`{ __enum: "Name", __variant: "V" }`) or a data-variant
/// constructor (`fn(..) { return { __enum: "Name", __variant: "V", values } }`).
/// Returns a `Ty::Enum` listing each variant's access type (unit → `Any`,
/// data → `Fn(params) -> Any`). Returns `None` for ordinary map literals.
fn enum_decl_ty(entries: &[(String, Expr)]) -> Option<Ty> {
    if entries.is_empty() {
        return None;
    }
    let mut enum_name: Option<String> = None;
    let mut variants: Vec<(String, Ty)> = Vec::with_capacity(entries.len());
    for (vname, val) in entries {
        let (ename, vty) = match &val.kind {
            ExprKind::Map(m) => {
                let en = map_str_field(m, "__enum")?;
                map_str_field(m, "__variant")?; // unit variant must carry __variant
                let inst = Ty::EnumInstance(en.clone());
                (en, inst)
            }
            ExprKind::FnLit(_, params, _, body) => {
                let en = fnlit_enum_name(body)?;
                let ptys = params
                    .iter()
                    .map(|p| p.ty.as_ref().map(Ty::from_type_expr).unwrap_or(Ty::Any))
                    .collect::<Vec<_>>();
                let inst = Ty::EnumInstance(en.clone());
                (en, Ty::Fn(ptys, Box::new(inst)))
            }
            _ => return None,
        };
        match &enum_name {
            None => enum_name = Some(ename),
            Some(e) if *e == ename => {}
            Some(_) => return None, // mixed enum names → not a single enum decl
        }
        variants.push((vname.clone(), vty));
    }
    Some(Ty::Enum(enum_name?, variants))
}

/// Find the closest field name to `name` among `fields` (edit distance ≤ 3),
/// for "did you mean?" diagnostics on unknown struct fields / enum variants.
fn closest_field(fields: &[(String, Ty)], name: &str) -> Option<String> {
    let mut best: Option<String> = None;
    let mut best_dist = usize::MAX;
    for (fname, _) in fields {
        let dist = lev_distance(name, fname);
        if dist < best_dist && dist <= 3 {
            best_dist = dist;
            best = Some(fname.clone());
        }
    }
    best
}

/// Rewrite `Ty::Generic(name)` to `Ty::EnumInstance(name)` wherever `name` is a
/// declared enum, recursing through containers. This is how a `: EnumName`
/// annotation (parsed as a generic-looking uppercase type) becomes a real enum
/// instance type for exhaustiveness checking.
fn resolve_enum_names(ty: Ty, enums: &std::collections::HashMap<String, Vec<String>>) -> Ty {
    match ty {
        Ty::Generic(name) if enums.contains_key(&name) => Ty::EnumInstance(name),
        Ty::Array(inner) => Ty::Array(Box::new(resolve_enum_names(*inner, enums))),
        Ty::Map(k, v) => Ty::Map(
            Box::new(resolve_enum_names(*k, enums)),
            Box::new(resolve_enum_names(*v, enums)),
        ),
        Ty::Option(inner) => Ty::Option(Box::new(resolve_enum_names(*inner, enums))),
        Ty::NonNull(inner) => Ty::NonNull(Box::new(resolve_enum_names(*inner, enums))),
        Ty::Result(ok, err) => Ty::Result(
            Box::new(resolve_enum_names(*ok, enums)),
            Box::new(resolve_enum_names(*err, enums)),
        ),
        Ty::Fn(params, ret) => Ty::Fn(
            params
                .into_iter()
                .map(|p| resolve_enum_names(p, enums))
                .collect(),
            Box::new(resolve_enum_names(*ret, enums)),
        ),
        other => other,
    }
}

#[derive(Default)]
struct Env {
    scopes: Vec<std::collections::HashMap<String, Ty>>,
    /// Declared enums: name → variant names, for annotation resolution and
    /// match-exhaustiveness checking.
    enums: std::collections::HashMap<String, Vec<String>>,
}

impl Env {
    fn new() -> Self {
        Env {
            scopes: vec![std::collections::HashMap::new()],
            enums: std::collections::HashMap::new(),
        }
    }

    /// Resolve a syntactic type annotation, mapping `: EnumName` to an enum
    /// instance type when `EnumName` is a declared enum.
    fn resolve_ty(&self, t: &TypeExpr) -> Ty {
        resolve_enum_names(Ty::from_type_expr(t), &self.enums)
    }

    /// Register an enum's variant set (used for exhaustiveness checks).
    fn register_enum(&mut self, name: String, variants: Vec<String>) {
        self.enums.insert(name, variants);
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

    let fn0 = |ret: Ty| Ty::Fn(vec![], Box::new(ret));
    let fn1 = |ret: Ty| Ty::Fn(vec![any.clone()], Box::new(ret));
    let fn2 = |ret: Ty| Ty::Fn(vec![any.clone(), any.clone()], Box::new(ret));
    let fn2s_n = |ret: Ty| Ty::Fn(vec![str_ty.clone(), num.clone()], Box::new(ret));

    for (name, ty) in [
        ("print", fn1(nil.clone())),
        ("len", fn1(num.clone())),
        ("str", fn1(str_ty.clone())),
        ("type", fn1(str_ty.clone())),
        (
            "import",
            Ty::Fn(vec![str_ty.clone()], Box::new(any.clone())),
        ),
        // Concurrency builtins used directly by IDE examples/stdlib wrappers.
        ("spawn", fn1(any.clone())),
        ("join", fn1(any.clone())),
        (
            "channel",
            Ty::Fn(
                vec![],
                Box::new(Ty::Map(Box::new(str_ty.clone()), Box::new(any.clone()))),
            ),
        ),
        ("send", fn2(nil.clone())),
        ("recv", fn1(any.clone())),
        // JSON builtins used directly by stdlib/json.sapp and IDE examples.
        ("json_parse", fn1(any.clone())),
        ("json_stringify", fn1(str_ty.clone())),
        ("read_line", fn0(str_ty.clone())),
        ("parse_number", fn1(any.clone())),
        ("parse_bool", fn1(any.clone())),
        // Precision-ladder constructors return their first-class types so
        // inference flows them (e.g. `let x = bigint(5)` types `x` as bigint).
        ("bigint", Ty::Fn(vec![any.clone()], Box::new(Ty::BigInt))),
        ("rational", Ty::Fn(vec![], Box::new(Ty::Rational))),
        ("decimal", Ty::Fn(vec![any.clone()], Box::new(Ty::Decimal))),
        ("str_trim", fn1(str_ty.clone())),
        ("str_split", fn2(Ty::Array(Box::new(str_ty.clone())))),
        // Polymorphic `slice(seq, start, end)` over string/array (backs `seq[a..b]`);
        // returns `any` because the element/result type follows the input. Used by
        // the self-hosted tokenizer (char-indexed substring) and formatter.
        (
            "slice",
            Ty::Fn(
                vec![any.clone(), num.clone(), num.clone()],
                Box::new(any.clone()),
            ),
        ),
        ("str_starts_with", fn2(bool_ty.clone())),
        ("array_push", fn2(nil.clone())),
        ("push", fn2(any.clone())),
        ("array_pop", fn1(any.clone())),
        ("strcharcodeat", fn2s_n(num.clone())),
        ("strfromcharcode", fn1(str_ty.clone())),
        ("str_from_char_code", fn1(str_ty.clone())),
        ("str_contains", fn2(bool_ty.clone())),
        ("map_keys", fn1(Ty::Array(Box::new(str_ty.clone())))),
        ("map_has", fn2(bool_ty.clone())),
        ("map_remove", fn2(nil.clone())),
        ("clock", fn0(num.clone())),
        ("now_iso", fn0(str_ty.clone())),
        ("num_pow", fn2(num.clone())),
        // Numeric builtins used throughout stdlib (math, graphics, colour).
        ("num_floor", fn1(num.clone())),
        ("num_ceil", fn1(num.clone())),
        ("num_sqrt", fn1(num.clone())),
        ("num_abs", fn1(num.clone())),
        ("num_mod", fn2(num.clone())),
        // Timing builtins used by async / IO stdlibs.
        ("sleep_ms", fn1(nil.clone())),
        ("now_ms", fn0(num.clone())),
        // Graphics builtins (pblang-graphics crate) — typed loosely as the
        // call surface is variadic and the VM does its own argument checking.
        ("gfxcreatewindow", fn1(any.clone())),
        ("gfxclose", fn1(nil.clone())),
        ("gfxisopen", fn1(bool_ty.clone())),
        ("gfxsize", fn1(any.clone())),
        ("gfxclear", fn1(nil.clone())),
        ("gfxdrawline", fn1(nil.clone())),
        ("gfxdrawrect", fn1(nil.clone())),
        ("gfxdrawpoint", fn1(nil.clone())),
        ("gfxpresent", fn1(nil.clone())),
        ("gfxpollevents", fn1(any.clone())),
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

    // Hoist phase 1: register enum declarations (desugared to `let Name = map{...}`)
    // first, so type annotations referencing them — `fn f(c: Color)` — resolve to
    // enum-instance types, and variant access works regardless of order.
    for s in stmts {
        if let StmtKind::Let(name, None, init) = &s.kind {
            if let ExprKind::Map(entries) = &init.kind {
                if let Some(Ty::Enum(ename, variants)) = enum_decl_ty(entries) {
                    env.register_enum(
                        ename.clone(),
                        variants.iter().map(|(n, _)| n.clone()).collect(),
                    );
                    env.insert(name.clone(), Ty::Enum(ename, variants));
                }
            }
        }
    }

    // Hoist phase 2: function signatures (now enum-aware in their annotations).
    for s in stmts {
        if let StmtKind::Fn(name, params, ret, body) = &s.kind {
            let ps = params
                .iter()
                .map(|p| p.ty.as_ref().map(|t| env.resolve_ty(t)).unwrap_or(Ty::Any))
                .collect::<Vec<_>>();
            // Struct constructors get a `Ty::Struct` return type (see
            // `struct_ctor_return_ty`); ordinary functions use their annotation.
            let r = struct_ctor_return_ty(params, body)
                .unwrap_or_else(|| ret.as_ref().map(|t| env.resolve_ty(t)).unwrap_or(Ty::Any));
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

            // Exhaustiveness: `match <enum-instance>.__variant { case "V": ... }`
            // with no `else` and no catch-all arm must cover every variant.
            if else_expr.is_none() {
                let has_catch_all = arms.iter().any(|(a, _)| {
                    matches!(
                        a,
                        pblang_core::MatchArm::Wildcard | pblang_core::MatchArm::Bind(_)
                    )
                });
                if !has_catch_all {
                    if let ExprKind::GetAttr(obj, attr) = &scrutinee.kind {
                        if attr == "__variant" {
                            // Re-infer the discriminant's owner without emitting
                            // duplicate diagnostics.
                            if let Ty::EnumInstance(ename) =
                                infer_expr(obj, env, &mut Vec::new())
                            {
                                if let Some(all) = env.enums.get(&ename).cloned() {
                                    let covered: std::collections::HashSet<&str> = arms
                                        .iter()
                                        .filter_map(|(a, _)| match a {
                                            pblang_core::MatchArm::Literal(le) => match &le.kind {
                                                ExprKind::Str(s) => Some(s.as_str()),
                                                _ => None,
                                            },
                                            _ => None,
                                        })
                                        .collect();
                                    let missing: Vec<String> = all
                                        .iter()
                                        .filter(|v| !covered.contains(v.as_str()))
                                        .cloned()
                                        .collect();
                                    if !missing.is_empty() {
                                        diags.push(Diagnostic {
                                            message: format!(
                                                "non-exhaustive match on enum '{}': missing variant(s) {}",
                                                ename,
                                                missing.join(", ")
                                            ),
                                            span: Some(e.span),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }

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
        ExprKind::GetAttr(obj, name) => {
            let obj_ty = infer_expr(obj, env, diags);
            match &obj_ty {
                Ty::Struct(sname, fields) => {
                    match fields.iter().find(|(fname, _)| fname == name) {
                        Some((_, fty)) => fty.clone(),
                        None => {
                            let mut msg = format!("no field '{}' on struct '{}'", name, sname);
                            if let Some(s) = closest_field(fields, name) {
                                msg.push_str(&format!(". Did you mean '{}'?", s));
                            }
                            diags.push(Diagnostic {
                                message: msg,
                                span: Some(e.span),
                            });
                            Ty::Any
                        }
                    }
                }
                Ty::Enum(ename, variants) => {
                    match variants.iter().find(|(vname, _)| vname == name) {
                        Some((_, vty)) => vty.clone(),
                        None => {
                            let mut msg = format!("no variant '{}' on enum '{}'", name, ename);
                            if let Some(s) = closest_field(variants, name) {
                                msg.push_str(&format!(". Did you mean '{}'?", s));
                            }
                            diags.push(Diagnostic {
                                message: msg,
                                span: Some(e.span),
                            });
                            Ty::Any
                        }
                    }
                }
                // Enum-instance access: the discriminant fields are well-known;
                // payload fields are variant-dependent, so stay lenient (`any`).
                Ty::EnumInstance(_) => match name.as_str() {
                    "__variant" | "__enum" => Ty::String,
                    "values" => Ty::Array(Box::new(Ty::Any)),
                    _ => Ty::Any,
                },
                _ => Ty::Any,
            }
        }
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
                .map(|p| p.ty.as_ref().map(|t| env.resolve_ty(t)).unwrap_or(Ty::Any))
                .collect::<Vec<_>>();
            let declared_ret = ret.as_ref().map(|t| env.resolve_ty(t));

            env.push();
            for p in params {
                let pty = p.ty.as_ref().map(|t| env.resolve_ty(t)).unwrap_or(Ty::Any);
                env.insert(p.name.clone(), pty);
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
                // Numeric ladder: result is the higher-precision type.
                (x, y) if x.is_numeric() && y.is_numeric() => Ty::numeric_widen(x, y),
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
            } else if !ta.is_numeric() {
                diags.push(Diagnostic {
                    message: format!("expected numeric operands, got {}", ta),
                    span: Some(e.span),
                });
            }
            if matches!(tb, Ty::Nil) {
                diags.push(Diagnostic {
                    message: "arithmetic on nil (right operand is nil)".into(),
                    span: Some(b.span),
                });
            } else if !tb.is_numeric() {
                diags.push(Diagnostic {
                    message: format!("expected numeric operands, got {}", tb),
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
            // Result follows the precision ladder (number/bigint/rational/decimal).
            if ta.is_numeric() && tb.is_numeric() {
                Ty::numeric_widen(&ta, &tb)
            } else {
                Ty::Number
            }
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
                let ann = env.resolve_ty(t);
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
                // An unannotated `let Name = map{...}` that matches the enum
                // desugaring binds the richer `Ty::Enum` so variant access and
                // data-variant calls are checked, and registers its variant set
                // (covers non-top-level enums that the hoist pass didn't see).
                let bound = match &init.kind {
                    ExprKind::Map(entries) => match enum_decl_ty(entries) {
                        Some(Ty::Enum(ename, variants)) => {
                            env.register_enum(
                                ename.clone(),
                                variants.iter().map(|(n, _)| n.clone()).collect(),
                            );
                            Ty::Enum(ename, variants)
                        }
                        _ => init_ty,
                    },
                    _ => init_ty,
                };
                env.insert(name.clone(), bound);
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
                .map(|p| p.ty.as_ref().map(|t| env.resolve_ty(t)).unwrap_or(Ty::Any))
                .collect::<Vec<_>>();
            let struct_ret = struct_ctor_return_ty(params, body);
            let declared_r = ret.as_ref().map(|t| env.resolve_ty(t)).unwrap_or(Ty::Any);
            // The public signature exposes the struct type for callers; the body
            // is checked against `Any` for constructors so the returned map
            // literal isn't flagged as a Map→Struct mismatch.
            let signature_ret = struct_ret.clone().unwrap_or_else(|| declared_r.clone());
            env.insert(name.clone(), Ty::Fn(ps.clone(), Box::new(signature_ret)));

            env.push();
            for p in params {
                let pty = p.ty.as_ref().map(|t| env.resolve_ty(t)).unwrap_or(Ty::Any);
                env.insert(p.name.clone(), pty);
            }
            let body_ret = if struct_ret.is_some() {
                Ty::Any
            } else {
                declared_r
            };
            ret_stack.push(Some(body_ret));
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
                let ann = env.resolve_ty(t);
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
            !Ty::Number.is_assignable_to(&Ty::Result(Box::new(Ty::Number), Box::new(Ty::String)))
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
    fn test_struct_construction_arg_type_mismatch() {
        // `struct` desugars to a typed constructor function, so passing the
        // wrong field type to the constructor is caught as an argument mismatch.
        let diags = typecheck_source(
            "struct Point { x: number, y: number }\nlet p = Point(1, \"two\")",
        );
        let mismatch: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("argument") && d.message.contains("type mismatch"))
            .collect();
        assert!(
            !mismatch.is_empty(),
            "expected struct-construction argument mismatch, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_struct_construction_well_typed_ok() {
        let diags = typecheck_source(
            "struct Point { x: number, y: number }\nlet p = Point(1, 2)",
        );
        let mismatch: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("argument") && d.message.contains("type mismatch"))
            .collect();
        assert!(
            mismatch.is_empty(),
            "well-typed struct construction should not warn: {:?}",
            mismatch
        );
    }

    #[test]
    fn test_bigint_constructor_type_flows() {
        // `bigint(x)` infers as bigint; assigning to a string annotation mismatches.
        let diags = typecheck_source("let x = bigint(5)\nlet s: string = x");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("cannot assign bigint to string")),
            "expected bigint->string mismatch, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_precision_type_annotations_ok() {
        // `: bigint` / `: rational` / `: decimal` annotations parse and accept
        // their constructor values without a mismatch.
        let diags = typecheck_source(
            "let a: bigint = bigint(5)\nlet r: rational = rational(\"1/3\")\nlet d: decimal = decimal(\"0.1\")",
        );
        let mism: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("cannot assign"))
            .collect();
        assert!(mism.is_empty(), "precision annotations should accept their values: {:?}", mism);
    }

    #[test]
    fn test_decimal_annotation_rejects_string() {
        let diags = typecheck_source("let d: decimal = \"hi\"");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("cannot assign string to decimal")),
            "expected string->decimal mismatch, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_bigint_arithmetic_not_flagged_as_non_number() {
        // bigint/decimal arithmetic must NOT raise "expected numeric operands".
        let diags = typecheck_source(
            "let t = bigint(2) ** 64\nlet u = decimal(\"1.5\") * decimal(\"2\")\nlet v = bigint(3) + bigint(4)",
        );
        let numeric_errs: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("expected numeric operands"))
            .collect();
        assert!(numeric_errs.is_empty(), "bigint/decimal arithmetic is numeric: {:?}", numeric_errs);
    }

    #[test]
    fn test_struct_field_access_unknown_field() {
        // Accessing a field the struct doesn't declare is a static error.
        let diags = typecheck_source(
            "struct Point { x: number, y: number }\nlet p = Point(1, 2)\nlet z = p.z",
        );
        let field_errs: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("no field") && d.message.contains("Point"))
            .collect();
        assert!(
            !field_errs.is_empty(),
            "expected unknown-field diagnostic, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_struct_field_access_known_field_ok() {
        // Accessing declared fields is fine.
        let diags = typecheck_source(
            "struct Point { x: number, y: number }\nlet p = Point(1, 2)\nlet a = p.x\nlet b = p.y",
        );
        let field_errs: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("no field"))
            .collect();
        assert!(
            field_errs.is_empty(),
            "valid field access should not warn: {:?}",
            field_errs
        );
    }

    #[test]
    fn test_struct_field_type_flows() {
        // p.x is number, so assigning it to a string-annotated binding mismatches.
        let diags = typecheck_source(
            "struct Point { x: number, y: number }\nlet p = Point(1, 2)\nlet s: string = p.x",
        );
        let mismatch: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.message.contains("cannot assign")
                    && d.message.contains("number")
                    && d.message.contains("string")
            })
            .collect();
        assert!(
            !mismatch.is_empty(),
            "field type should flow (number->string mismatch), got: {:?}",
            diags
        );
    }

    #[test]
    fn test_struct_internal_tag_field_ok() {
        // The implicit __struct tag is a real (string) field, not an error.
        let diags = typecheck_source(
            "struct Point { x: number, y: number }\nlet p = Point(1, 2)\nlet t = p.__struct",
        );
        let field_errs: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("no field"))
            .collect();
        assert!(
            field_errs.is_empty(),
            "__struct tag access should be allowed: {:?}",
            field_errs
        );
    }

    #[test]
    fn test_struct_field_did_you_mean() {
        // Misspelled field gets a did-you-mean suggestion.
        let diags = typecheck_source(
            "struct Point { x: number, y: number }\nlet p = Point(1, 2)\nlet z = p.xx",
        );
        let suggest: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("no field") && d.message.contains("Did you mean 'x'"))
            .collect();
        assert!(
            !suggest.is_empty(),
            "expected did-you-mean suggestion, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_enum_unknown_variant() {
        // Accessing a variant the enum doesn't declare is a static error.
        let diags = typecheck_source("enum Color { Red, Green, Blue }\nlet c = Color.Purple");
        let errs: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("no variant") && d.message.contains("Color"))
            .collect();
        assert!(
            !errs.is_empty(),
            "expected unknown-variant diagnostic, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_enum_known_variant_ok() {
        let diags = typecheck_source(
            "enum Color { Red, Green, Blue }\nlet a = Color.Red\nlet b = Color.Blue",
        );
        let errs: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("no variant"))
            .collect();
        assert!(errs.is_empty(), "valid variant access should not warn: {:?}", errs);
    }

    #[test]
    fn test_enum_did_you_mean() {
        let diags = typecheck_source("enum Color { Red, Green, Blue }\nlet c = Color.Rad");
        let suggest: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("no variant") && d.message.contains("Did you mean 'Red'"))
            .collect();
        assert!(
            !suggest.is_empty(),
            "expected did-you-mean suggestion, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_enum_data_variant_arg_check() {
        // Data variants are typed constructors: wrong arg type is caught.
        let diags = typecheck_source(
            "enum Shape { Empty, Circle(number) }\nlet c = Shape.Circle(\"x\")",
        );
        let mismatch: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("argument") && d.message.contains("type mismatch"))
            .collect();
        assert!(
            !mismatch.is_empty(),
            "expected data-variant argument mismatch, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_enum_data_variant_ok() {
        let diags = typecheck_source(
            "enum Shape { Empty, Circle(number) }\nlet c = Shape.Circle(5)",
        );
        let mismatch: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("argument") && d.message.contains("type mismatch"))
            .collect();
        assert!(
            mismatch.is_empty(),
            "well-typed data-variant construction should not warn: {:?}",
            mismatch
        );
    }

    #[test]
    fn test_enum_match_non_exhaustive() {
        // Annotated enum param: matching its discriminant without covering all
        // variants (and no else/catch-all) is a static error.
        let diags = typecheck_source(
            "enum Color { Red, Green, Blue }\nfn f(c: Color) { return match c.__variant { case \"Red\": 1; case \"Green\": 2; }; }",
        );
        let errs: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("non-exhaustive") && d.message.contains("Blue"))
            .collect();
        assert!(
            !errs.is_empty(),
            "expected non-exhaustive match naming Blue, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_enum_match_exhaustive_ok() {
        let diags = typecheck_source(
            "enum Color { Red, Green, Blue }\nfn f(c: Color) { return match c.__variant { case \"Red\": 1; case \"Green\": 2; case \"Blue\": 3; }; }",
        );
        let errs: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("non-exhaustive"))
            .collect();
        assert!(errs.is_empty(), "exhaustive match should not warn: {:?}", errs);
    }

    #[test]
    fn test_enum_match_else_is_exhaustive() {
        let diags = typecheck_source(
            "enum Color { Red, Green, Blue }\nfn f(c: Color) { return match c.__variant { case \"Red\": 1; else: 0 }; }",
        );
        let errs: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("non-exhaustive"))
            .collect();
        assert!(errs.is_empty(), "match with else is exhaustive: {:?}", errs);
    }

    #[test]
    fn test_enum_match_let_bound_instance() {
        // Inference path: a let-bound enum value is an instance, no annotation needed.
        let diags = typecheck_source(
            "enum Color { Red, Green, Blue }\nlet c = Color.Red\nlet r = match c.__variant { case \"Red\": 1; case \"Green\": 2; }",
        );
        let errs: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("non-exhaustive") && d.message.contains("Blue"))
            .collect();
        assert!(
            !errs.is_empty(),
            "expected non-exhaustive on let-bound enum, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_enum_match_unannotated_no_false_positive() {
        // Unannotated param is `any` → enum unknown → no exhaustiveness check.
        let diags = typecheck_source(
            "enum Color { Red, Green, Blue }\nfn f(c) { return match c.__variant { case \"Red\": 1; }; }",
        );
        let errs: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("non-exhaustive"))
            .collect();
        assert!(
            errs.is_empty(),
            "unannotated scrutinee must not trigger exhaustiveness: {:?}",
            errs
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
        let diags =
            typecheck_source("let m = import(\"stdlib/compiler/tokenizer.sapp\");\nprint len(m);");
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
    fn test_builtin_prelude_thread_and_json_helpers() {
        let diags = typecheck_source(
            r#"
            let ch = channel();
            spawn(fn() {
                send(ch.sender, "hello");
            });
            let msg = recv(ch.receiver);
            print json_stringify(msg);
            "#,
        );
        let undeclared: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("not declared"))
            .collect();
        assert!(
            undeclared.is_empty(),
            "thread/json builtins should be known: {:?}",
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
        let diags = typecheck_source("let f = fn(x: number) -> number { return x + \"oops\"; };");
        assert!(
            diags.iter().any(|d| d.message.contains("type mismatch")),
            "fn literal body should be type-checked: {:?}",
            diags
        );
    }
}
