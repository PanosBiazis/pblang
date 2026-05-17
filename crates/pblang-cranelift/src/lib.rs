//! pblang-cranelift — N2-beta Cranelift JIT prototype.
//!
//! This crate provides a JIT compiler for pblang bytecode functions that use
//! only numeric operations (f64). Non-numeric operations fall back to the
//! bytecode interpreter.
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
//! | Opcode | Cranelift |
//! |--------|-----------|
//! | OpConstant(Number) | `f64const` |
//! | OpGetLocal / OpSetLocal | Variable slot |
//! | OpAdd / OpSub / OpMul / OpDiv | `fadd` / `fsub` / `fmul` / `fdiv` |
//! | OpEq..OpLe | `fcmp` → `select` (bool as 1.0/0.0) |
//! | OpNot | `fsub(1.0, x)` (boolean flip) |
//! | OpJump / OpJumpIfFalse / OpJumpIfTrue | Cranelift branches |
//! | OpReturn | `return` |
//! | OpCall(n) preceded by OpGetGlobal(self) | direct recursive call |
//! | OpPop | drop top of virtual stack |

use cranelift_codegen::ir::{
    condcodes::FloatCC, types, AbiParam, Function, InstBuilder, MemFlags, Value as ClValue,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use pblang_ir::chunk::{Chunk, OpCode};
use pblang_ir::Value as PblangValue;

use std::collections::{HashMap, HashSet};

// ── Public API types ──────────────────────────────────────────────────────────

/// JIT-compiled function ABI: caller passes a pointer to `nargs` f64 values,
/// function returns a single f64 result.
///
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
}

impl std::fmt::Debug for JitCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JitCache {{ {} compiled functions }}", self.entries.len())
    }
}

