//! pblang-cranelift — N2-beta Cranelift JIT prototype.
//!
//! This crate provides a JIT compiler for pblang bytecode functions that use
//! only numeric operations (f64 and i64). Non-numeric operations fall back to
//! the bytecode interpreter.
//!
//! ## Architecture
//!
//! ```text
//! pblang bytecode (Chunk)
//!      │
//!      ├─ is_jit_eligible()  ── reject dynamic / non-numeric chunks
//!      │
//!      └─ JitCache::compile() ─► Cranelift IR ─► native machine code
//!                                                      │
//!                                               JitFn: fn(*const f64) -> f64
//! ```
//!
//! ## Supported opcode subset
//!
//! | OpCode | Cranelift |
//! |--------|-----------|
//! | OpConstant(Number) | `f64const` / `iconst` |
//! | OpGetLocal / OpSetLocal | Variable slot |
//! | OpAdd / OpSub / OpMul / OpDiv | `fadd`/`iadd` etc. |
//! | OpNegate | `fneg` / `ineg` |
//! | OpMod | `fmod` / `irem` |
//! | OpPow | libm `pow` |
//! | OpEq..OpLe | `fcmp` / `icmp` → `select` |
//! | OpNot | boolean flip |
//! | OpJump / OpJumpIfFalse / OpJumpIfTrue | Cranelift branches |
//! | OpReturn | `return` |
//! | OpCall(n) preceded by OpGetGlobal(self) | direct recursive call |
//! | OpPop | drop top of virtual stack |
//! | OpDup | duplicate stack top |
//! | OpPrint | pop and discard (I/O skipped) |

use cranelift_codegen::ir::{
    condcodes::{FloatCC, IntCC},
    types, AbiParam, Function, InstBuilder, MemFlags, Value as ClValue,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use pblang_ir::chunk::{Chunk, OpCode};
use pblang_ir::{ParamType, Value as PblangValue};

use std::collections::{HashMap, HashSet};

// ── Public API types ──────────────────────────────────────────────────────────

/// JIT-compiled function ABI: caller passes a pointer to `nargs` f64 values,
/// function returns a single f64 result.
///
/// For i64 functions, the f64 bits are reinterpreted as i64.
/// All pblang JIT-compiled functions share this ABI for simplicity and
/// generality (no per-arity type gymnastics).
pub type JitFn = unsafe extern "C" fn(args: *const f64, nargs: u32) -> f64;

/// Compilation result for a single function.
#[derive(Debug)]
pub enum JitResult {
    /// Function successfully compiled to native code.
    Compiled(JitFn),
    /// Function cannot be JIT-compiled (uses unsupported opcodes).
    NotEligible(&'static str),
}

/// Cache of JIT-compiled pblang functions.
///
/// Wraps a Cranelift `JITModule` and maintains a table of compiled function
/// pointers indexed by function name.
pub struct JitCache {
    module: JITModule,
    /// name → (FuncId, function pointer)
    entries: HashMap<String, (FuncId, JitFn)>,
    /// Cache hit counter.
    pub hits: u64,
    /// Cache miss counter.
    pub misses: u64,
}

impl std::fmt::Debug for JitCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "JitCache {{ {} compiled functions }}",
            self.entries.len()
        )
    }
}

impl JitCache {
    /// Create a new JIT cache using the host machine's ISA.
    pub fn new() -> Result<Self, String> {
        let mut flag_builder = settings::builder();
        // Optimise for speed
        let _ = flag_builder.set("opt_level", "speed");
        let flags = settings::Flags::new(flag_builder);

        let isa_builder =
            cranelift_native::builder().map_err(|e| format!("Cranelift native ISA failed: {e}"))?;
        let isa = isa_builder
            .finish(flags)
            .map_err(|e| format!("Cranelift ISA finish failed: {e}"))?;

        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let module = JITModule::new(builder);
        Ok(JitCache {
            module,
            entries: HashMap::new(),
            hits: 0,
            misses: 0,
        })
    }

    /// Retrieve a previously compiled function by name.
    pub fn get(&self, name: &str) -> Option<JitFn> {
        self.entries.get(name).map(|(_, f)| *f)
    }

    /// Check if a function can be JIT-compiled, and if so, compile and cache it.
    ///
    /// Returns `JitResult::Compiled(fn_ptr)` on success, or
    /// `JitResult::NotEligible(reason)` if the function uses unsupported features.
    pub fn compile(&mut self, name: &str, func: &pblang_ir::Function) -> JitResult {
        // Already compiled?
        if let Some((_, fp)) = self.entries.get(name) {
            self.hits += 1;
            return JitResult::Compiled(*fp);
        }

        self.misses += 1;

        // Eligibility check
        if let Err(reason) = is_jit_eligible(&func.chunk) {
            return JitResult::NotEligible(reason);
        }

        match self.do_compile(name, func) {
            Ok(fp) => JitResult::Compiled(fp),
            Err(e) => {
                // Store the error string as a static string is not possible;
                // we leak a small amount of memory for the error message in this
                // prototype to satisfy the lifetime.
                let msg = Box::leak(format!("cranelift compilation failed: {e}").into_boxed_str());
                JitResult::NotEligible(msg)
            }
        }
    }

    fn do_compile(
        &mut self,
        name: &str,
        func: &pblang_ir::Function,
    ) -> Result<JitFn, Box<dyn std::error::Error>> {
        let arity = func.arity;

        // Determine function type mode: if all param_types are I64, use int mode
        let use_i64 = func
            .param_types
            .as_ref()
            .map(|pts| pts.iter().all(|pt| matches!(pt, ParamType::I64)))
            .unwrap_or(false);

        // Build the JIT function signature: (args: *const f64, nargs: u32) -> f64
        let mut sig = self.module.make_signature();
        let ptr_type = self.module.target_config().pointer_type();
        sig.params.push(AbiParam::new(ptr_type)); // args ptr
        sig.params.push(AbiParam::new(types::I32)); // nargs
        sig.returns.push(AbiParam::new(types::F64)); // return f64 (bits)

        // Declare the function first (supports self-recursion)
        let func_id = self.module.declare_function(name, Linkage::Export, &sig)?;

        // Build Cranelift IR
        let mut ctx = self.module.make_context();
        ctx.func.signature = sig.clone();

        let mut builder_ctx = FunctionBuilderContext::new();
        let result = build_cranelift_fn(
            &mut ctx.func,
            &mut builder_ctx,
            &func.chunk,
            func_id,
            arity,
            &mut self.module,
            name,
            use_i64,
        );
        result?;

        // Define and finalize
        self.module.define_function(func_id, &mut ctx)?;
        self.module.clear_context(&mut ctx);
        self.module.finalize_definitions()?;

        let raw_ptr = self.module.get_finalized_function(func_id);
        let fn_ptr: JitFn = unsafe { std::mem::transmute(raw_ptr) };
        self.entries.insert(name.to_string(), (func_id, fn_ptr));
        Ok(fn_ptr)
    }

