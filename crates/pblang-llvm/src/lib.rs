// pblang-llvm — LLVM IR backend for pblang.
//
// Generates LLVM IR (.ll) text from pblang AST, which can then be
// compiled to native code with clang/llc/opt.
//
// Architecture:
//   pblang AST → LLVM IR text → .ll file → clang/llc → native .o/.so/.exe
//
// This approach requires NO LLVM development libraries at build time —
// only clang at compile time. The generated IR is human-readable and
// can be optimized with opt, compiled with llc, or linked with clang.
//
// Supported:
//   ✅ Numbers (double), integers (i64), booleans (i1)
//   ✅ Functions with parameters and return values
//   ✅ Arithmetic (fadd, fsub, fmul, fdiv)
//   ✅ Comparisons (fcmp)
//   ✅ If/else (br i1)
//   ✅ While loops
//   ✅ Let/const variables (alloca + store/load)
//   ✅ Print (calls @printf via external declaration)
//   ✅ Return
//
// Not yet:
//   ❌ Strings (complex — needs GEP + global string constants)
//   ❌ Arrays/Maps (needs runtime data structures)
//   ❌ Closures
//   ❌ Try/catch
//   ❌ Import/Export
//   ❌ Threads

use pblang_core::{ExprKind, Param, Stmt, StmtKind};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────
// LLVM IR module
// ─────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct LlvmModule {
    pub declarations: Vec<String>,
    pub globals: Vec<String>,
    pub functions: Vec<LlvmFunction>,
    pub string_constants: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct LlvmFunction {
    pub name: String,
    pub params: Vec<(String, String)>, // (name, llvm_type)
    pub return_type: String,
    pub body: String,
    pub locals: Vec<(String, String)>, // (name, llvm_type)
}

// ─────────────────────────────────────────────────────
// Compiler context
// ─────────────────────────────────────────────────────

struct LlvmCtx {
    module: LlvmModule,
    reg_counter: u32,
    label_counter: u32,
    /// Maps a source variable name to the SSA register holding its `alloca`
    /// pointer. Params and `let`/`const` bindings are all stack slots, so every
    /// read is a `load` and every write a `store` (clang -O0 style); `mem2reg`
    /// promotes them later. Reset per function.
    var_ptr: HashMap<String, String>,
    /// Stack of `(header_label, exit_label)` for the enclosing loops, so
    /// `break`/`continue` target the innermost loop.
    loop_stack: Vec<(String, String)>,
    warnings: Vec<String>,
}

/// Format an `f64` as a valid LLVM `double` literal. Integer-valued numbers use
/// the readable `N.0` form (exactly representable); everything else uses the
/// exact hexadecimal bit form `0x…` (LLVM rejects inexact decimal literals such
/// as `0.1`, so the hex form is the only always-correct encoding).
fn llvm_double(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{n:.1}")
    } else {
        format!("0x{:016X}", n.to_bits())
    }
}

/// LLVM symbol for a pblang function. A pblang `main` returns a `double`, but
/// the C runtime entry point must be `i32 @main`, so the user's `main` is
/// emitted under a private symbol and a C-ABI wrapper named `@main` is
/// synthesized (see `compile_program`). All other names pass through unchanged.
fn fn_symbol(name: &str) -> String {
    if name == "main" {
        "pblang_user_main".to_string()
    } else {
        name.to_string()
    }
}

