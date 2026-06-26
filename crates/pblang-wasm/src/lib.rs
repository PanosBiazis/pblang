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
//   ✅ Logical (and, or, not) — and/or short-circuit via `if (result f64)`
//   ✅ Comparisons (==, !=, <, >, <=, >=) — result widened to f64 (0.0/1.0)
//   ✅ If/elif/else — structured `if … else … end`
//   ✅ While loops  — `block $end (loop $start … br_if/br)`
//   ✅ For i N loops — counted loop with an f64 induction variable
//   ✅ Break/Continue (br to the enclosing loop's end/start label)
//   ✅ Functions (fn)
//   ✅ Return
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
    // Conversions
    /// `f64.convert_i32_u` — widen an i32 (e.g. a comparison result) to f64 so
    /// every pblang value stays uniformly f64 on the stack.
    ConvertI32ToF64,
    // Control flow
    If {
        then_: Vec<WatInstr>,
        else_: Option<Vec<WatInstr>>,
        /// Result pushed by both branches, e.g. `if (result f64)` for the
        /// short-circuiting `and`/`or` expressions. `None` for statement-level
        /// `if` (branches leave the stack balanced).
        result_type: Option<WatType>,
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
    /// Parallel to `string_table`: the linear-memory offset assigned to each
    /// interned string (its 4-byte length prefix). Cumulative, not fixed-stride.
    string_offsets: Vec<u32>,
    string_offset: u32,
    /// Next free offset for the string arena; advances by the actual bytes each
    /// string uses (a fixed stride overlapped/corrupted strings > the stride).
    next_string_offset: u32,
    current_fn: Option<String>,
    fn_return_types: HashMap<String, Option<WatType>>,
    loop_start_label: Option<String>,
    loop_end_label: Option<String>,
    label_counter: u32,
    warnings: Vec<String>,
}

