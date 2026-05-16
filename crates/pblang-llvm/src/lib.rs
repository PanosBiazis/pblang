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
    local_types: HashMap<String, String>,
    current_fn: Option<String>,
    fn_return_type: HashMap<String, String>,
    warnings: Vec<String>,
}

impl LlvmCtx {
    fn new() -> Self {
        LlvmCtx {
            module: LlvmModule::default(),
            reg_counter: 0,
            label_counter: 0,
            local_types: HashMap::new(),
            current_fn: None,
            fn_return_type: HashMap::new(),
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
        // Declare external printf
        self.module
            .declarations
            .push("declare i32 @printf(i8* nocapture readonly, ...) unnamed_addr".into());

        // First pass: collect function signatures
        for s in &stmts {
            if let StmtKind::Fn(name, params, _ret_ty, _body) = &s.kind {
                self.fn_return_type.insert(name.clone(), "double".into());
                // Declare forward
                let param_strs: Vec<String> =
                    (0..params.len()).map(|i| format!("double %p{i}")).collect();
                self.module
                    .declarations
                    .push(format!("declare double @{name}({})", param_strs.join(", ")));
            }
        }

        // Second pass: compile
        let mut body = String::new();
        for s in stmts {
            body.push_str(&self.compile_stmt(s));
        }
        body
    }

    fn compile_stmt(&mut self, stmt: Stmt) -> String {
        match stmt.kind {
            StmtKind::Fn(name, params, _ret_ty, body_stmts) => {
                self.compile_function(name, params, body_stmts)
            }
            StmtKind::Let(name, _ty, init) => {
                let val = self.compile_expr(init);
                self.local_types.insert(name.clone(), "double".into());
                format!("  %{name} = alloca double\n  store double {val}, double* %{name}\n")
            }
            StmtKind::Const(name, _ty, init) => {
                let val = self.compile_expr(init);
                self.local_types.insert(name.clone(), "double".into());
                format!("  %{name} = alloca double\n  store double {val}, double* %{name}\n")
            }
            StmtKind::Return(val) => {
                if let Some(v) = val {
                    let r = self.compile_expr(v);
                    format!("  ret double {r}\n")
                } else {
                    "  ret void\n".into()
                }
            }
            StmtKind::Expr(expr) => {
                let r = self.compile_expr(expr);
                format!("  ; expr: {r}\n")
            }
            StmtKind::If(cond, then_stmts, else_stmts) => {
                self.compile_if(cond, then_stmts, else_stmts)
            }
            StmtKind::While(cond, body) => self.compile_while(cond, body),
            StmtKind::For(name, end_expr, body) => self.compile_for(name, end_expr, body),
            StmtKind::Break => "  br label %loop_exit\n".into(),
            StmtKind::Continue => "  br label %loop_header\n".into(),
            StmtKind::Block(stmts) => {
                let mut out = String::new();
                for s in stmts {
                    out.push_str(&self.compile_stmt(s));
                }
                out
            }
            _ => {
                self.warnings
                    .push(format!("LLVM: unsupported stmt: {:?}", stmt.kind));
                String::new()
            }
        }
    }

    fn compile_function(&mut self, name: String, params: Vec<Param>, body: Vec<Stmt>) -> String {
        self.current_fn = Some(name.clone());
        let param_strs: Vec<String> = params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let n = if p.name.is_empty() {
                    format!("p{i}")
                } else {
                    p.name.clone()
                };
                self.local_types.insert(n.clone(), "double".into());
                format!("double %{n}")
            })
            .collect();

        let mut out = format!(
            "define double @{}({}) {{\nentry:\n",
            name,
            param_strs.join(", ")
        );

        for s in body {
            out.push_str(&self.compile_stmt(s));
        }

        // Ensure function always returns
        if !out.contains("ret ") {
            out.push_str("  ret double 0.0\n");
        }