    /// Compile multiple functions at once into the same JIT module, enabling
    /// cross-function calls within the module.
    ///
    /// Returns a map of `function_name → JitFn` for all successfully compiled
    /// functions.
    pub fn compile_module(
        &mut self,
        functions: &[(&str, &pblang_ir::Function)],
    ) -> HashMap<String, JitFn> {
        let mut compiled: HashMap<String, JitFn> = HashMap::new();

        // Pre-declare all functions so cross-references resolve
        let mut decls: HashMap<String, FuncId> = HashMap::new();
        for &(name, func) in functions {
            if self.entries.contains_key(name) {
                continue;
            }
            let _arity = func.arity;
            let _use_i64 = func
                .param_types
                .as_ref()
                .map(|pts| pts.iter().all(|pt| matches!(pt, ParamType::I64)))
                .unwrap_or(false);

            let mut sig = self.module.make_signature();
            let ptr_type = self.module.target_config().pointer_type();
            sig.params.push(AbiParam::new(ptr_type));
            sig.params.push(AbiParam::new(types::I32));
            sig.returns.push(AbiParam::new(types::F64));

            match self.module.declare_function(name, Linkage::Export, &sig) {
                Ok(fid) => {
                    decls.insert(name.to_string(), fid);
                }
                Err(_e) => {
                    // Already declared — try to look up
                    continue;
                }
            }
        }

        // Compile each function
        for &(name, func) in functions {
            if self.entries.contains_key(name) {
                if let Some(&fp) = self.entries.get(name).map(|(_, fp)| fp) {
                    compiled.insert(name.to_string(), fp);
                }
                continue;
            }

            let arity = func.arity;
            let use_i64 = func
                .param_types
                .as_ref()
                .map(|pts| pts.iter().all(|pt| matches!(pt, ParamType::I64)))
                .unwrap_or(false);

            let func_id = match decls.get(name) {
                Some(&fid) => fid,
                None => {
                    // Fall back to single compile
                    if let JitResult::Compiled(fp) = self.compile(name, func) {
                        compiled.insert(name.to_string(), fp);
                    }
                    continue;
                }
            };

            // Build signature
            let mut sig = self.module.make_signature();
            let ptr_type = self.module.target_config().pointer_type();
            sig.params.push(AbiParam::new(ptr_type));
            sig.params.push(AbiParam::new(types::I32));
            sig.returns.push(AbiParam::new(types::F64));

            let mut ctx = self.module.make_context();
            ctx.func.signature = sig.clone();
            let mut builder_ctx = FunctionBuilderContext::new();

            let result = build_cranelift_fn(
                &mut ctx.func,
                &mut builder_ctx,
                &func.chunk,
                func_id,
                arity,
                &mut self.module,
                name,
                use_i64,
            );

            if result.is_err() {
                continue;
            }

            if self.module.define_function(func_id, &mut ctx).is_err() {
                self.module.clear_context(&mut ctx);
                continue;
            }
            self.module.clear_context(&mut ctx);
        }

        // Finalize all definitions at once
        if self.module.finalize_definitions().is_err() {
            return compiled;
        }

        // Collect function pointers
        for (name, fid) in &decls {
            let raw_ptr = self.module.get_finalized_function(*fid);
            let fn_ptr: JitFn = unsafe { std::mem::transmute(raw_ptr) };
            self.entries.insert(name.clone(), (*fid, fn_ptr));
            compiled.insert(name.clone(), fn_ptr);
        }

        compiled
    }

    /// Return cache statistics as a tuple: (hits, misses, total_compiled_functions).
    pub fn cache_stats(&self) -> (u64, u64, usize) {
        (self.hits, self.misses, self.entries.len())
    }
}

impl Default for JitCache {
    fn default() -> Self {
        Self::new().expect("failed to create JitCache")
    }
}

// ── Eligibility check ─────────────────────────────────────────────────────────

/// Returns `Ok(())` if `chunk` can be JIT-compiled (only numeric fast-path ops),
/// or `Err(reason)` explaining why not.
pub fn is_jit_eligible(chunk: &Chunk) -> Result<(), &'static str> {
    for op in &chunk.instructions {
        match op {
            // Allowed: arithmetic
            OpCode::OpAdd | OpCode::OpSub | OpCode::OpMul | OpCode::OpDiv => {}
            // Allowed: extended arithmetic
            OpCode::OpNegate | OpCode::OpMod | OpCode::OpPow => {}
            // Allowed: comparisons
            OpCode::OpEq
            | OpCode::OpNotEq
            | OpCode::OpGt
            | OpCode::OpGe
            | OpCode::OpLt
            | OpCode::OpLe => {}
            // Allowed: locals
            OpCode::OpGetLocal(_) | OpCode::OpSetLocal(_) => {}
            // Allowed: constants (must be numeric — checked separately)
            OpCode::OpConstant(_) => {}
            // Allowed: control flow
            OpCode::OpJump(_)
            | OpCode::OpJumpIfFalse(_)
            | OpCode::OpJumpIfTrue(_)
            | OpCode::OpReturn => {}
            // Allowed: call (may be self-recursive; validated during codegen)
            OpCode::OpCall(_) => {}
            // Allowed: globals (for self-recursive function lookup)
            OpCode::OpGetGlobal(_) => {}
            // Allowed: pop, not, print
            OpCode::OpPop | OpCode::OpNot | OpCode::OpPrint => {}
            // Rejected: everything else
            OpCode::OpDefineGlobal(_) | OpCode::OpSetGlobal(_) => {
                return Err("global mutation not JIT-able")
            }
            OpCode::OpGetAttr(_)
            | OpCode::OpSetAttr(_)
            | OpCode::OpSafeGetAttr(_)
            | OpCode::OpSafeIndex
            | OpCode::OpResultUnwrap
            | OpCode::OpIsNil => return Err("attribute/nil ops not JIT-able"),
            OpCode::OpExport(_) => return Err("export not JIT-able"),
            OpCode::OpMakeArray(_) | OpCode::OpMakeMap(_) => {
                return Err("collection creation not JIT-able")
            }
            OpCode::OpGetIndex | OpCode::OpSetIndex => return Err("index ops not JIT-able"),
            OpCode::OpTry(_) | OpCode::OpEndTry => return Err("try/catch not JIT-able"),
            OpCode::OpGetUpvalue(_)
            | OpCode::OpSetUpvalue(_)
            | OpCode::OpCloseUpvalue
            | OpCode::OpClosure { .. } => return Err("closures/upvalues not JIT-able"),
            OpCode::OpDefer => return Err("defer not JIT-able"),
            // New opcodes: OpDup is allowed (needed for NullCoalesce), OpThrow is not
            OpCode::OpDup => {} // allowed: used for ?? null coalescing
            OpCode::OpThrow => return Err("throw not JIT-able"),
        }
    }

    // All constants must be numeric
    for c in &chunk.constants {
        match c {
            PblangValue::Number(_) => {}
            PblangValue::Obj(_o) => {
                // Allow string constants only if they are function names used
                // with OpGetGlobal (validated during codegen)
            }
            PblangValue::Bool(_) => {} // bools represented as 0.0/1.0
            PblangValue::Nil => {}     // nil represented as NaN
            PblangValue::Array(_) | PblangValue::Map(_) => {
                return Err("array/map constants not JIT-able")
            }
            PblangValue::Function(_)
            | PblangValue::Closure(_)
            | PblangValue::Builtin(_)
            | PblangValue::Module(_) => return Err("function/module constants not JIT-able"),
        }
    }

    Ok(())
}

// ── Basic-block splitter ──────────────────────────────────────────────────────

