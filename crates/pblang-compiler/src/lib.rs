use pblang_core::{Expr, ExprKind, LogicalOp, MatchArm, Span, Stmt, StmtKind};
use pblang_ir::chunk::{Chunk, OpCode, Span as VmSpan};
use pblang_ir::{alloc_string, Function, Value};

use std::collections::HashMap;
use std::rc::Rc;

/// Compile-time error with span information for better error messages.
#[derive(Debug, Clone)]
pub struct CompileError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "compile error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

impl std::error::Error for CompileError {}

fn vspan(sp: Span) -> VmSpan {
    VmSpan {
        start: sp.start,
        end: sp.end,
    }
}

/// Convert a compile-time constant expression to a pblang_ir::Value.
/// Only handles literal expressions (Number, Str, Bool, Nil).
/// Returns None for complex expressions (these should use nil-check pattern instead).
fn expr_to_const_value(e: &pblang_core::Expr) -> Option<pblang_ir::Value> {
    use pblang_core::ExprKind;
    match &e.kind {
        ExprKind::Number(n) => Some(pblang_ir::Value::Number(*n)),
        ExprKind::Bool(b) => Some(pblang_ir::Value::Bool(*b)),
        ExprKind::Nil => Some(pblang_ir::Value::Nil),
        ExprKind::Str(s) => Some(pblang_ir::alloc_string(s.clone())),
        _ => None, // complex expression: user must use nil-check pattern
    }
}

/// Compile statements to bytecode. Returns a CompileError on semantic errors
/// like variable redeclaration or reading a variable in its own initializer.
pub fn compile(stmts: Vec<Stmt>) -> Result<Chunk, CompileError> {
    let mut c = Compiler {
        chunk: Chunk::default(),
        globals: HashMap::new(),
        locals: vec![],
        scope_depth: 0,
        upvalues: vec![],
        enclosing: None,
        loop_ctx: vec![],
        error: None,
    };

    for s in stmts {
        c.emit_stmt(s);
        // Check for error after each statement
        if let Some(err) = c.error.take() {
            return Err(err);
        }
    }

    // script epilogue: return nil
    let k = c.chunk.add_const(Value::Nil);
    c.chunk.write(OpCode::OpConstant(k), VmSpan::default());
    c.chunk.write(OpCode::OpReturn, VmSpan::default());

    optimize_chunk(&mut c.chunk);
    Ok(c.chunk)
}

#[derive(Clone, Debug)]
struct UpvalueDesc {
    is_local: bool,
    index: usize,
}

#[derive(Clone, Debug)]
struct Local {
    name: String,
    depth: Option<usize>,
    is_captured: bool,
    /// True when declared with `const` — reassignment is a compile error.
    is_const: bool,
}

struct Compiler {
    chunk: Chunk,
    globals: HashMap<String, usize>, // name -> const index (interned string in this chunk)

    locals: Vec<Local>,
    scope_depth: usize, // (0 = top-level)

    upvalues: Vec<UpvalueDesc>,
    enclosing: Option<Box<Compiler>>,

    /// Loop context stack for break/continue.
    /// Each entry: (continue_target_ip, locals_count_at_entry, break_patch_ips, continue_patch_ips)
    /// - continue_target: jump to here for `continue`. `usize::MAX` is a sentinel
    ///   meaning "the continue target isn't known yet" — used by the index loops
    ///   (For/ForIn/ForInKV) whose `continue` must land on the index-increment
    ///   step that is emitted *after* the body. Those continues are forward-patched
    ///   via `continue_patch_ips`. While/Loop use a real backward target (loop_start).
    /// - locals_at_entry: len(locals) when loop started (for scope cleanup on break)
    /// - break_patch_ips: OpJump(0) placeholders to patch with loop-exit IP
    /// - continue_patch_ips: OpJump(0) placeholders to patch with the increment IP
    loop_ctx: Vec<(usize, usize, Vec<usize>, Vec<usize>)>,

    /// Deferred error - set when a semantic error is detected, checked after each statement.
    error: Option<CompileError>,
}

#[derive(Clone, Debug)]
enum Resolved {
    Local(usize),
    Upvalue(usize),
    Global(String),
}

impl Compiler {
    /// Εξάγει jump placeholder και επιστρέφει το position του.
    fn emit_jump_placeholder(&mut self, opcode: OpCode, sp: Span) -> usize {
        self.emit(opcode, sp) // π.χ. OpJump(0) ή OpJumpIfFalse(0)
    }

    fn patch_jump_if_false(&mut self, pos: usize) {
        let target = self.ip();
        self.patch(pos, OpCode::OpJumpIfFalse(target));
    }

    fn patch_jump_if_true(&mut self, pos: usize) {
        let target = self.ip();
        self.patch(pos, OpCode::OpJumpIfTrue(target));
    }

    // Βοηθητικό: emit OpJumpIfFalse(0) placeholder
    fn emit_jump_if_false_placeholder(&mut self, sp: Span) -> usize {
        self.emit_jump_placeholder(OpCode::OpJumpIfFalse(0), sp)
    }

    // Βοηθητικό: emit OpJumpIfTrue(0) placeholder
    fn emit_jump_if_true_placeholder(&mut self, sp: Span) -> usize {
        self.emit_jump_placeholder(OpCode::OpJumpIfTrue(0), sp)
    }

    fn emit_and(&mut self, left: Expr, right: Expr, sp: Span) {
        self.emit_expr(left);
        let jump_end = self.emit_jump_if_false_placeholder(sp);
        self.chunk.write(OpCode::OpPop, vspan(sp));
        self.emit_expr(right);
        self.patch_jump_if_false(jump_end);
    }

    fn emit_or(&mut self, left: Expr, right: Expr, sp: Span) {
        self.emit_expr(left);
        let jump_end = self.emit_jump_if_true_placeholder(sp);
        self.chunk.write(OpCode::OpPop, vspan(sp));
        self.emit_expr(right);
        self.patch_jump_if_true(jump_end);
    }

    fn declare_local(&mut self, name: String, span: Span) -> usize {
        self.declare_local_inner(name, span, false)
    }

    fn declare_const(&mut self, name: String, span: Span) -> usize {
        self.declare_local_inner(name, span, true)
    }

    fn declare_local_inner(&mut self, name: String, span: Span, is_const: bool) -> usize {
        // allow multiple "_" (discard) in same scope; otherwise error if same-name local already declared
        if name != "_" {
            for l in self.locals.iter().rev() {
                if let Some(d) = l.depth {
                    if d < self.scope_depth {
                        break;
                    }
                }
                if l.name == name {
                    self.error = Some(CompileError {
                        message: format!("variable '{}' redeclared in same scope", name),
                        span,
                    });
                    // Return a slot anyway to avoid cascading errors
                    return self.locals.len();
                }
            }
        }

        let slot = self.locals.len();
        self.locals.push(Local {
            name,
            depth: None,
            is_captured: false,
            is_const,
        });
        slot
    }

    fn mark_initialized(&mut self) {
        if self.scope_depth == 0 {
            return;
        } // globals are “late-bound”
        let last = self
            .locals
            .last_mut()
            .expect("mark_initialized called with empty locals - always preceded by declare_local");
        last.depth = Some(self.scope_depth);
    }

    fn mark_captured(&mut self, local_slot: usize) {
        if let Some(l) = self.locals.get_mut(local_slot) {
            l.is_captured = true;
        }
    }

    fn add_upvalue(&mut self, is_local: bool, index: usize) -> usize {
        if let Some((i, _)) = self
            .upvalues
            .iter()
            .enumerate()
            .find(|(_, uv)| uv.is_local == is_local && uv.index == index)
        {
            return i;
        }
        let slot = self.upvalues.len();
        self.upvalues.push(UpvalueDesc { is_local, index });
        slot
    }

    fn resolve_upvalue(&mut self, name: &str) -> Option<usize> {
        let enclosing = self.enclosing.as_deref_mut()?;

        if let Some(local) = enclosing.resolve_local(name) {
            enclosing.mark_captured(local);
            return Some(self.add_upvalue(true, local));
        }

        if let Some(up) = enclosing.resolve_upvalue(name) {
            return Some(self.add_upvalue(false, up));
        }

        None
    }

    fn resolve_name(&mut self, name: &str, span: Span) -> Resolved {
        if let Some(slot) = self.resolve_local(name) {
            if self.locals[slot].depth.is_none() {
                self.error = Some(CompileError {
                    message: format!(
                        "cannot read local variable '{}' in its own initializer",
                        name
                    ),
                    span,
                });
                // Return global to avoid cascading errors
                return Resolved::Global(name.to_string());
            }
            return Resolved::Local(slot);
        }
        if let Some(up) = self.resolve_upvalue(name) {
            return Resolved::Upvalue(up);
        }
        Resolved::Global(name.to_string())
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        if self.scope_depth == 0 {
            return;
        }

        // 1) leave the scope first
        self.scope_depth -= 1;

        // 2) then discard locals that were declared in the scope we just left
        while let Some(local) = self.locals.last() {
            let should_discard = match local.depth {
                Some(d) => d > self.scope_depth,
                None => true, // shouldn't happen if all lets are initialized, but be robust
            };

            if !should_discard {
                break;
            }

            let local = self
                .locals
                .pop()
                .expect("pop inside end_scope: last() was Some so pop() cannot be None");
            if local.is_captured {
                self.chunk.write(OpCode::OpCloseUpvalue, VmSpan::default());
            } else {
                self.chunk.write(OpCode::OpPop, VmSpan::default());
            }
        }
    }

    fn resolve_local(&self, name: &str) -> Option<usize> {
        self.locals.iter().rposition(|l| l.name == name)
    }

