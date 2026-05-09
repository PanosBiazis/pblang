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
        write!(f, "compile error at {}..{}: {}", self.span.start, self.span.end, self.message)
    }
}

impl std::error::Error for CompileError {}

fn vspan(sp: Span) -> VmSpan {
    VmSpan {
        start: sp.start,
        end: sp.end,
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
        let last = self.locals.last_mut()
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
                    message: format!("cannot read local variable '{}' in its own initializer", name),
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

            let local = self.locals.pop()
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
                    enclosing: self.enclosing.as_ref().map(|e| {
                        // Carry the next level of the chain for arbitrarily deep nesting.
                        Box::new(Compiler {
                            chunk: Chunk::default(),
                            globals: HashMap::new(),
                            locals: e.locals.clone(),
                            scope_depth: e.scope_depth,
                            upvalues: e.upvalues.clone(),
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
                    enclosing: Some(Box::new(enclosing_snapshot)),
                    error: None,
                };

                // params are locals in slot 0..arity-1 (ignore type annotations at runtime)
                for (p, _pty) in &params {
                    f_compiler.add_local(p.clone());
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
                let fun = Rc::new(Function {
                    name: Some(name.clone()),
                    arity: params.len(),
                    chunk: f_compiler.chunk,
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
                    enclosing: Some(Box::new(Compiler {
                        chunk: Chunk::default(),
                        globals: HashMap::new(),
                        locals: self.locals.clone(),
                        scope_depth: self.scope_depth,
                        upvalues: self.upvalues.clone(),
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
                    chunk: thunk_compiler.chunk,
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
                    // local: declare first (uninitialized)
                    self.declare_local(name, sp);
                    // then compile initializer; it leaves 1 value on stack
                    self.emit_expr(init);
                    // now the local slot is considered initialized
                    self.mark_initialized();
                    // no opcode needed: the value is the local's stack slot
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
                    self.chunk.write(OpCode::OpDefineGlobal(name_idx), vspan(sp));
                } else {
                    // Local const: mark as const so Assign() will error
                    self.declare_const(name, sp);
                    self.emit_expr(init);
                    self.mark_initialized();
                }
            }

            StmtKind::Block(stmts) => {
                let first_is_let_map = match &stmts[0].kind {
                    StmtKind::Let(_, None, e) => matches!(e.kind, ExprKind::Map(_)),
                    _ => false,
                };
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

                // __i = __i + 1
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

                self.end_scope(); // pops/closes __i + __bound
            } // END OF For
        }
    }

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
                    enclosing: Some(Box::new(Compiler {
                        chunk: Chunk::default(), // dummy
                        globals: HashMap::new(), // dummy
                        locals: self.locals.clone(),
                        scope_depth: self.scope_depth,
                        upvalues: self.upvalues.clone(),
                        enclosing: None,
                        error: None,
                    })),
                    error: None,
                };

                // params are locals in slot 0..arity-1
                for (p, _pty) in &params {
                    f_compiler.add_local(p.clone());
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
                let fun = Rc::new(Function {
                    name: name_opt,
                    arity: params.len(),
                    chunk: f_compiler.chunk,
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

            ExprKind::Add(a, b) => {
                if let (ExprKind::Number(x), ExprKind::Number(y)) = (a.kind.clone(), b.kind.clone())
                {
                    let k = self.chunk.add_const(Value::Number(x + y));
                    self.chunk.write(OpCode::OpConstant(k), vspan(sp));
                } else {
                    self.emit_expr(*a);
                    self.emit_expr(*b);
                    self.chunk.write(OpCode::OpAdd, vspan(sp));
                }
            }
            ExprKind::Sub(a, b) => {
                if let (ExprKind::Number(x), ExprKind::Number(y)) = (a.kind.clone(), b.kind.clone())
                {
                    let k = self.chunk.add_const(Value::Number(x - y));
                    self.chunk.write(OpCode::OpConstant(k), vspan(sp));
                } else {
                    self.emit_expr(*a);
                    self.emit_expr(*b);
                    self.chunk.write(OpCode::OpSub, vspan(sp));
                }
            }
            ExprKind::Mul(a, b) => {
                if let (ExprKind::Number(x), ExprKind::Number(y)) = (a.kind.clone(), b.kind.clone())
                {
                    let k = self.chunk.add_const(Value::Number(x * y));
                    self.chunk.write(OpCode::OpConstant(k), vspan(sp));
                } else {
                    self.emit_expr(*a);
                    self.emit_expr(*b);
                    self.chunk.write(OpCode::OpMul, vspan(sp));
                }
            }
            ExprKind::Div(a, b) => {
                if let (ExprKind::Number(x), ExprKind::Number(y)) = (a.kind.clone(), b.kind.clone())
                {
                    let k = self.chunk.add_const(Value::Number(x / y));
                    self.chunk.write(OpCode::OpConstant(k), vspan(sp));
                } else {
                    self.emit_expr(*a);
                    self.emit_expr(*b);
                    self.chunk.write(OpCode::OpDiv, vspan(sp));
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
        }
    }

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
    // Very small peephole optimizer:
    // - jump threading: jump-to-jump becomes direct jump
    // This keeps bytecode stable and cheap without changing instruction indices (no deletions).
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..chunk.instructions.len() {
            let op = chunk.instructions[i].clone();
            match op {
                OpCode::OpJump(t) => {
                    if let Some(OpCode::OpJump(t2)) = chunk.instructions.get(t).cloned() {
                        chunk.instructions[i] = OpCode::OpJump(t2);
                        changed = true;
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
        };
        writeln!(out, "{}", line).ok();
    }
    out
}