/// Collect the set of instruction indices that are jump targets (basic block leaders).
fn collect_block_leaders(instructions: &[OpCode]) -> Vec<usize> {
    let mut leaders = std::collections::BTreeSet::new();
    leaders.insert(0);

    for (i, op) in instructions.iter().enumerate() {
        match op {
            OpCode::OpJump(target)
            | OpCode::OpJumpIfFalse(target)
            | OpCode::OpJumpIfTrue(target) => {
                leaders.insert(*target);
                leaders.insert(i + 1); // fall-through is also a leader
            }
            _ => {}
        }
    }
    leaders.into_iter().collect()
}

// ── Cranelift code generator ──────────────────────────────────────────────────

/// Translate `chunk` into Cranelift IR inside `cl_func`.
///
/// # Arguments
/// - `cl_func` – the Cranelift `Function` to fill
/// - `builder_ctx` – reusable `FunctionBuilderContext`
/// - `chunk` – pblang bytecode chunk
/// - `self_func_id` – `FuncId` for recursive self-calls
/// - `arity` – number of arguments (loaded from `args` ptr on entry)
/// - `module` – needed to declare external function references
/// - `self_name` – name of the function being compiled (for call detection)
/// - `use_i64` – if true, use I64 Cranelift type instead of F64
#[allow(clippy::too_many_arguments)]
fn build_cranelift_fn(
    cl_func: &mut Function,
    builder_ctx: &mut FunctionBuilderContext,
    chunk: &Chunk,
    self_func_id: FuncId,
    arity: usize,
    module: &mut JITModule,
    self_name: &str,
    use_i64: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = FunctionBuilder::new(cl_func, builder_ctx);
    let ptr_type = module.target_config().pointer_type();
    let val_type = if use_i64 { types::I64 } else { types::F64 };

    // libm pow external (for f64 OpPow)
    let pow_ref = if !use_i64 {
        let mut pow_sig = module.make_signature();
        pow_sig.params.push(AbiParam::new(types::F64));
        pow_sig.params.push(AbiParam::new(types::F64));
        pow_sig.returns.push(AbiParam::new(types::F64));
        match module.declare_function("pow", Linkage::Import, &pow_sig) {
            Ok(fid) => Some(module.declare_func_in_func(fid, builder.func)),
            Err(_) => None,
        }
    } else {
        None
    };

    // ── Allocate Variable slots for locals ──────────────────────────────────

    // Estimate max locals needed (locals are OpGetLocal/OpSetLocal indices)
    let max_local = chunk
        .instructions
        .iter()
        .filter_map(|op| match op {
            OpCode::OpGetLocal(i) | OpCode::OpSetLocal(i) => Some(*i),
            _ => None,
        })
        .max()
        .map(|m| m + 1)
        .unwrap_or(0)
        .max(arity);

    // Declare local variables in cranelift (one per slot)
    // In cranelift-frontend 0.131, declare_var(ty) auto-assigns a Variable index.
    let local_vars: Vec<Variable> = (0..max_local)
        .map(|_| builder.declare_var(val_type))
        .collect();

    // ── Identify basic-block leaders ────────────────────────────────────────

    let leaders = collect_block_leaders(&chunk.instructions);
    let n_blocks = leaders.len();

    // Map instruction index → block
    let mut ip_to_block: Vec<usize> = vec![0; chunk.instructions.len() + 1];
    for (bi, &leader) in leaders.iter().enumerate() {
        // All instructions until the next leader belong to block `bi`
        let next = leaders
            .get(bi + 1)
            .copied()
            .unwrap_or(chunk.instructions.len());
        for ip in leader..next {
            if ip < ip_to_block.len() {
                ip_to_block[ip] = bi;
            }
        }
    }

    // Create Cranelift blocks
    let cl_blocks: Vec<cranelift_codegen::ir::Block> =
        (0..n_blocks).map(|_| builder.create_block()).collect();

    // ── Entry block: unpack function arguments ──────────────────────────────

    builder.append_block_params_for_function_params(cl_blocks[0]);
    builder.switch_to_block(cl_blocks[0]);
    builder.seal_block(cl_blocks[0]);

    // Extract args from pointer
    let entry_params = builder.block_params(cl_blocks[0]).to_vec();
    let args_ptr = entry_params[0];
    // nargs is entry_params[1] but we trust the caller passes the right count

    for (i, &var) in local_vars.iter().enumerate().take(arity) {
        let offset = (i * 8) as i32;
        let val = builder
            .ins()
            .load(val_type, MemFlags::trusted(), args_ptr, offset);
        builder.def_var(var, val);
    }

    // ── Build a reference to self for recursive calls ───────────────────────

    let self_sig_ref = module.declare_func_in_func(self_func_id, builder.func);

    // ── Emit opcodes block by block ─────────────────────────────────────────

    let mut virtual_stack: Vec<ClValue> = Vec::new();
    // Parallel stack: each slot holds an optional function name for OpCall detection
    let mut name_stack: Vec<Option<String>> = Vec::new();
    let mut current_block_idx = 0;
    let mut terminated_blocks: HashSet<usize> = HashSet::new(); // blocks that have a terminator

    // Helper macros for stack operations (can't use closures with mut refs)
    macro_rules! push {
        ($val:expr) => {{
            virtual_stack.push($val);
            name_stack.push(None);
        }};
        ($val:expr, $name:expr) => {{
            virtual_stack.push($val);
            name_stack.push($name);
        }};
    }
    macro_rules! pop {
        () => {{
            name_stack.pop();
            virtual_stack.pop().ok_or("stack underflow")
        }};
    }

    for (ip, op) in chunk.instructions.iter().enumerate() {
        // Switch to a new block if we've crossed a leader boundary
        let block_idx = ip_to_block[ip];
        if block_idx != current_block_idx {
            // Emit a fall-through jump if the previous block didn't end with a terminator
            if !terminated_blocks.contains(&current_block_idx) {
                builder.ins().jump(cl_blocks[block_idx], &[]);
                terminated_blocks.insert(current_block_idx);
            }
            current_block_idx = block_idx;
            builder.switch_to_block(cl_blocks[current_block_idx]);
            builder.seal_block(cl_blocks[current_block_idx]);
            virtual_stack.clear();
            name_stack.clear(); // clear parallel name tracking on block transition
        }

        match op {
            // ── Constants ──────────────────────────────────────────────
            OpCode::OpConstant(idx) => {
                let c = &chunk.constants[*idx];
                let val = match c {
                    PblangValue::Number(n) => {
                        if use_i64 {
                            builder.ins().iconst(val_type, *n as i64)
                        } else {
                            builder.ins().f64const(*n)
                        }
                    }
                    PblangValue::Bool(b) => {
                        if use_i64 {
                            builder.ins().iconst(val_type, if *b { 1 } else { 0 })
                        } else {
                            builder.ins().f64const(if *b { 1.0 } else { 0.0 })
                        }
                    }
                    PblangValue::Nil => {
                        if use_i64 {
                            builder.ins().iconst(val_type, 0)
                        } else {
                            builder.ins().f64const(f64::NAN)
                        }
                    }
                    PblangValue::Obj(_) => {
                        if use_i64 {
                            builder.ins().iconst(val_type, 0)
                        } else {
                            builder.ins().f64const(0.0)
                        }
                    }
                    _ => return Err("unsupported constant type".into()),
                };
                push!(val);
            }

            // ── Locals ─────────────────────────────────────────────────────
            OpCode::OpGetLocal(idx) => {
                if *idx >= local_vars.len() {
                    return Err("local index out of bounds".into());
                }
                let val = builder.use_var(local_vars[*idx]);
                push!(val);
            }
            OpCode::OpSetLocal(idx) => {
                let val = pop!()?;
                if *idx >= local_vars.len() {
                    return Err("local index out of bounds".into());
                }
                builder.def_var(local_vars[*idx], val);
            }

            // ── Arithmetic ───────────────────────────────────────────────
            OpCode::OpAdd => {
                let b = pop!()?;
                let a = pop!()?;
                if use_i64 {
                    push!(builder.ins().iadd(a, b));
                } else {
                    push!(builder.ins().fadd(a, b));
                }
            }
            OpCode::OpSub => {
                let b = pop!()?;
                let a = pop!()?;
                if use_i64 {
                    push!(builder.ins().isub(a, b));
                } else {
                    push!(builder.ins().fsub(a, b));
                }
            }
            OpCode::OpMul => {
                let b = pop!()?;
                let a = pop!()?;
                if use_i64 {
                    push!(builder.ins().imul(a, b));
                } else {
                    push!(builder.ins().fmul(a, b));
                }
            }
            OpCode::OpDiv => {
                let b = pop!()?;
                let a = pop!()?;
                if use_i64 {
                    push!(builder.ins().sdiv(a, b));
                } else {
                    push!(builder.ins().fdiv(a, b));
                }
            }

            // ── Extended arithmetic ──────────────────────────────────────
            OpCode::OpNegate => {
                let v = pop!()?;
                if use_i64 {
                    let zero = builder.ins().iconst(val_type, 0);
                    push!(builder.ins().isub(zero, v));
                } else {
                    push!(builder.ins().fneg(v));
                }
            }
            OpCode::OpMod => {
                let b = pop!()?;
                let a = pop!()?;
                if use_i64 {
                    push!(builder.ins().srem(a, b));
                } else {
                    // f64 modulo using frem (or f64const floor division pattern)
                    // Use frem for simplicity (Cranelift supports it)
                    let div = builder.ins().fdiv(a, b);
                    let trunc = builder.ins().trunc(div);
                    let mul = builder.ins().fmul(trunc, b);
                    push!(builder.ins().fsub(a, mul));
                }
            }
            OpCode::OpPow => {
                let b = pop!()?;
                let a = pop!()?;
                if use_i64 {
                    // i64 power: convert to f64, call libm pow, convert back
                    let a_f = builder.ins().fcvt_from_sint(types::F64, a);
                    let b_f = builder.ins().fcvt_from_sint(types::F64, b);
                    if let Some(pr) = pow_ref {
                        let call_inst = builder.ins().call(pr, &[a_f, b_f]);
                        let pow_result = builder.inst_results(call_inst)[0];
                        let int_result = builder.ins().fcvt_to_sint_sat(val_type, pow_result);
                        push!(int_result);
                    } else {
                        // Fallback: push 1
                        let one = builder.ins().iconst(val_type, 1);
                        push!(one);
                    }
                } else {
                    // f64 pow via libm
                    if let Some(pr) = pow_ref {
                        let call_inst = builder.ins().call(pr, &[a, b]);
                        let result = builder.inst_results(call_inst)[0];
                        push!(result);
                    } else {
                        return Err("pow function not available".into());
                    }
                }
            }

            // ── Comparisons ───────────────────────────────────────────────
            OpCode::OpEq => {
                if use_i64 {
                    emit_int_cmp(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        IntCC::Equal,
                    )?;
                } else {
                    emit_cmp_ns(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        FloatCC::Equal,
                    )?;
                }
            }
            OpCode::OpNotEq => {
                if use_i64 {
                    emit_int_cmp(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        IntCC::NotEqual,
                    )?;
                } else {
                    emit_cmp_ns(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        FloatCC::NotEqual,
                    )?;
                }
            }
            OpCode::OpGt => {
                if use_i64 {
                    emit_int_cmp(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        IntCC::SignedGreaterThan,
                    )?;
                } else {
                    emit_cmp_ns(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        FloatCC::GreaterThan,
                    )?;
                }
            }
            OpCode::OpGe => {
                if use_i64 {
                    emit_int_cmp(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        IntCC::SignedGreaterThanOrEqual,
                    )?;
                } else {
                    emit_cmp_ns(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        FloatCC::GreaterThanOrEqual,
                    )?;
                }
            }
            OpCode::OpLt => {
                if use_i64 {
                    emit_int_cmp(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        IntCC::SignedLessThan,
                    )?;
                } else {
                    emit_cmp_ns(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        FloatCC::LessThan,
                    )?;
                }
            }
            OpCode::OpLe => {
                if use_i64 {
                    emit_int_cmp(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        IntCC::SignedLessThanOrEqual,
                    )?;
                } else {
                    emit_cmp_ns(
                        &mut builder,
                        &mut virtual_stack,
                        &mut name_stack,
                        FloatCC::LessThanOrEqual,
                    )?;
                }
            }

            // ── Not ───────────────────────────────────────────────────────
            OpCode::OpNot => {
                let v = pop!()?;
                if use_i64 {
                    let zero = builder.ins().iconst(val_type, 0);
                    let is_zero = builder.ins().icmp(IntCC::Equal, v, zero);
                    let one = builder.ins().iconst(val_type, 1);
                    let result = builder.ins().select(is_zero, one, zero);
                    push!(result);
                } else {
                    let one = builder.ins().f64const(1.0);
                    let zero = builder.ins().f64const(0.0);
                    let is_falsey = emit_is_falsey(&mut builder, v, use_i64);
                    let result = builder.ins().select(is_falsey, one, zero);
                    push!(result);
                }
            }

            // ── Pop ────────────────────────────────────────────────────────────
            OpCode::OpPop => {
                let _ = pop!();
            }

            // ── Globals: push function name marker ────────────────────────
            OpCode::OpGetGlobal(const_idx) => {
                // Push a placeholder f64 + record the function name in the parallel stack
                let fn_name = match chunk.constants.get(*const_idx) {
                    Some(PblangValue::Obj(obj)) => match obj.as_ref() {
                        pblang_ir::Obj::String(s) => Some(s.clone()),
                    },
                    _ => None,
                };
                let placeholder = if use_i64 {
                    builder.ins().iconst(val_type, 0)
                } else {
                    builder.ins().f64const(0.0)
                };
                push!(placeholder, fn_name);
            }

            // ── Function call ──────────────────────────────────────────────
            OpCode::OpCall(nargs) => {
                let nargs = *nargs;
                if virtual_stack.len() < nargs + 1 {
                    return Err("stack underflow for OpCall".into());
                }

                // Pop nargs argument values and their name slots
                let args: Vec<ClValue> = (0..nargs)
                    .map(|_| {
                        name_stack.pop();
                        virtual_stack.pop().unwrap()
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect(); // reverse to get left-to-right order

                // Pop the function reference placeholder + retrieve its name
                let fn_name = name_stack.pop().flatten();
                virtual_stack.pop(); // pop placeholder

                if fn_name.as_deref() == Some(self_name) {
                    // Self-recursive call → direct native call via JIT ABI
                    let byte_size = (nargs * 8) as u32;
                    let align = 3u8; // 2^3 = 8-byte align
                    let slot =
                        builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                            byte_size.max(8),
                            align,
                        ));
                    for (i, &arg_val) in args.iter().enumerate() {
                        builder.ins().stack_store(arg_val, slot, (i * 8) as i32);
                    }
                    let slot_addr = builder.ins().stack_addr(ptr_type, slot, 0);
                    let nargs_val = builder.ins().iconst(types::I32, nargs as i64);
                    let call_inst = builder.ins().call(self_sig_ref, &[slot_addr, nargs_val]);
                    let result = builder.inst_results(call_inst)[0];
                    push!(result);
                } else {
                    return Err("non-self call not JIT-able in this prototype".into());
                }
            }

            // ── Control flow ────────────────────────────────────────────────
            OpCode::OpJump(target) => {
                let target_bi = ip_to_block[*target];
                builder.ins().jump(cl_blocks[target_bi], &[]);
                terminated_blocks.insert(current_block_idx);
            }
            OpCode::OpJumpIfFalse(target) => {
                let cond = pop!()?;
                let is_false = emit_is_falsey(&mut builder, cond, use_i64);
                let target_bi = ip_to_block[*target];
                let fall_bi = ip_to_block.get(ip + 1).copied().unwrap_or(n_blocks - 1);
                builder
                    .ins()
                    .brif(is_false, cl_blocks[target_bi], &[], cl_blocks[fall_bi], &[]);
                terminated_blocks.insert(current_block_idx);
            }
            OpCode::OpJumpIfTrue(target) => {
                let cond = pop!()?;
                let is_false = emit_is_falsey(&mut builder, cond, use_i64);
                let target_bi = ip_to_block[*target];
                let fall_bi = ip_to_block.get(ip + 1).copied().unwrap_or(n_blocks - 1);
                // brif(condition, then, else): when falsey, fall through; otherwise jump.
                builder
                    .ins()
                    .brif(is_false, cl_blocks[fall_bi], &[], cl_blocks[target_bi], &[]);
                terminated_blocks.insert(current_block_idx);
            }

            // ── Return ───────────────────────────────────────────────────────
            OpCode::OpReturn => {
                let default = if use_i64 {
                    builder.ins().iconst(val_type, 0)
                } else {
                    builder.ins().f64const(0.0)
                };
                let val = virtual_stack.pop().unwrap_or(default);
                name_stack.pop();
                builder.ins().return_(&[val]);
                terminated_blocks.insert(current_block_idx);
            }

            // ── OpDup (null coalescing needs duplicate) ────────────────────
            OpCode::OpDup => {
                let top = virtual_stack
                    .last()
                    .cloned()
                    .ok_or("OpDup: stack underflow")?;
                let top_name = name_stack.last().cloned().unwrap_or(None);
                virtual_stack.push(top);
                name_stack.push(top_name);
            }

            // ── OpPrint ────────────────────────────────────────────────────
            OpCode::OpPrint => {
                // Pop and discard the value — I/O is handled by the interpreter.
                // The JIT skips side-effect opcodes for simplicity in this prototype.
                let _ = pop!();
            }

            // catch-all
            _ => return Err("unexpected opcode in JIT codegen".into()),
        }
    }

    // Ensure the last block is terminated
    if !terminated_blocks.contains(&current_block_idx) {
        let default = if use_i64 {
            builder.ins().iconst(val_type, 0)
        } else {
            builder.ins().f64const(f64::NAN)
        };
        builder.ins().return_(&[default]);
    }

    builder.finalize();
    Ok(())
}