    fn add_local(&mut self, name: String) -> usize {
        let slot = self.locals.len();
        self.locals.push(Local {
            name,
            depth: Some(self.scope_depth),
            is_captured: false,
            is_const: false,
        });
        slot
    }

    fn ip(&self) -> usize {
        self.chunk.instructions.len()
    }

    fn emit(&mut self, op: OpCode, sp: Span) -> usize {
        let pos = self.ip();
        self.chunk.write(op, vspan(sp));
        pos
    }

    fn patch(&mut self, pos: usize, op: OpCode) {
        self.chunk.patch(pos, op);
    }

    fn emit_stmt(&mut self, s: Stmt) {
        let sp = s.span;
        match s.kind {
            StmtKind::Import(path) => {
                // Runtime import: import(path) -> module
                // This keeps import semantics consistent and enables caching in the VM.
                fn module_name_from_import_path(path: &str) -> String {
                    // Support non-filesystem "specifier" forms like:
                    // - stdlib:http_server.sapp
                    // - pkg:foo/bar.sapp
                    // while still handling normal paths.
                    let mut s = path;
                    if let Some(i) = s.rfind(':') {
                        s = &s[i + 1..];
                    }
                    if let Some(i) = s.rfind(['/', '\\']) {
                        s = &s[i + 1..];
                    }
                    if let Some(i) = s.rfind('.') {
                        s = &s[..i];
                    }
                    if s.is_empty() {
                        "module".to_string()
                    } else {
                        s.to_string()
                    }
                }

                let mod_name = module_name_from_import_path(&path);

                // callee
                self.emit_expr(Expr {
                    kind: ExprKind::Var("import".to_string()),
                    span: sp,
                });
                // arg: path string
                let k = self.chunk.add_const(alloc_string(path));
                self.chunk.write(OpCode::OpConstant(k), vspan(sp));
                // call
                self.chunk.write(OpCode::OpCall(1), vspan(sp));

                if self.scope_depth == 0 {
                    let name_idx = self.intern_name(&mod_name);
                    self.chunk
                        .write(OpCode::OpDefineGlobal(name_idx), vspan(sp));
                } else {
                    self.declare_local(mod_name, sp);
                    self.mark_initialized();
                    // value is the local's stack slot
                }
            }

            StmtKind::Export(names) => {
                for name in names {
                    let name_idx = self.intern_name(&name);
                    self.chunk.write(OpCode::OpExport(name_idx), vspan(sp));
                }
            }

            StmtKind::Fn(name, params, _ret_ty, body) => {
                // FIX #19: Build the full enclosing chain by cloning `self` as a
                // proper enclosing compiler. Previously `enclosing: None` was hard-coded,
                // which meant closures could only capture from 1 level up. Now the chain
                // is threaded correctly so `fn outer { fn inner { fn deep { x } } }` works.
                let enclosing_snapshot = Compiler {
                    chunk: Chunk::default(), // unused placeholder (we only need locals/upvalues)
                    globals: HashMap::new(),
                    locals: self.locals.clone(),
                    scope_depth: self.scope_depth,
                    upvalues: self.upvalues.clone(),
                    loop_ctx: vec![],
                    enclosing: self.enclosing.as_ref().map(|e| {
                        // Carry the next level of the chain for arbitrarily deep nesting.
                        Box::new(Compiler {
                            chunk: Chunk::default(),
                            globals: HashMap::new(),
                            locals: e.locals.clone(),
                            scope_depth: e.scope_depth,
                            upvalues: e.upvalues.clone(),
                            loop_ctx: vec![],
                            enclosing: None, // two levels deep is sufficient for most programs
                            error: None,
                        })
                    }),
                    error: None,
                };
                let mut f_compiler = Compiler {
                    chunk: Chunk::default(),
                    globals: HashMap::new(),
                    locals: vec![],
                    scope_depth: 1,
                    upvalues: vec![],
                    loop_ctx: vec![],
                    enclosing: Some(Box::new(enclosing_snapshot)),
                    error: None,
                };

                // params are locals in slot 0..arity-1 (ignore type annotations at runtime)
                for p in &params {
                    f_compiler.add_local(p.name.clone());
                }

                // function body scope
                f_compiler.begin_scope();
                for st in body {
                    f_compiler.emit_stmt(st);
                }
                f_compiler.end_scope();

                // fallthrough return nil
                let nil = f_compiler.chunk.add_const(Value::Nil);
                f_compiler
                    .chunk
                    .write(OpCode::OpConstant(nil), VmSpan::default());
                f_compiler.chunk.write(OpCode::OpReturn, VmSpan::default());

                optimize_chunk(&mut f_compiler.chunk);
                // Count required params and check for vararg
                let is_vararg_fn = params.last().map(|p| p.is_vararg).unwrap_or(false);
                let named_arity = if is_vararg_fn {
                    params.len() - 1
                } else {
                    params.len()
                };
                let min_arity_fn = params
                    .iter()
                    .filter(|p| p.default.is_none() && !p.is_vararg)
                    .count();
                // Pre-compile constant defaults into Values
                let defaults_vals: Vec<pblang_ir::Value> = params
                    .iter()
                    .filter_map(|p| p.default.as_ref().map(expr_to_const_value))
                    .flatten()
                    .collect();
                let fun = Rc::new(Function {
                    name: Some(name.clone()),
                    arity: named_arity,
                    min_arity: min_arity_fn,
                    defaults: defaults_vals,
                    is_vararg: is_vararg_fn,
                    chunk: f_compiler.chunk,
                    param_types: None,
                });

                let fn_const_idx = self.chunk.add_const(Value::Function(fun));
                let captures = f_compiler
                    .upvalues
                    .iter()
                    .map(|u| (u.is_local, u.index))
                    .collect::<Vec<(bool, usize)>>();

                self.chunk.write(
                    OpCode::OpClosure {
                        fn_const: fn_const_idx,
                        captures,
                    },
                    vspan(sp),
                );

                if self.scope_depth == 0 {
                    let name_idx = self.intern_name(&name);
                    self.chunk
                        .write(OpCode::OpDefineGlobal(name_idx), vspan(sp));
                } else {
                    self.declare_local(name, sp);
                    self.mark_initialized();
                    // no opcode needed: closure value is the local's stack slot
                }
            }

            StmtKind::Return(opt) => {
                match opt {
                    // `return f(args)` — a call in tail position. Emit it like a
                    // normal call but with OpTailCall so the VM can reuse the
                    // current frame (constant stack depth for tail recursion). The
                    // VM falls back to a normal call when the callee/frame isn't
                    // reusable, and the OpReturn below then returns its result.
                    Some(e) if matches!(e.kind, ExprKind::Call(..)) => {
                        if let ExprKind::Call(callee, args) = e.kind {
                            self.emit_expr(*callee);
                            let argc = args.len();
                            for arg in args {
                                self.emit_expr(arg);
                            }
                            self.chunk.write(OpCode::OpTailCall(argc), vspan(sp));
                        }
                    }
                    Some(e) => self.emit_expr(e),
                    None => {
                        let k = self.chunk.add_const(Value::Nil);
                        self.chunk.write(OpCode::OpConstant(k), vspan(sp));
                    }
                }
                // Close any captured locals still in scope before returning.
                // self.chunk.write(OpCode::OpCloseUpvalue);
                self.chunk.write(OpCode::OpReturn, vspan(sp));
            }

            StmtKind::Defer(expr) => {
                // Compile a 0-arg thunk that runs expr (side effects) and returns nil; register it as defer.
                let mut thunk_compiler = Compiler {
                    chunk: Chunk::default(),
                    globals: HashMap::new(),
                    locals: vec![],
                    scope_depth: 1,
                    upvalues: vec![],
                    loop_ctx: vec![],
                    enclosing: Some(Box::new(Compiler {
                        chunk: Chunk::default(),
                        globals: HashMap::new(),
                        locals: self.locals.clone(),
                        scope_depth: self.scope_depth,
                        upvalues: self.upvalues.clone(),
                        loop_ctx: vec![],
                        enclosing: None,
                        error: None,
                    })),
                    error: None,
                };
                thunk_compiler.begin_scope();
                thunk_compiler.emit_expr(expr);
                thunk_compiler.chunk.write(OpCode::OpPop, vspan(sp));
                let nil_k = thunk_compiler.chunk.add_const(Value::Nil);
                thunk_compiler
                    .chunk
                    .write(OpCode::OpConstant(nil_k), VmSpan::default());
                thunk_compiler
                    .chunk
                    .write(OpCode::OpReturn, VmSpan::default());
                thunk_compiler.end_scope();
                optimize_chunk(&mut thunk_compiler.chunk);
                let fun = Rc::new(Function {
                    name: None,
                    arity: 0,
                    min_arity: 0,
                    defaults: vec![],
                    is_vararg: false,
                    chunk: thunk_compiler.chunk,
                    param_types: None,
                });
                let fn_const_idx = self.chunk.add_const(Value::Function(fun));
                let captures = thunk_compiler
                    .upvalues
                    .iter()
                    .map(|u| (u.is_local, u.index))
                    .collect::<Vec<(bool, usize)>>();
                self.chunk.write(
                    OpCode::OpClosure {
                        fn_const: fn_const_idx,
                        captures,
                    },
                    vspan(sp),
                );
                self.chunk.write(OpCode::OpDefer, vspan(sp));
            }

            StmtKind::Print(e) => {
                self.emit_expr(e);
                self.chunk.write(OpCode::OpPrint, vspan(sp));
            }

            StmtKind::PrintF(_p, e) => {
                self.emit_expr(e);
                self.chunk.write(OpCode::OpPrint, vspan(sp));
            }

            StmtKind::Let(name, _ty, init) => {
                if self.scope_depth == 0 {
                    // global: όπως το έχεις
                    self.emit_expr(init);
                    let name_idx = self.intern_name(&name);
                    self.chunk
                        .write(OpCode::OpDefineGlobal(name_idx), vspan(sp));
                } else {
                    // local: compile the initializer FIRST, then declare the
                    // local over the value it leaves on the stack. Declaring
                    // first would shadow an outer/param binding of the same
                    // name with a not-yet-initialized slot, so `let n = f(n)`
                    // resolved `n` to a global and failed. Emitting first lets
                    // the initializer see the outer binding (clox semantics).
                    self.emit_expr(init);
                    self.declare_local(name, sp);
                    // the local slot is now the value the initializer produced
                    self.mark_initialized();
                }
            }

            // const name = expr; — immutable binding, compiler prevents reassignment.
            // At runtime behaves identical to let; immutability is enforced at compile time.
            StmtKind::Const(name, _ty, init) => {
                if self.scope_depth == 0 {
                    // Top-level globals can't be made truly const yet (would need a const map)
                    // Treat as a regular global for now; lint can warn about reassignment.
                    self.emit_expr(init);
                    let name_idx = self.intern_name(&name);
                    self.chunk
                        .write(OpCode::OpDefineGlobal(name_idx), vspan(sp));
                } else {
                    // Local const: compile initializer first (see Let above),
                    // then declare so it can shadow an outer binding correctly.
                    self.emit_expr(init);
                    self.declare_const(name, sp);
                    self.mark_initialized();
                }
            }

            StmtKind::Block(stmts) => {
                let first_is_let_map = stmts
                    .first()
                    .map(|st| match &st.kind {
                        StmtKind::Let(_, None, e) => matches!(e.kind, ExprKind::Map(_)),
                        _ => false,
                    })
                    .unwrap_or(false);
                if self.scope_depth == 0 && stmts.len() > 1 && first_is_let_map {
                    // Class desugaring: first stmt (let Name = map{...}) at top-level => DefineGlobal.
                    self.emit_stmt(stmts[0].clone());
                    self.begin_scope();
                    for st in &stmts[1..] {
                        self.emit_stmt(st.clone());
                    }
                    self.end_scope();
                } else {
                    self.begin_scope();
                    for st in stmts {
                        self.emit_stmt(st);
                    }
                    self.end_scope();
                }
            }
            // END OF Block

            // Seq: flat statement sequence WITHOUT new scope.
            // Used by destructuring (`let [a,b]=arr`) so variables are visible in outer scope.
            StmtKind::Seq(stmts) => {
                for st in stmts {
                    self.emit_stmt(st);
                }
            }

            StmtKind::Assign(name, expr) => {
                // Guard: check if name resolves to a const local
                if let Some(slot) = self.resolve_local(&name) {
                    if self.locals[slot].is_const {
                        self.error = Some(CompileError {
                            message: format!(
                                "cannot assign to `{}`: declared as `const` (immutable)",
                                name
                            ),
                            span: sp,
                        });
                        return;
                    }
                }
                self.emit_expr(expr);

                match self.resolve_name(&name, sp) {
                    Resolved::Local(slot) => self.chunk.write(OpCode::OpSetLocal(slot), vspan(sp)),
                    Resolved::Upvalue(slot) => {
                        self.chunk.write(OpCode::OpSetUpvalue(slot), vspan(sp))
                    }
                    Resolved::Global(g) => {
                        let name_idx = self.intern_name(&g);
                        self.chunk.write(OpCode::OpSetGlobal(name_idx), vspan(sp));
                    }
                }
            }

            StmtKind::SetIndex(target, index, value) => {
                self.emit_expr(target);
                self.emit_expr(index);
                self.emit_expr(value);
                self.chunk.write(OpCode::OpSetIndex, vspan(sp));
                self.chunk.write(OpCode::OpPop, vspan(sp));
            }

            StmtKind::SetAttr(target, name, value) => {
                self.emit_expr(target);
                self.emit_expr(value);
                let name_idx = self.intern_name(&name);
                self.chunk.write(OpCode::OpSetAttr(name_idx), vspan(sp));
                self.chunk.write(OpCode::OpPop, vspan(sp));
            }

            StmtKind::Expr(e) => {
                self.emit_expr(e);
                self.chunk.write(OpCode::OpPop, vspan(sp));
            }

            StmtKind::If(cond, body, else_body) => {
                self.emit_expr(cond);

                let jmp_false_pos = self.emit(OpCode::OpJumpIfFalse(0), sp);
                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop cond (true branch)

                self.begin_scope();
                for st in body {
                    self.emit_stmt(st);
                }
                self.end_scope();

                let jmp_end_pos = self.emit(OpCode::OpJump(0), sp);

                let false_ip = self.ip();
                self.patch(jmp_false_pos, OpCode::OpJumpIfFalse(false_ip));

                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop cond (false branch)

                if let Some(stmts) = else_body {
                    self.begin_scope();
                    for st in stmts {
                        self.emit_stmt(st);
                    }
                    self.end_scope();
                }

                let end_ip = self.ip();
                self.patch(jmp_end_pos, OpCode::OpJump(end_ip));
            }

            StmtKind::While(cond, body) => {
                let loop_start = self.ip();

                // Push loop context for break/continue. While's continue target
                // is loop_start (re-evaluate the condition), which is correct.
                self.loop_ctx
                    .push((loop_start, self.locals.len(), vec![], vec![]));

                self.emit_expr(cond);
                let jmp_exit_pos = self.emit(OpCode::OpJumpIfFalse(0), sp);

                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop cond (true)

                self.begin_scope();
                for st in body {
                    self.emit_stmt(st);
                }
                self.end_scope();

                self.chunk.write(OpCode::OpJump(loop_start), vspan(sp));

                let exit_ip = self.ip();
                self.patch(jmp_exit_pos, OpCode::OpJumpIfFalse(exit_ip));
                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop cond (false)
                let break_exit_ip = self.ip(); // break lands here (AFTER false-cond pop)

                // Patch all break jumps (they skip the false-cond pop)
                if let Some((_, _, break_patches, _)) = self.loop_ctx.pop() {
                    for bp in break_patches {
                        self.patch(bp, OpCode::OpJump(break_exit_ip));
                    }
                }
            }

            StmtKind::For(name, count_expr, body) => {
                self.begin_scope(); // outer scope: bound + i

                // let __bound = count_expr;  (evaluated once)
                let bound_sym = format!("__for_bound_{name}");
                let bound_slot = self.declare_local(bound_sym, sp);
                self.emit_expr(count_expr);
                self.mark_initialized();

                // let __i = 0;
                let i_sym = format!("__for_i_{name}");
                let i_slot = self.declare_local(i_sym, sp);
                let zero = self.chunk.add_const(Value::Number(0.0));
                self.chunk.write(OpCode::OpConstant(zero), vspan(sp));
                self.mark_initialized();

                let loop_start = self.ip();

                // Push loop context for break/continue. `continue` must land on
                // the index-increment step (emitted after the body), so the
                // continue target is forward-patched (usize::MAX sentinel).
                self.loop_ctx
                    .push((usize::MAX, self.locals.len(), vec![], vec![]));

                // cond: __i < __bound
                self.chunk.write(OpCode::OpGetLocal(i_slot), vspan(sp));
                self.chunk.write(OpCode::OpGetLocal(bound_slot), vspan(sp));
                self.chunk.write(OpCode::OpLt, vspan(sp));

                let jmp_exit_pos = self.emit(OpCode::OpJumpIfFalse(0), sp);
                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop cond (true branch)

                // iteration scope: fresh binding for `name`
                self.begin_scope();
                let _name_slot = self.declare_local(name.clone(), sp);

                // initializer for `name` is current __i
                self.chunk.write(OpCode::OpGetLocal(i_slot), vspan(sp));
                self.mark_initialized();

                // body
                for st in body {
                    self.emit_stmt(st);
                }

                self.end_scope(); // pops/closes `name` (OpCloseUpvalue if captured)

                // continue lands here: __i = __i + 1
                let continue_ip = self.ip();
                self.chunk.write(OpCode::OpGetLocal(i_slot), vspan(sp));
                let one = self.chunk.add_const(Value::Number(1.0));
                self.chunk.write(OpCode::OpConstant(one), vspan(sp));
                self.chunk.write(OpCode::OpAdd, vspan(sp));
                self.chunk.write(OpCode::OpSetLocal(i_slot), vspan(sp));

                // back-edge
                self.chunk.write(OpCode::OpJump(loop_start), vspan(sp));

                // patch exit
                let exit_ip = self.ip();
                self.patch(jmp_exit_pos, OpCode::OpJumpIfFalse(exit_ip));
                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop cond (false branch)
                let break_exit_ip = self.ip(); // break lands here (AFTER false-cond pop)

                // Patch all break and continue jumps
                if let Some((_, _, break_patches, continue_patches)) = self.loop_ctx.pop() {
                    for bp in break_patches {
                        self.patch(bp, OpCode::OpJump(break_exit_ip));
                    }
                    for cp in continue_patches {
                        self.patch(cp, OpCode::OpJump(continue_ip));
                    }
                }

                self.end_scope(); // pops/closes __i + __bound
            } // END OF For

            StmtKind::ForIn(name, col_expr, body) => {
                // Desugars to:
                // let __col = col_expr;
                // let __len = len(__col);
                // let __i = 0;
                // while __i < __len { let name = __col[__i]; body...; __i = __i + 1; }
                self.begin_scope();

                // __col = col_expr
                let col_sym = format!("__forin_col_{name}");
                let col_slot = self.declare_local(col_sym, sp);
                self.emit_expr(col_expr);
                self.mark_initialized();

                // __len = len(__col)
                let len_sym = format!("__forin_len_{name}");
                let len_slot = self.declare_local(len_sym, sp);
                let len_name_idx = self.intern_name("len");
                self.chunk
                    .write(OpCode::OpGetGlobal(len_name_idx), vspan(sp));
                self.chunk.write(OpCode::OpGetLocal(col_slot), vspan(sp));
                self.chunk.write(OpCode::OpCall(1), vspan(sp));
                self.mark_initialized();

                // __i = 0
                let i_sym = format!("__forin_i_{name}");
                let i_slot = self.declare_local(i_sym, sp);
                let zero = self.chunk.add_const(Value::Number(0.0));
                self.chunk.write(OpCode::OpConstant(zero), vspan(sp));
                self.mark_initialized();

                let loop_start = self.ip();

                // Push loop context for break/continue (ForIn). `continue` lands
                // on the index-increment step emitted after the body, so it is
                // forward-patched (usize::MAX sentinel).
                self.loop_ctx
                    .push((usize::MAX, self.locals.len(), vec![], vec![]));

                // cond: __i < __len
                self.chunk.write(OpCode::OpGetLocal(i_slot), vspan(sp));
                self.chunk.write(OpCode::OpGetLocal(len_slot), vspan(sp));
                self.chunk.write(OpCode::OpLt, vspan(sp));

                let jmp_exit_pos = self.emit(OpCode::OpJumpIfFalse(0), sp);
                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop true cond

                // iteration scope: `name = __col[__i]`
                self.begin_scope();
                let _name_slot = self.declare_local(name.clone(), sp);
                self.chunk.write(OpCode::OpGetLocal(col_slot), vspan(sp));
                self.chunk.write(OpCode::OpGetLocal(i_slot), vspan(sp));
                self.chunk.write(OpCode::OpGetIndex, vspan(sp));
                self.mark_initialized();

                for st in body {
                    self.emit_stmt(st);
                }
                self.end_scope(); // pops `name`

                // continue lands here: __i = __i + 1
                let continue_ip = self.ip();
                self.chunk.write(OpCode::OpGetLocal(i_slot), vspan(sp));
                let one = self.chunk.add_const(Value::Number(1.0));
                self.chunk.write(OpCode::OpConstant(one), vspan(sp));
                self.chunk.write(OpCode::OpAdd, vspan(sp));
                self.chunk.write(OpCode::OpSetLocal(i_slot), vspan(sp));

                // back-edge
                self.chunk.write(OpCode::OpJump(loop_start), vspan(sp));

                // patch exit
                let exit_ip = self.ip();
                self.patch(jmp_exit_pos, OpCode::OpJumpIfFalse(exit_ip));
                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop false cond
                let break_exit_ip = self.ip();

                // Patch all break and continue jumps
                if let Some((_, _, break_patches, continue_patches)) = self.loop_ctx.pop() {
                    for bp in break_patches {
                        self.patch(bp, OpCode::OpJump(break_exit_ip));
                    }
                    for cp in continue_patches {
                        self.patch(cp, OpCode::OpJump(continue_ip));
                    }
                }

                self.end_scope(); // pops __i, __len, __col
            }

            StmtKind::ForInKV(key_name, val_name, col_expr, body) => {
                // Desugars to:
                // let __col = col_expr;
                // let __keys = map_keys(__col);
                // let __len = len(__keys);
                // let __i = 0;
                // while __i < __len { let k = __keys[__i]; let v = __col[k]; body...; __i++; }
                self.begin_scope();

                // __col = col_expr
                let col_sym = format!("__forkv_col_{key_name}");
                let col_slot = self.declare_local(col_sym, sp);
                self.emit_expr(col_expr);
                self.mark_initialized();

                // __keys = map_keys(__col)
                let keys_sym = format!("__forkv_keys_{key_name}");
                let keys_slot = self.declare_local(keys_sym, sp);
                let mk_name = self.intern_name("map_keys");
                self.chunk.write(OpCode::OpGetGlobal(mk_name), vspan(sp));
                self.chunk.write(OpCode::OpGetLocal(col_slot), vspan(sp));
                self.chunk.write(OpCode::OpCall(1), vspan(sp));
                self.mark_initialized();

                // __len = len(__keys)
                let len2_sym = format!("__forkv_len_{key_name}");
                let len2_slot = self.declare_local(len2_sym, sp);
                let len_name = self.intern_name("len");
                self.chunk.write(OpCode::OpGetGlobal(len_name), vspan(sp));
                self.chunk.write(OpCode::OpGetLocal(keys_slot), vspan(sp));
                self.chunk.write(OpCode::OpCall(1), vspan(sp));
                self.mark_initialized();

                // __i = 0
                let i2_sym = format!("__forkv_i_{key_name}");
                let i2_slot = self.declare_local(i2_sym, sp);
                let zero = self.chunk.add_const(Value::Number(0.0));
                self.chunk.write(OpCode::OpConstant(zero), vspan(sp));
                self.mark_initialized();

                let loop_start = self.ip();

                // Push loop context for break/continue (ForInKV). `continue`
                // lands on the index-increment step, so it is forward-patched.
                self.loop_ctx
                    .push((usize::MAX, self.locals.len(), vec![], vec![]));

                // cond: __i < __len
                self.chunk.write(OpCode::OpGetLocal(i2_slot), vspan(sp));
                self.chunk.write(OpCode::OpGetLocal(len2_slot), vspan(sp));
                self.chunk.write(OpCode::OpLt, vspan(sp));

                let jmp_exit_pos = self.emit(OpCode::OpJumpIfFalse(0), sp);
                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop true cond

                // iteration scope
                self.begin_scope();

                // k = __keys[__i]
                let k_slot = self.declare_local(key_name.clone(), sp);
                self.chunk.write(OpCode::OpGetLocal(keys_slot), vspan(sp));
                self.chunk.write(OpCode::OpGetLocal(i2_slot), vspan(sp));
                self.chunk.write(OpCode::OpGetIndex, vspan(sp));
                self.mark_initialized();

                // v = __col[k]
                let _v_slot = self.declare_local(val_name.clone(), sp);
                self.chunk.write(OpCode::OpGetLocal(col_slot), vspan(sp));
                self.chunk.write(OpCode::OpGetLocal(k_slot), vspan(sp));
                self.chunk.write(OpCode::OpGetIndex, vspan(sp));
                self.mark_initialized();

                for st in body {
                    self.emit_stmt(st);
                }
                self.end_scope(); // pops k, v

                // continue lands here: __i = __i + 1
                let continue_ip_kv = self.ip();
                self.chunk.write(OpCode::OpGetLocal(i2_slot), vspan(sp));
                let one = self.chunk.add_const(Value::Number(1.0));
                self.chunk.write(OpCode::OpConstant(one), vspan(sp));
                self.chunk.write(OpCode::OpAdd, vspan(sp));
                self.chunk.write(OpCode::OpSetLocal(i2_slot), vspan(sp));

                // back-edge
                self.chunk.write(OpCode::OpJump(loop_start), vspan(sp));

                // patch exit
                let exit_ip = self.ip();
                self.patch(jmp_exit_pos, OpCode::OpJumpIfFalse(exit_ip));
                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop false cond

                let break_exit_ip_kv = self.ip();

                // Patch all break and continue jumps (ForInKV)
                if let Some((_, _, break_patches, continue_patches)) = self.loop_ctx.pop() {
                    for cp in continue_patches {
                        self.patch(cp, OpCode::OpJump(continue_ip_kv));
                    }
                    for bp in break_patches {
                        self.patch(bp, OpCode::OpJump(break_exit_ip_kv));
                    }
                }

                self.end_scope(); // pops __i, __len, __keys, __col
            }

            // break; — emit jump to loop exit (patched after loop completes)
            StmtKind::Break => {
                // Pop locals introduced inside the loop before jumping out
                let locals_to_pop = if let Some(&(_, locals_at, _, _)) = self.loop_ctx.last() {
                    self.locals.len().saturating_sub(locals_at)
                } else {
                    0
                };
                for _ in 0..locals_to_pop {
                    self.chunk.write(OpCode::OpPop, vspan(sp));
                }
                let bp = self.chunk.instructions.len();
                self.chunk.write(OpCode::OpJump(0), vspan(sp)); // placeholder
                if let Some((_, _, patches, _)) = self.loop_ctx.last_mut() {
                    patches.push(bp);
                }
            }

            // continue; — advance to the next loop iteration.
            //
            // Must pop every local declared inside the loop body before
            // jumping; otherwise those locals leak onto the stack across
            // iterations and the compile-time slot tracker reads stale
            // values from the prior iteration. Matches the break-codegen
            // pop loop above. Without this, `let pos = i + N` after a
            // `continue` returns the previous iteration's `pos`.
            //
            // The jump target depends on loop shape: While/Loop have a real
            // backward target (loop_start, re-evaluating the condition). The
            // index loops (For/ForIn/ForInKV) use the `usize::MAX` sentinel —
            // their continue must land on the index-increment step emitted
            // *after* the body, so we forward-patch via continue_patch_ips.
            // Jumping to loop_start there would skip the increment and spin
            // forever.
            StmtKind::Continue => {
                let locals_to_pop = if let Some(&(_, locals_at, _, _)) = self.loop_ctx.last() {
                    self.locals.len().saturating_sub(locals_at)
                } else {
                    0
                };
                for _ in 0..locals_to_pop {
                    self.chunk.write(OpCode::OpPop, vspan(sp));
                }
                match self.loop_ctx.last().map(|(t, _, _, _)| *t) {
                    Some(target) if target != usize::MAX => {
                        // While/Loop: jump back to the condition/loop start.
                        self.chunk.write(OpCode::OpJump(target), vspan(sp));
                    }
                    Some(_) => {
                        // Index loop: forward-patch to the increment step.
                        let pos = self.chunk.instructions.len();
                        self.chunk.write(OpCode::OpJump(0), vspan(sp)); // placeholder
                        if let Some((_, _, _, continue_patches)) = self.loop_ctx.last_mut() {
                            continue_patches.push(pos);
                        }
                    }
                    None => {
                        // continue outside loop: emit a no-op jump to self
                        let here = self.chunk.instructions.len();
                        self.chunk.write(OpCode::OpJump(here + 1), vspan(sp));
                    }
                }
            }

            // throw expr; — raise a RuntimeError
            StmtKind::Throw(e) => {
                self.emit_expr(e);
                self.chunk.write(OpCode::OpThrow, vspan(sp));
            }

            // loop { body } — infinite loop with break
            StmtKind::Loop(body) => {
                let loop_start = self.ip();

                // Push loop context: continue goes back to loop_start, break patches will be collected
                self.loop_ctx
                    .push((loop_start, self.locals.len(), vec![], vec![]));

                self.begin_scope();
                for st in body {
                    self.emit_stmt(st);
                }
                self.end_scope();

                // back-edge: always jump back
                self.chunk.write(OpCode::OpJump(loop_start), vspan(sp));

                // patch all break jumps
                let exit_ip = self.ip();
                if let Some((_, _, break_patches, _)) = self.loop_ctx.pop() {
                    for bp in break_patches {
                        self.patch(bp, OpCode::OpJump(exit_ip));
                    }
                }
            }
        } // end emit_stmt match
    } // end emit_stmt fn

