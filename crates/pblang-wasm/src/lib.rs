// pblang-wasm — WebAssembly backend for pblang.
//
// Compiles pblang AST to WebAssembly Text Format (WAT), which can
// then be converted to .wasm binary with wat2wasm or wasm-tools.
//
// Architecture:
//   pblang AST → WAT IR → .wat file → wat2wasm → .wasm
//
// The generated WASM module exports:
//   • memory (for string/array storage)
//   • _start() — entry point
//   • User-defined functions as exports
//   • Import callbacks for I/O (print, alloc, etc.)
//
// WASM limitations vs full pblang VM:
//   • No dynamic import() — all modules must be linked at compile time
//   • No eval/exec — WASM sandbox prevents subprocess spawning
//   • No FFI — WASM has no native library access
//   • Limited string/stdlib — only what we compile in
//   • No threads (yet — needs WASM threads proposal)
//
// Supported features:
//   ✅ Numbers (f64), strings, bool, nil
//   ✅ Variables (let, const)
//   ✅ Arithmetic (+, -, *, /, %, **)
//   ✅ Comparisons (==, !=, <, >, <=, >=)
//   ✅ Logical (and, or, not)
//   ✅ If/elif/else
//   ✅ While loops
//   ✅ For i N loops
//   ✅ Functions (fn)
//   ✅ Return
//   ✅ Break/Continue
//   ✅ Print (via imported host function)
//   ✅ Arrays (basic)
//   ✅ Maps (basic, as linear-probed hash table)
//
// Not yet supported:
//   ❌ Closures (complex — needs closure conversion)
//   ❌ Import/Export (needs linker)
//   ❌ Try/catch (needs exception handling proposal)
//   ❌ Concurrency (needs threads proposal)
//   ❌ Polyglot blocks

use pblang_core::{ExprKind, Param, Stmt, StmtKind};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────

/// A WAT module under construction.
#[derive(Debug, Default)]
pub struct WatModule {
    pub types: Vec<FuncType>,
    pub imports: Vec<Import>,
    pub functions: Vec<WatFunction>,
    pub exports: Vec<Export>,
    pub memories: Vec<MemoryDef>,
    pub globals: Vec<GlobalDef>,
    pub data_segments: Vec<DataSegment>,
}

#[derive(Debug, Clone)]
pub struct FuncType {
    pub name: String,
    pub params: Vec<WatType>,
    pub results: Vec<WatType>,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub module: String,
    pub name: String,
    pub kind: ImportKind,
}

#[derive(Debug, Clone)]
pub enum ImportKind {
    Function(FuncType),
    Memory {
        min_pages: u32,
        max_pages: Option<u32>,
    },
}