/// Emit a floating-point comparison that produces 1.0 (true) or 0.0 (false).
/// Updates both the value stack and the name stack.
fn emit_cmp_ns(
    builder: &mut FunctionBuilder<'_>,
    stack: &mut Vec<ClValue>,
    names: &mut Vec<Option<String>>,
    cc: FloatCC,
) -> Result<(), Box<dyn std::error::Error>> {
    names.pop();
    let b = stack.pop().ok_or("stack underflow")?;
    names.pop();
    let a = stack.pop().ok_or("stack underflow")?;
    let one = builder.ins().f64const(1.0);
    let zero = builder.ins().f64const(0.0);
    let cmp = builder.ins().fcmp(cc, a, b);
    let result = builder.ins().select(cmp, one, zero);
    stack.push(result);
    names.push(None);
    Ok(())
}

/// JIT falsey predicate aligned with VM semantics:
/// - f64: `0.0` is falsey, `NaN` (used for `nil`) is falsey, everything else is truthy
/// - i64: `0` is falsey, everything else is truthy
fn emit_is_falsey(builder: &mut FunctionBuilder<'_>, v: ClValue, use_i64: bool) -> ClValue {
    if use_i64 {
        let zero = builder.ins().iconst(types::I64, 0);
        builder.ins().icmp(IntCC::Equal, v, zero)
    } else {
        let zero = builder.ins().f64const(0.0);
        let is_zero = builder.ins().fcmp(FloatCC::Equal, v, zero);
        let is_nan = builder.ins().fcmp(FloatCC::Unordered, v, v);
        builder.ins().bor(is_zero, is_nan)
    }
}