    fn emit_expr(&mut self, e: Expr) {
        let sp = e.span;
        match e.kind {
            ExprKind::GetAttr(obj, name) => {
                self.emit_expr(*obj);
                let name_idx = self.intern_name(&name);
                self.chunk.write(OpCode::OpGetAttr(name_idx), vspan(sp));
            }

            ExprKind::Index(obj, ix) => {
                self.emit_expr(*obj);
                self.emit_expr(*ix);
                self.chunk.write(OpCode::OpGetIndex, vspan(sp));
            }

            ExprKind::Array(items) => {
                let n = items.len();
                for it in items {
                    self.emit_expr(it);
                }
                self.chunk.write(OpCode::OpMakeArray(n), vspan(sp));
            }

            ExprKind::Map(pairs) => {
                let n = pairs.len();
                for (k, v) in pairs {
                    let kidx = self.chunk.add_const(alloc_string(k));
                    self.chunk.write(OpCode::OpConstant(kidx), vspan(sp));
                    self.emit_expr(v);
                }
                self.chunk.write(OpCode::OpMakeMap(n), vspan(sp));
            }

            ExprKind::Try(try_e, catch_e) => {
                // try <expr> catch <expr>
                // Lowering:
                //   OpTry(handler_ip)
                //   <try_e>
                //   OpEndTry
                //   OpJump(end_ip)
                // handler_ip:
                //   OpPop   // pop pushed error value
                //   <catch_e>
                // end_ip:
                let try_pos = self.emit(OpCode::OpTry(0), sp);
                self.emit_expr(*try_e);
                self.chunk.write(OpCode::OpEndTry, vspan(sp));
                let j_end = self.emit(OpCode::OpJump(0), sp);

                let handler_ip = self.ip();
                self.patch(try_pos, OpCode::OpTry(handler_ip));
                self.chunk.write(OpCode::OpPop, vspan(sp));
                self.emit_expr(*catch_e);

                let end_ip = self.ip();
                self.patch(j_end, OpCode::OpJump(end_ip));
            }

            ExprKind::Match(scrutinee, arms, else_expr) => {
                // Desugar:
                //   match SCR { case PAT: BODY; ...; else: E }  ==>
                //   {
                //       let __match = SCR;
                //       let __res = nil;
                //       if cond1 { /* bind? */ __res = BODY1; }
                //       else if cond2 { /* bind? */ __res = BODY2; }
                //       else { __res = E; }
                //       __res
                //   }
                //
                // where:
                //   - Literal arm: condN = (__match == LIT)
                //   - Bind arm   : condN = true; then-body binds `name` to __match before BODY
                //   - Wildcard   : condN = true.
                //
                // This keeps match semantics in terms of existing expression/statement constructs
                // (locals, if/else, block expr) and avoids custom stack juggling.
                let match_name = format!("__match_{}", sp.start);
                let res_name = format!("__match_res_{}", sp.start);

                // let __match = <scrutinee>;
                let let_match = Stmt {
                    kind: StmtKind::Let(match_name.clone(), None, *scrutinee),
                    span: sp,
                };

                // let __match_res = nil;
                let let_res = Stmt {
                    kind: StmtKind::Let(
                        res_name.clone(),
                        None,
                        Expr {
                            kind: ExprKind::Nil,
                            span: sp,
                        },
                    ),
                    span: sp,
                };

                // Build nested if/else chain from arms (iterate in reverse so we can nest).
                let mut else_block: Option<Vec<Stmt>> = else_expr.map(|e| {
                    vec![Stmt {
                        kind: StmtKind::Assign(res_name.clone(), *e),
                        span: sp,
                    }]
                });

                for (arm, body) in arms.into_iter().rev() {
                    let mut then_stmts: Vec<Stmt> = Vec::new();
                    let cond_expr = match arm {
                        MatchArm::Literal(lit) => {
                            // if __match == LIT { __res = BODY; }
                            then_stmts.push(Stmt {
                                kind: StmtKind::Assign(res_name.clone(), body),
                                span: sp,
                            });
                            Expr {
                                kind: ExprKind::Eq(
                                    Box::new(Expr {
                                        kind: ExprKind::Var(match_name.clone()),
                                        span: sp,
                                    }),
                                    Box::new(lit),
                                ),
                                span: sp,
                            }
                        }
                        MatchArm::Bind(name) => {
                            // if true { let name = __match; __res = BODY; }
                            then_stmts.push(Stmt {
                                kind: StmtKind::Let(
                                    name,
                                    None,
                                    Expr {
                                        kind: ExprKind::Var(match_name.clone()),
                                        span: sp,
                                    },
                                ),
                                span: sp,
                            });
                            then_stmts.push(Stmt {
                                kind: StmtKind::Assign(res_name.clone(), body),
                                span: sp,
                            });
                            Expr {
                                kind: ExprKind::Bool(true),
                                span: sp,
                            }
                        }
                        MatchArm::Wildcard => {
                            // if true { __res = BODY; }
                            then_stmts.push(Stmt {
                                kind: StmtKind::Assign(res_name.clone(), body),
                                span: sp,
                            });
                            Expr {
                                kind: ExprKind::Bool(true),
                                span: sp,
                            }
                        }
                        MatchArm::BindGuard(name, guard) => {
                            // Correct desugaring for `case n if guard: body`:
                            //   { let n = __match; if guard { __res = body; } else { <remaining arms> } }
                            // We consume else_block here so the fall-through is inside the guard check.
                            let remaining = else_block.take();
                            then_stmts.push(Stmt {
                                kind: StmtKind::Let(
                                    name,
                                    None,
                                    Expr {
                                        kind: ExprKind::Var(match_name.clone()),
                                        span: sp,
                                    },
                                ),
                                span: sp,
                            });
                            let inner_then = vec![Stmt {
                                kind: StmtKind::Assign(res_name.clone(), body),
                                span: sp,
                            }];
                            then_stmts.push(Stmt {
                                kind: StmtKind::If(guard, inner_then, remaining),
                                span: sp,
                            });
                            // The outer if always enters (true), so remaining arms
                            // are handled inside via the inner else.
                            Expr {
                                kind: ExprKind::Bool(true),
                                span: sp,
                            }
                        }
                    };

                    let if_stmt = Stmt {
                        kind: StmtKind::If(cond_expr, then_stmts, else_block.take()),
                        span: sp,
                    };
                    else_block = Some(vec![if_stmt]);
                }

                let mut block_stmts = vec![let_match, let_res];
                if let Some(mut root) = else_block {
                    block_stmts.append(&mut root);
                }

                let tail_expr = Expr {
                    kind: ExprKind::Var(res_name),
                    span: sp,
                };

                let desugared = Expr {
                    kind: ExprKind::BlockExpr(block_stmts, Some(Box::new(tail_expr))),
                    span: sp,
                };

                self.emit_expr(desugared);
            }

            ExprKind::IfExpr(cond, then_br, else_br) => {
                let tmp = format!("__if_{}", sp.start);
                let nil_expr = Expr {
                    kind: ExprKind::Nil,
                    span: sp,
                };
                let arm_stmts = |arm: &Expr| -> Vec<Stmt> {
                    match &arm.kind {
                        ExprKind::BlockExpr(inner, tail) => {
                            let mut stmts = inner.clone();
                            if let Some(t) = tail {
                                stmts.push(Stmt {
                                    kind: StmtKind::Assign(tmp.clone(), (**t).clone()),
                                    span: sp,
                                });
                            }
                            stmts
                        }
                        _ => vec![Stmt {
                            kind: StmtKind::Assign(tmp.clone(), arm.clone()),
                            span: sp,
                        }],
                    }
                };
                let desugared = Expr {
                    kind: ExprKind::BlockExpr(
                        vec![
                            Stmt {
                                kind: StmtKind::Let(tmp.clone(), None, nil_expr),
                                span: sp,
                            },
                            Stmt {
                                kind: StmtKind::If(
                                    *cond,
                                    arm_stmts(&then_br),
                                    Some(arm_stmts(&else_br)),
                                ),
                                span: sp,
                            },
                        ],
                        Some(Box::new(Expr {
                            kind: ExprKind::Var(tmp),
                            span: sp,
                        })),
                    ),
                    span: sp,
                };
                self.emit_expr(desugared);
            }

            ExprKind::BlockExpr(stmts, tail) => {
                // Desugar:
                // { stmts; expr? }  ==>  (fn(){ stmts; return expr; })()
                // This avoids stack gymnastics and gives correct lexical scoping via call frames.

                let mut body = stmts;
                if let Some(tail_expr) = tail {
                    body.push(Stmt {
                        kind: StmtKind::Return(Some(*tail_expr)),
                        span: sp,
                    });
                }

                let fn_lit = Expr {
                    kind: ExprKind::FnLit(None, vec![], None, body),
                    span: sp,
                };

                let call = Expr {
                    kind: ExprKind::Call(Box::new(fn_lit), vec![]),
                    span: sp,
                };

                self.emit_expr(call);
            }

            ExprKind::Call(callee, args) => {
                // VM expects: callee, arg1..argN
                self.emit_expr(*callee);

                let argc = args.len();
                for arg in args {
                    self.emit_expr(arg);
                }

                self.chunk.write(OpCode::OpCall(argc), vspan(sp));
            }

            ExprKind::FnLit(name_opt, params, _ret_ty, body) => {
                // Compile function literal into its own chunk.
                let mut f_compiler = Compiler {
                    chunk: Chunk::default(),
                    globals: HashMap::new(),
                    locals: vec![],
                    scope_depth: 1,
                    upvalues: vec![],
                    loop_ctx: vec![],
                    enclosing: Some(Box::new(Compiler {
                        chunk: Chunk::default(), // dummy
                        globals: HashMap::new(), // dummy
                        locals: self.locals.clone(),
                        scope_depth: self.scope_depth,
                        upvalues: self.upvalues.clone(),
                        loop_ctx: vec![],
                        enclosing: None,
                        error: None,
                    })),
                    error: None,
                };

                // params are locals in slot 0..arity-1
                for p in &params {
                    f_compiler.add_local(p.name.clone());
                }

                // function body scope
                f_compiler.begin_scope();
                for st in body {
                    f_compiler.emit_stmt(st);
                }
                f_compiler.end_scope();

                // fallthrough return nil
                let nil = f_compiler.chunk.add_const(Value::Nil);
                f_compiler
                    .chunk
                    .write(OpCode::OpConstant(nil), VmSpan::default());
                f_compiler.chunk.write(OpCode::OpReturn, VmSpan::default());

                optimize_chunk(&mut f_compiler.chunk);
                let is_vararg_lit = params.last().map(|p| p.is_vararg).unwrap_or(false);
                let named_arity_lit = if is_vararg_lit {
                    params.len() - 1
                } else {
                    params.len()
                };
                let min_arity_lit = params
                    .iter()
                    .filter(|p| p.default.is_none() && !p.is_vararg)
                    .count();
                let defaults_lit: Vec<pblang_ir::Value> = params
                    .iter()
                    .filter_map(|p| p.default.as_ref().map(expr_to_const_value))
                    .flatten()
                    .collect();
                let fun = Rc::new(Function {
                    name: name_opt,
                    arity: named_arity_lit,
                    min_arity: min_arity_lit,
                    defaults: defaults_lit,
                    is_vararg: is_vararg_lit,
                    chunk: f_compiler.chunk,
                    param_types: None,
                });

                let fn_const_idx = self.chunk.add_const(Value::Function(fun));

                let captures = f_compiler
                    .upvalues
                    .iter()
                    .map(|u| (u.is_local, u.index))
                    .collect::<Vec<(bool, usize)>>();

                self.chunk.write(
                    OpCode::OpClosure {
                        fn_const: fn_const_idx,
                        captures,
                    },
                    vspan(sp),
                );
            } // END OF FnLit

            ExprKind::Logical { left, op, right } => match op {
                LogicalOp::And => self.emit_and(*left, *right, sp),
                LogicalOp::Or => self.emit_or(*left, *right, sp),
            }, // END OF Logical

            ExprKind::Number(n) => {
                let k = self.chunk.add_const(Value::Number(n));
                self.chunk.write(OpCode::OpConstant(k), vspan(sp));
            }

            ExprKind::Str(s) => {
                //let k = self.chunk.add_const(Value::Str(s));
                let k = self.chunk.add_const(alloc_string(s));
                self.chunk.write(OpCode::OpConstant(k), vspan(sp));
            }

            ExprKind::Bool(b) => {
                let k = self.chunk.add_const(Value::Bool(b));
                self.chunk.write(OpCode::OpConstant(k), vspan(sp));
            }

            ExprKind::Var(name) => match self.resolve_name(&name, sp) {
                Resolved::Local(slot) => self.chunk.write(OpCode::OpGetLocal(slot), vspan(sp)),
                Resolved::Upvalue(slot) => self.chunk.write(OpCode::OpGetUpvalue(slot), vspan(sp)),
                Resolved::Global(g) => {
                    let name_idx = self.intern_name(&g);
                    self.chunk.write(OpCode::OpGetGlobal(name_idx), vspan(sp));
                }
            },

            ExprKind::Nil => {
                let k = self.chunk.add_const(Value::Nil);
                self.chunk.write(OpCode::OpConstant(k), vspan(sp));
            }

            // Left-associative arithmetic chains are flattened iteratively so a
            // very long flat chain (`1+1+1+…`, which the lexical nesting guard
            // can't see — it has no brackets) compiles without O(depth) native
            // recursion that would overflow the stack. Two prior O(n²)/overflow
            // traps are gone: (1) the old per-node `(a.kind.clone(), b.kind.clone())`
            // const-fold guard deep-cloned the whole left subtree at every level;
            // (2) recursing down the left spine. The emitted bytecode is identical
            // to the old recursive path, and the peephole pass in `optimize_chunk`
            // still constant-folds literal pairs. Right operands use the normal
            // recursive path — safe, since a deep *right* spine needs parentheses
            // (`1+(1+(1+…)))`) and is already rejected by the bracket-depth guard.
            kind @ (ExprKind::Add(..)
            | ExprKind::Sub(..)
            | ExprKind::Mul(..)
            | ExprKind::Div(..)) => {
                fn arith_op(k: &ExprKind) -> Option<OpCode> {
                    match k {
                        ExprKind::Add(..) => Some(OpCode::OpAdd),
                        ExprKind::Sub(..) => Some(OpCode::OpSub),
                        ExprKind::Mul(..) => Some(OpCode::OpMul),
                        ExprKind::Div(..) => Some(OpCode::OpDiv),
                        _ => None,
                    }
                }
                // Descend the left spine, recording (op, right_operand, op_span)
                // outermost-first; `cur` ends at the deepest-left leaf.
                let mut spine: Vec<(OpCode, Expr, Span)> = Vec::new();
                let mut cur = Expr { kind, span: sp };
                while let Some(op) = arith_op(&cur.kind) {
                    let cur_span = cur.span;
                    let (a, b) = match cur.kind {
                        ExprKind::Add(a, b)
                        | ExprKind::Sub(a, b)
                        | ExprKind::Mul(a, b)
                        | ExprKind::Div(a, b) => (a, b),
                        _ => unreachable!("arith_op gated this match"),
                    };
                    spine.push((op, *b, cur_span));
                    cur = *a;
                }
                self.emit_expr(cur);
                // Replay innermost-first: emit each right operand then its op.
                while let Some((op, right, opsp)) = spine.pop() {
                    self.emit_expr(right);
                    self.chunk.write(op, vspan(opsp));
                }
            }

            ExprKind::Eq(a, b) => {
                self.emit_expr(*a);
                self.emit_expr(*b);
                self.chunk.write(OpCode::OpEq, vspan(sp));
            }
            ExprKind::NotEq(a, b) => {
                self.emit_expr(*a);
                self.emit_expr(*b);
                self.chunk.write(OpCode::OpNotEq, vspan(sp));
            }
            ExprKind::Gt(a, b) => {
                self.emit_expr(*a);
                self.emit_expr(*b);
                self.chunk.write(OpCode::OpGt, vspan(sp));
            }
            ExprKind::Ge(a, b) => {
                self.emit_expr(*a);
                self.emit_expr(*b);
                self.chunk.write(OpCode::OpGe, vspan(sp));
            }
            ExprKind::Lt(a, b) => {
                self.emit_expr(*a);
                self.emit_expr(*b);
                self.chunk.write(OpCode::OpLt, vspan(sp));
            }
            ExprKind::Le(a, b) => {
                self.emit_expr(*a);
                self.emit_expr(*b);
                self.chunk.write(OpCode::OpLe, vspan(sp));
            }

            // --- Safety / Rust-inspired expressions ---

            // `!expr` — logical NOT
            ExprKind::Not(inner) => {
                self.emit_expr(*inner);
                self.chunk.write(OpCode::OpNot, vspan(sp));
            }

            // `expr?.field` — nil-safe attribute access: if nil push nil, else get attr
            ExprKind::SafeGetAttr(obj, name) => {
                self.emit_expr(*obj);
                let name_idx = self.intern_name(&name);
                self.chunk.write(OpCode::OpSafeGetAttr(name_idx), vspan(sp));
            }

            // `expr?[idx]` — nil-safe index access: if nil push nil, else get index
            ExprKind::SafeIndex(obj, idx) => {
                self.emit_expr(*obj);
                // Desugar: dup obj, check nil, if nil jump over, else emit idx + OpGetIndex
                // OpSafeIndex pops obj first, then reads the index off the stack
                // For simplicity: emit obj, then emit a new OpSafeIndex that reads the next two pops
                // Implementation: OpIsNil + branch + OpGetIndex
                // We'll emit the index BEFORE the opcode so the VM can pop(obj), pop(idx)
                // Actually: push obj, push idx, OpSafeIndex(pops both, checks obj)
                self.emit_expr(*idx);
                self.chunk.write(OpCode::OpSafeIndex, vspan(sp));
            }

            // `expr?` — Result unwrap: if ok==false throw RuntimeError, else push value
            ExprKind::ResultUnwrap(inner) => {
                self.emit_expr(*inner);
                self.chunk.write(OpCode::OpResultUnwrap, vspan(sp));
            }

            // `a ?? b` — null coalescing: if a is nil, use b
            // Stack trace:
            //   eval a            ; [a]
            //   OpDup             ; [a, a]
            //   OpIsNil           ; pops top a, pushes bool [a, is_nil]
            //   OpJumpIfFalse(not_nil_label)  ; PEEK (no pop): if NOT nil (false), jump [a, false]
            //   ; nil branch:
            //   OpPop             ; pop bool (true) [a(=nil)]
            //   OpPop             ; pop nil []
            //   eval b            ; [b]
            //   OpJump(end)
            //   not_nil_label:
            //   OpPop             ; pop bool (false) [a]
            //   end:
            ExprKind::NullCoalesce(a, b) => {
                self.emit_expr(*a);
                self.chunk.write(OpCode::OpDup, vspan(sp));
                self.chunk.write(OpCode::OpIsNil, vspan(sp));
                let jmp_not_nil = self.emit(OpCode::OpJumpIfFalse(0), sp);
                // --- nil branch (is_nil = true, peek didn't pop) ---
                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop is_nil bool (true)
                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop nil
                self.emit_expr(*b);
                let jmp_end = self.emit(OpCode::OpJump(0), sp);
                // --- not-nil branch ---
                let not_nil_ip = self.ip();
                self.patch(jmp_not_nil, OpCode::OpJumpIfFalse(not_nil_ip));
                self.chunk.write(OpCode::OpPop, vspan(sp)); // pop is_nil bool (false)
                                                            // (original a value remains on stack)
                let end_ip = self.ip();
                self.patch(jmp_end, OpCode::OpJump(end_ip));
            }

            // `f"Hello {name}"` — f-string interpolation
            ExprKind::FString(parts) => {
                use pblang_core::FStringPart;
                if parts.is_empty() {
                    let k = self.chunk.add_const(alloc_string(""));
                    self.chunk.write(OpCode::OpConstant(k), vspan(sp));
                    return;
                }
                let mut first = true;
                for part in parts {
                    match part {
                        FStringPart::Literal(s) => {
                            let k = self.chunk.add_const(alloc_string(s));
                            self.chunk.write(OpCode::OpConstant(k), vspan(sp));
                        }
                        FStringPart::Interp(e) => {
                            // Emit: str(expr) — use str builtin via OpGetGlobal + OpCall
                            let str_name = self.intern_name("str");
                            self.chunk.write(OpCode::OpGetGlobal(str_name), vspan(sp));
                            self.emit_expr(*e);
                            self.chunk.write(OpCode::OpCall(1), vspan(sp));
                        }
                    }
                    if !first {
                        self.chunk.write(OpCode::OpAdd, vspan(sp)); // concatenate
                    }
                    first = false;
                }
            }

            // `a ** b` — exponentiation. Emits the OpPow opcode (which routes
            // through `Value::arith(Pow)`, identical to the former `num_pow`
            // call for numbers/bigint/rational/decimal) so that a `__pow__`/
            // `__rpow__` operator overload can be dispatched on the slow path.
            ExprKind::Pow(a, b) => {
                self.emit_expr(*a);
                self.emit_expr(*b);
                self.chunk.write(OpCode::OpPow, vspan(sp));
            }

            // `a % b` — modulo. Emits OpMod (≡ `Value::arith(Mod)`, the same math
            // the former `num_mod` call performed) so `__mod__`/`__rmod__`
            // overloads can be dispatched on the slow path.
            ExprKind::Mod(a, b) => {
                self.emit_expr(*a);
                self.emit_expr(*b);
                self.chunk.write(OpCode::OpMod, vspan(sp));
            }
        } // end emit_expr match
    } // end emit_expr fn