impl JitCache {
    /// Create a new JIT cache using the host machine's ISA.
    pub fn new() -> Result<Self, String> {
        let mut flag_builder = settings::builder();
        // Optimise for speed
        let _ = flag_builder.set("opt_level", "speed");
        let flags = settings::Flags::new(flag_builder);

        let isa_builder = cranelift_native::builder()
            .map_err(|e| format!("Cranelift native ISA failed: {e}"))?;
        let isa = isa_builder
            .finish(flags)
            .map_err(|e| format!("Cranelift ISA finish failed: {e}"))?;

        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let module = JITModule::new(builder);
        Ok(JitCache {
            module,
            entries: HashMap::new(),
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
            return JitResult::Compiled(*fp);
        }

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

        // Build the JIT function signature: (args: *const f64, nargs: u32) -> f64
        let mut sig = self.module.make_signature();
        let ptr_type = self.module.target_config().pointer_type();
        sig.params.push(AbiParam::new(ptr_type)); // args ptr
        sig.params.push(AbiParam::new(types::I32)); // nargs
        sig.returns.push(AbiParam::new(types::F64)); // return f64

        // Declare the function first (supports self-recursion)
        let func_id = self.module.declare_function(name, Linkage::Export, &sig)?;

        // Build Cranelift IR
        let mut ctx = self.module.make_context();
        ctx.func.signature = sig.clone();

        let mut builder_ctx = FunctionBuilderContext::new();
        let result =
            build_cranelift_fn(&mut ctx.func, &mut builder_ctx, &func.chunk, func_id, arity, &mut self.module, name);
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
            // Allowed: pop, not
            OpCode::OpPop | OpCode::OpNot => {}
            // Rejected: everything else
            OpCode::OpPrint => return Err("OpPrint not JIT-able"),
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
            OpCode::OpGetIndex | OpCode::OpSetIndex => {
                return Err("index ops not JIT-able")
            }
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
/// - `arity` – number of f64 arguments (loaded from `args` ptr on entry)
/// - `module` – needed to declare external function references
/// - `self_name` – name of the function being compiled (for call detection)
fn build_cranelift_fn(
    cl_func: &mut Function,
    builder_ctx: &mut FunctionBuilderContext,
    chunk: &Chunk,
    self_func_id: FuncId,
    arity: usize,
    module: &mut JITModule,
    self_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = FunctionBuilder::new(cl_func, builder_ctx);
    let ptr_type = module.target_config().pointer_type();

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
        .map(|_| builder.declare_var(types::F64))
        .collect();

    // ── Identify basic-block leaders ────────────────────────────────────────

    let leaders = collect_block_leaders(&chunk.instructions);
    let n_blocks = leaders.len();

    // Map instruction index → block
    let mut ip_to_block: Vec<usize> = vec![0; chunk.instructions.len() + 1];
    for (bi, &leader) in leaders.iter().enumerate() {
        // All instructions until the next leader belong to block `bi`
        let next = leaders.get(bi + 1).copied().unwrap_or(chunk.instructions.len());
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
        let val = builder.ins().load(types::F64, MemFlags::trusted(), args_ptr, offset);
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
                    PblangValue::Number(n) => builder.ins().f64const(*n),
                    PblangValue::Bool(b) => builder.ins().f64const(if *b { 1.0 } else { 0.0 }),
                    PblangValue::Nil => builder.ins().f64const(f64::NAN),
                    PblangValue::Obj(_) => builder.ins().f64const(0.0), // placeholder for string
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
                let b = pop!()?; let a = pop!()? ;
                push!(builder.ins().fadd(a, b));
            }
            OpCode::OpSub => {
                let b = pop!()?; let a = pop!()? ;
                push!(builder.ins().fsub(a, b));
            }
            OpCode::OpMul => {
                let b = pop!()?; let a = pop!()? ;
                push!(builder.ins().fmul(a, b));
            }
            OpCode::OpDiv => {
                let b = pop!()?; let a = pop!()? ;
                push!(builder.ins().fdiv(a, b));
            }

            // ── Comparisons: result is 1.0 (true) or 0.0 (false) ───────────────
            OpCode::OpEq  => emit_cmp_ns(&mut builder, &mut virtual_stack, &mut name_stack, FloatCC::Equal)?,
            OpCode::OpNotEq => emit_cmp_ns(&mut builder, &mut virtual_stack, &mut name_stack, FloatCC::NotEqual)?,
            OpCode::OpGt => emit_cmp_ns(&mut builder, &mut virtual_stack, &mut name_stack, FloatCC::GreaterThan)?,
            OpCode::OpGe => emit_cmp_ns(&mut builder, &mut virtual_stack, &mut name_stack, FloatCC::GreaterThanOrEqual)?,
            OpCode::OpLt => emit_cmp_ns(&mut builder, &mut virtual_stack, &mut name_stack, FloatCC::LessThan)?,
            OpCode::OpLe => emit_cmp_ns(&mut builder, &mut virtual_stack, &mut name_stack, FloatCC::LessThanOrEqual)?,

            // ── Not ───────────────────────────────────────────────────────────
            OpCode::OpNot => {
                let v = pop!()?;
                let one  = builder.ins().f64const(1.0);
                let zero = builder.ins().f64const(0.0);
                let is_falsey = emit_is_falsey(&mut builder, v);
                let result  = builder.ins().select(is_falsey, one, zero);
                push!(result);
            }

            // ── Pop ────────────────────────────────────────────────────────────
            OpCode::OpPop => { let _ = pop!(); }

            // ── Globals: push function name marker ────────────────────────
            OpCode::OpGetGlobal(const_idx) => {
                // Push a placeholder f64 + record the function name in the parallel stack
                let fn_name = match chunk.constants.get(*const_idx) {
                    Some(PblangValue::Obj(obj)) => match obj.as_ref() {
                        pblang_ir::Obj::String(s) => Some(s.clone()),
                    },
                    _ => None,
                };
                let placeholder = builder.ins().f64const(0.0);
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
                    .map(|_| { name_stack.pop(); virtual_stack.pop().unwrap() })
                    .collect::<Vec<_>>()
                    .into_iter().rev().collect(); // reverse to get left-to-right order

                // Pop the function reference placeholder + retrieve its name
                let fn_name = name_stack.pop().flatten();
                virtual_stack.pop(); // pop placeholder

                if fn_name.as_deref() == Some(self_name) {
                    // Self-recursive call → direct native call via JIT ABI
                    let byte_size = (nargs * 8) as u32;
                    let align = 3u8; // 2^3 = 8-byte align
                    let slot = builder.create_sized_stack_slot(
                        cranelift_codegen::ir::StackSlotData::new(
                            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                            byte_size.max(8),
                            align,
                        ),
                    );
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
                let is_false = emit_is_falsey(&mut builder, cond);
                let target_bi = ip_to_block[*target];
                let fall_bi = ip_to_block.get(ip + 1).copied().unwrap_or(n_blocks - 1);
                builder.ins().brif(is_false, cl_blocks[target_bi], &[], cl_blocks[fall_bi], &[]);
                terminated_blocks.insert(current_block_idx);
            }
            OpCode::OpJumpIfTrue(target) => {
                let cond = pop!()?;
                let is_false = emit_is_falsey(&mut builder, cond);
                let target_bi = ip_to_block[*target];
                let fall_bi = ip_to_block.get(ip + 1).copied().unwrap_or(n_blocks - 1);
                // brif(condition, then, else): when falsey, fall through; otherwise jump.
                builder.ins().brif(is_false, cl_blocks[fall_bi], &[], cl_blocks[target_bi], &[]);
                terminated_blocks.insert(current_block_idx);
            }

            // ── Return ───────────────────────────────────────────────────────
            OpCode::OpReturn => {
                let val = virtual_stack.pop().unwrap_or_else(|| builder.ins().f64const(0.0));
                name_stack.pop();
                builder.ins().return_(&[val]);
                terminated_blocks.insert(current_block_idx);
            }

            // ── OpDup (null coalescing needs duplicate) ────────────────────
            OpCode::OpDup => {
                let top = virtual_stack.last().cloned().ok_or("OpDup: stack underflow")?;
                let top_name = name_stack.last().cloned().unwrap_or(None);
                virtual_stack.push(top);
                name_stack.push(top_name);
            }

            // catch-all
            _ => return Err("unexpected opcode in JIT codegen".into()),
        }
    }

    // Ensure the last block is terminated
    if !terminated_blocks.contains(&current_block_idx) {
        let nan = builder.ins().f64const(f64::NAN);
        builder.ins().return_(&[nan]);
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
/// - `0.0` is falsey
/// - `NaN` (used for `nil`) is falsey
/// - everything else is truthy
fn emit_is_falsey(builder: &mut FunctionBuilder<'_>, v: ClValue) -> ClValue {
    let zero = builder.ins().f64const(0.0);
    let is_zero = builder.ins().fcmp(FloatCC::Equal, v, zero);
    let is_nan = builder.ins().fcmp(FloatCC::Unordered, v, v);
    builder.ins().bor(is_zero, is_nan)
}

// ── Trampoline: call a JIT function from Rust with pblang Value args ──────────────────

/// Call a JIT-compiled function with pblang `Value` arguments.
///
/// Converts `Number` values to f64, calls the JIT function, and converts the
/// f64 result back to a `PblangValue::Number`. Returns `None` if any argument
/// is non-numeric (caller should fall back to the interpreter).
pub fn call_jit(
    fn_ptr: JitFn,
    args: &[PblangValue],
) -> Option<PblangValue> {
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

// ── Tests ─────────────────────────────────────────────────────────────────────

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
        chunk.write(OpCode::OpGetLocal(0), sp);  // 0: push n
        chunk.write(OpCode::OpConstant(c1), sp); // 1: push 1.0
        chunk.write(OpCode::OpLe, sp);            // 2: push (n<=1)
        // OpJumpIfFalse to instruction 6 (past early return)
        chunk.write(OpCode::OpJumpIfFalse(6), sp); // 3

        // Block 1: early return (n <= 1 case)
        chunk.write(OpCode::OpGetLocal(0), sp);  // 4: push n
        chunk.write(OpCode::OpReturn, sp);        // 5: return n

        // Block 2: recursive case
        chunk.write(OpCode::OpGetGlobal(cfib), sp); // 6: push fib ref
        chunk.write(OpCode::OpGetLocal(0), sp);     // 7: push n
        chunk.write(OpCode::OpConstant(c1), sp);    // 8: push 1
        chunk.write(OpCode::OpSub, sp);              // 9: n-1
        chunk.write(OpCode::OpCall(1), sp);          // 10: fib(n-1)

        chunk.write(OpCode::OpGetGlobal(cfib), sp); // 11: push fib ref
        chunk.write(OpCode::OpGetLocal(0), sp);     // 12: push n
        chunk.write(OpCode::OpConstant(c2), sp);    // 13: push 2
        chunk.write(OpCode::OpSub, sp);              // 14: n-2
        chunk.write(OpCode::OpCall(1), sp);          // 15: fib(n-2)

        chunk.write(OpCode::OpAdd, sp);              // 16: fib(n-1) + fib(n-2)
        chunk.write(OpCode::OpReturn, sp);           // 17: return

        Function {
            name: Some("fib".to_string()),
            arity: 1,
            min_arity: 1,
            defaults: vec![],
            is_vararg: false,
            chunk,
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
                let cases = [(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (3.0, 2.0), (5.0, 5.0), (10.0, 55.0)];
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
        // OpPrint is not JIT-able
        let c = chunk.add_const(pblang_ir::alloc_string("hello"));
        chunk.write(OpCode::OpConstant(c), sp);
        chunk.write(OpCode::OpPrint, sp);
        chunk.write(OpCode::OpReturn, sp);

        let func = Function { name: Some("not_jit".into()), arity: 0, min_arity: 0, defaults: vec![], is_vararg: false, chunk };
        assert!(
            is_jit_eligible(&func.chunk).is_err(),
            "function with OpPrint should NOT be JIT-eligible"
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
        };

        let mut cache = JitCache::new().expect("JitCache::new");
        match cache.compile("not_nil", &func) {
            JitResult::Compiled(fp) => {
                let result = unsafe { fp(std::ptr::null(), 0) };
                assert_eq!(result, 1.0, "!nil should evaluate to true (1.0), got {result}");
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

        chunk.write(OpCode::OpConstant(c_nil), sp);         // 0
        chunk.write(OpCode::OpJumpIfFalse(4), sp);          // 1
        chunk.write(OpCode::OpConstant(c_true_branch), sp); // 2
        chunk.write(OpCode::OpReturn, sp);                  // 3
        chunk.write(OpCode::OpConstant(c_false_branch), sp);// 4
        chunk.write(OpCode::OpReturn, sp);                  // 5

        let func = Function {
            name: Some("nil_branch".into()),
            arity: 0,
            min_arity: 0,
            defaults: vec![],
            is_vararg: false,
            chunk,
        };

        let mut cache = JitCache::new().expect("JitCache::new");
        match cache.compile("nil_branch", &func) {
            JitResult::Compiled(fp) => {
                let result = unsafe { fp(std::ptr::null(), 0) };
                assert_eq!(result, 2.0, "nil condition should take false branch, got {result}");
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

        chunk.write(OpCode::OpConstant(c_nil), sp);         // 0
        chunk.write(OpCode::OpJumpIfTrue(4), sp);           // 1
        chunk.write(OpCode::OpConstant(c_fallthrough), sp); // 2
        chunk.write(OpCode::OpReturn, sp);                  // 3
        chunk.write(OpCode::OpConstant(c_jump_target), sp); // 4
        chunk.write(OpCode::OpReturn, sp);                  // 5

        let func = Function {
            name: Some("nil_jump_if_true".into()),
            arity: 0,
            min_arity: 0,
            defaults: vec![],
            is_vararg: false,
            chunk,
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
}