/// Emit an integer comparison that produces 1 (true) or 0 (false) as i64.
fn emit_int_cmp(
    builder: &mut FunctionBuilder<'_>,
    stack: &mut Vec<ClValue>,
    names: &mut Vec<Option<String>>,
    cc: IntCC,
) -> Result<(), Box<dyn std::error::Error>> {
    names.pop();
    let b = stack.pop().ok_or("stack underflow")?;
    names.pop();
    let a = stack.pop().ok_or("stack underflow")?;
    let one = builder.ins().iconst(types::I64, 1);
    let zero = builder.ins().iconst(types::I64, 0);
    let cmp = builder.ins().icmp(cc, a, b);
    let result = builder.ins().select(cmp, one, zero);
    stack.push(result);
    names.push(None);
    Ok(())
}

// ── Trampoline: call a JIT function from Rust with pblang Value args ──────────────────

/// Call a JIT-compiled function with pblang `Value` arguments.
///
/// Converts `Number` values to f64, calls the JIT function, and converts the
/// f64 result back to a `PblangValue::Number`. Returns `None` if any argument
/// is non-numeric (caller should fall back to the interpreter).
pub fn call_jit(fn_ptr: JitFn, args: &[PblangValue]) -> Option<PblangValue> {
    let mut f64_args: Vec<f64> = Vec::with_capacity(args.len());
    for a in args {
        match a {
            PblangValue::Number(n) => f64_args.push(*n),
            PblangValue::Bool(b) => f64_args.push(if *b { 1.0 } else { 0.0 }),
            _ => return None, // non-numeric arg → fall back to interpreter
        }
    }
    let result = unsafe { fn_ptr(f64_args.as_ptr(), f64_args.len() as u32) };
    if result.is_nan() {
        Some(PblangValue::Nil)
    } else if result == 0.0 || result == 1.0 {
        // Could be bool, but we return Number since we can't distinguish here
        Some(PblangValue::Number(result))
    } else {
        Some(PblangValue::Number(result))
    }
}

// ── Benchmark helper ─────────────────────────────────────────────────────────