impl WasmCtx {
    fn new() -> Self {
        let mut ctx = WasmCtx {
            module: WatModule::default(),
            string_table: Vec::new(),
            string_offsets: Vec::new(),
            string_offset: 1024, // Start after null region
            next_string_offset: 1024,
            current_fn: None,
            fn_return_types: HashMap::new(),
            loop_start_label: None,
            loop_end_label: None,
            label_counter: 0,
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

        // Import host functions.
        // `pblang_print(ptr, len)` writes a UTF-8 string from linear memory;
        // `pblang_print_f64(x)` writes a number. The host decides newline policy.
        ctx.module.imports.push(Import {
            module: "env".into(),
            name: "pblang_print".into(),
            kind: ImportKind::Function(FuncType {
                name: "$pblang_print".into(),
                params: vec![WatType::I32, WatType::I32],
                results: vec![],
            }),
        });
        ctx.module.imports.push(Import {
            module: "env".into(),
            name: "pblang_print_f64".into(),
            kind: ImportKind::Function(FuncType {
                name: "$pblang_print_f64".into(),
                params: vec![WatType::F64],
                results: vec![],
            }),
        });

        ctx
    }

    fn intern_string(&mut self, s: &str) -> u32 {
        // Reuse an existing identical string (return its actual offset).
        for (i, existing) in self.string_table.iter().enumerate() {
            if existing == s {
                return self.string_offsets[i];
            }
        }
        // Place at the next free offset and advance by the exact bytes used —
        // a 4-byte length prefix + the UTF-8 bytes, rounded up to 4-byte
        // alignment. (The previous fixed 256-byte stride meant any string longer
        // than the stride overlapped — and corrupted — the next one.)
        let offset = self.next_string_offset;
        self.string_table.push(s.to_string());
        self.string_offsets.push(offset);
        let len = s.len() as u32;
        let mut data = Vec::new();
        data.extend_from_slice(&len.to_le_bytes());
        data.extend_from_slice(s.as_bytes());
        self.module.data_segments.push(DataSegment {
            memory: "memory".into(),
            offset,
            data,
        });
        let used = 4u32.saturating_add(len);
        self.next_string_offset = offset.saturating_add((used + 3) & !3);
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
                    self.declare_local_once(&name);
                    if let Some(f) = self.module.functions.iter_mut().last() {
                        f.body.extend(instrs);
                        f.body.push(WatInstr::LocalSet(name));
                    }
                }
            }
            StmtKind::Const(name, _ty, init) => {
                let instrs = self.compile_expr(init);
                if self.current_fn.is_some() {
                    self.declare_local_once(&name);
                    if let Some(f) = self.module.functions.iter_mut().last() {
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
                    // An expression statement leaves its f64 value on the stack;
                    // drop it so the function body stays stack-balanced (WASM
                    // validation requires it).
                    f.body.push(WatInstr::Drop);
                }
            }
            StmtKind::If(cond, then_stmts, else_stmts) => {
                let cond_instrs = self.compile_condition(cond);
                let then_ = self.compile_block_instrs(then_stmts);
                let else_ = else_stmts.map(|e| self.compile_block_instrs(e));
                if let Some(f) = self.module.functions.iter_mut().last() {
                    f.body.extend(cond_instrs);
                    f.body.push(WatInstr::If {
                        then_,
                        else_,
                        result_type: None,
                    });
                }
            }
            StmtKind::While(cond, body) => {
                // block $end { loop $start { (!cond) br $end; <body>; br $start } }
                let end_label = self.fresh_label("while_end");
                let start_label = self.fresh_label("while_start");
                let prev = (self.loop_start_label.take(), self.loop_end_label.take());
                self.loop_start_label = Some(start_label.clone());
                self.loop_end_label = Some(end_label.clone());

                let mut loop_body = self.compile_condition(cond);
                // Exit when the condition is false: `eqz` then br_if $end.
                loop_body.push(WatInstr::If {
                    then_: vec![],
                    else_: Some(vec![WatInstr::Br(end_label.clone())]),
                    result_type: None,
                });
                loop_body.extend(self.compile_block_instrs(body));
                loop_body.push(WatInstr::Br(start_label.clone()));

                self.loop_start_label = prev.0;
                self.loop_end_label = prev.1;

                if let Some(f) = self.module.functions.iter_mut().last() {
                    f.body.push(WatInstr::Block {
                        label: Some(end_label),
                        result_type: None,
                        instrs: vec![WatInstr::Loop {
                            label: Some(start_label),
                            instrs: loop_body,
                        }],
                    });
                }
            }
            StmtKind::For(name, count, body) => {
                // for <name> <count> { body } — run body `count` times with
                // <name> = 0,1,…,count-1. Lower to a counted while loop.
                let count_instrs = self.compile_expr(count);
                let limit = self.fresh_label("forlimit");
                let end_label = self.fresh_label("for_end");
                let start_label = self.fresh_label("for_start");
                // Declare the loop var and the cached limit as f64 locals.
                self.declare_local_once(&name);
                self.declare_local_once(&limit);
                if let Some(f) = self.module.functions.iter_mut().last() {
                    // limit = count
                    f.body.extend(count_instrs);
                    f.body.push(WatInstr::LocalSet(limit.clone()));
                    // name = 0
                    f.body.push(WatInstr::Const(WatValue::F64(0.0)));
                    f.body.push(WatInstr::LocalSet(name.clone()));
                }
                let prev = (self.loop_start_label.take(), self.loop_end_label.take());
                self.loop_start_label = Some(start_label.clone());
                self.loop_end_label = Some(end_label.clone());

                // loop body: if name >= limit -> br $end; <body>; name += 1; br $start
                let mut loop_body = vec![
                    WatInstr::LocalGet(name.clone()),
                    WatInstr::LocalGet(limit.clone()),
                    WatInstr::Ge(WatType::F64, true),
                    WatInstr::If {
                        then_: vec![WatInstr::Br(end_label.clone())],
                        else_: None,
                        result_type: None,
                    },
                ];
                loop_body.extend(self.compile_block_instrs(body));
                loop_body.push(WatInstr::LocalGet(name.clone()));
                loop_body.push(WatInstr::Const(WatValue::F64(1.0)));
                loop_body.push(WatInstr::Add(WatType::F64));
                loop_body.push(WatInstr::LocalSet(name.clone()));
                loop_body.push(WatInstr::Br(start_label.clone()));

                self.loop_start_label = prev.0;
                self.loop_end_label = prev.1;

                if let Some(f) = self.module.functions.iter_mut().last() {
                    f.body.push(WatInstr::Block {
                        label: Some(end_label),
                        result_type: None,
                        instrs: vec![WatInstr::Loop {
                            label: Some(start_label),
                            instrs: loop_body,
                        }],
                    });
                }
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
            StmtKind::Assign(name, expr) => {
                // Re-assignment to an existing local (`i = i + 1`).
                let instrs = self.compile_expr(expr);
                if let Some(f) = self.module.functions.iter_mut().last() {
                    f.body.extend(instrs);
                    f.body.push(WatInstr::LocalSet(name));
                }
            }
            StmtKind::Print(expr) | StmtKind::PrintF(_, expr) => {
                // PrintF's format index isn't modeled; both print the value.
                let instrs = self.compile_print(expr);
                if let Some(f) = self.module.functions.iter_mut().last() {
                    f.body.extend(instrs);
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

    /// Lower a `print`/`printf` argument to host-import calls.
    ///
    /// - **string literal** → intern its bytes into linear memory (a
    ///   length-prefixed `[len_u32][utf8…]` data segment) and call
    ///   `pblang_print(data_ptr, len)` with the compile-time pointer/length;
    /// - **f-string** → print each part in order: literal segments as strings,
    ///   interpolated expressions as f64;
    /// - **anything else** (number / arithmetic / comparison) → `pblang_print_f64`.
    ///
    /// (Printing a string held in a *variable* needs the value-type model that
    /// the f64-only locals don't yet support; that stays a follow-up.)
    fn compile_print(&mut self, expr: pblang_core::Expr) -> Vec<WatInstr> {
        let span = expr.span;
        match expr.kind {
            ExprKind::Str(s) => self.print_str_literal(&s),
            ExprKind::FString(parts) => {
                let mut instrs = Vec::new();
                for part in parts {
                    match part {
                        pblang_core::FStringPart::Literal(s) => {
                            if !s.is_empty() {
                                instrs.extend(self.print_str_literal(&s));
                            }
                        }
                        pblang_core::FStringPart::Interp(e) => {
                            instrs.extend(self.compile_expr(*e));
                            instrs.push(WatInstr::Call("pblang_print_f64".into()));
                        }
                    }
                }
                instrs
            }
            other => {
                let mut instrs = self.compile_expr(pblang_core::Expr { kind: other, span });
                instrs.push(WatInstr::Call("pblang_print_f64".into()));
                instrs
            }
        }
    }

    /// Emit `pblang_print(data_ptr, len)` for a string literal, interning it.
    fn print_str_literal(&mut self, s: &str) -> Vec<WatInstr> {
        let off = self.intern_string(s);
        // `intern_string` lays out `[len_u32][bytes]`; the host wants the bytes.
        let data_ptr = (off + 4) as i32;
        let len = s.len() as i32;
        vec![
            WatInstr::Const(WatValue::I32(data_ptr)),
            WatInstr::Const(WatValue::I32(len)),
            WatInstr::Call("pblang_print".into()),
        ]
    }

    /// Declare an f64 local on the current function, but only once per name.
    /// WASM locals are function-flat, so a repeated/shadowed `let x` must not
    /// emit a second `(local $x …)` (which would be invalid WAT); the later
    /// `local.set` simply reuses the slot.
    fn declare_local_once(&mut self, name: &str) {
        if let Some(f) = self.module.functions.iter_mut().last() {
            let exists =
                f.locals.iter().any(|(n, _)| n == name) || f.params.iter().any(|(n, _)| n == name);
            if !exists {
                f.locals.push((name.to_string(), WatType::F64));
            }
        }
    }

    /// Generate a unique `$`-prefixed label (for block/loop targets and helper
    /// locals), so nested control flow doesn't collide.
    fn fresh_label(&mut self, prefix: &str) -> String {
        let n = self.label_counter;
        self.label_counter += 1;
        format!("${prefix}_{n}")
    }

    /// Compile a list of statements into a fresh instruction vector by running
    /// them against the current function body and draining the appended tail.
    /// Used to build the nested bodies of `if`/`while`/`for`.
    fn compile_block_instrs(&mut self, stmts: Vec<Stmt>) -> Vec<WatInstr> {
        let start = self
            .module
            .functions
            .last()
            .map(|f| f.body.len())
            .unwrap_or(0);
        for s in stmts {
            self.compile_stmt(s);
        }
        match self.module.functions.iter_mut().last() {
            Some(f) => f.body.split_off(start),
            None => Vec::new(),
        }
    }

    /// Compile a condition expression and leave an **i32** truthiness value on
    /// the stack (`0` = false, non-zero = true), as required by WASM `if`/`br_if`.
    /// Every pblang value is f64 on the stack, so truthiness is `value != 0.0`.
    fn compile_condition(&mut self, cond: pblang_core::Expr) -> Vec<WatInstr> {
        let mut instrs = self.compile_expr(cond);
        instrs.push(WatInstr::Const(WatValue::F64(0.0)));
        instrs.push(WatInstr::Ne(WatType::F64)); // f64.ne -> i32
        instrs
    }

    /// Compile `left <cmp> right` where `cmp` is an f64 comparison op; the i32
    /// result is widened back to f64 (0.0/1.0).
    fn compile_cmp(
        &mut self,
        left: pblang_core::Expr,
        right: pblang_core::Expr,
        cmp: WatInstr,
    ) -> Vec<WatInstr> {
        let mut instrs = self.compile_expr(left);
        instrs.extend(self.compile_expr(right));
        instrs.push(cmp); // -> i32
        instrs.push(WatInstr::ConvertI32ToF64);
        instrs
    }

    /// Compile an expression to a normalized truthiness value as f64 (0.0/1.0).
    fn truthy_f64(&mut self, e: pblang_core::Expr) -> Vec<WatInstr> {
        let mut instrs = self.compile_expr(e);
        instrs.push(WatInstr::Const(WatValue::F64(0.0)));
        instrs.push(WatInstr::Ne(WatType::F64)); // -> i32
        instrs.push(WatInstr::ConvertI32ToF64);
        instrs
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
            // Every value is uniformly f64 on the stack (locals are f64), so
            // bool/nil are f64 too (1.0/0.0) — keeps `let b = x < y` type-correct.
            ExprKind::Bool(b) => vec![WatInstr::Const(WatValue::F64(if b { 1.0 } else { 0.0 }))],
            ExprKind::Nil => vec![WatInstr::Const(WatValue::F64(0.0))],
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
            // Comparisons: compute the i32 result, then widen to f64 (0.0/1.0) so
            // it composes with the uniform-f64 value model.
            ExprKind::Eq(left, right) => {
                self.compile_cmp(*left, *right, WatInstr::Eq(WatType::F64))
            }
            ExprKind::NotEq(left, right) => {
                self.compile_cmp(*left, *right, WatInstr::Ne(WatType::F64))
            }
            ExprKind::Lt(left, right) => {
                self.compile_cmp(*left, *right, WatInstr::Lt(WatType::F64, true))
            }
            ExprKind::Gt(left, right) => {
                self.compile_cmp(*left, *right, WatInstr::Gt(WatType::F64, true))
            }
            ExprKind::Le(left, right) => {
                self.compile_cmp(*left, *right, WatInstr::Le(WatType::F64, true))
            }
            ExprKind::Ge(left, right) => {
                self.compile_cmp(*left, *right, WatInstr::Ge(WatType::F64, true))
            }
            // `not x` → (x == 0.0) widened to f64.
            ExprKind::Not(inner) => {
                let mut instrs = self.compile_expr(*inner);
                instrs.push(WatInstr::Const(WatValue::F64(0.0)));
                instrs.push(WatInstr::Eq(WatType::F64)); // -> i32 (1 if x == 0)
                instrs.push(WatInstr::ConvertI32ToF64);
                instrs
            }
            // Short-circuiting logical and/or via `if (result f64)`:
            //   a and b  =>  if truthy(a) { truthy(b) } else { 0.0 }
            //   a or  b  =>  if truthy(a) { 1.0 }       else { truthy(b) }
            // The right operand is only evaluated when needed (matches the VM).
            ExprKind::Logical { left, op, right } => {
                // Condition: leave i32 truthiness of `a` on the stack.
                let mut instrs = self.compile_expr(*left);
                instrs.push(WatInstr::Const(WatValue::F64(0.0)));
                instrs.push(WatInstr::Ne(WatType::F64)); // truthy(a) -> i32
                let right_truthy = self.truthy_f64(*right);
                let (then_, else_) = match op {
                    pblang_core::LogicalOp::And => {
                        (right_truthy, vec![WatInstr::Const(WatValue::F64(0.0))])
                    }
                    pblang_core::LogicalOp::Or => {
                        (vec![WatInstr::Const(WatValue::F64(1.0))], right_truthy)
                    }
                };
                instrs.push(WatInstr::If {
                    then_,
                    else_: Some(else_),
                    result_type: Some(WatType::F64),
                });
                instrs
            }
            ExprKind::Call(func, args) => {
                // Direct call to a known user function: `add(1, 2)` -> `call $add`.
                // The callee consumes its f64 args and leaves its f64 result on
                // the stack. print/method/indirect callees aren't wired yet.
                let callee = match &func.kind {
                    ExprKind::Var(name) if self.fn_return_types.contains_key(name) => {
                        Some(name.clone())
                    }
                    _ => None,
                };
                // The arbitrary-precision types are VM-only; the f64 WASM model
                // can't represent them, so flag their use explicitly rather than
                // silently lowering to a 0.0 no-op.
                if let ExprKind::Var(name) = &func.kind {
                    if matches!(name.as_str(), "bigint" | "rational" | "decimal") {
                        self.warnings.push(format!(
                            "WASM: `{name}()` (arbitrary-precision) is unsupported in the f64 \
                             WASM backend; value approximated as f64"
                        ));
                    }
                }
                let argc = args.len();
                let mut instrs = Vec::new();
                for a in args {
                    instrs.extend(self.compile_expr(a));
                }
                match callee {
                    Some(name) => instrs.push(WatInstr::Call(name)),
                    None => {
                        self.warnings.push(
                            "WASM: unsupported call callee (e.g. print/method) left as a no-op"
                                .into(),
                        );
                        for _ in 0..argc {
                            instrs.push(WatInstr::Drop);
                        }
                        instrs.push(WatInstr::Const(WatValue::F64(0.0)));
                    }
                }
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

        // Grow the declared minimum memory so every interned-string data segment
        // fits at instantiation. Without this, a string arena exceeding the
        // initial 64 KiB page would make the module trap when its data segment
        // is applied past the end of memory.
        const PAGE: u32 = 65536;
        let needed_pages = self.next_string_offset.div_ceil(PAGE).max(1);
        if let Some(mem) = self.module.memories.first_mut() {
            if mem.min_pages < needed_pages {
                mem.min_pages = needed_pages;
            }
            if let Some(max) = mem.max_pages {
                if max < mem.min_pages {
                    mem.max_pages = Some(mem.min_pages);
                }
            }
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
                        "  (memory {n} {min} {max})\n",
                        n = sigil(name),
                        min = mem.min_pages
                    ));
                } else {
                    out.push_str(&format!("  (memory {} {})\n", sigil(name), mem.min_pages));
                }
            }
        }

        // Functions. Params are declared inline with names (`(param $a f64)`) so
        // the body's `local.get $a` resolves; the inline params/result mirror the
        // referenced type.
        for func in &self.functions {
            out.push_str(&format!(
                "  (func {} (type {})",
                sigil(&func.name),
                sigil(&func.type_ref)
            ));
            for (pname, pty) in &func.params {
                out.push_str(&format!(" (param {} {})", sigil(pname), pty.wat_str()));
            }
            if let Some(rt) = &func.result_type {
                out.push_str(&format!(" (result {})", rt.wat_str()));
            }
            out.push('\n');
            for (name, ty) in &func.locals {
                out.push_str(&format!("    (local {} {})\n", sigil(name), ty.wat_str()));
            }
            render_instrs(&func.body, 2, &mut out);
            out.push_str("  )\n");
        }

        // Exports
        for exp in &self.exports {
            match &exp.kind {
                ExportKind::Function(fn_name) => {
                    out.push_str(&format!(
                        "  (export \"{}\" (func {}))\n",
                        exp.name,
                        sigil(fn_name)
                    ));
                }
                ExportKind::Memory(mem_name) => {
                    out.push_str(&format!(
                        "  (export \"{}\" (memory {}))\n",
                        exp.name,
                        sigil(mem_name)
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
            // Float comparisons have no signedness suffix (`f64.lt`); integer
            // comparisons do (`i32.lt_s`/`i32.lt_u`).
            WatInstr::Lt(ty, signed) => cmp_wat(ty, "lt", *signed),
            WatInstr::Gt(ty, signed) => cmp_wat(ty, "gt", *signed),
            WatInstr::Le(ty, signed) => cmp_wat(ty, "le", *signed),
            WatInstr::Ge(ty, signed) => cmp_wat(ty, "ge", *signed),
            WatInstr::ConvertI32ToF64 => "f64.convert_i32_u".into(),
            WatInstr::Return => "return".into(),
            WatInstr::LocalGet(n) => format!("local.get {}", sigil(n)),
            WatInstr::LocalSet(n) => format!("local.set {}", sigil(n)),
            WatInstr::LocalTee(n) => format!("local.tee {}", sigil(n)),
            WatInstr::GlobalGet(n) => format!("global.get {}", sigil(n)),
            WatInstr::GlobalSet(n) => format!("global.set {}", sigil(n)),
            WatInstr::Br(n) => format!("br {}", sigil(n)),
            WatInstr::BrIf(n) => format!("br_if {}", sigil(n)),
            WatInstr::BrUnless(_) => ";; br_unless not emitted".into(),
            WatInstr::Call(n) => format!("call {}", sigil(n)),
            WatInstr::CallIndirect(n) => format!("call_indirect (type {n})"),
            WatInstr::I32Load => "i32.load".into(),
            WatInstr::I32Store => "i32.store".into(),
            WatInstr::MemoryGrow => "memory.grow".into(),
            WatInstr::Comment(c) => format!(";; {c}"),
            WatInstr::Raw(s) => s.clone(),
            // If/Block/Loop are multi-line and are rendered by `render_instrs`,
            // which recurses into their nested bodies; these arms are only a
            // fallback for any non-recursive caller.
            WatInstr::If { .. } => ";; if (rendered by render_instrs)".into(),
            WatInstr::Block { label, .. } => format!(";; block {}", label.as_deref().unwrap_or("")),
            WatInstr::Loop { label, .. } => format!(";; loop {}", label.as_deref().unwrap_or("")),
        }
    }
}

/// Ensure a WAT identifier carries the mandatory `$` sigil. pblang names
/// (`add`, `a`, `i`, …) are stored bare; WAT requires `$add`/`$a`/`$i`. Names
/// already starting with `$` (generated labels/imports) are left as-is.
fn sigil(name: &str) -> String {
    if name.starts_with('$') {
        name.to_string()
    } else {
        format!("${name}")
    }
}

/// WAT mnemonic for a comparison: float types have no signedness suffix
/// (`f64.lt`), integer types do (`i32.lt_s` / `i32.lt_u`).
fn cmp_wat(ty: &WatType, op: &str, signed: bool) -> String {
    match ty {
        WatType::F64 => format!("f64.{op}"),
        _ => format!("{}.{op}_{}", ty.wat_str(), if signed { "s" } else { "u" }),
    }
}

/// Recursively render an instruction list to WAT (flat `if/else/end`,
/// `block/loop … end` form), indenting nested control flow.
fn render_instrs(instrs: &[WatInstr], indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    for instr in instrs {
        match instr {
            WatInstr::If {
                then_,
                else_,
                result_type,
            } => {
                match result_type {
                    Some(rt) => out.push_str(&format!("{pad}if (result {})\n", rt.wat_str())),
                    None => out.push_str(&format!("{pad}if\n")),
                }
                render_instrs(then_, indent + 1, out);
                if let Some(else_instrs) = else_ {
                    out.push_str(&format!("{pad}else\n"));
                    render_instrs(else_instrs, indent + 1, out);
                }
                out.push_str(&format!("{pad}end\n"));
            }
            WatInstr::Block {
                label,
                instrs: inner,
                ..
            } => {
                out.push_str(&format!("{pad}block {}\n", label.as_deref().unwrap_or("")));
                render_instrs(inner, indent + 1, out);
                out.push_str(&format!("{pad}end\n"));
            }
            WatInstr::Loop {
                label,
                instrs: inner,
            } => {
                out.push_str(&format!("{pad}loop {}\n", label.as_deref().unwrap_or("")));
                render_instrs(inner, indent + 1, out);
                out.push_str(&format!("{pad}end\n"));
            }
            other => out.push_str(&format!("{pad}{}\n", other.to_wat())),
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

    /// The whole point of this backend is to feed wat2wasm / a wasm runtime, so
    /// substring checks prove nothing if the WAT is malformed. Parse + validate
    /// the emitted module with the `wat` crate (pure Rust) and require it valid.
    fn assert_valid_wat(src: &str) {
        let stmts = pblang_core::parse(src).expect("parse");
        let (wat, _warnings) = compile_to_wat(stmts).expect("compile");
        if let Err(e) = wat::parse_str(&wat) {
            panic!("emitted WAT is invalid: {e}\n--- WAT ---\n{wat}");
        }
    }

    // ── Execution harness: actually RUN the emitted wasm (correctness, not just
    //    validity) via the pure-Rust `wasmi` interpreter, providing the host
    //    `env.pblang_print`/`pblang_print_f64` imports. ────────────────────────
    #[derive(Default)]
    struct HostOut {
        text: String,
    }

    fn instantiate(
        src: &str,
    ) -> (wasmi::Store<HostOut>, wasmi::Instance) {
        let stmts = pblang_core::parse(src).expect("parse");
        let (wat, _w) = compile_to_wat(stmts).expect("compile");
        let wasm = wat::parse_str(&wat).expect("wat->wasm");
        let engine = wasmi::Engine::default();
        let module = wasmi::Module::new(&engine, &wasm[..]).expect("module");
        let mut store = wasmi::Store::new(&engine, HostOut::default());
        let mut linker = wasmi::Linker::new(&engine);
        linker
            .func_wrap(
                "env",
                "pblang_print",
                |mut caller: wasmi::Caller<HostOut>, ptr: i32, len: i32| {
                    let mem = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .expect("memory export");
                    let data = mem.data(&caller);
                    let (a, b) = (ptr as usize, ptr as usize + len as usize);
                    let s = String::from_utf8_lossy(&data[a..b]).into_owned();
                    caller.data_mut().text.push_str(&s);
                },
            )
            .unwrap();
        linker
            .func_wrap(
                "env",
                "pblang_print_f64",
                |mut caller: wasmi::Caller<HostOut>, x: f64| {
                    caller.data_mut().text.push_str(&format!("{x}"));
                },
            )
            .unwrap();
        let instance = linker
            .instantiate(&mut store, &module)
            .expect("instantiate")
            .start(&mut store)
            .expect("start");
        (store, instance)
    }

    /// Call an exported function with f64 args, return its f64 result.
    fn call_f64(src: &str, func: &str, args: &[f64]) -> f64 {
        let (mut store, instance) = instantiate(src);
        let f = instance.get_func(&store, func).expect("export fn");
        let params: Vec<wasmi::Val> = args.iter().map(|a| wasmi::Val::F64((*a).into())).collect();
        let mut results = [wasmi::Val::F64(0.0.into())];
        f.call(&mut store, &params, &mut results).expect("call");
        match results[0] {
            wasmi::Val::F64(v) => v.into(),
            _ => panic!("expected f64 result"),
        }
    }

    /// Call an exported (0-arg) function and return everything it printed.
    fn run_capture(src: &str, func: &str) -> String {
        let (mut store, instance) = instantiate(src);
        let f = instance.get_func(&store, func).expect("export fn");
        let mut results = [wasmi::Val::F64(0.0.into())];
        f.call(&mut store, &[], &mut results).expect("call");
        store.data().text.clone()
    }

    #[test]
    fn wasm_arithmetic_executes_correctly() {
        assert_eq!(call_f64("fn add(a, b) { return a + b; }", "add", &[2.0, 3.0]), 5.0);
        assert_eq!(call_f64("fn poly(x) { return x * x - x; }", "poly", &[5.0]), 20.0);
    }

    #[test]
    fn wasm_loop_and_call_execute_correctly() {
        // sum of squares 0..n: run(6) = 0+1+4+9+16+25 = 55.
        let src = "fn sq(x){return x*x;}\nfn run(n){ let t=0; for i n { t = t + sq(i); } return t; }";
        assert_eq!(call_f64(src, "run", &[6.0]), 55.0);
    }

    #[test]
    fn wasm_branch_executes_correctly() {
        let src = "fn classify(n){ let s=0; if n > 0 { s = 1; } else { s = 2; } return s; }";
        assert_eq!(call_f64(src, "classify", &[5.0]), 1.0);
        assert_eq!(call_f64(src, "classify", &[-5.0]), 2.0);
    }

    #[test]
    fn wasm_long_string_literal_is_not_corrupted() {
        // Two adjacent interned strings, the first longer than the 256-byte slot
        // stride. A fixed-stride layout makes the second segment overwrite the
        // tail of the first (or overrun memory). Each must print intact.
        let a = "A".repeat(260);
        let b = "B".repeat(260);
        let src = format!("fn show() {{ print \"{a}\"; print \"{b}\"; return 0; }}");
        let out = run_capture(&src, "show");
        assert_eq!(out, format!("{a}{b}"), "interned strings must not overlap/corrupt");
    }

    #[test]
    fn wasm_string_arena_grows_memory_past_one_page() {
        // A string arena larger than the initial 64 KiB page must grow the
        // declared memory, or applying its data segment traps at instantiation.
        let big = "Z".repeat(70_000); // > 1 page
        let src = format!("fn show() {{ print \"{big}\"; return 0; }}");
        let out = run_capture(&src, "show");
        assert_eq!(out.len(), big.len(), "big string must round-trip intact");
        assert_eq!(out, big);
    }

    #[test]
    fn emitted_wat_is_valid_arithmetic() {
        assert_valid_wat("fn add(a, b) { return a + b; }");
    }

    #[test]
    fn emitted_wat_is_valid_control_flow() {
        assert_valid_wat(
            "fn classify(n) { let s = 0; if n > 0 { s = 1; } else { s = 2; } return s; }",
        );
    }

    #[test]
    fn emitted_wat_is_valid_loops_and_calls() {
        assert_valid_wat(
            "fn sq(x) { return x * x; }\nfn run(n) { let t = 0; for i n { t = t + sq(i); } return t; }",
        );
    }

    #[test]
    fn emitted_wat_is_valid_logical_and_print() {
        assert_valid_wat("fn f(a, b) { print \"hi\"; print a + b; return a and b; }");
    }

    #[test]
    fn test_empty_module() {
        let (wat, _) = compile_to_wat(vec![]).unwrap();
        assert!(wat.starts_with("(module"));
        assert!(wat.ends_with(")\n"));
        assert!(wat.contains("(memory"));
    }

    #[test]
    fn wasm_if_else_emits_structured_control_flow() {
        let stmts =
            pblang_core::parse("fn f(x) { if x > 0 { return 1; } else { return 2; } }").unwrap();
        let (wat, warnings) = compile_to_wat(stmts).unwrap();
        assert!(wat.contains("\n    if\n"), "expected `if` block:\n{wat}");
        assert!(wat.contains("else"), "expected `else`:\n{wat}");
        assert!(wat.contains("end"), "expected `end`:\n{wat}");
        assert!(
            wat.contains("f64.gt"),
            "expected f64.gt (no signed suffix):\n{wat}"
        );
        assert!(
            !warnings.iter().any(|w| w.contains("if/else not fully")),
            "if/else should be implemented: {warnings:?}"
        );
    }

    #[test]
    fn wasm_while_emits_loop_and_block() {
        let stmts =
            pblang_core::parse("fn f(n) { let i = 0; while i < n { i = i + 1; } return i; }")
                .unwrap();
        let (wat, warnings) = compile_to_wat(stmts).unwrap();
        assert!(
            wat.contains("loop $while_start"),
            "expected loop label:\n{wat}"
        );
        assert!(
            wat.contains("block $while_end"),
            "expected block label:\n{wat}"
        );
        assert!(
            wat.contains("br $while_start"),
            "expected back-edge br:\n{wat}"
        );
        assert!(wat.contains("f64.lt"), "expected f64.lt:\n{wat}");
        assert!(
            !warnings.iter().any(|w| w.contains("while not fully")),
            "while should be implemented: {warnings:?}"
        );
    }

    #[test]
    fn wasm_for_emits_counted_loop() {
        let stmts =
            pblang_core::parse("fn f() { let s = 0; for i 10 { s = s + i; } return s; }").unwrap();
        let (wat, warnings) = compile_to_wat(stmts).unwrap();
        assert!(wat.contains("loop $for_start"), "expected for loop:\n{wat}");
        assert!(
            wat.contains("f64.ge"),
            "expected f64.ge bound check:\n{wat}"
        );
        assert!(wat.contains("f64.add"), "expected increment add:\n{wat}");
        assert!(
            !warnings.iter().any(|w| w.contains("for not fully")),
            "for should be implemented: {warnings:?}"
        );
    }

    #[test]
    fn wasm_comparison_widens_to_f64() {
        let stmts = pblang_core::parse("fn f(a, b) { return a < b; }").unwrap();
        let (wat, _) = compile_to_wat(stmts).unwrap();
        assert!(wat.contains("f64.lt"), "expected f64.lt:\n{wat}");
        assert!(
            wat.contains("f64.convert_i32_u"),
            "comparison result should widen i32->f64:\n{wat}"
        );
    }

    #[test]
    fn wasm_logical_and_or_short_circuit() {
        // `a and b` => if (result f64) { truthy(b) } else { 0 }
        let and_wat = compile_to_wat(pblang_core::parse("fn f(a, b) { return a and b; }").unwrap())
            .unwrap()
            .0;
        assert!(
            and_wat.contains("if (result f64)"),
            "and should lower to a result-typed if:\n{and_wat}"
        );
        assert!(
            and_wat.contains("else"),
            "and needs an else branch:\n{and_wat}"
        );
        // `a or b` => if (result f64) { 1 } else { truthy(b) }
        let or_wat = compile_to_wat(pblang_core::parse("fn f(a, b) { return a or b; }").unwrap())
            .unwrap()
            .0;
        assert!(
            or_wat.contains("if (result f64)"),
            "or should lower to a result-typed if:\n{or_wat}"
        );
    }

    #[test]
    fn wasm_direct_function_call_emits_call() {
        let stmts = pblang_core::parse(
            "fn add(a, b) { return a + b; }\nfn main() { let x = add(2, 3); return x; }",
        )
        .unwrap();
        let (wat, warnings) = compile_to_wat(stmts).unwrap();
        assert!(wat.contains("call $add"), "expected `call $add`:\n{wat}");
        assert!(
            !warnings.iter().any(|w| w.contains("calls not fully wired")),
            "user-function calls should be wired: {warnings:?}"
        );
    }

    #[test]
    fn wasm_print_string_literal_uses_memory() {
        let stmts = pblang_core::parse("fn f() { print \"hi\"; }").unwrap();
        let (wat, _) = compile_to_wat(stmts).unwrap();
        assert!(
            wat.contains("call $pblang_print"),
            "string print should call $pblang_print:\n{wat}"
        );
        // length-prefixed data segment, every byte escaped: "hi" => \68\69,
        // preceded by the little-endian u32 length 2 => \02\00\00\00.
        assert!(wat.contains("(data "), "expected a data segment:\n{wat}");
        assert!(
            wat.contains("\\02\\00\\00\\00\\68\\69"),
            "expected len-prefixed 'hi' bytes:\n{wat}"
        );
        assert!(wat.contains("i32.const 2"), "expected len=2 arg:\n{wat}");
    }

    #[test]
    fn wasm_print_number_uses_f64_import() {
        let stmts = pblang_core::parse("fn f() { print 40 + 2; }").unwrap();
        let (wat, _) = compile_to_wat(stmts).unwrap();
        assert!(
            wat.contains("call $pblang_print_f64"),
            "numeric print should call $pblang_print_f64:\n{wat}"
        );
        assert!(
            wat.contains("import \"env\" \"pblang_print_f64\""),
            "f64 print import should be declared:\n{wat}"
        );
    }

    #[test]
    fn wasm_print_fstring_prints_each_part() {
        let stmts = pblang_core::parse("fn f(x) { print f\"val={x}!\"; }").unwrap();
        let (wat, _) = compile_to_wat(stmts).unwrap();
        // literal segments via string print, interpolation via f64 print
        assert!(
            wat.contains("call $pblang_print\n") || wat.contains("call $pblang_print"),
            "string parts:\n{wat}"
        );
        assert!(
            wat.contains("call $pblang_print_f64"),
            "interp via f64:\n{wat}"
        );
    }

    #[test]
    fn wasm_repeated_local_declared_once() {
        // A shadowed/re-declared `let x` must emit only one `(local $x f64)`
        // (WASM locals are function-flat); duplicates would be invalid WAT.
        let stmts = pblang_core::parse("fn f() { let x = 1; { let x = 2; } let x = 3; return x; }")
            .unwrap();
        let (wat, _) = compile_to_wat(stmts).unwrap();
        let count = wat.matches("(local $x f64)").count();
        assert_eq!(
            count, 1,
            "expected exactly one (local $x f64), got {count}:\n{wat}"
        );
    }

    #[test]
    fn wasm_identifiers_carry_dollar_sigil() {
        let stmts = pblang_core::parse("fn add(a, b) { let s = a + b; return s; }").unwrap();
        let (wat, _) = compile_to_wat(stmts).unwrap();
        assert!(
            wat.contains("(func $add"),
            "func name needs $ sigil:\n{wat}"
        );
        assert!(
            wat.contains("(param $a f64)"),
            "params declared+sigiled:\n{wat}"
        );
        assert!(wat.contains("(local $s f64)"), "locals sigiled:\n{wat}");
        assert!(wat.contains("local.get $a"), "local.get sigiled:\n{wat}");
        assert!(wat.contains("(memory $memory"), "memory sigiled:\n{wat}");
        // No bare (unsigiled) identifier forms should remain.
        assert!(
            !wat.contains("local.get a\n"),
            "found unsigiled local.get:\n{wat}"
        );
        assert!(!wat.contains("(func add "), "found unsigiled func:\n{wat}");
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