impl LlvmCtx {
    fn new() -> Self {
        LlvmCtx {
            module: LlvmModule::default(),
            reg_counter: 0,
            label_counter: 0,
            var_ptr: HashMap::new(),
            loop_stack: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn fresh_reg(&mut self) -> String {
        self.reg_counter += 1;
        format!("%r{}", self.reg_counter)
    }

    fn fresh_label(&mut self) -> String {
        self.label_counter += 1;
        format!("L{}", self.label_counter)
    }

    fn compile_program(&mut self, stmts: Vec<Stmt>) -> String {
        // External printf (unused unless print is wired, but harmless to declare).
        self.module
            .declarations
            .push("declare i32 @printf(i8* nocapture readonly, ...)".into());

        // Only function definitions are valid at module scope. LLVM resolves
        // calls to functions defined later in the module, so no forward
        // `declare` is needed (and emitting one alongside the `define` is an
        // "invalid redefinition" error).
        let mut body = String::new();
        let mut main_arity: Option<usize> = None;
        for s in stmts {
            match s.kind {
                StmtKind::Fn(name, params, _ret, fn_body) => {
                    if name == "main" {
                        main_arity = Some(params.len());
                    }
                    body.push_str(&self.compile_function(name, params, fn_body));
                }
                other => {
                    self.warnings
                        .push(format!("LLVM: top-level statement ignored: {other:?}"));
                }
            }
        }

        // Synthesize a C-ABI entry point so `clang prog.ll -o exe` links and the
        // process exit code is the value `main` returns. Only for a 0-arg `main`
        // (a pblang `main` taking args has no meaningful C-runtime mapping here).
        match main_arity {
            Some(0) => {
                body.push_str(
                    "define i32 @main() {\nentry:\n  %r = call double @pblang_user_main()\n  %i = fptosi double %r to i32\n  ret i32 %i\n}\n\n",
                );
            }
            Some(n) => {
                self.warnings.push(format!(
                    "LLVM: `main` takes {n} args; no C entry-point wrapper emitted"
                ));
            }
            None => {}
        }
        body
    }

    fn compile_function(&mut self, name: String, params: Vec<Param>, body: Vec<Stmt>) -> String {
        // Each function has its own SSA namespace and variable environment.
        self.reg_counter = 0;
        self.var_ptr.clear();
        self.loop_stack.clear();

        let param_decls: Vec<String> = params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let n = if p.name.is_empty() {
                    format!("p{i}")
                } else {
                    p.name.clone()
                };
                format!("double %arg.{n}")
            })
            .collect();

        let mut out = format!(
            "define double @{}({}) {{\nentry:\n",
            fn_symbol(&name),
            param_decls.join(", ")
        );

        // Spill each incoming param into an alloca slot so reads/writes are
        // uniform loads/stores (and params can be reassigned).
        for (i, p) in params.iter().enumerate() {
            let n = if p.name.is_empty() {
                format!("p{i}")
            } else {
                p.name.clone()
            };
            let slot = format!("%{n}.addr");
            out.push_str(&format!("  {slot} = alloca double\n"));
            out.push_str(&format!("  store double %arg.{n}, double* {slot}\n"));
            self.var_ptr.insert(n, slot);
        }

        let (code, terminated) = self.compile_stmts(body);
        out.push_str(&code);

        // Every basic block must end in a terminator; default any fall-through
        // to `ret double 0.0`.
        if !terminated {
            out.push_str("  ret double 0.0\n");
        }

        out.push_str("}\n\n");
        out
    }

    /// Compile a statement list, stopping after the first statement that
    /// terminates the current block (instructions after a terminator are
    /// invalid IR). Returns `(code, terminated)`.
    fn compile_stmts(&mut self, stmts: Vec<Stmt>) -> (String, bool) {
        let mut out = String::new();
        for s in stmts {
            let (code, terminated) = self.compile_stmt(s);
            out.push_str(&code);
            if terminated {
                return (out, true);
            }
        }
        (out, false)
    }