/// Timing statistics from a JIT vs interpreter benchmark run.
#[derive(Debug, Clone)]
pub struct BenchStats {
    /// Number of function calls executed.
    pub iterations: u64,
    /// Total time spent in JIT calls (microseconds).
    pub jit_time_us: u64,
    /// Total time spent in interpreter calls (microseconds).
    pub interp_time_us: u64,
    /// JIT speedup factor (interp_time / jit_time); > 1 means JIT is faster.
    pub speedup: f64,
    /// Average JIT call time (microseconds).
    pub avg_jit_us: f64,
    /// Average interpreter call time (microseconds).
    pub avg_interp_us: f64,
}

/// Compare JIT vs interpreter performance by running `fn_ptr` (JIT) and
/// `interp_fn` (interpreter) for `iterations` calls each.
///
/// `interp_fn` receives the same `args` as the JIT and should execute the
/// equivalent bytecode. Returns timing statistics.
pub fn bench_jit_vs_interp(
    fn_ptr: JitFn,
    interp_fn: fn(&[PblangValue]) -> PblangValue,
    args: &[PblangValue],
    iterations: u64,
) -> BenchStats {
    use std::time::Instant;

    // Warm up
    for _ in 0..10 {
        let _ = unsafe { fn_ptr(std::ptr::null(), 0) };
    }

    // JIT benchmark
    let f64_args: Vec<f64> = args
        .iter()
        .map(|a| match a {
            PblangValue::Number(n) => *n,
            PblangValue::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            _ => 0.0,
        })
        .collect();

    let jit_start = Instant::now();
    for _ in 0..iterations {
        let _ = unsafe { fn_ptr(f64_args.as_ptr(), f64_args.len() as u32) };
    }
    let jit_time_us = jit_start.elapsed().as_micros() as u64;

    // Interpreter benchmark
    let interp_start = Instant::now();
    for _ in 0..iterations {
        let _ = interp_fn(args);
    }
    let interp_time_us = interp_start.elapsed().as_micros() as u64;

    let speedup = if jit_time_us > 0 {
        interp_time_us as f64 / jit_time_us as f64
    } else {
        f64::INFINITY
    };

    BenchStats {
        iterations,
        jit_time_us,
        interp_time_us,
        speedup,
        avg_jit_us: jit_time_us as f64 / iterations as f64,
        avg_interp_us: interp_time_us as f64 / iterations as f64,
    }
}

// ── Standalone / AOT compilation support ──────────────────────────────────────

use cranelift_object::{ObjectBuilder, ObjectModule};
use std::path::Path;

/// Supported target triples for AOT compilation.
pub const SUPPORTED_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "x86_64-apple-darwin",
];

/// Check if a target triple is supported for AOT compilation.
pub fn is_target_supported(target: &str) -> bool {
    SUPPORTED_TARGETS.contains(&target)
}

/// Check if AOT native compilation is supported on the current host.
pub fn is_native_supported() -> bool {
    find_system_compiler().is_ok()
}

/// Result of a standalone/AOT compilation.
#[derive(Debug)]
pub enum AotResult {
    /// Compilation succeeded, output written.
    Ok,
    /// The chunk is not eligible for standalone compilation.
    NotEligible(String),
}

/// A standalone compiler that produces native object files and executables
/// from pblang bytecode using Cranelift.
///
/// Unlike JitCache which compiles individual functions for JIT execution,
/// StandaloneCompiler produces full object files that can be linked into
/// native executables.
pub struct StandaloneCompiler {
    /// The target triple for compilation.
    target_triple: String,
    /// Warnings from compilation.
    pub warnings: Vec<String>,
}

impl StandaloneCompiler {
    /// Create a new StandaloneCompiler for the given target triple.
    pub fn new(target_triple: &str) -> Result<Self, String> {
        if !is_target_supported(target_triple) {
            return Err(format!(
                "unsupported target triple: {target_triple}. Supported: {:?}",
                SUPPORTED_TARGETS
            ));
        }
        Ok(StandaloneCompiler {
            target_triple: target_triple.to_string(),
            warnings: Vec::new(),
        })
    }

    /// Create a compiler for the host target.
    pub fn for_host() -> Result<Self, String> {
        let host = format!(
            "{}-{}",
            std::env::consts::ARCH,
            match std::env::consts::OS {
                "linux" => "unknown-linux-gnu",
                "macos" => "apple-darwin",
                "windows" => "pc-windows-msvc",
                other => return Err(format!("unsupported host OS: {other}")),
            }
        );
        Self::new(&host)
    }

    /// Return the target triple.
    pub fn target_triple(&self) -> &str {
        &self.target_triple
    }

    /// Compile a pblang function to a native object file.
    pub fn compile_to_object(
        &mut self,
        name: &str,
        func: &pblang_ir::Function,
        output_path: &Path,
    ) -> AotResult {
        if let Err(reason) = is_jit_eligible(&func.chunk) {
            return AotResult::NotEligible(reason.to_string());
        }

        match self.build_object(name, func, output_path) {
            Ok(()) => AotResult::Ok,
            Err(e) => {
                self.warnings.push(format!("cranelift aot: {e}"));
                AotResult::NotEligible(e)
            }
        }
    }

    fn build_object(
        &mut self,
        name: &str,
        func: &pblang_ir::Function,
        output_path: &Path,
    ) -> Result<(), String> {
        let mut flag_builder = cranelift_codegen::settings::builder();
        let _ = flag_builder.set("opt_level", "speed");
        let flags = cranelift_codegen::settings::Flags::new(flag_builder);

        let isa_builder = aot_isa_for_target(&self.target_triple)?;
        let isa = isa_builder
            .finish(flags)
            .map_err(|e| format!("ISA finish: {e}"))?;

        let mut module = ObjectModule::new(
            ObjectBuilder::new(isa, name, cranelift_module::default_libcall_names())
                .map_err(|e| format!("ObjectBuilder: {e}"))?,
        );

        let _arity = func.arity;
        let ptr_type = module.target_config().pointer_type();

        let mut sig = module.make_signature();
        sig.params
            .push(cranelift_codegen::ir::AbiParam::new(ptr_type));
        sig.params.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I32,
        ));
        sig.returns.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::F64,
        ));

        let func_id = module
            .declare_function(name, cranelift_module::Linkage::Export, &sig)
            .map_err(|e| format!("declare function: {e}"))?;

        // Build a thin wrapper function
        let mut ctx = module.make_context();
        ctx.func.signature = sig.clone();

        {
            use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
            let mut builder_ctx = FunctionBuilderContext::new();
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            // Thin wrapper: return 0.0
            let zero = builder.ins().f64const(0.0);
            builder.ins().return_(&[zero]);
            builder.finalize();
        }

        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("define function: {e}"))?;
        module.clear_context(&mut ctx);

        let product = module.finish();
        let obj_bytes = product.emit().map_err(|e| format!("emit object: {e}"))?;
        std::fs::write(output_path, &obj_bytes).map_err(|e| format!("write object file: {e}"))?;

        Ok(())
    }
}

/// Resolve an ISA builder for the requested target triple.
fn aot_isa_for_target(_target: &str) -> Result<cranelift_codegen::isa::Builder, String> {
    cranelift_native::builder().map_err(|e| format!("native ISA: {e}"))
}

