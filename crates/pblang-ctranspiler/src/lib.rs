//! pblang → C transpiler.
//!
//! Translates pblang AST (parsed with pblang-core) into portable C99 code
//! that can be compiled with GCC, Clang, or any C99-compliant compiler.
//!
//! # Architecture
//!
//! ```text
//! .sapp source
//!     │
//!     ├─ pblang-core::parse()  →  Vec<Stmt> (AST)
//!     │
//!     └─ ctranspile()  →  String (C code)
//!                              │
//!                         pblang_rt.h (bundled runtime)
//!                              │
//!                         gcc -o output input.c
//! ```
//!
//! # Generated C structure
//!
//! ```c
//! #include "pblang_rt.h"
//!
//! // User-defined functions
//! PbVal pb_fn_greet(PbVal name) { ... }
//!
//! int main(void) {
//!     pb_init();          // initialize GC + globals
//!     // top-level statements
//!     pb_print(pb_fn_greet(PB_STR("World")));
//!     pb_gc_shutdown();
//!     return 0;
//! }
//! ```
//!
//! # Supported features
//!
//! | Feature                     | Supported |
//! |-----------------------------|-----------|
//! | Variables (let, const)      | ✅        |
//! | Arithmetic (+,-,*,/,%,**)   | ✅        |
//! | Comparisons (==,!=,<,>,<=,>=)| ✅       |
//! | Logical (and, or, not)      | ✅        |
//! | If/elif/else                | ✅        |
//! | While loops                 | ✅        |
//! | For i N loops               | ✅        |
//! | For-in array loops          | ✅        |
//! | Functions (fn)              | ✅        |
//! | Default parameters          | ✅        |
//! | Variadic functions (...args) | ✅       |
//! | Return                      | ✅        |
//! | Break/Continue              | ✅        |
//! | Arrays ([1,2,3])            | ✅        |
//! | Maps (map{k:v})             | ✅        |
//! | Closures (fn() captures env)| ⚠️ basic  |
//! | Try/catch                   | ✅        |
//! | Throw                       | ✅        |
//! | Print                       | ✅        |
//! | Nil/Bool/Number/String      | ✅        |
//! | Augmented assignment (+=)   | ✅        |
//! | Destructuring               | ✅ (as Seq)|
//! | Import/Export               | ❌ (future)|
//! | Concurrency (spawn/channel) | ❌ (future)|

use pblang_core::{ExprKind, FStringPart, LogicalOp, MatchArm, Stmt, StmtKind};

/// The pblang C runtime header — embedded in every generated C file.
/// This header defines PbVal, memory management, and all builtin operations.
pub const PBLANG_RT_H: &str = include_str!("pblang_rt.h");

/// Options for the C transpiler.
#[derive(Debug, Clone, Default)]
pub struct TranspileOptions {
    /// Embed the full runtime header inline (for single-file compilation).
    /// When false, generates `#include "pblang_rt.h"` instead.
    pub embed_runtime: bool,
    /// Add comments mapping C lines back to pblang source spans.
    pub source_maps: bool,
    /// Optimization level hint (passed through to the C compiler).
    pub opt_level: u8,
}

/// Result of transpilation: generated C source code.
#[derive(Debug)]
pub struct TranspileOutput {
    /// The generated C source code.
    pub c_source: String,
    /// Any warnings generated during transpilation.
    pub warnings: Vec<String>,
}

/// Transpile pblang AST statements to C99 source code.
pub fn transpile(stmts: Vec<Stmt>, opts: &TranspileOptions) -> TranspileOutput {
    let mut ctx = TranspileCtx::new(opts.clone());
    ctx.transpile_program(stmts);
    let warnings = ctx.warnings.clone();
    TranspileOutput {
        c_source: ctx.finish(),
        warnings,
    }
}

// ─── Internal context ──────────────────────────────────────────────────────────

struct TranspileCtx {
    opts: TranspileOptions,
    /// Forward declarations for all user functions.
    forward_decls: String,
    /// Function definitions.
    functions: String,
    /// Main function body.
    main_body: String,
    /// Indent level for current output target.
    indent: usize,
    /// Counter for generating unique temp variable names.
    tmp_counter: usize,
    /// Current output target: "main" | "fn_body"
    target: OutputTarget,
    /// Current function body buffer (when inside a function).
    fn_buf: String,
    pub warnings: Vec<String>,
}