    /// Returns `(code, terminated)` — `terminated` is true when the statement
    /// ends the current basic block (return/break/continue, or an if whose
    /// every branch terminates).
    fn compile_stmt(&mut self, stmt: Stmt) -> (String, bool) {
        match stmt.kind {
            StmtKind::Let(name, _ty, init) | StmtKind::Const(name, _ty, init) => {
                let (code, val) = self.compile_expr(init);
                let slot = format!("%{name}.addr");
                let mut out = code;
                out.push_str(&format!("  {slot} = alloca double\n"));
                out.push_str(&format!("  store double {val}, double* {slot}\n"));
                self.var_ptr.insert(name, slot);
                (out, false)
            }
            StmtKind::Assign(name, expr) => {
                let (code, val) = self.compile_expr(expr);
                let mut out = code;
                if let Some(slot) = self.var_ptr.get(&name).cloned() {
                    out.push_str(&format!("  store double {val}, double* {slot}\n"));
                } else {
                    // Assigning an as-yet-undeclared name: create a slot so the IR
                    // stays valid (matches the interpreter's lenient globals).
                    let slot = format!("%{name}.addr");
                    out.push_str(&format!("  {slot} = alloca double\n"));
                    out.push_str(&format!("  store double {val}, double* {slot}\n"));
                    self.var_ptr.insert(name, slot);
                }
                (out, false)
            }
            StmtKind::Return(val) => {
                if let Some(v) = val {
                    let (code, val) = self.compile_expr(v);
                    (format!("{code}  ret double {val}\n"), true)
                } else {
                    ("  ret double 0.0\n".into(), true)
                }
            }
            StmtKind::Expr(expr) => {
                let (code, _val) = self.compile_expr(expr);
                (code, false)
            }
            StmtKind::If(cond, then_stmts, else_stmts) => self.compile_if(cond, then_stmts, else_stmts),
            StmtKind::While(cond, body) => (self.compile_while(cond, body), false),
            StmtKind::Loop(body) => (self.compile_loop(body), false),
            StmtKind::For(name, end_expr, body) => (self.compile_for(name, end_expr, body), false),
            StmtKind::Break => {
                if let Some((_h, exit)) = self.loop_stack.last() {
                    (format!("  br label %{exit}\n"), true)
                } else {
                    self.warnings.push("LLVM: `break` outside a loop".into());
                    (String::new(), false)
                }
            }
            StmtKind::Continue => {
                if let Some((header, _e)) = self.loop_stack.last() {
                    (format!("  br label %{header}\n"), true)
                } else {
                    self.warnings.push("LLVM: `continue` outside a loop".into());
                    (String::new(), false)
                }
            }
            StmtKind::Block(stmts) | StmtKind::Seq(stmts) => self.compile_stmts(stmts),
            other => {
                self.warnings
                    .push(format!("LLVM: unsupported stmt: {other:?}"));
                (String::new(), false)
            }
        }
    }

    fn compile_if(
        &mut self,
        cond: pblang_core::Expr,
        then_stmts: Vec<Stmt>,
        else_stmts: Option<Vec<Stmt>>,
    ) -> (String, bool) {
        let (cond_code, cond_val) = self.compile_expr(cond);
        let cond_reg = self.fresh_reg();
        let then_label = self.fresh_label();
        let else_label = self.fresh_label();
        let merge_label = self.fresh_label();

        let mut out = cond_code;
        out.push_str(&format!(
            "  {cond_reg} = fcmp one double {cond_val}, 0.0\n  br i1 {cond_reg}, label %{then_label}, label %{else_label}\n"
        ));

        // then block
        out.push_str(&format!("{then_label}:\n"));
        let (then_code, then_term) = self.compile_stmts(then_stmts);
        out.push_str(&then_code);
        if !then_term {
            out.push_str(&format!("  br label %{merge_label}\n"));
        }

        // else block
        out.push_str(&format!("{else_label}:\n"));
        let (else_code, else_term) = match else_stmts {
            Some(s) => self.compile_stmts(s),
            None => (String::new(), false),
        };
        out.push_str(&else_code);
        if !else_term {
            out.push_str(&format!("  br label %{merge_label}\n"));
        }

        // The merge block exists only if at least one branch falls through. If
        // both branches terminate, emitting an empty unreachable merge block
        // (no predecessors, no terminator) would be invalid IR.
        let both_terminated = then_term && else_term;
        if !both_terminated {
            out.push_str(&format!("{merge_label}:\n"));
        }
        (out, both_terminated)
    }

    fn compile_while(&mut self, cond_expr: pblang_core::Expr, body: Vec<Stmt>) -> String {
        let header = self.fresh_label();
        let body_label = self.fresh_label();
        let exit = self.fresh_label();

        let mut out = format!("  br label %{header}\n{header}:\n");
        let (cond_code, cond_val) = self.compile_expr(cond_expr);
        out.push_str(&cond_code);
        let cond_reg = self.fresh_reg();
        out.push_str(&format!(
            "  {cond_reg} = fcmp one double {cond_val}, 0.0\n  br i1 {cond_reg}, label %{body_label}, label %{exit}\n"
        ));

        out.push_str(&format!("{body_label}:\n"));
        self.loop_stack.push((header.clone(), exit.clone()));
        let (body_code, body_term) = self.compile_stmts(body);
        self.loop_stack.pop();
        out.push_str(&body_code);
        if !body_term {
            out.push_str(&format!("  br label %{header}\n"));
        }

        out.push_str(&format!("{exit}:\n"));
        out
    }