    fn intern_name(&mut self, name: &str) -> usize {
        if let Some(&idx) = self.globals.get(name) {
            return idx;
        }
        let idx = self.chunk.add_const(alloc_string(name.to_string()));
        self.globals.insert(name.to_string(), idx);
        idx
    }
}

fn optimize_chunk(chunk: &mut Chunk) {
    // Pass 1: Constant folding — replace constant arithmetic at compile time.
    // Walk the instruction list looking for pairs: OpConstant(a) + OpConstant(b) + OpXxx
    // where OpXxx is a pure arithmetic/comparison op. Replace with a single OpConstant(result).
    // Fold to a FIXPOINT so constant chains like (((2+3)+4)+5) collapse fully:
    // one sweep only reaches the first pair, leaving the folded result adjacent to
    // the next literal for the following sweep to combine. Each fold is individually
    // correct and the basic-block guard is conservative, so iterating never
    // mis-folds; a pass cap bounds the work.
    let fold_cap = chunk.instructions.len() + 1;
    let mut fold_round = 0usize;
    loop {
    let n = chunk.instructions.len();
    let mut folded_any = false;
    // Collect jump/branch targets so we never fold across a basic-block boundary.
    // Control flow can enter the middle of a 3-instruction window — e.g. a try/catch's
    // success path jumps onto the RHS `OpConstant` of a following comparison — and folding
    // the window into one constant would corrupt that entry. (This was the root cause of
    // `(try x catch <num>) <op> <num>` mis-evaluating.)
    let mut jump_targets: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for op in &chunk.instructions {
        match op {
            OpCode::OpJump(t)
            | OpCode::OpJumpIfFalse(t)
            | OpCode::OpJumpIfTrue(t)
            | OpCode::OpTry(t) => {
                jump_targets.insert(*t);
            }
            _ => {}
        }
    }
    let mut i = 0usize;
    while i + 2 < n {
        // Peek at three consecutive instructions
        let op0 = chunk.instructions.get(i).cloned();
        let op1 = chunk.instructions.get(i + 1).cloned();
        let op2 = chunk.instructions.get(i + 2).cloned();

        // Pattern: OpConstant(a), OpConstant(b), BinaryOp → fold to OpConstant(result),
        // but only if no branch can land on i+1/i+2 (same basic block).
        if jump_targets.contains(&(i + 1)) || jump_targets.contains(&(i + 2)) {
            i += 1;
            continue;
        }
        if let (Some(OpCode::OpConstant(ia)), Some(OpCode::OpConstant(ib)), Some(ref arith)) =
            (op0, op1, op2)
        {
            let va = chunk.constants.get(ia).cloned();
            let vb = chunk.constants.get(ib).cloned();
            let folded: Option<Value> = match (va, vb, arith) {
                (Some(Value::Number(a)), Some(Value::Number(b)), OpCode::OpAdd) => {
                    Some(Value::Number(a + b))
                }
                (Some(Value::Number(a)), Some(Value::Number(b)), OpCode::OpSub) => {
                    Some(Value::Number(a - b))
                }
                (Some(Value::Number(a)), Some(Value::Number(b)), OpCode::OpMul) => {
                    Some(Value::Number(a * b))
                }
                (Some(Value::Number(a)), Some(Value::Number(b)), OpCode::OpDiv) if b != 0.0 => {
                    Some(Value::Number(a / b))
                }
                (Some(Value::Number(a)), Some(Value::Number(b)), OpCode::OpEq) => {
                    Some(Value::Bool(a == b))
                }
                (Some(Value::Number(a)), Some(Value::Number(b)), OpCode::OpNotEq) => {
                    Some(Value::Bool(a != b))
                }
                (Some(Value::Number(a)), Some(Value::Number(b)), OpCode::OpLt) => {
                    Some(Value::Bool(a < b))
                }
                (Some(Value::Number(a)), Some(Value::Number(b)), OpCode::OpLe) => {
                    Some(Value::Bool(a <= b))
                }
                (Some(Value::Number(a)), Some(Value::Number(b)), OpCode::OpGt) => {
                    Some(Value::Bool(a > b))
                }
                (Some(Value::Number(a)), Some(Value::Number(b)), OpCode::OpGe) => {
                    Some(Value::Bool(a >= b))
                }
                // String concatenation folding
                _ => None,
            };
            if let Some(result) = folded {
                folded_any = true;
                let new_idx = chunk.constants.len();
                chunk.constants.push(result);
                // Replace the 3-instruction sequence with OpConstant(new_idx) + 2 x OpPop-and-skip
                // Since we can't delete instructions (jump targets), replace with OpConstant + OpJump(skip)
                // Simpler: Replace [OpConstant(a), OpConstant(b), BinaryOp] with
                //          [OpConstant(result), OpJump(i+3), <dead OpConstant(b)>]
                // But this is complex. Use a different approach: replace with NOP pattern.
                // For now, just fold by replacing op2 (the arith op) with OpConstant and
                // turning op0/op1 into "load nil then pop" via OpConstant(nil)+OpPop+OpConstant(nil)+OpPop.
                // Actually, simplest safe approach: mark op0 and op1 as no-ops (OpJump(i+1) = jump to self+1).
                // We use OpJump(next) for op0 and op1, and OpConstant(result) for the operator slot.
                let span0 = chunk.spans.get(i).copied().unwrap_or_default();
                let span2 = chunk.spans.get(i + 2).copied().unwrap_or_default();
                // Turn the 3-instruction window into:
                // i+0: OpJump(i+1)  (skip to next, effectively no-op)
                // i+1: OpJump(i+2)  (skip to next)
                // i+2: OpConstant(new_idx)  (the folded result)
                chunk.instructions[i] = OpCode::OpJump(i + 1);
                chunk.instructions[i + 1] = OpCode::OpJump(i + 2);
                chunk.instructions[i + 2] = OpCode::OpConstant(new_idx);
                chunk.spans[i] = span0;
                chunk.spans[i + 2] = span2;
                i += 3;
                continue;
            }
        }
        i += 1;
    }
    fold_round += 1;
    if !folded_any || fold_round > fold_cap {
        break;
    }
    }

    // Pass 2: Jump threading — jump-to-jump becomes direct jump
    // This keeps bytecode stable and cheap without changing instruction indices (no deletions).
    // `thread_cap` bounds the fixpoint: legitimate jump chains resolve in at most
    // `len` passes, but a pathological jump CYCLE (e.g. a 3-cycle of OpJumps) would
    // otherwise oscillate forever and hang the compiler. The cap turns that into a
    // (harmless) early stop instead of a denial of service.
    let thread_cap = chunk.instructions.len() + 1;
    let mut thread_round = 0usize;
    let mut changed = true;
    while changed && thread_round <= thread_cap {
        thread_round += 1;
        changed = false;
        for i in 0..chunk.instructions.len() {
            let op = chunk.instructions[i].clone();
            match op {
                OpCode::OpJump(t) => {
                    if let Some(OpCode::OpJump(t2)) = chunk.instructions.get(t).cloned() {
                        if t != i && t2 != i {
                            // avoid self-jumps
                            chunk.instructions[i] = OpCode::OpJump(t2);
                            changed = true;
                        }
                    }
                }
                OpCode::OpJumpIfFalse(t) => {
                    if let Some(OpCode::OpJump(t2)) = chunk.instructions.get(t).cloned() {
                        chunk.instructions[i] = OpCode::OpJumpIfFalse(t2);
                        changed = true;
                    }
                }
                OpCode::OpJumpIfTrue(t) => {
                    if let Some(OpCode::OpJump(t2)) = chunk.instructions.get(t).cloned() {
                        chunk.instructions[i] = OpCode::OpJumpIfTrue(t2);
                        changed = true;
                    }
                }
                _ => {}
            }
        }
    }

    // Pass 3: Peephole — remove redundant instruction pairs.
    // Like pass 1, these pair-removals are only sound WITHIN a basic block: if a
    // branch can land on the SECOND instruction of the pair, replacing it with a
    // no-op jump changes the stack effect on that entry path (the same class of
    // bug pass 1's basic-block guard fixed). So recompute jump targets and never
    // collapse a pair whose second slot is a branch target.
    let pe_targets: std::collections::HashSet<usize> = chunk
        .instructions
        .iter()
        .filter_map(|op| match op {
            OpCode::OpJump(t)
            | OpCode::OpJumpIfFalse(t)
            | OpCode::OpJumpIfTrue(t)
            | OpCode::OpTry(t) => Some(*t),
            _ => None,
        })
        .collect();
    let n2 = chunk.instructions.len();
    let mut i = 0;
    while i + 1 < n2 {
        let op0 = chunk.instructions.get(i).cloned();
        let op1 = chunk.instructions.get(i + 1).cloned();

        // A branch into the middle of the pair makes removal unsafe — skip it.
        let pair_entered = pe_targets.contains(&(i + 1));

        // Dup + Pop → remove both (net zero effect), replace with OpJump(next)
        if !pair_entered
            && matches!(op0, Some(OpCode::OpDup))
            && matches!(op1, Some(OpCode::OpPop))
        {
            chunk.instructions[i] = OpCode::OpJump(i + 2);
            chunk.instructions[i + 1] = OpCode::OpJump(i + 2);
            i += 2;
            continue;
        }

        // Not + Not → remove both (double negation), replace with OpJump(next)
        if !pair_entered
            && matches!(op0, Some(OpCode::OpNot))
            && matches!(op1, Some(OpCode::OpNot))
        {
            chunk.instructions[i] = OpCode::OpJump(i + 2);
            chunk.instructions[i + 1] = OpCode::OpJump(i + 2);
            i += 2;
            continue;
        }

        // IsNil followed immediately by Not → IsNotNil optimization
        // (IsNil+Not = check if not nil). Can fold but keep for now.

        // Jump to immediate next instruction → dead jump, replace with OpJump(next)
        if let Some(OpCode::OpJump(target)) = op0 {
            if target == i + 1 {
                // Jump to next instruction is dead code
                chunk.instructions[i] = OpCode::OpJump(i + 1); // stays as no-op
            }
        }

        i += 1;
    }
}

