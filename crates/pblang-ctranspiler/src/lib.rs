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
//! | Closures (fn() captures env)| ✅        |
//! | First-class functions       | ✅        |
//! | Try/catch                   | ✅        |
//! | Throw                       | ✅        |
//! | Print                       | ✅        |
//! | Nil/Bool/Number/String      | ✅        |
//! | Augmented assignment (+=)   | ✅        |
//! | Destructuring               | ✅ (as Seq)|
//! | Import/Export               | ❌ (future)|
//! | Concurrency (spawn/channel) | ❌ (future)|

use std::collections::HashMap;
use std::collections::HashSet;

use pblang_core::{ExprKind, FStringPart, LogicalOp, MatchArm, Param, Stmt, StmtKind};

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
    /// Top-level user function names collected before emission.
    function_names: HashSet<String>,
    /// Top-level user function arities (named param count), for closure wrappers.
    function_arities: HashMap<String, usize>,
    /// Lexical variable scopes for generated PbVal variables.
    scopes: Vec<HashSet<String>>,
    /// Variables boxed as shared single-element arrays (heap cells): a local that
    /// is captured by a nested closure AND mutated. Reads -> pb_index(x,0),
    /// writes -> pb_arr_set(x,0,v); the box is captured by reference so all
    /// closures + the enclosing scope share one mutable cell (VM upvalue parity).
    boxed: HashSet<String>,
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
            function_names: HashSet::new(),
            function_arities: HashMap::new(),
            scopes: vec![HashSet::new()],
            boxed: HashSet::new(),
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

    fn push_scope(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn declare_var(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn is_local_var(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }

    /// C expression that READS a local variable (a boxed var dereferences its cell).
    fn read_var(&self, name: &str) -> String {
        if self.boxed.contains(name) {
            format!("pb_index({name}, PB_NUM(0.0))")
        } else {
            name.to_string()
        }
    }

    /// Variables a closure body references from the enclosing scope, in
    /// first-seen order. These are captured by value. Names bound inside the
    /// closure (its params, and its own let/for/fn bindings) are excluded, and
    /// only names that are locals in the CURRENT (enclosing) scope are kept —
    /// builtins and top-level functions are not captured (they're global in C).
    fn compute_captures(&self, params: &[Param], body: &[Stmt]) -> Vec<String> {
        let mut bound: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut out: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        walk_stmts(body, &mut bound, &mut out, &mut seen);
        out.into_iter().filter(|n| self.is_local_var(n)).collect()
    }

    /// Emit a function-literal / closure body and return the C expression that
    /// constructs its value. Captures enclosing locals by value; a captured var
    /// that is MUTATED inside the body aliases its env slot so the mutation
    /// persists across the closure's calls (by-reference-style). Captureless
    /// closures become static singletons (no heap allocation).
    fn emit_closure(&mut self, params: &[Param], body: &[Stmt]) -> String {
        let arity = params.len();
        // Capture analysis happens in the ENCLOSING scope (before we switch).
        let captures = self.compute_captures(params, body);
        // Env is built NOW, in the parent: a boxed parent var's C variable already
        // holds the shared cell array, so the raw name shares it; a plain capture
        // copies the current value. Either way it's just the variable name.
        let env_exprs: Vec<String> = captures.iter().map(|c| c.clone()).collect();
        // Which captures are boxed in the PARENT -> they stay boxed in the child.
        let inherited: Vec<String> = captures
            .iter()
            .filter(|c| self.boxed.contains(*c))
            .cloned()
            .collect();

        // The child's own boxed locals + inherited boxed captures.
        let mut child_boxed = compute_boxed(params, body);
        for c in &inherited {
            child_boxed.insert(c.clone());
        }

        let id = self.fresh_tmp();
        let c_fn_name = format!("pb_clo{id}");
        self.forward_decls
            .push_str(&format!("static PbVal {c_fn_name}(PbVal*, PbVal*, size_t);\n"));

        let prev_buf = std::mem::take(&mut self.fn_buf);
        let prev_target = std::mem::replace(&mut self.target, OutputTarget::Function);
        let prev_indent = self.indent;
        let prev_scopes = std::mem::replace(&mut self.scopes, vec![HashSet::new()]);
        let prev_boxed = std::mem::replace(&mut self.boxed, child_boxed);
        self.indent = 1;

        self.fn_buf.push_str(&format!(
            "static PbVal {c_fn_name}(PbVal* __env, PbVal* __args, size_t __nargs) {{\n"
        ));
        self.emit("(void)__env; (void)__args; (void)__nargs;");
        for (i, p) in params.iter().enumerate() {
            let value = format!("(__nargs > {i}) ? __args[{i}] : PB_NIL");
            if self.boxed.contains(&p.name) {
                self.emit(&format!("PbVal {pn} = pb_arr_lit(1, {value});", pn = p.name));
            } else {
                self.emit(&format!("PbVal {pn} = {value};", pn = p.name));
            }
            self.declare_var(&p.name);
        }
        for (j, cap) in captures.iter().enumerate() {
            // Captured value (a plain value, or — for boxed vars — the shared cell).
            self.emit(&format!("PbVal {cap} = __env[{j}];"));
            self.declare_var(cap);
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
        self.scopes = prev_scopes;
        self.boxed = prev_boxed;

        if captures.is_empty() {
            self.forward_decls.push_str(&format!(
                "static PbClosure pb_clov{id} = {{ {c_fn_name}, {arity}, 0, NULL }};\n"
            ));
            format!("pb_fn_as_val(&pb_clov{id})")
        } else {
            let env = env_exprs.join(", ");
            let k = captures.len();
            format!("pb_make_closure((PbFnPtr){c_fn_name}, {arity}, (PbVal[]){{{env}}}, {k})")
        }
    }

    fn transpile_program(&mut self, stmts: Vec<Stmt>) {
        // First pass: collect all top-level fn declarations for forward decls
        for s in &stmts {
            if let StmtKind::Fn(name, params, _ret, _body) = &s.kind {
                self.function_names.insert(name.clone());
                let arity = params.len();
                self.function_arities.insert(name.clone(), arity);
                let param_str = (0..arity)
                    .map(|i| format!("PbVal a{i}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.forward_decls
                    .push_str(&format!("PbVal pb_fn_{name}({param_str});\n"));
                // Uniform-ABI wrapper so the function can be used as a value,
                // plus a static closure singleton (captureless -> no malloc).
                let named = if params.last().map(|p| p.is_vararg).unwrap_or(false) {
                    arity.saturating_sub(1)
                } else {
                    arity
                };
                self.forward_decls
                    .push_str(&format!("PbVal pb_fnw_{name}(PbVal*, PbVal*, size_t);\n"));
                self.forward_decls.push_str(&format!(
                    "static PbClosure pb_clov_{name} = {{ pb_fnw_{name}, {named}, 0, NULL }};\n"
                ));
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
                if self.boxed.contains(&name) {
                    self.emit(&format!("PbVal {name} = pb_arr_lit(1, {rhs});"));
                } else {
                    self.emit(&format!("PbVal {name} = {rhs};"));
                }
                self.declare_var(&name);
            }
            StmtKind::Const(name, _ty, init) => {
                let rhs = self.emit_expr_str(&init.kind);
                if self.boxed.contains(&name) {
                    self.emit(&format!("PbVal {name} = pb_arr_lit(1, {rhs}); /* const */"));
                } else {
                    self.emit(&format!("PbVal {name} = {rhs}; /* const */"));
                }
                self.declare_var(&name);
            }
            StmtKind::Assign(name, expr) => {
                let rhs = self.emit_expr_str(&expr.kind);
                if self.boxed.contains(&name) {
                    self.emit(&format!("pb_arr_set({name}, PB_NUM(0.0), {rhs});"));
                } else {
                    self.emit(&format!("{name} = {rhs};"));
                }
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
                self.push_scope();
                for s in then_body {
                    self.emit_stmt(s);
                }
                self.pop_scope();
                self.indent -= 1;
                if let Some(else_stmts) = else_body {
                    self.emit("} else {");
                    self.indent += 1;
                    self.push_scope();
                    for s in else_stmts {
                        self.emit_stmt(s);
                    }
                    self.pop_scope();
                    self.indent -= 1;
                }
                self.emit("}");
            }
            StmtKind::While(cond, body) => {
                let cond_s = self.emit_expr_str(&cond.kind);
                self.emit(&format!("while (pb_truthy({cond_s})) {{"));
                self.indent += 1;
                self.push_scope();
                for s in body {
                    self.emit_stmt(s);
                }
                self.pop_scope();
                self.indent -= 1;
                self.emit("}");
            }
            StmtKind::For(var, count, body) => {
                let cnt = self.emit_expr_str(&count.kind);
                let tmp_n = self.fresh_tmp();
                self.emit(&format!("{{ PbVal {tmp_n} = {cnt}; for (PbVal {var} = PB_NUM(0.0); pb_truthy(pb_lt({var}, {tmp_n})); {var} = PB_NUM({var}.num + 1.0)) {{"));
                self.indent += 1;
                self.push_scope();
                self.declare_var(&var);
                for s in body {
                    self.emit_stmt(s);
                }
                self.pop_scope();
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
                self.push_scope();
                self.declare_var(&var);
                for s in body {
                    self.emit_stmt(s);
                }
                self.pop_scope();
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
                self.push_scope();
                self.declare_var(&k);
                self.declare_var(&v);
                for s in body {
                    self.emit_stmt(s);
                }
                self.pop_scope();
                self.indent -= 1;
                self.emit("}}");
            }
            StmtKind::Loop(body) => {
                self.emit("for (;;) {");
                self.indent += 1;
                self.push_scope();
                for s in body {
                    self.emit_stmt(s);
                }
                self.pop_scope();
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
            StmtKind::Fn(name, params, _ret, body) if self.target == OutputTarget::Function => {
                // A nested function (inside a function body) is a local closure
                // bound to a name. If it is boxed (captured + reassigned, e.g.
                // self-recursive), declare the cell first so the closure can
                // capture the box and call itself through it.
                if self.boxed.contains(&name) {
                    self.emit(&format!("PbVal {name} = pb_arr_lit(1, PB_NIL);"));
                    self.declare_var(&name);
                    let val = self.emit_closure(&params, &body);
                    self.emit(&format!("pb_arr_set({name}, PB_NUM(0.0), {val});"));
                } else {
                    let val = self.emit_closure(&params, &body);
                    self.emit(&format!("PbVal {name} = {val};"));
                    self.declare_var(&name);
                }
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
                let prev_scopes = std::mem::replace(&mut self.scopes, vec![HashSet::new()]);
                let prev_boxed = std::mem::replace(&mut self.boxed, compute_boxed(&params, &body));
                self.indent = 1;

                // Function header
                self.fn_buf
                    .push_str(&format!("PbVal pb_fn_{name}({param_str}) {{\n"));

                // Bind named params (a captured+mutated param is boxed).
                for (i, p) in params.iter().enumerate().take(named_arity) {
                    let default_expr = p
                        .default
                        .as_ref()
                        .map(|d| self.emit_expr_str(&d.kind))
                        .unwrap_or_else(|| "PB_NIL".to_string());
                    let value = if p.default.is_some() {
                        format!("pb_is_nil(a{i}) ? {default_expr} : a{i}")
                    } else {
                        format!("a{i}")
                    };
                    if self.boxed.contains(&p.name) {
                        self.emit(&format!("PbVal {pn} = pb_arr_lit(1, {value});", pn = p.name));
                    } else {
                        self.emit(&format!("PbVal {pn} = {value};", pn = p.name));
                    }
                    self.declare_var(&p.name);
                }

                // Vararg binding
                if is_vararg {
                    if let Some(vp) = params.last() {
                        self.emit(&format!("PbVal {vname} = __varargs;", vname = vp.name));
                        self.declare_var(&vp.name);
                    }
                }

                // Function body
                for s in body {
                    self.emit_stmt(s);
                }
                self.emit("return PB_NIL; /* fallthrough */");

                self.fn_buf.push_str("}\n\n");

                // Uniform-ABI wrapper: lets this function be used as a first-class
                // value (stored, passed, called indirectly via pb_call).
                let mut call_args: Vec<String> = (0..named_arity)
                    .map(|i| format!("(__n > {i}) ? __a[{i}] : PB_NIL"))
                    .collect();
                if is_vararg {
                    call_args.push("pb_arr_new(0)".to_string());
                }
                self.fn_buf.push_str(&format!(
                    "PbVal pb_fnw_{name}(PbVal* __e, PbVal* __a, size_t __n) {{\n    (void)__e; (void)__a; (void)__n;\n    return pb_fn_{name}({});\n}}\n\n",
                    call_args.join(", ")
                ));

                let fn_code = std::mem::replace(&mut self.fn_buf, prev_buf);
                self.functions.push_str(&fn_code);
                self.target = prev_target;
                self.indent = prev_indent;
                self.scopes = prev_scopes;
                self.boxed = prev_boxed;
            }
            StmtKind::Block(stmts) => {
                self.emit("{");
                self.indent += 1;
                self.push_scope();
                for s in stmts {
                    self.emit_stmt(s);
                }
                self.pop_scope();
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
                if let Some(b) = builtin_c_name(name) {
                    b.to_string()
                } else if self.is_local_var(name) {
                    self.read_var(name)
                } else if self.function_names.contains(name) {
                    // A top-level function used as a VALUE -> the static closure
                    // singleton (no heap allocation).
                    format!("pb_fn_as_val(&pb_clov_{name})")
                } else {
                    name.clone()
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
                // Short-circuit: evaluate the left once; the ternary only evaluates
                // the right when needed (a statement-expression keeps it inline and
                // avoids re-evaluating the left). `and` returns left if falsy else
                // right; `or` returns left if truthy else right (matches the VM).
                let l = self.emit_expr_str(&left.kind);
                let r = self.emit_expr_str(&right.kind);
                let t = self.fresh_tmp();
                match op {
                    LogicalOp::And => format!("({{ PbVal {t} = ({l}); pb_truthy({t}) ? ({r}) : {t}; }})"),
                    LogicalOp::Or => format!("({{ PbVal {t} = ({l}); pb_truthy({t}) ? {t} : ({r}); }})"),
                }
            }
            ExprKind::NullCoalesce(a, b) => {
                let a = self.emit_expr_str(&a.kind);
                let b = self.emit_expr_str(&b.kind);
                format!("pb_coalesce({a}, {b})")
            }
            ExprKind::Call(callee, args) => {
                let args_s = args
                    .iter()
                    .map(|a| self.emit_expr_str(&a.kind))
                    .collect::<Vec<_>>()
                    .join(", ");
                // Fast direct path: builtins and top-level functions called by name
                // (not shadowed by a local) compile to a plain positional C call.
                if let ExprKind::Var(name) = &callee.kind {
                    if !self.is_local_var(name) {
                        // A user-defined function shadows a builtin of the same
                        // name (matches the VM), so check function_names FIRST.
                        if self.function_names.contains(name) {
                            // pad omitted trailing args with PB_NIL so default
                            // parameters fill in (the body uses pb_is_nil checks).
                            let arity = self.function_arities.get(name).copied().unwrap_or(args.len());
                            let mut a = args_s.clone();
                            let mut k = args.len();
                            while k < arity {
                                if a.is_empty() { a.push_str("PB_NIL"); } else { a.push_str(", PB_NIL"); }
                                k += 1;
                            }
                            return format!("pb_fn_{name}({a})");
                        }
                        // range() is variadic (1-3 args); dispatch to an
                        // arity-specialized helper so the fast positional path
                        // still applies.
                        if name == "range" && (1..=3).contains(&args.len()) {
                            return format!("pb_builtin_range{}({args_s})", args.len());
                        }
                        if let Some(b) = builtin_c_name(name) {
                            return format!("{b}({args_s})");
                        }
                    }
                }
                // Indirect path: call a closure value through the uniform ABI.
                let callee_s = self.emit_expr_str(&callee.kind);
                let n = args.len();
                if n == 0 {
                    format!("pb_call({callee_s}, NULL, 0)")
                } else {
                    format!("pb_call({callee_s}, (PbVal[]){{{args_s}}}, {n})")
                }
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
            ExprKind::FnLit(_name, params, _ret, body) => self.emit_closure(params, body),
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
                // Pre-declare any bound names: the if-chain assigns to them via the
                // comma-expression condition, and the arm bodies read them, so they
                // must be declared (and in scope) before the chain. Dedupe so a name
                // bound by two arms isn't redeclared.
                let mut declared_binds: std::collections::HashSet<String> = std::collections::HashSet::new();
                for (arm, _) in arms.iter() {
                    if let MatchArm::Bind(name) | MatchArm::BindGuard(name, _) = arm {
                        if declared_binds.insert(name.clone()) {
                            self.declare_var(name);
                            self.emit(&format!("PbVal {name} = PB_NIL;"));
                        }
                    }
                }
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

/// Map a pblang builtin name to its C runtime function, if it is one.
/// Shared by the variable and call emitters so both treat builtins identically.
fn builtin_c_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "len" => "pb_builtin_len",
        "str" => "pb_builtin_str",
        "type" => "pb_builtin_type",
        "print" => "pb_builtin_print",
        "array_push" => "pb_builtin_array_push",
        "push" => "pb_builtin_array_push",
        "array_pop" => "pb_builtin_array_pop",
        "map_keys" => "pb_builtin_map_keys",
        "map_has" => "pb_builtin_map_has",
        "str_index_of" => "pb_builtin_str_index_of",
        "num_min" => "pb_builtin_num_min",
        "num_max" => "pb_builtin_num_max",
        "json_parse" => "pb_builtin_json_parse",
        "json_stringify" => "pb_builtin_json_stringify",
        "sha256" => "pb_builtin_sha256",
        "sleep_ms" => "pb_builtin_sleep_ms",
        "clock" => "pb_builtin_clock",
        "now_ms" => "pb_builtin_now_ms",
        "parse_number" => "pb_builtin_parse_number",
        "num_sqrt" => "pb_builtin_num_sqrt",
        "num_floor" => "pb_builtin_num_floor",
        "num_ceil" => "pb_builtin_num_ceil",
        "num_abs" => "pb_builtin_num_abs",
        "num_pow" => "pb_builtin_num_pow",
        "str_contains" => "pb_builtin_str_contains",
        "str_starts_with" => "pb_builtin_str_starts_with",
        "str_ends_with" => "pb_builtin_str_ends_with",
        "str_slice" => "pb_builtin_str_slice",
        "str_split" => "pb_builtin_str_split",
        "str_trim" => "pb_builtin_str_trim",
        "str_upper" => "pb_builtin_str_upper",
        "str_lower" => "pb_builtin_str_lower",
        "str_replace_all" => "pb_builtin_str_replace_all",
        "str_repeat" => "pb_builtin_str_repeat",
        "str_join" => "pb_builtin_str_join",
        "num_mod" => "pb_builtin_num_mod",
        "clamp" => "pb_builtin_clamp",
        "sum" => "pb_builtin_sum",
        "product" => "pb_builtin_product",
        "min_of" => "pb_builtin_min_of",
        "max_of" => "pb_builtin_max_of",
        "map_remove" => "pb_builtin_map_remove",
        "num_ln" => "pb_builtin_num_ln",
        "num_exp" => "pb_builtin_num_exp",
        "array_sort" => "pb_builtin_array_sort",
        "array_unique" => "pb_builtin_array_unique",
        "str_pad_left" => "pb_builtin_str_pad_left",
        "str_pad_right" => "pb_builtin_str_pad_right",
        "array_take" => "pb_builtin_array_take",
        "array_drop" => "pb_builtin_array_drop",
        "array_chunk" => "pb_builtin_array_chunk",
        "array_flatten" => "pb_builtin_array_flatten",
        "array_zip" => "pb_builtin_array_zip",
        "enumerate" => "pb_builtin_enumerate",
        "assert" => "pb_builtin_assert",
        "error" => "pb_builtin_error",
        _ => return None,
    })
}

// ── Boxing (heap-cell upvalue) analysis ──────────────────────────────────────
// A local is BOXED if it is captured by a nested closure AND assigned somewhere
// (so the mutation must be shared). Boxed locals become single-element arrays.

/// Raw free variables of a closure body (referenced, not bound within it).
/// walk_stmts already recurses into nested closures, so this is transitive.
fn closure_free_vars(params: &[Param], body: &[Stmt]) -> Vec<String> {
    let mut bound: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    walk_stmts(body, &mut bound, &mut out, &mut seen);
    out
}

/// Names captured by nested closures/fns defined directly in `stmts` (this
/// function's scope) — collected WITHOUT descending into those closures.
fn collect_nested_free(stmts: &[Stmt], out: &mut HashSet<String>) {
    for s in stmts {
        match &s.kind {
            StmtKind::Fn(_, params, _, body) => {
                for v in closure_free_vars(params, body) { out.insert(v); }
            }
            StmtKind::Let(_, _, e) | StmtKind::Const(_, _, e) | StmtKind::Assign(_, e)
            | StmtKind::Print(e) | StmtKind::PrintF(_, e) | StmtKind::Throw(e)
            | StmtKind::Defer(e) | StmtKind::Expr(e) => cnf_expr(&e.kind, out),
            StmtKind::Return(o) => { if let Some(e) = o { cnf_expr(&e.kind, out); } }
            StmtKind::SetIndex(a, b, c) => { cnf_expr(&a.kind, out); cnf_expr(&b.kind, out); cnf_expr(&c.kind, out); }
            StmtKind::SetAttr(a, _, c) => { cnf_expr(&a.kind, out); cnf_expr(&c.kind, out); }
            StmtKind::If(c, a, b) => { cnf_expr(&c.kind, out); collect_nested_free(a, out); if let Some(eb) = b { collect_nested_free(eb, out); } }
            StmtKind::While(c, b) => { cnf_expr(&c.kind, out); collect_nested_free(b, out); }
            StmtKind::Loop(b) => collect_nested_free(b, out),
            StmtKind::For(_, e, b) | StmtKind::ForIn(_, e, b) => { cnf_expr(&e.kind, out); collect_nested_free(b, out); }
            StmtKind::ForInKV(_, _, e, b) => { cnf_expr(&e.kind, out); collect_nested_free(b, out); }
            StmtKind::Block(b) | StmtKind::Seq(b) => collect_nested_free(b, out),
            _ => {}
        }
    }
}

fn cnf_expr(e: &ExprKind, out: &mut HashSet<String>) {
    match e {
        ExprKind::FnLit(_, params, _, body) => {
            for v in closure_free_vars(params, body) { out.insert(v); }
        }
        ExprKind::Call(c, args) => { cnf_expr(&c.kind, out); for a in args { cnf_expr(&a.kind, out); } }
        ExprKind::GetAttr(o, _) | ExprKind::SafeGetAttr(o, _) | ExprKind::Not(o) | ExprKind::ResultUnwrap(o) => cnf_expr(&o.kind, out),
        ExprKind::Index(o, i) | ExprKind::SafeIndex(o, i) => { cnf_expr(&o.kind, out); cnf_expr(&i.kind, out); }
        ExprKind::Array(items) => { for it in items { cnf_expr(&it.kind, out); } }
        ExprKind::Map(pairs) => { for (_, v) in pairs { cnf_expr(&v.kind, out); } }
        ExprKind::Add(a, b) | ExprKind::Sub(a, b) | ExprKind::Mul(a, b) | ExprKind::Div(a, b)
        | ExprKind::Mod(a, b) | ExprKind::Pow(a, b) | ExprKind::Eq(a, b) | ExprKind::NotEq(a, b)
        | ExprKind::Gt(a, b) | ExprKind::Ge(a, b) | ExprKind::Lt(a, b) | ExprKind::Le(a, b)
        | ExprKind::Try(a, b) | ExprKind::NullCoalesce(a, b) => { cnf_expr(&a.kind, out); cnf_expr(&b.kind, out); }
        ExprKind::Logical { left, right, .. } => { cnf_expr(&left.kind, out); cnf_expr(&right.kind, out); }
        ExprKind::IfExpr(c, t, e2) => { cnf_expr(&c.kind, out); cnf_expr(&t.kind, out); cnf_expr(&e2.kind, out); }
        ExprKind::Match(sc, arms, e2) => {
            cnf_expr(&sc.kind, out);
            for (_, b) in arms { cnf_expr(&b.kind, out); }
            if let Some(e3) = e2 { cnf_expr(&e3.kind, out); }
        }
        ExprKind::FString(parts) => { for p in parts { if let FStringPart::Interp(e2) = p { cnf_expr(&e2.kind, out); } } }
        ExprKind::BlockExpr(stmts, tail) => { collect_nested_free(stmts, out); if let Some(t) = tail { cnf_expr(&t.kind, out); } }
        _ => {}
    }
}

/// Names assigned anywhere (incl. inside nested closures), plus nested fn names
/// (their let-style binding counts). Descends into closures.
fn collect_assigned_deep(stmts: &[Stmt], out: &mut HashSet<String>) {
    for s in stmts {
        match &s.kind {
            StmtKind::Assign(n, e) => { out.insert(n.clone()); ad_expr(&e.kind, out); }
            StmtKind::Fn(name, _, _, body) => { out.insert(name.clone()); collect_assigned_deep(body, out); }
            StmtKind::Let(_, _, e) | StmtKind::Const(_, _, e) | StmtKind::Print(e)
            | StmtKind::PrintF(_, e) | StmtKind::Throw(e) | StmtKind::Defer(e)
            | StmtKind::Expr(e) => ad_expr(&e.kind, out),
            StmtKind::Return(o) => { if let Some(e) = o { ad_expr(&e.kind, out); } }
            StmtKind::SetIndex(a, b, c) => { ad_expr(&a.kind, out); ad_expr(&b.kind, out); ad_expr(&c.kind, out); }
            StmtKind::SetAttr(a, _, c) => { ad_expr(&a.kind, out); ad_expr(&c.kind, out); }
            StmtKind::If(c, a, b) => { ad_expr(&c.kind, out); collect_assigned_deep(a, out); if let Some(eb) = b { collect_assigned_deep(eb, out); } }
            StmtKind::While(c, b) => { ad_expr(&c.kind, out); collect_assigned_deep(b, out); }
            StmtKind::Loop(b) => collect_assigned_deep(b, out),
            StmtKind::For(_, e, b) | StmtKind::ForIn(_, e, b) => { ad_expr(&e.kind, out); collect_assigned_deep(b, out); }
            StmtKind::ForInKV(_, _, e, b) => { ad_expr(&e.kind, out); collect_assigned_deep(b, out); }
            StmtKind::Block(b) | StmtKind::Seq(b) => collect_assigned_deep(b, out),
            _ => {}
        }
    }
}

fn ad_expr(e: &ExprKind, out: &mut HashSet<String>) {
    match e {
        ExprKind::FnLit(_, _, _, body) => collect_assigned_deep(body, out),
        ExprKind::Call(c, args) => { ad_expr(&c.kind, out); for a in args { ad_expr(&a.kind, out); } }
        ExprKind::GetAttr(o, _) | ExprKind::SafeGetAttr(o, _) | ExprKind::Not(o) | ExprKind::ResultUnwrap(o) => ad_expr(&o.kind, out),
        ExprKind::Index(o, i) | ExprKind::SafeIndex(o, i) => { ad_expr(&o.kind, out); ad_expr(&i.kind, out); }
        ExprKind::Array(items) => { for it in items { ad_expr(&it.kind, out); } }
        ExprKind::Map(pairs) => { for (_, v) in pairs { ad_expr(&v.kind, out); } }
        ExprKind::Add(a, b) | ExprKind::Sub(a, b) | ExprKind::Mul(a, b) | ExprKind::Div(a, b)
        | ExprKind::Mod(a, b) | ExprKind::Pow(a, b) | ExprKind::Eq(a, b) | ExprKind::NotEq(a, b)
        | ExprKind::Gt(a, b) | ExprKind::Ge(a, b) | ExprKind::Lt(a, b) | ExprKind::Le(a, b)
        | ExprKind::Try(a, b) | ExprKind::NullCoalesce(a, b) => { ad_expr(&a.kind, out); ad_expr(&b.kind, out); }
        ExprKind::Logical { left, right, .. } => { ad_expr(&left.kind, out); ad_expr(&right.kind, out); }
        ExprKind::IfExpr(c, t, e2) => { ad_expr(&c.kind, out); ad_expr(&t.kind, out); ad_expr(&e2.kind, out); }
        ExprKind::Match(sc, arms, e2) => {
            ad_expr(&sc.kind, out);
            for (_, b) in arms { ad_expr(&b.kind, out); }
            if let Some(e3) = e2 { ad_expr(&e3.kind, out); }
        }
        ExprKind::FString(parts) => { for p in parts { if let FStringPart::Interp(e2) = p { ad_expr(&e2.kind, out); } } }
        ExprKind::BlockExpr(stmts, tail) => { collect_assigned_deep(stmts, out); if let Some(t) = tail { ad_expr(&t.kind, out); } }
        _ => {}
    }
}

/// This function's own local declarations (let/const + nested fn names), NOT
/// descending into nested closures.
fn collect_local_decls(stmts: &[Stmt], out: &mut HashSet<String>) {
    for s in stmts {
        match &s.kind {
            StmtKind::Let(n, _, _) | StmtKind::Const(n, _, _) => { out.insert(n.clone()); }
            StmtKind::Fn(name, _, _, _) => { out.insert(name.clone()); }
            StmtKind::If(_, a, b) => { collect_local_decls(a, out); if let Some(eb) = b { collect_local_decls(eb, out); } }
            StmtKind::While(_, b) | StmtKind::Loop(b) | StmtKind::For(_, _, b)
            | StmtKind::ForIn(_, _, b) | StmtKind::ForInKV(_, _, _, b)
            | StmtKind::Block(b) | StmtKind::Seq(b) => collect_local_decls(b, out),
            _ => {}
        }
    }
}

/// Locals to box: captured by a nested closure AND assigned AND declared here.
fn compute_boxed(params: &[Param], body: &[Stmt]) -> HashSet<String> {
    let mut captured = HashSet::new();
    collect_nested_free(body, &mut captured);
    let mut assigned = HashSet::new();
    collect_assigned_deep(body, &mut assigned);
    let mut locals: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
    collect_local_decls(body, &mut locals);
    captured
        .into_iter()
        .filter(|n| assigned.contains(n) && locals.contains(n))
        .collect()
}

/// Record a referenced variable that is not bound inside the closure.
fn record_ref(name: &str, bound: &HashSet<String>, out: &mut Vec<String>, seen: &mut HashSet<String>) {
    if !bound.contains(name) && !seen.contains(name) {
        seen.insert(name.to_string());
        out.push(name.to_string());
    }
}

/// Walk a statement list for free-variable collection, threading `bound`
/// (names bound so far at this level — e.g. earlier `let`s are visible later).
fn walk_stmts(stmts: &[Stmt], bound: &mut HashSet<String>, out: &mut Vec<String>, seen: &mut HashSet<String>) {
    for s in stmts {
        walk_stmt(&s.kind, bound, out, seen);
    }
}

fn walk_block(stmts: &[Stmt], bound: &HashSet<String>, out: &mut Vec<String>, seen: &mut HashSet<String>) {
    // A nested block gets its own scope: bindings inside don't leak outward.
    let mut inner = bound.clone();
    walk_stmts(stmts, &mut inner, out, seen);
}

fn walk_stmt(s: &StmtKind, bound: &mut HashSet<String>, out: &mut Vec<String>, seen: &mut HashSet<String>) {
    match s {
        StmtKind::Print(e) | StmtKind::PrintF(_, e) | StmtKind::Throw(e) | StmtKind::Defer(e) | StmtKind::Expr(e) => {
            walk_expr(&e.kind, bound, out, seen);
        }
        StmtKind::Return(opt) => {
            if let Some(e) = opt { walk_expr(&e.kind, bound, out, seen); }
        }
        StmtKind::Let(n, _, e) | StmtKind::Const(n, _, e) => {
            walk_expr(&e.kind, bound, out, seen); // rhs sees the outer scope
            bound.insert(n.clone());
        }
        StmtKind::Assign(n, e) => {
            // Assignment target must exist as a C variable -> treat as a use.
            record_ref(n, bound, out, seen);
            walk_expr(&e.kind, bound, out, seen);
        }
        StmtKind::SetIndex(t, i, v) => {
            walk_expr(&t.kind, bound, out, seen);
            walk_expr(&i.kind, bound, out, seen);
            walk_expr(&v.kind, bound, out, seen);
        }
        StmtKind::SetAttr(t, _, v) => {
            walk_expr(&t.kind, bound, out, seen);
            walk_expr(&v.kind, bound, out, seen);
        }
        StmtKind::If(c, then_b, else_b) => {
            walk_expr(&c.kind, bound, out, seen);
            walk_block(then_b, bound, out, seen);
            if let Some(eb) = else_b { walk_block(eb, bound, out, seen); }
        }
        StmtKind::While(c, body) => {
            walk_expr(&c.kind, bound, out, seen);
            walk_block(body, bound, out, seen);
        }
        StmtKind::Loop(body) => walk_block(body, bound, out, seen),
        StmtKind::For(n, e, body) | StmtKind::ForIn(n, e, body) => {
            walk_expr(&e.kind, bound, out, seen);
            let mut inner = bound.clone();
            inner.insert(n.clone());
            walk_stmts(body, &mut inner, out, seen);
        }
        StmtKind::ForInKV(k, v, e, body) => {
            walk_expr(&e.kind, bound, out, seen);
            let mut inner = bound.clone();
            inner.insert(k.clone());
            inner.insert(v.clone());
            walk_stmts(body, &mut inner, out, seen);
        }
        StmtKind::Fn(name, params, _ret, body) => {
            bound.insert(name.clone());
            let mut inner = bound.clone();
            for p in params { inner.insert(p.name.clone()); }
            walk_stmts(body, &mut inner, out, seen);
        }
        StmtKind::Block(stmts) => walk_block(stmts, bound, out, seen),
        StmtKind::Seq(stmts) => walk_stmts(stmts, bound, out, seen),
        StmtKind::Break | StmtKind::Continue | StmtKind::Import(_) | StmtKind::Export(_) => {}
    }
}

fn walk_expr(e: &ExprKind, bound: &HashSet<String>, out: &mut Vec<String>, seen: &mut HashSet<String>) {
    match e {
        ExprKind::Var(n) => record_ref(n, bound, out, seen),
        ExprKind::Number(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::Nil => {}
        ExprKind::Call(c, args) => {
            walk_expr(&c.kind, bound, out, seen);
            for a in args { walk_expr(&a.kind, bound, out, seen); }
        }
        ExprKind::GetAttr(o, _) | ExprKind::SafeGetAttr(o, _) | ExprKind::Not(o) | ExprKind::ResultUnwrap(o) => {
            walk_expr(&o.kind, bound, out, seen);
        }
        ExprKind::Index(o, i) | ExprKind::SafeIndex(o, i) => {
            walk_expr(&o.kind, bound, out, seen);
            walk_expr(&i.kind, bound, out, seen);
        }
        ExprKind::Array(items) => { for it in items { walk_expr(&it.kind, bound, out, seen); } }
        ExprKind::Map(pairs) => { for (_, v) in pairs { walk_expr(&v.kind, bound, out, seen); } }
        ExprKind::Add(a, b) | ExprKind::Sub(a, b) | ExprKind::Mul(a, b) | ExprKind::Div(a, b)
        | ExprKind::Mod(a, b) | ExprKind::Pow(a, b) | ExprKind::Eq(a, b) | ExprKind::NotEq(a, b)
        | ExprKind::Gt(a, b) | ExprKind::Ge(a, b) | ExprKind::Lt(a, b) | ExprKind::Le(a, b)
        | ExprKind::Try(a, b) | ExprKind::NullCoalesce(a, b) => {
            walk_expr(&a.kind, bound, out, seen);
            walk_expr(&b.kind, bound, out, seen);
        }
        ExprKind::Logical { left, right, .. } => {
            walk_expr(&left.kind, bound, out, seen);
            walk_expr(&right.kind, bound, out, seen);
        }
        ExprKind::IfExpr(c, t, e2) => {
            walk_expr(&c.kind, bound, out, seen);
            walk_expr(&t.kind, bound, out, seen);
            walk_expr(&e2.kind, bound, out, seen);
        }
        ExprKind::Match(sc, arms, else_e) => {
            walk_expr(&sc.kind, bound, out, seen);
            for (arm, body) in arms {
                let mut inner = bound.clone();
                match arm {
                    MatchArm::Bind(n) => { inner.insert(n.clone()); }
                    MatchArm::BindGuard(n, g) => { inner.insert(n.clone()); walk_expr(&g.kind, &inner, out, seen); }
                    MatchArm::Literal(_) | MatchArm::Wildcard => {}
                }
                walk_expr(&body.kind, &inner, out, seen);
            }
            if let Some(e2) = else_e { walk_expr(&e2.kind, bound, out, seen); }
        }
        ExprKind::FString(parts) => {
            for p in parts {
                if let FStringPart::Interp(e2) = p { walk_expr(&e2.kind, bound, out, seen); }
            }
        }
        ExprKind::BlockExpr(stmts, tail) => {
            let mut inner = bound.clone();
            walk_stmts(stmts, &mut inner, out, seen);
            if let Some(t) = tail { walk_expr(&t.kind, &inner, out, seen); }
        }
        ExprKind::FnLit(_, params, _ret, body) => {
            // A nested closure: its free vars that come from OUR scope must also
            // be captured by us (so we can pass them down). Recurse with the
            // nested params bound.
            let mut inner = bound.clone();
            for p in params { inner.insert(p.name.clone()); }
            walk_stmts(body, &mut inner, out, seen);
        }
    }
}