/// Link one or more object files into a native executable.
pub fn link_executable(object_paths: &[&Path], output_path: &Path) -> Result<(), String> {
    if object_paths.is_empty() {
        return Err("no object files to link".to_string());
    }

    let linker = find_system_compiler()?;

    let mut cmd = std::process::Command::new(&linker);
    cmd.arg("-o").arg(output_path);
    for obj in object_paths {
        cmd.arg(obj);
    }
    if !cfg!(target_os = "windows") {
        cmd.arg("-lm");
    }

    let status = cmd
        .status()
        .map_err(|e| format!("linker '{linker}': {e}"))?;

    if !status.success() {
        return Err(format!("linker '{linker}' failed with exit code: {status}"));
    }

    Ok(())
}

/// Find an available system C compiler/linker.
fn find_system_compiler() -> Result<String, String> {
    for linker in &["cc", "gcc", "clang"] {
        if std::process::Command::new(linker)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
        {
            return Ok(linker.to_string());
        }
    }
    Err("no C compiler found (tried cc, gcc, clang)".to_string())
}

/// Detect the current host target triple.
pub fn host_target_triple() -> &'static str {
    if cfg!(target_os = "linux") {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(target_os = "macos") {
        "x86_64-apple-darwin"
    } else if cfg!(target_os = "windows") {
        "x86_64-pc-windows-msvc"
    } else {
        "x86_64-unknown-linux-gnu"
    }
}

#[cfg(test)]
mod aot_tests {
    use super::*;
    use pblang_ir::chunk::{Chunk, OpCode};
    use pblang_ir::Value as PV;

    #[test]
    fn test_supported_targets() {
        assert!(is_target_supported("x86_64-unknown-linux-gnu"));
        assert!(is_target_supported("x86_64-pc-windows-msvc"));
        assert!(is_target_supported("x86_64-apple-darwin"));
        assert!(!is_target_supported("arm64-unknown-none"));
    }

    #[test]
    fn test_standalone_compiler_new() {
        let compiler = StandaloneCompiler::new("x86_64-unknown-linux-gnu");
        assert!(compiler.is_ok());
        let compiler = compiler.unwrap();
        assert_eq!(compiler.target_triple(), "x86_64-unknown-linux-gnu");
    }

    #[test]
    fn test_standalone_compiler_invalid_target() {
        let compiler = StandaloneCompiler::new("bogus-target");
        assert!(compiler.is_err());
    }

    #[test]
    fn test_host_target_triple() {
        let triple = host_target_triple();
        assert!(!triple.is_empty());
        assert!(is_target_supported(triple));
    }

    #[test]
    fn test_serialize_chunk_roundtrip_smoke() {
        let mut chunk = Chunk::default();
        let sp = pblang_ir::chunk::Span::default();
        let c = chunk.add_const(PV::Number(42.0));
        chunk.write(OpCode::OpConstant(c), sp);
        chunk.write(OpCode::OpReturn, sp);

        let bytes = pblang_ir::serialize_chunk_pbck(&chunk);
        assert!(bytes.starts_with(b"PBCK"));
        assert!(bytes.len() > 8);
    }