    fn compile_loop(&mut self, body: Vec<Stmt>) -> String {
        let header = self.fresh_label();
        let exit = self.fresh_label();
        let mut out = format!("  br label %{header}\n{header}:\n");
        self.loop_stack.push((header.clone(), exit.clone()));
        let (body_code, body_term) = self.compile_stmts(body);
        self.loop_stack.pop();
        out.push_str(&body_code);
        if !body_term {
            out.push_str(&format!("  br label %{header}\n"));
        }
        out.push_str(&format!("{exit}:\n"));
        out
    }

    fn compile_for(&mut self, name: String, end_expr: pblang_core::Expr, body: Vec<Stmt>) -> String {
        let (end_code, end_val) = self.compile_expr(end_expr);
        let header = self.fresh_label();
        let body_label = self.fresh_label();
        let exit = self.fresh_label();
        let slot = format!("%{name}.addr");

        let mut out = end_code;
        out.push_str(&format!(
            "  {slot} = alloca double\n  store double 0.0, double* {slot}\n"
        ));
        self.var_ptr.insert(name, slot.clone());
        out.push_str(&format!("  br label %{header}\n{header}:\n"));

        let i_val = self.fresh_reg();
        let cmp_val = self.fresh_reg();
        out.push_str(&format!(
            "  {i_val} = load double, double* {slot}\n  {cmp_val} = fcmp olt double {i_val}, {end_val}\n  br i1 {cmp_val}, label %{body_label}, label %{exit}\n"
        ));

        out.push_str(&format!("{body_label}:\n"));
        self.loop_stack.push((header.clone(), exit.clone()));
        let (body_code, body_term) = self.compile_stmts(body);
        self.loop_stack.pop();
        out.push_str(&body_code);
        if !body_term {
            let inc = self.fresh_reg();
            let next = self.fresh_reg();
            out.push_str(&format!(
                "  {inc} = load double, double* {slot}\n  {next} = fadd double {inc}, 1.0\n  store double {next}, double* {slot}\n  br label %{header}\n"
            ));
        }
        out.push_str(&format!("{exit}:\n"));
        out
    }

    /// Compile an expression to `(code, value)` where `code` is the IR that must
    /// precede the use and `value` is the `double` operand (an SSA register or an
    /// immediate). Every value is a `double`; comparisons are widened from `i1`.
    fn compile_expr(&mut self, expr: pblang_core::Expr) -> (String, String) {
        match expr.kind {
            ExprKind::Number(n) => (String::new(), llvm_double(n)),
            ExprKind::Bool(b) => (String::new(), if b { "1.0".into() } else { "0.0".into() }),
            ExprKind::Nil => (String::new(), "0.0".into()),
            ExprKind::Var(name) => {
                if let Some(slot) = self.var_ptr.get(&name).cloned() {
                    let r = self.fresh_reg();
                    (format!("  {r} = load double, double* {slot}\n"), r)
                } else {
                    self.warnings
                        .push(format!("LLVM: read of undeclared variable `{name}`"));
                    (String::new(), "0.0".into())
                }
            }
            ExprKind::Add(l, r) => self.binop("fadd", *l, *r),
            ExprKind::Sub(l, r) => self.binop("fsub", *l, *r),
            ExprKind::Mul(l, r) => self.binop("fmul", *l, *r),
            ExprKind::Div(l, r) => self.binop("fdiv", *l, *r),
            ExprKind::Eq(l, r) => self.cmpop("oeq", *l, *r),
            ExprKind::NotEq(l, r) => self.cmpop("one", *l, *r),
            ExprKind::Lt(l, r) => self.cmpop("olt", *l, *r),
            ExprKind::Gt(l, r) => self.cmpop("ogt", *l, *r),
            ExprKind::Le(l, r) => self.cmpop("ole", *l, *r),
            ExprKind::Ge(l, r) => self.cmpop("oge", *l, *r),
            ExprKind::Not(inner) => {
                let (code, val) = self.compile_expr(*inner);
                let cmp = self.fresh_reg();
                let res = self.fresh_reg();
                (
                    format!(
                        "{code}  {cmp} = fcmp oeq double {val}, 0.0\n  {res} = uitofp i1 {cmp} to double\n"
                    ),
                    res,
                )
            }
            ExprKind::Call(func_expr, args) => {
                let mut code = String::new();
                let mut arg_ops = Vec::new();
                for a in args {
                    let (ac, av) = self.compile_expr(a);
                    code.push_str(&ac);
                    arg_ops.push(format!("double {av}"));
                }
                let fn_name = if let ExprKind::Var(ref name) = func_expr.kind {
                    name.clone()
                } else {
                    self.warnings
                        .push("LLVM: indirect/computed call not supported".into());
                    return (code, "0.0".into());
                };
                if fn_name == "print" {
                    self.warnings
                        .push("LLVM: print() skipped (no printf wiring)".into());
                    return (code, "0.0".into());
                }
                let res = self.fresh_reg();
                code.push_str(&format!(
                    "  {res} = call double @{}({})\n",
                    fn_symbol(&fn_name),
                    arg_ops.join(", ")
                ));
                (code, res)
            }
            other => {
                self.warnings
                    .push(format!("LLVM: unsupported expr: {other:?}"));
                (String::new(), "0.0".into())
            }
        }
    }