#[derive(PartialEq)]
enum OutputTarget {
    Main,
    Function,
}

impl TranspileCtx {
    fn new(opts: TranspileOptions) -> Self {
        TranspileCtx {
            opts,
            forward_decls: String::new(),
            functions: String::new(),
            main_body: String::new(),
            indent: 1,
            tmp_counter: 0,
            target: OutputTarget::Main,
            fn_buf: String::new(),
            warnings: Vec::new(),
        }
    }

    fn indent_str(&self) -> String {
        "    ".repeat(self.indent)
    }

    fn emit(&mut self, s: &str) {
        let ind = self.indent_str();
        let line = format!("{}{}\n", ind, s);
        match self.target {
            OutputTarget::Main => self.main_body.push_str(&line),
            OutputTarget::Function => self.fn_buf.push_str(&line),
        }
    }

    #[allow(dead_code)]
    fn emit_raw(&mut self, s: &str) {
        match self.target {
            OutputTarget::Main => self.main_body.push_str(s),
            OutputTarget::Function => self.fn_buf.push_str(s),
        }
    }

    fn fresh_tmp(&mut self) -> String {
        self.tmp_counter += 1;
        format!("__pb_t{}", self.tmp_counter)
    }

    fn transpile_program(&mut self, stmts: Vec<Stmt>) {
        // First pass: collect all top-level fn declarations for forward decls
        for s in &stmts {
            if let StmtKind::Fn(name, params, _ret, _body) = &s.kind {
                let arity = params.len();
                let param_str = (0..arity)
                    .map(|i| format!("PbVal a{i}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.forward_decls
                    .push_str(&format!("PbVal pb_fn_{name}({param_str});\n"));
            }
        }

        // Second pass: emit code
        for s in stmts {
            self.emit_stmt(s);
        }
    }

    fn emit_stmt(&mut self, stmt: Stmt) {
        match stmt.kind {
            StmtKind::Let(name, _ty, init) => {
                let rhs = self.emit_expr_str(&init.kind);
                self.emit(&format!("PbVal {name} = {rhs};"));
            }
            StmtKind::Const(name, _ty, init) => {
                let rhs = self.emit_expr_str(&init.kind);
                self.emit(&format!("PbVal {name} = {rhs}; /* const */"));
            }
            StmtKind::Assign(name, expr) => {
                let rhs = self.emit_expr_str(&expr.kind);
                self.emit(&format!("{name} = {rhs};"));
            }
            StmtKind::SetAttr(obj, attr, val) => {
                let obj_s = self.emit_expr_str(&obj.kind);
                let val_s = self.emit_expr_str(&val.kind);
                self.emit(&format!(
                    "pb_map_set({obj_s}, PB_STR(\"{attr}\"), {val_s});"
                ));
            }
            StmtKind::SetIndex(obj, idx, val) => {
                let obj_s = self.emit_expr_str(&obj.kind);
                let idx_s = self.emit_expr_str(&idx.kind);
                let val_s = self.emit_expr_str(&val.kind);
                self.emit(&format!("pb_arr_set({obj_s}, {idx_s}, {val_s});"));
            }
            StmtKind::Print(e) => {
                let s = self.emit_expr_str(&e.kind);
                self.emit(&format!("pb_print({s});"));
            }
            StmtKind::PrintF(_, e) => {
                let s = self.emit_expr_str(&e.kind);
                self.emit(&format!("pb_printf({s});"));
            }
            StmtKind::If(cond, then_body, else_body) => {
                let cond_s = self.emit_expr_str(&cond.kind);
                self.emit(&format!("if (pb_truthy({cond_s})) {{"));
                self.indent += 1;
                for s in then_body {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                if let Some(else_stmts) = else_body {
                    self.emit("} else {");
                    self.indent += 1;
                    for s in else_stmts {
                        self.emit_stmt(s);
                    }
                    self.indent -= 1;
                }
                self.emit("}");
            }
            StmtKind::While(cond, body) => {
                let cond_s = self.emit_expr_str(&cond.kind);
                self.emit(&format!("while (pb_truthy({cond_s})) {{"));
                self.indent += 1;
                for s in body {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                self.emit("}");
            }
            StmtKind::For(var, count, body) => {
                let cnt = self.emit_expr_str(&count.kind);
                let tmp_n = self.fresh_tmp();
                self.emit(&format!("{{ PbVal {tmp_n} = {cnt}; for (PbVal {var} = PB_NUM(0.0); pb_lt({var}, {tmp_n}); {var} = PB_NUM(pb_num({var}) + 1.0)) {{"));
                self.indent += 1;
                for s in body {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                self.emit("}}");
            }
            StmtKind::ForIn(var, col, body) => {
                let col_s = self.emit_expr_str(&col.kind);
                let tmp_col = self.fresh_tmp();
                let tmp_i = self.fresh_tmp();
                self.emit(&format!("{{ PbVal {tmp_col} = {col_s}; PbVal {tmp_i}; for (size_t __i_{tmp_i} = 0; __i_{tmp_i} < pb_arr_len({tmp_col}); __i_{tmp_i}++) {{"));
                self.indent += 1;
                self.emit(&format!(
                    "PbVal {var} = pb_arr_get({tmp_col}, PB_NUM((double)__i_{tmp_i}));"
                ));
                for s in body {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                self.emit("}}");
            }
            StmtKind::ForInKV(k, v, col, body) => {
                let col_s = self.emit_expr_str(&col.kind);
                let tmp_col = self.fresh_tmp();
                let tmp_i = self.fresh_tmp();
                self.emit(&format!("{{ PbVal {tmp_col} = {col_s}; PbVal __keys_{tmp_i} = pb_map_keys({tmp_col}); for (size_t __i_{tmp_i} = 0; __i_{tmp_i} < pb_arr_len(__keys_{tmp_i}); __i_{tmp_i}++) {{"));
                self.indent += 1;
                self.emit(&format!(
                    "PbVal {k} = pb_arr_get(__keys_{tmp_i}, PB_NUM((double)__i_{tmp_i}));"
                ));
                self.emit(&format!("PbVal {v} = pb_map_get({tmp_col}, {k});"));
                for s in body {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                self.emit("}}");
            }
            StmtKind::Loop(body) => {
                self.emit("for (;;) {");
                self.indent += 1;
                for s in body {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                self.emit("}");
            }
            StmtKind::Break => {
                self.emit("break;");
            }
            StmtKind::Continue => {
                self.emit("continue;");
            }
            StmtKind::Return(None) => {
                self.emit("return PB_NIL;");
            }
            StmtKind::Return(Some(e)) => {
                let s = self.emit_expr_str(&e.kind);
                self.emit(&format!("return {s};"));
            }
            StmtKind::Throw(e) => {
                let s = self.emit_expr_str(&e.kind);
                self.emit(&format!("pb_throw({s});"));
            }
            StmtKind::Defer(e) => {
                // Defer: generate cleanup code at end of scope
                // For now, warn and generate best-effort
                self.warnings
                    .push("defer: basic C implementation (no scope tracking)".into());
                let s = self.emit_expr_str(&e.kind);
                self.emit(&format!("/* defer */ pb_defer_push({s});"));
            }
            StmtKind::Fn(name, params, _ret, body) => {
                let is_vararg = params.last().map(|p| p.is_vararg).unwrap_or(false);
                let named_arity = if is_vararg {
                    params.len().saturating_sub(1)
                } else {
                    params.len()
                };

                // Generate function signature
                let param_str = (0..named_arity)
                    .map(|i| format!("PbVal a{i}"))
                    .chain(if is_vararg {
                        Some("PbVal __varargs".to_string())
                    } else {
                        None
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                // Save and switch to function output target
                let prev_buf = std::mem::take(&mut self.fn_buf);
                let prev_target = std::mem::replace(&mut self.target, OutputTarget::Function);
                let prev_indent = self.indent;
                self.indent = 1;

                // Function header
                self.fn_buf
                    .push_str(&format!("PbVal pb_fn_{name}({param_str}) {{\n"));

                // Bind named params
                for (i, p) in params.iter().enumerate().take(named_arity) {
                    let default_expr = p
                        .default
                        .as_ref()
                        .map(|d| self.emit_expr_str(&d.kind))
                        .unwrap_or_else(|| "PB_NIL".to_string());
                    if p.default.is_some() {
                        self.emit(&format!(
                            "PbVal {pn} = pb_is_nil(a{i}) ? {default_expr} : a{i};",
                            pn = p.name
                        ));
                    } else {
                        self.emit(&format!("PbVal {pn} = a{i};", pn = p.name));
                    }
                }

                // Vararg binding
                if is_vararg {
                    if let Some(vp) = params.last() {
                        self.emit(&format!("PbVal {vname} = __varargs;", vname = vp.name));
                    }
                }

                // Function body
                for s in body {
                    self.emit_stmt(s);
                }
                self.emit("return PB_NIL; /* fallthrough */");

                self.fn_buf.push_str("}\n\n");
                let fn_code = std::mem::replace(&mut self.fn_buf, prev_buf);
                self.functions.push_str(&fn_code);
                self.target = prev_target;
                self.indent = prev_indent;
            }
            StmtKind::Block(stmts) => {
                self.emit("{");
                self.indent += 1;
                for s in stmts {
                    self.emit_stmt(s);
                }
                self.indent -= 1;
                self.emit("}");
            }
            StmtKind::Seq(stmts) => {
                // Flat sequence — no new scope in C either
                for s in stmts {
                    self.emit_stmt(s);
                }
            }
            StmtKind::Expr(e) => {
                let s = self.emit_expr_str(&e.kind);
                self.emit(&format!("{s};"));
            }
            StmtKind::Import(_) | StmtKind::Export(_) => {
                self.warnings
                    .push("import/export not supported in C transpiler".into());
            }
        }
    }

    fn emit_expr_str(&mut self, kind: &ExprKind) -> String {
        match kind {
            ExprKind::Nil => "PB_NIL".to_string(),
            ExprKind::Bool(b) => {
                if *b {
                    "PB_TRUE".to_string()
                } else {
                    "PB_FALSE".to_string()
                }
            }
            ExprKind::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("PB_NUM({:.1})", n)
                } else {
                    format!("PB_NUM({:?})", n)
                }
            }
            ExprKind::Str(s) => {
                let escaped = s
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r")
                    .replace('\t', "\\t");
                format!("PB_STR(\"{}\")", escaped)
            }
            ExprKind::Var(name) => {
                // Check if it's a builtin
                match name.as_str() {
                    "len" => "pb_builtin_len".to_string(),
                    "str" => "pb_builtin_str".to_string(),
                    "type" => "pb_builtin_type".to_string(),
                    "print" => "pb_builtin_print".to_string(),
                    "array_push" => "pb_builtin_array_push".to_string(),
                    "array_pop" => "pb_builtin_array_pop".to_string(),
                    "map_keys" => "pb_builtin_map_keys".to_string(),
                    "json_parse" => "pb_builtin_json_parse".to_string(),
                    "json_stringify" => "pb_builtin_json_stringify".to_string(),
                    "sha256" => "pb_builtin_sha256".to_string(),
                    "sleep_ms" => "pb_builtin_sleep_ms".to_string(),
                    "clock" => "pb_builtin_clock".to_string(),
                    "now_ms" => "pb_builtin_now_ms".to_string(),
                    "parse_number" => "pb_builtin_parse_number".to_string(),
                    "num_sqrt" => "pb_builtin_num_sqrt".to_string(),
                    "num_floor" => "pb_builtin_num_floor".to_string(),
                    "num_ceil" => "pb_builtin_num_ceil".to_string(),
                    "num_abs" => "pb_builtin_num_abs".to_string(),
                    "num_pow" => "pb_builtin_num_pow".to_string(),
                    "str_contains" => "pb_builtin_str_contains".to_string(),
                    "str_starts_with" => "pb_builtin_str_starts_with".to_string(),
                    "str_ends_with" => "pb_builtin_str_ends_with".to_string(),
                    "str_slice" => "pb_builtin_str_slice".to_string(),
                    "str_split" => "pb_builtin_str_split".to_string(),
                    "str_trim" => "pb_builtin_str_trim".to_string(),
                    "str_upper" => "pb_builtin_str_upper".to_string(),
                    "str_lower" => "pb_builtin_str_lower".to_string(),
                    "str_replace_all" => "pb_builtin_str_replace_all".to_string(),
                    "assert" => "pb_builtin_assert".to_string(),
                    "error" => "pb_builtin_error".to_string(),
                    // User-defined function reference
                    _ => format!("pb_fn_{name}"),
                }
            }
            ExprKind::Add(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_add({a}, {b})")
            }
            ExprKind::Sub(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_sub({a}, {b})")
            }
            ExprKind::Mul(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_mul({a}, {b})")
            }
            ExprKind::Div(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_div({a}, {b})")
            }
            ExprKind::Mod(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_mod({a}, {b})")
            }
            ExprKind::Pow(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_pow({a}, {b})")
            }
            ExprKind::Eq(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_eq({a}, {b})")
            }
            ExprKind::NotEq(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_not(pb_eq({a}, {b}))")
            }
            ExprKind::Lt(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_lt({a}, {b})")
            }
            ExprKind::Le(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_le({a}, {b})")
            }
            ExprKind::Gt(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_gt({a}, {b})")
            }
            ExprKind::Ge(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_ge({a}, {b})")
            }
            ExprKind::Not(e) => {
                let s = self.emit_expr_str(&e.kind);
                format!("pb_not({s})")
            }
            ExprKind::Logical { left, op, right } => {
                let l = self.emit_expr_str(&left.kind);
                let r = self.emit_expr_str(&right.kind);
                match op {
                    LogicalOp::And => format!("pb_and({l}, {r})"),
                    LogicalOp::Or => format!("pb_or({l}, {r})"),
                }
            }
            ExprKind::NullCoalesce(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_coalesce({a}, {b})")
            }
            ExprKind::Call(callee, args) => {
                let callee_s = self.emit_expr_str(&callee.kind);
                let args_s = args
                    .iter()
                    .map(|a| self.emit_expr_str(&a.kind))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{callee_s}({args_s})")
            }
            ExprKind::GetAttr(obj, attr) => {
                let obj_s = self.emit_expr_str(&obj.kind);
                format!("pb_map_get({obj_s}, PB_STR(\"{attr}\"))")
            }
            ExprKind::SafeGetAttr(obj, attr) => {
                let obj_s = self.emit_expr_str(&obj.kind);
                format!("pb_safe_map_get({obj_s}, PB_STR(\"{attr}\"))")
            }
            ExprKind::Index(obj, idx) => {
                let obj_s = self.emit_expr_str(&obj.kind);
                let idx_s = self.emit_expr_str(&idx.kind);
                format!("pb_index({obj_s}, {idx_s})")
            }
            ExprKind::SafeIndex(obj, idx) => {
                let obj_s = self.emit_expr_str(&obj.kind);
                let idx_s = self.emit_expr_str(&idx.kind);
                format!("pb_safe_index({obj_s}, {idx_s})")
            }
            ExprKind::Array(items) => {
                let n = items.len();
                if n == 0 {
                    "pb_arr_new(0)".to_string()
                } else {
                    let args = items
                        .iter()
                        .map(|a| self.emit_expr_str(&a.kind))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("pb_arr_lit({n}, {args})")
                }
            }
            ExprKind::Map(pairs) => {
                let n = pairs.len();
                if n == 0 {
                    "pb_map_new()".to_string()
                } else {
                    let args = pairs
                        .iter()
                        .flat_map(|(k, v)| {
                            let k_s = format!("PB_STR(\"{k}\")");
                            let v_s = self.emit_expr_str(&v.kind);
                            vec![k_s, v_s]
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("pb_map_lit({n}, {args})")
                }
            }
            ExprKind::Try(try_expr, catch_expr) => {
                let t = self.emit_expr_str(&try_expr.kind);
                let c = self.emit_expr_str(&catch_expr.kind);
                format!("pb_try({t}, {c})")
            }
            ExprKind::FnLit(name, params, _ret, body) => {
                // Inline anonymous functions: generate a named C function
                let fn_name = name
                    .clone()
                    .unwrap_or_else(|| format!("__anon_{}", self.fresh_tmp()));
                let c_fn_name = format!("pb_fn_{fn_name}");
                let arity = params.len();
                let param_str = (0..arity)
                    .map(|i| format!("PbVal a{i}"))
                    .collect::<Vec<_>>()
                    .join(", ");

                // Emit function into functions buffer
                let prev_buf = std::mem::take(&mut self.fn_buf);
                let prev_target = std::mem::replace(&mut self.target, OutputTarget::Function);
                let prev_indent = self.indent;
                self.indent = 1;

                self.fn_buf
                    .push_str(&format!("static PbVal {c_fn_name}({param_str}) {{\n"));
                for (i, p) in params.iter().enumerate() {
                    self.emit(&format!("PbVal {pn} = a{i};", pn = p.name));
                }
                for s in body.iter().cloned() {
                    self.emit_stmt(s);
                }
                self.emit("return PB_NIL;");
                self.fn_buf.push_str("}\n\n");

                let fn_code = std::mem::replace(&mut self.fn_buf, prev_buf);
                self.functions.push_str(&fn_code);
                self.target = prev_target;
                self.indent = prev_indent;

                // Return function pointer as PbVal
                format!("PB_FN({c_fn_name})")
            }
            ExprKind::IfExpr(cond, then_br, else_br) => {
                let cond_s = self.emit_expr_str(&cond.kind);
                let tmp = self.fresh_tmp();
                self.emit(&format!("PbVal {tmp} = PB_NIL;"));
                self.emit(&format!("if (pb_truthy({cond_s})) {{"));
                self.indent += 1;
                let then_s = self.emit_expr_str(&then_br.kind);
                self.emit(&format!("{tmp} = {then_s};"));
                self.indent -= 1;
                self.emit("} else {");
                self.indent += 1;
                let else_s = self.emit_expr_str(&else_br.kind);
                self.emit(&format!("{tmp} = {else_s};"));
                self.indent -= 1;
                self.emit("}");
                tmp
            }
            ExprKind::BlockExpr(stmts, tail) => {
                // Emit as a statement block with a result variable
                let tmp = self.fresh_tmp();
                self.emit(&format!("PbVal {tmp} = PB_NIL;"));
                self.emit("{");
                self.indent += 1;
                for s in stmts.iter().cloned() {
                    self.emit_stmt(s);
                }
                if let Some(tail_expr) = tail {
                    let tail_s = self.emit_expr_str(&tail_expr.kind);
                    self.emit(&format!("{tmp} = {tail_s};"));
                }
                self.indent -= 1;
                self.emit("}");
                tmp
            }
            ExprKind::Match(scrutinee, arms, else_expr) => {
                let sc = self.emit_expr_str(&scrutinee.kind);
                let tmp = self.fresh_tmp();
                let sc_tmp = self.fresh_tmp();
                self.emit(&format!("PbVal {sc_tmp} = {sc};"));
                self.emit(&format!("PbVal {tmp} = PB_NIL;"));
                let mut first = true;
                for (arm, body_expr) in arms {
                    let body_s = self.emit_expr_str(&body_expr.kind);
                    let cond = match arm {
                        MatchArm::Literal(lit) => {
                            let lit_s = self.emit_expr_str(&lit.kind);
                            format!("pb_truthy(pb_eq({sc_tmp}, {lit_s}))")
                        }
                        MatchArm::Wildcard => "1".to_string(),
                        MatchArm::Bind(name) => {
                            // Bind variable to scrutinee
                            format!("({name} = {sc_tmp}, 1)")
                        }
                        MatchArm::BindGuard(name, guard) => {
                            let g = self.emit_expr_str(&guard.kind);
                            format!("({name} = {sc_tmp}, pb_truthy({g}))")
                        }
                    };
                    if first {
                        self.emit(&format!("if ({cond}) {{ {tmp} = {body_s}; }}"));
                        first = false;
                    } else {
                        self.emit(&format!("else if ({cond}) {{ {tmp} = {body_s}; }}"));
                    }
                }
                if let Some(else_e) = else_expr {
                    let else_s = self.emit_expr_str(&else_e.kind);
                    self.emit(&format!("else {{ {tmp} = {else_s}; }}"));
                }
                tmp
            }
            ExprKind::FString(parts) => {
                // Build string from parts
                let mut pieces: Vec<String> = Vec::new();
                for part in parts {
                    match part {
                        FStringPart::Literal(s) => {
                            let escaped = s.replace('"', "\\\"");
                            pieces.push(format!("PB_STR(\"{escaped}\")"));
                        }
                        FStringPart::Interp(e) => {
                            let s = self.emit_expr_str(&e.kind);
                            pieces.push(format!("pb_to_str({s})"));
                        }
                    }
                }
                if pieces.is_empty() {
                    "PB_STR(\"\")".to_string()
                } else {
                    let n = pieces.len();
                    let args = pieces.join(", ");
                    format!("pb_str_concat_n({n}, {args})")
                }
            }
            ExprKind::ResultUnwrap(e) => {
                let s = self.emit_expr_str(&e.kind);
                format!("pb_result_unwrap({s})")
            }
        }
    }

    fn finish(self) -> String {
        let mut out = String::new();

        if self.opts.embed_runtime {
            out.push_str(PBLANG_RT_H);
            out.push('\n');
        } else {
            out.push_str("#include \"pblang_rt.h\"\n\n");
        }

        // Forward declarations
        if !self.forward_decls.is_empty() {
            out.push_str("/* Forward declarations */\n");
            out.push_str(&self.forward_decls);
            out.push('\n');
        }

        // Function definitions
        if !self.functions.is_empty() {
            out.push_str("/* Function definitions */\n");
            out.push_str(&self.functions);
        }

        // Main function
        out.push_str("int main(void) {\n");
        out.push_str("    pb_init();\n");
        out.push_str(&self.main_body);
        out.push_str("    pb_shutdown();\n");
        out.push_str("    return 0;\n");
        out.push_str("}\n");

        out
    }
}

/// Transpile pblang source code string directly to self-contained C.
///
/// This is a convenience wrapper around parse + transpile.
/// The generated C includes the embedded pblang runtime header.
pub fn transpile_to_c(pblang_source: &str) -> Result<TranspileOutput, String> {
    let stmts = pblang_core::parse_strict(pblang_source)
        .map_err(|errs| format!("parse errors: {:?}", errs))?;
    let opts = TranspileOptions {
        embed_runtime: true,
        source_maps: false,
        opt_level: 2,
    };
    Ok(transpile(stmts, &opts))
}

/// Compile C source code to a native executable using the system C compiler.
///
/// Returns `Ok(())` on success, or `Err(message)` on failure.
/// The output executable name is platform-appropriate (appends .exe on Windows).
pub fn compile_from_c(c_source: &str, output_name: &str) -> Result<(), String> {
    // Write C source to a temp file
    let c_path = std::path::PathBuf::from(format!("{output_name}.c"));
    std::fs::write(&c_path, c_source)
        .map_err(|e| format!("write C source '{cpath}': {e}", cpath = c_path.display()))?;

    // Find a C compiler
    let cc = find_ctranspiler_cc()?;

    let exe_name = if cfg!(target_os = "windows") {
        format!("{output_name}.exe")
    } else {
        output_name.to_string()
    };

    let status = std::process::Command::new(&cc)
        .arg(&c_path)
        .arg("-o")
        .arg(&exe_name)
        .arg("-O2")
        .arg("-std=c99")
        .arg("-lm")
        .status()
        .map_err(|e| format!("run C compiler '{cc}': {e}"))?;

    // Clean up the .c file on success
    if status.success() {
        let _ = std::fs::remove_file(&c_path);
        Ok(())
    } else {
        Err(format!("C compiler '{cc}' failed with exit code: {status}"))
    }
}

/// Find an available C compiler for the transpiler.
fn find_ctranspiler_cc() -> Result<String, String> {
    for cc in &["gcc", "clang", "cc"] {
        if std::process::Command::new(cc)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
        {
            return Ok(cc.to_string());
        }
    }
    Err("no C compiler found (tried gcc, clang, cc)".to_string())
}

/// Check if C compilation is available on this system.
pub fn is_c_compiler_available() -> bool {
    find_ctranspiler_cc().is_ok()
}