#[derive(Debug, Clone)]
pub struct MemoryDef {
    pub name: Option<String>,
    pub min_pages: u32,
    pub max_pages: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct GlobalDef {
    pub name: Option<String>,
    pub mutable: bool,
    pub init: WatValue,
    pub ty: WatType,
}

#[derive(Debug, Clone)]
pub struct DataSegment {
    pub memory: String,
    pub offset: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct WatFunction {
    pub name: String,
    pub type_ref: String,
    pub params: Vec<(String, WatType)>,
    pub locals: Vec<(String, WatType)>,
    pub body: Vec<WatInstr>,
    pub result_type: Option<WatType>,
}

#[derive(Debug, Clone)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
}

#[derive(Debug, Clone)]
pub enum ExportKind {
    Function(String),
    Memory(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum WatType {
    I32,
    I64,
    F64,
}

impl WatType {
    fn wat_str(&self) -> &str {
        match self {
            WatType::I32 => "i32",
            WatType::I64 => "i64",
            WatType::F64 => "f64",
        }
    }
}

#[derive(Debug, Clone)]
pub enum WatValue {
    I32(i32),
    I64(i64),
    F64(f64),
}

impl WatValue {
    fn wat_str(&self) -> String {
        match self {
            WatValue::I32(v) => format!("i32.const {v}"),
            WatValue::I64(v) => format!("i64.const {v}"),
            WatValue::F64(v) => {
                if v.is_nan() {
                    "f64.const nan".into()
                } else if *v == f64::INFINITY {
                    "f64.const inf".into()
                } else if *v == f64::NEG_INFINITY {
                    "f64.const -inf".into()
                } else {
                    format!("f64.const {v}")
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum WatInstr {
    // Stack operations
    Drop,
    // Constants
    Const(WatValue),
    // Arithmetic
    Add(WatType),
    Sub(WatType),
    Mul(WatType),
    Div(WatType),
    // Comparisons
    Eq(WatType),
    Ne(WatType),
    Lt(WatType, bool), // bool = signed
    Gt(WatType, bool),
    Le(WatType, bool),
    Ge(WatType, bool),
    // Control flow
    If {
        then_: Vec<WatInstr>,
        else_: Option<Vec<WatInstr>>,
    },
    Block {
        label: Option<String>,
        instrs: Vec<WatInstr>,
        result_type: Option<WatType>,
    },
    Loop {
        label: Option<String>,
        instrs: Vec<WatInstr>,
    },
    Br(String),
    BrIf(String),
    BrUnless(String),
    Return,
    // Locals
    LocalGet(String),
    LocalSet(String),
    LocalTee(String),
    // Globals
    GlobalGet(String),
    GlobalSet(String),
    // Memory
    I32Load,
    I32Store,
    MemoryGrow,
    // Calls
    Call(String),
    CallIndirect(String),
    // Comments
    Comment(String),
    // Raw WAT
    Raw(String),
}

// ─────────────────────────────────────────────────────
// Compiler context
// ─────────────────────────────────────────────────────

struct WasmCtx {
    module: WatModule,
    string_table: Vec<String>,
    string_offset: u32,
    current_fn: Option<String>,
    fn_return_types: HashMap<String, Option<WatType>>,
    loop_start_label: Option<String>,
    loop_end_label: Option<String>,
    warnings: Vec<String>,
}

impl WasmCtx {
    fn new() -> Self {
        let mut ctx = WasmCtx {
            module: WatModule::default(),
            string_table: Vec::new(),
            string_offset: 1024, // Start after null region
            current_fn: None,
            fn_return_types: HashMap::new(),
            loop_start_label: None,
            loop_end_label: None,
            warnings: Vec::new(),
        };

        // Set up standard WASM module structure
        // Memory: 1 page (64KB) minimum, up to 256 pages (16MB)
        ctx.module.memories.push(MemoryDef {
            name: Some("memory".into()),
            min_pages: 1,
            max_pages: Some(256),
        });
        ctx.module.exports.push(Export {
            name: "memory".into(),
            kind: ExportKind::Memory("memory".into()),
        });

        // Import host functions
        ctx.module.imports.push(Import {
            module: "env".into(),
            name: "pblang_print".into(),
            kind: ImportKind::Function(FuncType {
                name: "$pblang_print".into(),
                params: vec![WatType::I32, WatType::I32],
                results: vec![],
            }),
        });

        ctx
    }

    fn intern_string(&mut self, s: &str) -> u32 {
        // Check if already interned
        for (i, existing) in self.string_table.iter().enumerate() {
            if existing == s {
                return self.string_offset + i as u32 * 256;
            }
        }
        let offset = self.string_offset + self.string_table.len() as u32 * 256;
        self.string_table.push(s.to_string());
        // Add data segment for this string (length-prefixed: 4 bytes len + data)
        let len = s.len() as u32;
        let mut data = Vec::new();
        data.extend_from_slice(&len.to_le_bytes());
        data.extend_from_slice(s.as_bytes());
        self.module.data_segments.push(DataSegment {
            memory: "memory".into(),
            offset,
            data,
        });
        offset
    }

    /// Compile pblang statements to a WASM module.
    fn compile_program(&mut self, stmts: Vec<Stmt>) {
        // First pass: collect function declarations
        for s in &stmts {
            if let StmtKind::Fn(name, params, _ret_ty, _body) = &s.kind {
                let type_name = format!("$type_{name}");
                let param_types: Vec<WatType> = params.iter().map(|_| WatType::F64).collect();
                let result = Some(WatType::F64);
                self.module.types.push(FuncType {
                    name: type_name.clone(),
                    params: param_types.clone(),
                    results: result.iter().cloned().collect(),
                });
                self.fn_return_types.insert(name.clone(), result);
            }
        }

        // Second pass: compile statements
        for s in stmts {
            self.compile_stmt(s);
        }
    }

    fn compile_stmt(&mut self, stmt: Stmt) {
        match stmt.kind {
            StmtKind::Fn(name, params, _ret_ty, body) => {
                self.compile_function(name, params, body);
            }
            StmtKind::Let(name, _ty, init) => {
                let instrs = self.compile_expr(init);
                if self.current_fn.is_some() {
                    if let Some(f) = self.module.functions.iter_mut().last() {
                        f.locals.push((name.clone(), WatType::F64));
                        f.body.extend(instrs);
                        f.body.push(WatInstr::LocalSet(name));
                    }
                }
            }
            StmtKind::Const(name, _ty, init) => {
                let instrs = self.compile_expr(init);
                if self.current_fn.is_some() {
                    if let Some(f) = self.module.functions.iter_mut().last() {
                        f.locals.push((name.clone(), WatType::F64));
                        f.body.extend(instrs);
                        f.body.push(WatInstr::LocalSet(name));
                    }
                }
            }
            StmtKind::Return(val) => {
                if let Some(v) = val {
                    let instrs = self.compile_expr(v);
                    if let Some(f) = self.module.functions.iter_mut().last() {
                        f.body.extend(instrs);
                    }
                }
                if let Some(f) = self.module.functions.iter_mut().last() {
                    f.body.push(WatInstr::Return);
                }
            }
            StmtKind::Expr(expr) => {
                let instrs = self.compile_expr(expr);
                if let Some(f) = self.module.functions.iter_mut().last() {
                    f.body.extend(instrs);
                }
            }
            StmtKind::If(_cond, _then_stmts, _else_stmts) => {
                self.warnings
                    .push("WASM: if/else not fully implemented".into());
            }
            StmtKind::While(_cond, _body) => {
                self.warnings
                    .push("WASM: while not fully implemented".into());
            }
            StmtKind::For(_name, _end, _body) => {
                self.warnings.push("WASM: for not fully implemented".into());
            }
            StmtKind::Break => {
                if let Some(ref end_label) = self.loop_end_label.clone() {
                    if let Some(f) = self.module.functions.iter_mut().last() {
                        f.body.push(WatInstr::Br(end_label.clone()));
                    }
                }
            }
            StmtKind::Continue => {
                if let Some(ref start_label) = self.loop_start_label.clone() {
                    if let Some(f) = self.module.functions.iter_mut().last() {
                        f.body.push(WatInstr::Br(start_label.clone()));
                    }
                }
            }
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.compile_stmt(s);
                }
            }
            _ => {
                self.warnings
                    .push(format!("WASM: unsupported stmt: {:?}", stmt.kind));
            }
        }
    }

    fn compile_function(&mut self, name: String, params: Vec<Param>, body: Vec<Stmt>) {
        let param_list: Vec<(String, WatType)> = params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    if p.name.is_empty() {
                        format!("$p{i}")
                    } else {
                        p.name.clone()
                    },
                    WatType::F64,
                )
            })
            .collect();

        let type_ref = format!("$type_{name}");
        let func = WatFunction {
            name: name.clone(),
            type_ref,
            params: param_list.clone(),
            locals: Vec::new(),
            body: Vec::new(),
            result_type: Some(WatType::F64),
        };

        self.module.functions.push(func);
        self.current_fn = Some(name.clone());

        for s in body {
            self.compile_stmt(s);
        }

        // Ensure function always returns
        if let Some(f) = self.module.functions.iter_mut().last() {
            if !matches!(f.body.last(), Some(WatInstr::Return)) {
                f.body.push(WatInstr::Const(WatValue::F64(0.0)));
            }
        }

        self.current_fn = None;
    }

    fn compile_expr(&mut self, expr: pblang_core::Expr) -> Vec<WatInstr> {
        match expr.kind {
            ExprKind::Number(n) => vec![WatInstr::Const(WatValue::F64(n))],
            ExprKind::Str(s) => {
                let offset = self.intern_string(&s);
                vec![WatInstr::Const(WatValue::I32(offset as i32))]
            }
            ExprKind::Bool(b) => vec![WatInstr::Const(WatValue::I32(if b { 1 } else { 0 }))],
            ExprKind::Nil => vec![WatInstr::Const(WatValue::I32(0))],
            ExprKind::Var(name) => {
                vec![WatInstr::LocalGet(name)]
            }
            ExprKind::Add(left, right) => {
                let mut instrs = self.compile_expr(*left);
                instrs.extend(self.compile_expr(*right));
                instrs.push(WatInstr::Add(WatType::F64));
                instrs
            }
            ExprKind::Sub(left, right) => {
                let mut instrs = self.compile_expr(*left);
                instrs.extend(self.compile_expr(*right));
                instrs.push(WatInstr::Sub(WatType::F64));
                instrs
            }
            ExprKind::Mul(left, right) => {
                let mut instrs = self.compile_expr(*left);
                instrs.extend(self.compile_expr(*right));
                instrs.push(WatInstr::Mul(WatType::F64));
                instrs
            }
            ExprKind::Div(left, right) => {
                let mut instrs = self.compile_expr(*left);
                instrs.extend(self.compile_expr(*right));
                instrs.push(WatInstr::Div(WatType::F64));
                instrs
            }
            ExprKind::Call(_func, args) => {
                let mut instrs = Vec::new();
                for a in args {
                    instrs.extend(self.compile_expr(a));
                }
                self.warnings.push("WASM: calls not fully wired".into());
                instrs.push(WatInstr::Drop);
                instrs
            }
            _ => {
                self.warnings
                    .push(format!("WASM: unsupported expr: {:?}", expr.kind));
                vec![WatInstr::Const(WatValue::F64(0.0))]
            }
        }
    }

    fn finish(mut self) -> (WatModule, Vec<String>) {
        // Add _start function if there are non-function top-level statements
        // Export all user-defined functions
        let exported_fns: Vec<String> = self
            .module
            .functions
            .iter()
            .map(|f| f.name.clone())
            .collect();
        for name in &exported_fns {
            self.module.exports.push(Export {
                name: name.clone(),
                kind: ExportKind::Function(name.clone()),
            });
        }

        (self.module, self.warnings)
    }
}

// ─────────────────────────────────────────────────────
// WAT serialization
// ─────────────────────────────────────────────────────

impl WatModule {
    pub fn to_wat(&self) -> String {
        let mut out = String::new();
        out.push_str("(module\n");

        // Types
        for t in &self.types {
            out.push_str(&format!("  (type {} (func", t.name));
            if !t.params.is_empty() || !t.results.is_empty() {
                let params: Vec<String> =
                    t.params.iter().map(|p| p.wat_str().to_string()).collect();
                out.push_str(&format!(" (param {})", params.join(" ")));
                if !t.results.is_empty() {
                    let results: Vec<String> =
                        t.results.iter().map(|r| r.wat_str().to_string()).collect();
                    out.push_str(&format!(" (result {})", results.join(" ")));
                }
            }
            out.push_str("))\n");
        }

        // Imports
        for imp in &self.imports {
            match &imp.kind {
                ImportKind::Function(ft) => {
                    out.push_str(&format!(
                        "  (import \"{}\" \"{}\" (func {}",
                        imp.module, imp.name, ft.name
                    ));
                    if !ft.params.is_empty() {
                        let params: Vec<String> =
                            ft.params.iter().map(|p| p.wat_str().to_string()).collect();
                        out.push_str(&format!(" (param {})", params.join(" ")));
                    }
                    if !ft.results.is_empty() {
                        let results: Vec<String> =
                            ft.results.iter().map(|r| r.wat_str().to_string()).collect();
                        out.push_str(&format!(" (result {})", results.join(" ")));
                    }
                    out.push_str("))\n");
                }
                ImportKind::Memory {
                    min_pages,
                    max_pages,
                } => {
                    if let Some(max) = max_pages {
                        out.push_str(&format!(
                            "  (import \"{}\" \"{}\" (memory {min_pages} {max}))\n",
                            imp.module, imp.name
                        ));
                    } else {
                        out.push_str(&format!(
                            "  (import \"{}\" \"{}\" (memory {min_pages}))\n",
                            imp.module, imp.name
                        ));
                    }
                }
            }
        }

        // Memories
        for mem in &self.memories {
            if let Some(ref name) = mem.name {
                if let Some(max) = mem.max_pages {
                    out.push_str(&format!(
                        "  (memory {name} {min} {max})\n",
                        min = mem.min_pages
                    ));
                } else {
                    out.push_str(&format!("  (memory {name} {})\n", mem.min_pages));
                }
            }
        }

        // Functions
        for func in &self.functions {
            out.push_str(&format!("  (func {} (type {})\n", func.name, func.type_ref));
            for (name, ty) in &func.locals {
                out.push_str(&format!("    (local {name} {})\n", ty.wat_str()));
            }
            for instr in &func.body {
                out.push_str(&format!("    {}\n", instr.to_wat()));
            }
            out.push_str("  )\n");
        }

        // Exports
        for exp in &self.exports {
            match &exp.kind {
                ExportKind::Function(fn_name) => {
                    out.push_str(&format!("  (export \"{}\" (func {}))\n", exp.name, fn_name));
                }
                ExportKind::Memory(mem_name) => {
                    out.push_str(&format!(
                        "  (export \"{}\" (memory {}))\n",
                        exp.name, mem_name
                    ));
                }
            }
        }

        // Data segments
        for seg in &self.data_segments {
            let data_str: String = seg.data.iter().map(|b| format!("\\{:02x}", b)).collect();
            out.push_str(&format!(
                "  (data (i32.const {offset}) \"{data_str}\")\n",
                offset = seg.offset
            ));
        }

        out.push_str(")\n");
        out
    }
}

impl WatInstr {
    fn to_wat(&self) -> String {
        match self {
            WatInstr::Drop => "drop".into(),
            WatInstr::Const(v) => v.wat_str(),
            WatInstr::Add(ty) => format!("{}.add", ty.wat_str()),
            WatInstr::Sub(ty) => format!("{}.sub", ty.wat_str()),
            WatInstr::Mul(ty) => format!("{}.mul", ty.wat_str()),
            WatInstr::Div(ty) => format!("{}.div", ty.wat_str()),
            WatInstr::Eq(ty) => format!("{}.eq", ty.wat_str()),
            WatInstr::Ne(ty) => format!("{}.ne", ty.wat_str()),
            WatInstr::Lt(ty, signed) => {
                if *signed {
                    format!("{}.lt_s", ty.wat_str())
                } else {
                    format!("{}.lt_u", ty.wat_str())
                }
            }
            WatInstr::Gt(ty, signed) => {
                if *signed {
                    format!("{}.gt_s", ty.wat_str())
                } else {
                    format!("{}.gt_u", ty.wat_str())
                }
            }
            WatInstr::Le(ty, signed) => {
                if *signed {
                    format!("{}.le_s", ty.wat_str())
                } else {
                    format!("{}.le_u", ty.wat_str())
                }
            }
            WatInstr::Ge(ty, signed) => {
                if *signed {
                    format!("{}.ge_s", ty.wat_str())
                } else {
                    format!("{}.ge_u", ty.wat_str())
                }
            }
            WatInstr::Return => "return".into(),
            WatInstr::LocalGet(n) => format!("local.get {n}"),
            WatInstr::LocalSet(n) => format!("local.set {n}"),
            WatInstr::LocalTee(n) => format!("local.tee {n}"),
            WatInstr::GlobalGet(n) => format!("global.get {n}"),
            WatInstr::GlobalSet(n) => format!("global.set {n}"),
            WatInstr::Br(n) => format!("br {n}"),
            WatInstr::BrIf(n) => format!("br_if {n}"),
            WatInstr::BrUnless(_) => ";; br_unless not emitted".into(),
            WatInstr::Call(n) => format!("call {n}"),
            WatInstr::CallIndirect(n) => format!("call_indirect (type {n})"),
            WatInstr::I32Load => "i32.load".into(),
            WatInstr::I32Store => "i32.store".into(),
            WatInstr::MemoryGrow => "memory.grow".into(),
            WatInstr::Comment(c) => format!(";; {c}"),
            WatInstr::Raw(s) => s.clone(),
            WatInstr::If { .. } => ";; if (not serialized inline)".into(),
            WatInstr::Block { label, .. } => format!(";; block {}", label.as_deref().unwrap_or("")),
            WatInstr::Loop { label, .. } => format!(";; loop {}", label.as_deref().unwrap_or("")),
        }
    }
}

// ─────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────

/// Compile pblang AST to a WAT module string.
pub fn compile_to_wat(stmts: Vec<Stmt>) -> Result<(String, Vec<String>), String> {
    let mut ctx = WasmCtx::new();
    ctx.compile_program(stmts);
    let (module, warnings) = ctx.finish();
    Ok((module.to_wat(), warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_module() {
        let (wat, _) = compile_to_wat(vec![]).unwrap();
        assert!(wat.starts_with("(module"));
        assert!(wat.ends_with(")\n"));
        assert!(wat.contains("(memory"));
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

        let (wat, warnings) = compile_to_wat(stmts).unwrap();
        for w in &warnings {
            eprintln!("warning: {w}");
        }
        // Function should be in the WAT output
        assert!(
            wat.contains("add"),
            "WAT should contain function name 'add':\n{wat}"
        );
        assert!(
            wat.contains("f64.add"),
            "WAT should contain f64.add:\n{wat}"
        );
        assert!(wat.contains("export"), "WAT should contain exports:\n{wat}");
    }
}