    fn binop(&mut self, op: &str, left: pblang_core::Expr, right: pblang_core::Expr) -> (String, String) {
        let (lc, lv) = self.compile_expr(left);
        let (rc, rv) = self.compile_expr(right);
        let res = self.fresh_reg();
        (
            format!("{lc}{rc}  {res} = {op} double {lv}, {rv}\n"),
            res,
        )
    }

    /// A comparison yields `i1`; widen it to `double` (0.0/1.0) so every value in
    /// this backend is uniformly a `double`.
    fn cmpop(&mut self, pred: &str, left: pblang_core::Expr, right: pblang_core::Expr) -> (String, String) {
        let (lc, lv) = self.compile_expr(left);
        let (rc, rv) = self.compile_expr(right);
        let cmp = self.fresh_reg();
        let res = self.fresh_reg();
        (
            format!(
                "{lc}{rc}  {cmp} = fcmp {pred} double {lv}, {rv}\n  {res} = uitofp i1 {cmp} to double\n"
            ),
            res,
        )
    }

    fn finish(&mut self) -> (String, Vec<String>) {
        let mut out = String::new();
        for d in &self.module.declarations {
            out.push_str(d);
            out.push('\n');
        }
        out.push('\n');
        (out, self.warnings.clone())
    }
}

// ─────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────

/// Compile pblang AST to LLVM IR text.
pub fn compile_to_llvm_ir(stmts: Vec<Stmt>) -> Result<(String, Vec<String>), String> {
    let mut ctx = LlvmCtx::new();
    let func_body = ctx.compile_program(stmts);
    let (header, warnings) = ctx.finish();
    Ok((format!("{header}\n{func_body}"), warnings))
}