/// Format chunk as human-readable bytecode (for debugging / reverse engineering).
pub fn disasm(chunk: &Chunk) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    writeln!(out, "=== constants ({} total) ===", chunk.constants.len()).ok();
    for (i, c) in chunk.constants.iter().enumerate() {
        let preview = match c {
            Value::Number(n) => format!("{n}"),
            Value::Bool(b) => format!("{b}"),
            Value::Nil => "nil".into(),
            Value::Obj(o) => match o.as_ref() {
                pblang_ir::Obj::String(s) => format!("{:?}", s.as_str()),
                pblang_ir::Obj::BigInt(n) => format!("{n}n"),
                pblang_ir::Obj::Rational(r) => format!("{r}"),
                pblang_ir::Obj::Decimal(d) => format!("{d}"),
            },
            Value::Array(_) => "<array>".into(),
            Value::Map(_) => "<map>".into(),
            Value::Function(_) => "<fn>".into(),
            Value::Closure(_) => "<closure>".into(),
            Value::Builtin(b) => format!("<builtin {}>", b.name),
            Value::Module(_) => "<module>".into(),
        };
        writeln!(out, "  [{i}] {preview}").ok();
    }
    writeln!(out, "=== bytecode ===").ok();
    for (i, op) in chunk.instructions.iter().enumerate() {
        let line = match op {
            OpCode::OpConstant(idx) => format!("  {:4} OpConstant {}", i, idx),
            OpCode::OpAdd => format!("  {:4} OpAdd", i),
            OpCode::OpSub => format!("  {:4} OpSub", i),
            OpCode::OpMul => format!("  {:4} OpMul", i),
            OpCode::OpDiv => format!("  {:4} OpDiv", i),
            OpCode::OpEq => format!("  {:4} OpEq", i),
            OpCode::OpNotEq => format!("  {:4} OpNotEq", i),
            OpCode::OpGt => format!("  {:4} OpGt", i),
            OpCode::OpGe => format!("  {:4} OpGe", i),
            OpCode::OpLt => format!("  {:4} OpLt", i),
            OpCode::OpLe => format!("  {:4} OpLe", i),
            OpCode::OpPop => format!("  {:4} OpPop", i),
            OpCode::OpPrint => format!("  {:4} OpPrint", i),
            OpCode::OpDefineGlobal(idx) => format!("  {:4} OpDefineGlobal {}", i, idx),
            OpCode::OpGetGlobal(idx) => format!("  {:4} OpGetGlobal {}", i, idx),
            OpCode::OpSetGlobal(idx) => format!("  {:4} OpSetGlobal {}", i, idx),
            OpCode::OpGetAttr(idx) => format!("  {:4} OpGetAttr {}", i, idx),
            OpCode::OpSetAttr(idx) => format!("  {:4} OpSetAttr {}", i, idx),
            OpCode::OpExport(idx) => format!("  {:4} OpExport {}", i, idx),
            OpCode::OpMakeArray(n) => format!("  {:4} OpMakeArray {}", i, n),
            OpCode::OpMakeMap(n) => format!("  {:4} OpMakeMap {}", i, n),
            OpCode::OpGetIndex => format!("  {:4} OpGetIndex", i),
            OpCode::OpSetIndex => format!("  {:4} OpSetIndex", i),
            OpCode::OpTry(ip) => format!("  {:4} OpTry -> {}", i, ip),
            OpCode::OpEndTry => format!("  {:4} OpEndTry", i),
            OpCode::OpGetLocal(slot) => format!("  {:4} OpGetLocal {}", i, slot),
            OpCode::OpSetLocal(slot) => format!("  {:4} OpSetLocal {}", i, slot),
            OpCode::OpClosure { fn_const, captures } => {
                format!(
                    "  {:4} OpClosure fn_const={} captures={:?}",
                    i,
                    fn_const,
                    captures.len()
                )
            }
            OpCode::OpGetUpvalue(idx) => format!("  {:4} OpGetUpvalue {}", i, idx),
            OpCode::OpSetUpvalue(idx) => format!("  {:4} OpSetUpvalue {}", i, idx),
            OpCode::OpCloseUpvalue => format!("  {:4} OpCloseUpvalue", i),
            OpCode::OpCall(argc) => format!("  {:4} OpCall {}", i, argc),
            OpCode::OpTailCall(argc) => format!("  {:4} OpTailCall {}", i, argc),
            OpCode::OpReturn => format!("  {:4} OpReturn", i),
            OpCode::OpDefer => format!("  {:4} OpDefer", i),
            OpCode::OpJump(ip) => format!("  {:4} OpJump -> {}", i, ip),
            OpCode::OpJumpIfFalse(ip) => format!("  {:4} OpJumpIfFalse -> {}", i, ip),
            OpCode::OpJumpIfTrue(ip) => format!("  {:4} OpJumpIfTrue -> {}", i, ip),
            OpCode::OpNot => format!("  {:4} OpNot", i),
            OpCode::OpSafeGetAttr(idx) => format!("  {:4} OpSafeGetAttr({})", i, idx),
            OpCode::OpSafeIndex => format!("  {:4} OpSafeIndex", i),
            OpCode::OpResultUnwrap => format!("  {:4} OpResultUnwrap", i),
            OpCode::OpIsNil => format!("  {:4} OpIsNil", i),
            OpCode::OpDup => format!("  {:4} OpDup", i),
            OpCode::OpThrow => format!("  {:4} OpThrow", i),
            OpCode::OpNegate => format!("  {:4} OpNegate", i),
            OpCode::OpMod => format!("  {:4} OpMod", i),
            OpCode::OpPow => format!("  {:4} OpPow", i),
        };
        writeln!(out, "{}", line).ok();
    }
    out
}