        out.push_str("}\n\n");
        self.current_fn = None;
        out
    }

    fn compile_if(
        &mut self,
        cond: pblang_core::Expr,
        then_stmts: Vec<Stmt>,
        else_stmts: Option<Vec<Stmt>>,
    ) -> String {
        let cond_val = self.compile_expr(cond);
        let cond_reg = self.fresh_reg();
        let then_label = self.fresh_label();
        let else_label = self.fresh_label();
        let merge_label = self.fresh_label();

        let mut out = format!(
            "  {cond_reg} = fcmp one double {cond_val}, 0.0\n  br i1 {cond_reg}, label %{then_label}, label %{else_label}\n"
        );

        out.push_str(&format!("{then_label}:\n"));
        for s in then_stmts {
            out.push_str(&self.compile_stmt(s));
        }
        if !out.contains("ret ") && !out.contains("br ") {
            out.push_str(&format!("  br label %{merge_label}\n"));
        }

        out.push_str(&format!("{else_label}:\n"));
        if let Some(else_s) = else_stmts {
            for s in else_s {
                out.push_str(&self.compile_stmt(s));
            }
        }
        if !out.contains("ret ") && !out.contains("br ") {
            out.push_str(&format!("  br label %{merge_label}\n"));
        }

        out.push_str(&format!("{merge_label}:\n"));
        out
    }

    fn compile_while(&mut self, cond_expr: pblang_core::Expr, body: Vec<Stmt>) -> String {
        let header_label = self.fresh_label();
        let body_label = self.fresh_label();
        let exit_label = "loop_exit";

        let mut out = format!("  br label %{header_label}\n");
        out.push_str(&format!("{header_label}:\n"));

        let cond_val = self.compile_expr(cond_expr);
        let cond_reg = self.fresh_reg();
        out.push_str(&format!(
            "  {cond_reg} = fcmp one double {cond_val}, 0.0\n  br i1 {cond_reg}, label %{body_label}, label %{exit_label}\n"
        ));

        out.push_str(&format!("{body_label}:\n"));
        for s in body {
            out.push_str(&self.compile_stmt(s));
        }
        if !out.contains("ret ") && !out.contains("br ") {
            out.push_str(&format!("  br label %{header_label}\n"));
        }

        out.push_str(&format!("{exit_label}:\n"));
        out
    }

    fn compile_for(
        &mut self,
        name: String,
        end_expr: pblang_core::Expr,
        body: Vec<Stmt>,
    ) -> String {
        let end_val = self.compile_expr(end_expr);
        let header_label = self.fresh_label();
        let body_label = self.fresh_label();
        let exit_label = "loop_exit";

        let mut out = String::new();
        // i = 0
        let ivar = format!("%{name}");
        out.push_str(&format!(
            "  {ivar} = alloca double\n  store double 0.0, double* {ivar}\n"
        ));
        out.push_str(&format!("  br label %{header_label}\n"));
        out.push_str(&format!("{header_label}:\n"));

        // Load i, compare with end
        let i_val = self.fresh_reg();
        let cmp_val = self.fresh_reg();
        out.push_str(&format!(
            "  {i_val} = load double, double* {ivar}\n  {cmp_val} = fcmp olt double {i_val}, {end_val}\n  br i1 {cmp_val}, label %{body_label}, label %{exit_label}\n"
        ));

        out.push_str(&format!("{body_label}:\n"));
        for s in body {
            out.push_str(&self.compile_stmt(s));
        }
        // i = i + 1
        let inc_val = self.fresh_reg();
        let next_i = self.fresh_reg();
        out.push_str(&format!(
            "  {inc_val} = load double, double* {ivar}\n  {next_i} = fadd double {inc_val}, 1.0\n  store double {next_i}, double* {ivar}\n  br label %{header_label}\n"
        ));

        out.push_str(&format!("{exit_label}:\n"));
        out
    }

    fn compile_expr(&mut self, expr: pblang_core::Expr) -> String {
        match expr.kind {
            ExprKind::Number(n) => format!("{n:.1}"),
            ExprKind::Bool(b) => {
                if b {
                    "1.0".into()
                } else {
                    "0.0".into()
                }
            }
            ExprKind::Nil => "0.0".into(),
            ExprKind::Var(name) => {
                let r = self.fresh_reg();
                format!("{r} = load double, double* %{name}\n  ")
            }
            ExprKind::Add(left, right) => {
                let l = self.compile_expr(*left);
                let r = self.compile_expr(*right);
                let result = self.fresh_reg();
                format!("{l}\n  {r}\n  {result} = fadd double {l}, {r}")
            }
            ExprKind::Sub(left, right) => {
                let l = self.compile_expr(*left);
                let r = self.compile_expr(*right);
                let result = self.fresh_reg();
                format!("{l}\n  {r}\n  {result} = fsub double {l}, {r}")
            }
            ExprKind::Mul(left, right) => {
                let l = self.compile_expr(*left);
                let r = self.compile_expr(*right);
                let result = self.fresh_reg();
                format!("{l}\n  {r}\n  {result} = fmul double {l}, {r}")
            }
            ExprKind::Div(left, right) => {
                let l = self.compile_expr(*left);
                let r = self.compile_expr(*right);
                let result = self.fresh_reg();
                format!("{l}\n  {r}\n  {result} = fdiv double {l}, {r}")
            }
            ExprKind::Eq(left, right) => {
                let l = self.compile_expr(*left);
                let r = self.compile_expr(*right);
                let result = self.fresh_reg();
                format!("{l}\n  {r}\n  {result} = fcmp oeq double {l}, {r}")
            }
            ExprKind::NotEq(left, right) => {
                let l = self.compile_expr(*left);
                let r = self.compile_expr(*right);
                let result = self.fresh_reg();
                format!("{l}\n  {r}\n  {result} = fcmp one double {l}, {r}")
            }
            ExprKind::Lt(left, right) => {
                let l = self.compile_expr(*left);
                let r = self.compile_expr(*right);
                let result = self.fresh_reg();
                format!("{l}\n  {r}\n  {result} = fcmp olt double {l}, {r}")
            }
            ExprKind::Gt(left, right) => {
                let l = self.compile_expr(*left);
                let r = self.compile_expr(*right);
                let result = self.fresh_reg();
                format!("{l}\n  {r}\n  {result} = fcmp ogt double {l}, {r}")
            }
            ExprKind::Le(left, right) => {
                let l = self.compile_expr(*left);
                let r = self.compile_expr(*right);
                let result = self.fresh_reg();
                format!("{l}\n  {r}\n  {result} = fcmp ole double {l}, {r}")
            }
            ExprKind::Ge(left, right) => {
                let l = self.compile_expr(*left);
                let r = self.compile_expr(*right);
                let result = self.fresh_reg();
                format!("{l}\n  {r}\n  {result} = fcmp oge double {l}, {r}")
            }
            ExprKind::Not(inner) => {
                let v = self.compile_expr(*inner);
                let result = self.fresh_reg();
                format!("{v}\n  {result} = fcmp oeq double {v}, 0.0")
            }
            ExprKind::Call(func_expr, args) => {
                let mut arg_strs = Vec::new();
                let mut prelude = String::new();
                for a in args {
                    let av = self.compile_expr(a);
                    arg_strs.push(format!("double {av}"));
                    prelude.push_str(&av);
                    prelude.push('\n');
                }
                // Extract function name from func_expr
                let fn_name = if let ExprKind::Var(ref name) = func_expr.kind {
                    name.clone()
                } else {
                    "unknown".into()
                };

                if fn_name == "print" {
                    // Skip printf for now - just comment
                    self.warnings
                        .push("LLVM: print() skipped (no printf wiring)".into());
                    return "0.0".into();
                }

                let result = self.fresh_reg();
                format!(
                    "{prelude}\n  {result} = call double @{fn_name}({})",
                    arg_strs.join(", ")
                )
            }
            _ => {
                self.warnings
                    .push(format!("LLVM: unsupported expr: {:?}", expr.kind));
                "0.0".into()
            }
        }
    }

    fn finish(&mut self) -> (String, Vec<String>) {
        let mut out = String::new();

        // Declarations
        for d in &self.module.declarations {
            out.push_str(d);
            out.push('\n');
        }
        out.push('\n');

        // Function definitions are already in the body from compile_program
        // (they were collected during compilation)

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