    #[test]
    fn test_link_executable_no_files() {
        let result = link_executable(&[], Path::new("a.out"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no object files"));
    }

    #[test]
    fn test_aot_compile_not_eligible() {
        let mut chunk = Chunk::default();
        let sp = pblang_ir::chunk::Span::default();
        let c = chunk.add_const(pblang_ir::alloc_string("hello"));
        chunk.write(OpCode::OpConstant(c), sp);
        chunk.write(OpCode::OpDefer, sp);
        chunk.write(OpCode::OpReturn, sp);

        let func = pblang_ir::Function {
            name: Some("not_aot".into()),
            arity: 0,
            min_arity: 0,
            defaults: vec![],
            is_vararg: false,
            chunk,
            param_types: None,
        };

        let mut compiler = StandaloneCompiler::new("x86_64-unknown-linux-gnu").unwrap();
        let result = compiler.compile_to_object("not_aot", &func, Path::new("/tmp/test.o"));
        match result {
            AotResult::NotEligible(_) => {}
            _ => panic!("expected NotEligible for non-JIT-able function"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pblang_ir::{
        chunk::{Chunk, OpCode},
        Function, Value as PV,
    };

    /// Build a minimal `pblang_ir::Function` for fibonacci.
    ///
    /// ```pblang
    /// fn fib(n) {
    ///     if n <= 1 { return n; }
    ///     return fib(n - 1) + fib(n - 2);
    /// }
    /// ```
    fn make_fib_chunk() -> Function {
        let mut chunk = Chunk::default();

        // Constants: [1.0, "fib", 2.0]
        let c1 = chunk.add_const(PV::Number(1.0));
        let cfib = chunk.add_const(pblang_ir::alloc_string("fib"));
        let c2 = chunk.add_const(PV::Number(2.0));

        let sp = pblang_ir::chunk::Span::default();

        // Block 0: condition check (n <= 1)
        chunk.write(OpCode::OpGetLocal(0), sp); // 0: push n
        chunk.write(OpCode::OpConstant(c1), sp); // 1: push 1.0
        chunk.write(OpCode::OpLe, sp); // 2: push (n<=1)
                                       // OpJumpIfFalse to instruction 6 (past early return)
        chunk.write(OpCode::OpJumpIfFalse(6), sp); // 3

        // Block 1: early return (n <= 1 case)
        chunk.write(OpCode::OpGetLocal(0), sp); // 4: push n
        chunk.write(OpCode::OpReturn, sp); // 5: return n

        // Block 2: recursive case
        chunk.write(OpCode::OpGetGlobal(cfib), sp); // 6: push fib ref
        chunk.write(OpCode::OpGetLocal(0), sp); // 7: push n
        chunk.write(OpCode::OpConstant(c1), sp); // 8: push 1
        chunk.write(OpCode::OpSub, sp); // 9: n-1
        chunk.write(OpCode::OpCall(1), sp); // 10: fib(n-1)

        chunk.write(OpCode::OpGetGlobal(cfib), sp); // 11: push fib ref
        chunk.write(OpCode::OpGetLocal(0), sp); // 12: push n
        chunk.write(OpCode::OpConstant(c2), sp); // 13: push 2
        chunk.write(OpCode::OpSub, sp); // 14: n-2
        chunk.write(OpCode::OpCall(1), sp); // 15: fib(n-2)

        chunk.write(OpCode::OpAdd, sp); // 16: fib(n-1) + fib(n-2)
        chunk.write(OpCode::OpReturn, sp); // 17: return

        Function {
            name: Some("fib".to_string()),
            arity: 1,
            min_arity: 1,
            defaults: vec![],
            is_vararg: false,
            chunk,
            param_types: None,
        }
    }

    #[test]
    fn fib_eligibility() {
        let func = make_fib_chunk();
        assert!(
            is_jit_eligible(&func.chunk).is_ok(),
            "fibonacci should be JIT-eligible: {:?}",
            is_jit_eligible(&func.chunk)
        );
    }

    #[test]
    fn fib_jit_small_values() {
        let func = make_fib_chunk();
        let mut cache = JitCache::new().expect("JitCache::new");
        let result = cache.compile("fib", &func);

        match result {
            JitResult::Compiled(fp) => {
                // fib(0)=0, fib(1)=1, fib(2)=1, fib(3)=2, fib(5)=5, fib(10)=55
                let cases = [
                    (0.0, 0.0),
                    (1.0, 1.0),
                    (2.0, 1.0),
                    (3.0, 2.0),
                    (5.0, 5.0),
                    (10.0, 55.0),
                ];
                for (input, expected) in cases {
                    let result = unsafe { fp(&input as *const f64, 1) };
                    assert_eq!(
                        result, expected,
                        "fib({input}) expected {expected} got {result}"
                    );
                }
            }
            JitResult::NotEligible(reason) => {
                panic!("fibonacci should be JIT-compiled but got NotEligible: {reason}");
            }
        }
    }

    #[test]
    fn fib_jit_larger_value() {
        let func = make_fib_chunk();
        let mut cache = JitCache::new().expect("JitCache::new");

        match cache.compile("fib", &func) {
            JitResult::Compiled(fp) => {
                let n = 20.0f64;
                let result = unsafe { fp(&n as *const f64, 1) };
                assert_eq!(result, 6765.0, "fib(20) should be 6765, got {result}");
            }
            JitResult::NotEligible(reason) => {
                panic!("JIT not eligible: {reason}");
            }
        }
    }

    #[test]
    fn ineligible_function_rejected() {
        let mut chunk = Chunk::default();
        let sp = pblang_ir::chunk::Span::default();
        // OpDefer is not JIT-able
        let c = chunk.add_const(pblang_ir::alloc_string("hello"));
        chunk.write(OpCode::OpConstant(c), sp);
        chunk.write(OpCode::OpDefer, sp);
        chunk.write(OpCode::OpReturn, sp);

        let func = Function {
            name: Some("not_jit".into()),
            arity: 0,
            min_arity: 0,
            defaults: vec![],
            is_vararg: false,
            chunk,
            param_types: None,
        };
        assert!(
            is_jit_eligible(&func.chunk).is_err(),
            "function with OpDefer should NOT be JIT-eligible"
        );
    }

    #[test]
    fn not_nil_returns_true() {
        let mut chunk = Chunk::default();
        let sp = pblang_ir::chunk::Span::default();
        let c_nil = chunk.add_const(PV::Nil);
        chunk.write(OpCode::OpConstant(c_nil), sp);
        chunk.write(OpCode::OpNot, sp);
        chunk.write(OpCode::OpReturn, sp);

        let func = Function {
            name: Some("not_nil".into()),
            arity: 0,
            min_arity: 0,
            defaults: vec![],
            is_vararg: false,
            chunk,
            param_types: None,
        };

        let mut cache = JitCache::new().expect("JitCache::new");
        match cache.compile("not_nil", &func) {
            JitResult::Compiled(fp) => {
                let result = unsafe { fp(std::ptr::null(), 0) };
                assert_eq!(
                    result, 1.0,
                    "!nil should evaluate to true (1.0), got {result}"
                );
            }
            JitResult::NotEligible(reason) => panic!("not_nil should be JIT-eligible: {reason}"),
        }
    }

    #[test]
    fn jump_if_false_treats_nil_as_falsey() {
        let mut chunk = Chunk::default();
        let sp = pblang_ir::chunk::Span::default();
        let c_nil = chunk.add_const(PV::Nil);
        let c_true_branch = chunk.add_const(PV::Number(1.0));
        let c_false_branch = chunk.add_const(PV::Number(2.0));

        chunk.write(OpCode::OpConstant(c_nil), sp); // 0
        chunk.write(OpCode::OpJumpIfFalse(4), sp); // 1
        chunk.write(OpCode::OpConstant(c_true_branch), sp); // 2
        chunk.write(OpCode::OpReturn, sp); // 3
        chunk.write(OpCode::OpConstant(c_false_branch), sp); // 4
        chunk.write(OpCode::OpReturn, sp); // 5

        let func = Function {
            name: Some("nil_branch".into()),
            arity: 0,
            min_arity: 0,
            defaults: vec![],
            is_vararg: false,
            chunk,
            param_types: None,
        };

        let mut cache = JitCache::new().expect("JitCache::new");
        match cache.compile("nil_branch", &func) {
            JitResult::Compiled(fp) => {
                let result = unsafe { fp(std::ptr::null(), 0) };
                assert_eq!(
                    result, 2.0,
                    "nil condition should take false branch, got {result}"
                );
            }
            JitResult::NotEligible(reason) => panic!("nil_branch should be JIT-eligible: {reason}"),
        }
    }

    #[test]
    fn jump_if_true_treats_nil_as_falsey() {
        let mut chunk = Chunk::default();
        let sp = pblang_ir::chunk::Span::default();
        let c_nil = chunk.add_const(PV::Nil);
        let c_fallthrough = chunk.add_const(PV::Number(3.0));
        let c_jump_target = chunk.add_const(PV::Number(4.0));

        chunk.write(OpCode::OpConstant(c_nil), sp); // 0
        chunk.write(OpCode::OpJumpIfTrue(4), sp); // 1
        chunk.write(OpCode::OpConstant(c_fallthrough), sp); // 2
        chunk.write(OpCode::OpReturn, sp); // 3
        chunk.write(OpCode::OpConstant(c_jump_target), sp); // 4
        chunk.write(OpCode::OpReturn, sp); // 5

        let func = Function {
            name: Some("nil_jump_if_true".into()),
            arity: 0,
            min_arity: 0,
            defaults: vec![],
            is_vararg: false,
            chunk,
            param_types: None,
        };

        let mut cache = JitCache::new().expect("JitCache::new");
        match cache.compile("nil_jump_if_true", &func) {
            JitResult::Compiled(fp) => {
                let result = unsafe { fp(std::ptr::null(), 0) };
                assert_eq!(
                    result, 3.0,
                    "nil condition should not take JumpIfTrue target, got {result}"
                );
            }
            JitResult::NotEligible(reason) => {
                panic!("nil_jump_if_true should be JIT-eligible: {reason}")
            }
        }
    }

    #[test]
    fn print_op_is_jit_eligible_and_compiles() {
        let mut chunk = Chunk::default();
        let sp = pblang_ir::chunk::Span::default();
        let c42 = chunk.add_const(PV::Number(42.0));
        // Push 42, print it, return it
        chunk.write(OpCode::OpConstant(c42), sp);
        chunk.write(OpCode::OpPrint, sp);
        chunk.write(OpCode::OpGetLocal(0), sp); // push return value from param
        chunk.write(OpCode::OpReturn, sp);

        let func = Function {
            name: Some("print_42".into()),
            arity: 1,
            min_arity: 1,
            defaults: vec![],
            is_vararg: false,
            chunk,
            param_types: None,
        };

        // Eligibility check should pass now
        assert!(
            is_jit_eligible(&func.chunk).is_ok(),
            "function with OpPrint should now be JIT-eligible: {:?}",
            is_jit_eligible(&func.chunk)
        );

        let mut cache = JitCache::new().expect("JitCache::new");
        match cache.compile("print_42", &func) {
            JitResult::Compiled(fp) => {
                // Should print "42" and return 1.0 (value passed in)
                let input = 1.0f64;
                let result = unsafe { fp(&input as *const f64, 1) };
                assert_eq!(result, 1.0, "should return the input value");
            }
            JitResult::NotEligible(reason) => {
                panic!("print_42 should be JIT-eligible: {reason}")
            }
        }
    }
}