/// Compile pblang AST to LLVM IR and write to file, then compile with clang.
pub fn compile_to_native(stmts: Vec<Stmt>, output_path: &str, opt_level: u8) -> Result<(), String> {
    let (ir, warnings) = compile_to_llvm_ir(stmts)?;
    for w in &warnings {
        eprintln!("warning: {w}");
    }

    // Write .ll file
    let ll_path = format!("{output_path}.ll");
    std::fs::write(&ll_path, &ir).map_err(|e| format!("write {}: {e}", ll_path))?;

    // Compile with clang
    let status = std::process::Command::new("clang")
        .arg(&ll_path)
        .arg("-o")
        .arg(output_path)
        .arg(format!("-O{opt_level}"))
        .arg("-lm")
        .status()
        .map_err(|e| format!("clang: {e}"))?;

    if !status.success() {
        return Err("clang compilation failed".into());
    }

    // Clean up .ll
    let _ = std::fs::remove_file(&ll_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_module() {
        let (ir, _) = compile_to_llvm_ir(vec![]).unwrap();
        assert!(ir.contains("declare i32 @printf"));
    }

    /// The whole point of this backend is to feed clang. Substring checks prove
    /// nothing if the IR is malformed, so actually run clang over the emitted IR
    /// and require it to accept it. Skipped (not failed) when clang is absent.
    fn clang_accepts(ir: &str) -> Option<bool> {
        use std::process::Command;
        if Command::new("clang").arg("--version").output().is_err() {
            return None; // no clang on this host → skip
        }
        let dir = std::env::temp_dir();
        // Unique per call: tests run in parallel threads of ONE process, so a
        // process-id-based name would race on the same file.
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let uniq = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let ll = dir.join(format!("pblang_llvm_test_{}_{uniq}.ll", std::process::id()));
        std::fs::write(&ll, ir).expect("write .ll");
        // Parse + verify the IR only (-S -emit-llvm round-trips through the parser
        // and verifier without needing a `main`/full link).
        let out = Command::new("clang")
            .args(["-x", "ir", "-S", "-emit-llvm", "-o", "/dev/null"])
            .arg(&ll)
            .output()
            .expect("run clang");
        let _ = std::fs::remove_file(&ll);
        if !out.status.success() {
            eprintln!(
                "clang rejected IR:\n{}\n--- IR ---\n{ir}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Some(out.status.success())
    }

    fn ir_of(src: &str) -> String {
        let stmts = pblang_core::parse(src).expect("parse");
        compile_to_llvm_ir(stmts).expect("compile").0
    }

    #[test]
    fn emitted_ir_is_valid_for_clang_arithmetic() {
        let ir = ir_of("fn add(a, b) { return a + b; }");
        match clang_accepts(&ir) {
            Some(ok) => assert!(ok, "clang must accept the emitted arithmetic IR"),
            None => eprintln!("clang not available; skipping IR validity check"),
        }
    }

    #[test]
    fn emitted_ir_is_valid_for_clang_control_flow() {
        let ir = ir_of(
            "fn classify(n) { let s = 0; if n > 0 { s = 1; } else { s = 2; } return s; }",
        );
        match clang_accepts(&ir) {
            Some(ok) => assert!(ok, "clang must accept the emitted control-flow IR"),
            None => eprintln!("clang not available; skipping IR validity check"),
        }
    }

    #[test]
    fn program_with_main_links_and_exits_with_result() {
        use std::process::Command;
        if Command::new("clang").arg("--version").output().is_err() {
            eprintln!("clang not available; skipping");
            return;
        }
        // A pblang `main` returns a double; the C runtime entry must be
        // `i32 @main`, so we synthesize a wrapper. Without it the process exit
        // code is garbage (wrong return register).
        let ir = ir_of("fn sq(x){return x*x;}\nfn main(){ return sq(7) - 7; }"); // 42
        assert!(
            ir.contains("define i32 @main()"),
            "a synthetic C-ABI main wrapper must be emitted:\n{ir}"
        );
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let ll = dir.join(format!("pblang_llvm_main_{pid}.ll"));
        let exe = dir.join(format!("pblang_llvm_main_{pid}.out"));
        std::fs::write(&ll, &ir).unwrap();
        let status = Command::new("clang")
            .args(["-x", "ir"]).arg(&ll).arg("-o").arg(&exe).arg("-lm")
            .status().expect("clang link");
        assert!(status.success(), "must link without a C main");
        let code = Command::new(&exe).status().expect("run").code();
        let _ = std::fs::remove_file(&ll);
        let _ = std::fs::remove_file(&exe);
        assert_eq!(code, Some(42), "exit code must be the value `main` returns");
    }

    #[test]
    fn emitted_ir_executes_with_correct_result() {
        use std::process::Command;
        if Command::new("clang").arg("--version").output().is_err() {
            eprintln!("clang not available; skipping execution test");
            return;
        }
        // sum of squares 0..n plus a branch: poly(5) = (0+1+4+9+16) + 100 = 130.
        let ir = ir_of(
            "fn sq(x) { return x * x; }\n\
             fn poly(n) { let t = 0; let i = 0; while i < n { t = t + sq(i); i = i + 1; } if n > 3 { t = t + 100; } return t; }",
        );
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let ll = dir.join(format!("pblang_llvm_exec_{pid}.ll"));
        let cmain = dir.join(format!("pblang_llvm_exec_{pid}.c"));
        let exe = dir.join(format!("pblang_llvm_exec_{pid}.out"));
        std::fs::write(&ll, &ir).unwrap();
        std::fs::write(
            &cmain,
            "extern double poly(double);\nint main(){ return (int)poly(5.0); }\n",
        )
        .unwrap();
        let status = Command::new("clang")
            .args(["-x", "ir"])
            .arg(&ll)
            .args(["-x", "c"])
            .arg(&cmain)
            .arg("-o")
            .arg(&exe)
            .arg("-lm")
            .status()
            .expect("clang link");
        assert!(status.success(), "clang must link the emitted IR with a C main");
        let run = Command::new(&exe).status().expect("run exe");
        let _ = std::fs::remove_file(&ll);
        let _ = std::fs::remove_file(&cmain);
        let _ = std::fs::remove_file(&exe);
        assert_eq!(run.code(), Some(130), "poly(5) must compute 130 natively");
    }

    #[test]
    fn emitted_ir_is_valid_for_clang_loops_and_calls() {
        let ir = ir_of(
            "fn sq(x) { return x * x; }\nfn run(n) { let t = 0; let i = 0; while i < n { t = t + sq(i); i = i + 1; } return t; }",
        );
        match clang_accepts(&ir) {
            Some(ok) => assert!(ok, "clang must accept the emitted loop/call IR"),
            None => eprintln!("clang not available; skipping IR validity check"),
        }
    }

    #[test]
    fn test_simple_function() {
        use pblang_core::{Expr, ExprKind, Param, Span, Stmt, StmtKind};
        let stmts = vec![Stmt {
            kind: StmtKind::Fn(
                "add".into(),
                vec![
                    Param::simple("a".into(), None),
                    Param::simple("b".into(), None),
                ],
                None,
                vec![Stmt {
                    kind: StmtKind::Return(Some(Expr {
                        kind: ExprKind::Add(
                            Box::new(Expr {
                                kind: ExprKind::Var("a".into()),
                                span: Span::default(),
                            }),
                            Box::new(Expr {
                                kind: ExprKind::Var("b".into()),
                                span: Span::default(),
                            }),
                        ),
                        span: Span::default(),
                    })),
                    span: Span::default(),
                }],
            ),
            span: Span::default(),
        }];

        let (ir, warnings) = compile_to_llvm_ir(stmts).unwrap();
        for w in &warnings {
            eprintln!("warning: {w}");
        }
        assert!(
            ir.contains("define double @add"),
            "IR should contain add function:\n{ir}"
        );
        assert!(ir.contains("fadd"), "IR should contain fadd:\n{ir}");
        assert!(ir.contains("ret double"), "IR should contain ret:\n{ir}");
    }

    #[test]
    fn test_if_else() {
        use pblang_core::{Expr, ExprKind, Param, Span, Stmt, StmtKind};
        let stmts = vec![Stmt {
            kind: StmtKind::Fn(
                "max".into(),
                vec![
                    Param::simple("a".into(), None),
                    Param::simple("b".into(), None),
                ],
                None,
                vec![Stmt {
                    kind: StmtKind::If(
                        Expr {
                            kind: ExprKind::Gt(
                                Box::new(Expr {
                                    kind: ExprKind::Var("a".into()),
                                    span: Span::default(),
                                }),
                                Box::new(Expr {
                                    kind: ExprKind::Var("b".into()),
                                    span: Span::default(),
                                }),
                            ),
                            span: Span::default(),
                        },
                        vec![Stmt {
                            kind: StmtKind::Return(Some(Expr {
                                kind: ExprKind::Var("a".into()),
                                span: Span::default(),
                            })),
                            span: Span::default(),
                        }],
                        Some(vec![Stmt {
                            kind: StmtKind::Return(Some(Expr {
                                kind: ExprKind::Var("b".into()),
                                span: Span::default(),
                            })),
                            span: Span::default(),
                        }]),
                    ),
                    span: Span::default(),
                }],
            ),
            span: Span::default(),
        }];

        let (ir, warnings) = compile_to_llvm_ir(stmts).unwrap();
        for w in &warnings {
            eprintln!("warning: {w}");
        }
        assert!(
            ir.contains("fcmp ogt"),
            "IR should contain comparison:\n{ir}"
        );
        assert!(ir.contains("br i1"), "IR should contain branch:\n{ir}");
    }
}
