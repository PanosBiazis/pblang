use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub mod chunk {
    use super::Value;
    use std::rc::Rc;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub struct Span {
        pub start: usize,
        pub end: usize,
    }

    #[derive(Clone, Debug)]
    pub enum OpCode {
        OpConstant(usize),

        // arithmetic
        OpAdd,
        OpSub,
        OpMul,
        OpDiv,

        // comparisons (push Bool)
        OpEq,
        OpNotEq,
        OpGt,
        OpGe,
        OpLt,
        OpLe,

        // stack
        OpPop,
        OpPrint,

        // globals
        OpDefineGlobal(usize),
        OpGetGlobal(usize),
        OpSetGlobal(usize),

        // member access
        OpGetAttr(usize), // const idx -> must be Obj::String(attr_name)
        OpSetAttr(usize), // const idx -> attr_name, pops (obj,value), pushes value

        // module exports
        OpExport(usize), // const idx -> export name string

        // collections
        OpMakeArray(usize), // pops N values, pushes Array
        OpMakeMap(usize),   // pops 2*N values: (key,value)..., pushes Map
        OpGetIndex,         // pops (obj, index), pushes value
        OpSetIndex,         // pops (obj, index, value), pushes value

        // error handling
        OpTry(usize), // push handler (absolute IP)
        OpEndTry,     // pop handler

        // locals (slot indexes)
        OpGetLocal(usize),
        OpSetLocal(usize),

        // functions
        OpClosure {
            fn_const: usize,
            captures: Vec<(bool, usize)>,
        }, // (is_local, index)
        OpGetUpvalue(usize),
        OpSetUpvalue(usize),
        OpCloseUpvalue,

        OpCall(usize), // argc
        OpReturn,
        OpDefer, // pop closure from stack, register as defer for current frame (run on scope exit)

        // control flow (absolute instruction pointers)
        OpJump(usize),
        OpJumpIfFalse(usize),
        OpJumpIfTrue(usize),

        // logic
        OpNot,

        // extended arithmetic
        OpNegate, // arithmetic negation (unary minus)
        OpMod,    // modulo (fmod for floats, irem for ints)
        OpPow,    // power (pow for floats, fallback for ints)

        // --- Safety / Rust-inspired opcodes ---
        /// Pops obj; if nil pushes nil, else gets named attribute (const-idx).
        OpSafeGetAttr(usize),
        /// Pops obj; if nil pushes nil, then pops index and does safe get-index.
        /// (Implemented as: dup, check nil, jump past index, else fall through to OpGetIndex)
        OpSafeIndex,
        /// Pops a Result map {ok, value/error}:
        ///   ok==true  → push .value
        ///   ok==false → raise RuntimeError(error message)
        ///   other     → push as-is (non-Result value is passed through)
        OpResultUnwrap,
        /// Pops value; pushes true if nil, false otherwise
        OpIsNil,
        /// Duplicates the top of the value stack: push(peek())
        OpDup,
        /// Pops a value and raises it as a RuntimeError.
        /// If the value is a string, uses it as the message.
        /// If the value is a map with an "message" key, uses that.
        /// Otherwise converts to string and uses as message.
        OpThrow,
    }

    #[derive(Debug, Default, Clone)]
    pub struct Chunk {
        pub instructions: Vec<OpCode>,
        pub constants: Vec<Value>,
        pub spans: Vec<Span>,
        pub source_name: Option<Rc<String>>,
        pub source: Option<Rc<String>>,
    }

    impl Chunk {
        pub fn add_const(&mut self, v: Value) -> usize {
            self.constants.push(v);
            self.constants.len() - 1
        }

        pub fn write(&mut self, op: OpCode, sp: Span) {
            self.instructions.push(op);
            self.spans.push(sp);
        }

        pub fn patch(&mut self, pos: usize, op: OpCode) {
            self.instructions[pos] = op;
        }
    }
}

pub use chunk::{Chunk, OpCode, Span as VmSpan};

#[derive(Debug, Clone)]
pub struct FrameTrace {
    pub function: Option<String>,
    pub span: Option<chunk::Span>,
    pub source_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub message: String,
    pub span: Option<chunk::Span>,
    pub source_name: Option<String>,
    pub source: Option<Rc<String>>,
    pub stack: Vec<FrameTrace>,
}

pub trait VmHost {
    fn err(&self, msg: String) -> RuntimeError;
    /// Downcast hook used by the VM for host-specific subsystems (concurrency handles, etc.).
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    fn import_module(&mut self, path: &str) -> Result<Value, RuntimeError>;
    /// Optional tracing/cycle collection hook. Default is a no-op.
    ///
    /// Returns `(arrays_cleared, maps_cleared, modules_cleared)`.
    fn gc_collect_cycles(&mut self) -> (usize, usize, usize) {
        (0, 0, 0)
    }

    /// Capability flags for sandboxing. When embedding, set these to restrict what builtins can do.
    fn allow_fs(&self) -> bool {
        true
    }
    fn allow_net(&self) -> bool {
        true
    }
    fn allow_http(&self) -> bool {
        true
    }
    fn allow_sqlite(&self) -> bool {
        true
    }
    fn allow_ffi(&self) -> bool {
        false
    }
    fn allow_exec(&self) -> bool {
        false
    }

    /// FFI guardrails: when not overridden, allow all (no allowlist). Override to restrict libraries/symbols.
    fn ffi_allowed_library(&self, _path: &str) -> bool {
        true
    }
    fn ffi_allowed_symbol(&self, _symbol: &str) -> bool {
        true
    }

    /// IO timeout in milliseconds for network (and optionally fs) builtins. When set, http_get/http_post
    /// and tcp streams use this timeout; default None means no timeout.
    fn io_timeout_ms(&self) -> Option<u64> {
        None
    }

    /// Whether spawn() is allowed (sandbox). When false, spawn builtin returns a runtime error.
    fn allow_spawn(&self) -> bool {
        true
    }

    /// Limits to apply to the child VM when spawn is used. None = no limits in child.
    fn spawn_limits(&self) -> Option<VmLimits> {
        None
    }

    /// Maximum byte length of JSON accepted from **external** ingress points
    /// (FFI calls, AI provider responses, package registry index). Builtins
    /// that parse JSON from untrusted external sources should consult this
    /// cap. The `json_parse` builtin called from pblang code is *not* gated
    /// by this — only external bytes are. Default 10 MB.
    fn max_external_json_bytes(&self) -> usize {
        10 * 1024 * 1024
    }

    // Runtime memory-safety tracker (optional; the VM host implements these).

    fn mem_allocate(&mut self, _value: Value, _is_mutable: bool) -> Result<f64, RuntimeError> {
        Err(self.err("allocate: memory safety not supported by this host".into()))
    }

    fn mem_borrow_immutable(&mut self, _handle: f64) -> Result<f64, RuntimeError> {
        Err(self.err("borrow_immutable: memory safety not supported by this host".into()))
    }

    fn mem_borrow_mutable(&mut self, _handle: f64) -> Result<f64, RuntimeError> {
        Err(self.err("borrow_mutable: memory safety not supported by this host".into()))
    }

    fn mem_borrow_unique(&mut self, _handle: f64) -> Result<f64, RuntimeError> {
        Err(self.err("borrow_unique: memory safety not supported by this host".into()))
    }

    fn mem_release_borrow_id(&mut self, _borrow_id: f64) -> Result<(), RuntimeError> {
        Err(self.err("release_borrow: memory safety not supported by this host".into()))
    }

    fn mem_release_borrow_type(
        &mut self,
        _handle: f64,
        _borrow_type: u8,
    ) -> Result<(), RuntimeError> {
        Err(self.err("release_borrow: memory safety not supported by this host".into()))
    }

    fn mem_load(&mut self, _handle: f64) -> Result<Value, RuntimeError> {
        Err(self.err("load: memory safety not supported by this host".into()))
    }

    fn mem_store(&mut self, _handle: f64, _value: Value) -> Result<(), RuntimeError> {
        Err(self.err("store: memory safety not supported by this host".into()))
    }

    fn mem_deallocate(&mut self, _handle: f64) -> Result<(), RuntimeError> {
        Err(self.err("deallocate: memory safety not supported by this host".into()))
    }

    fn mem_push_lifetime(&mut self) -> Result<f64, RuntimeError> {
        Err(self.err("push_lifetime: memory safety not supported by this host".into()))
    }

    fn mem_pop_lifetime(&mut self) -> Result<f64, RuntimeError> {
        Err(self.err("pop_lifetime: memory safety not supported by this host".into()))
    }

    fn mem_stats(&self) -> Result<Value, RuntimeError> {
        Err(self.err("memory_stats: memory safety not supported by this host".into()))
    }
}

/// Limits for a spawned VM (instruction count, stack depth, etc.). Used by VmHost::spawn_limits().
#[derive(Clone, Debug, Default)]
pub struct VmLimits {
    pub max_instructions: Option<u64>,
    pub max_stack_depth: Option<usize>,
    pub max_memory_bytes: Option<usize>,
    pub io_timeout_ms: Option<u64>,
}

#[derive(Clone, Copy)]
pub struct BuiltinFn {
    pub name: &'static str,
    pub arity: Option<usize>,
    pub fun: fn(&mut dyn VmHost, Vec<Value>) -> Result<Value, RuntimeError>,
}

impl std::fmt::Debug for BuiltinFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BuiltinFn({})", self.name)
    }
}

#[derive(Clone, Debug)]
pub enum Obj {
    String(String),
}

pub type ObjRef = Rc<Obj>;

static STRING_ALLOCS: AtomicUsize = AtomicUsize::new(0);

pub fn alloc_string<S: Into<String>>(s: S) -> Value {
    STRING_ALLOCS.fetch_add(1, Ordering::Relaxed);
    Value::Obj(Rc::new(Obj::String(s.into())))
}

pub fn string_alloc_count() -> usize {
    STRING_ALLOCS.load(Ordering::Relaxed)
}

#[derive(Clone, Debug)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Nil,
    Obj(ObjRef),

    Function(Rc<Function>),
    Closure(Rc<Closure>),
    Builtin(BuiltinFn),
    Module(Rc<RefCell<Module>>),

    Array(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<HashMap<String, Value>>>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Nil => write!(f, "nil"),
            Value::Obj(o) => match o.as_ref() {
                Obj::String(s) => write!(f, "{s}"),
            },
            Value::Function(_) => write!(f, "fn"),
            Value::Closure(_) => write!(f, "closure"),
            Value::Builtin(b) => write!(f, "<builtin {}>", b.name),
            Value::Module(_) => write!(f, "<module>"),
            Value::Array(a) => {
                let a = a.borrow();
                write!(f, "[")?;
                for (i, v) in a.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::Map(m) => {
                let m = m.borrow();
                write!(f, "map{{")?;
                let mut first = true;
                for (k, v) in m.iter() {
                    if !first {
                        write!(f, ", ")?;
                    }
                    first = false;
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

#[derive(Debug)]
pub struct Function {
    pub name: Option<String>,
    /// Total number of named parameters (required + optional).
    pub arity: usize,
    /// Number of required parameters (those without default values).
    /// Calls with fewer than min_arity args are a runtime error.
    /// Calls with argc in [min_arity, arity] use defaults for missing args.
    pub min_arity: usize,
    /// Pre-compiled default values for optional parameters.
    /// `defaults[i]` corresponds to parameter at index `min_arity + i`.
    /// Only constant-expression defaults (numbers, strings, bools, nil) are stored here.
    /// Complex defaults: fn foo(x, y = nil) { if y == nil { y = compute(); } }
    pub defaults: Vec<Value>,
    /// When true, the last parameter is a variadic "rest" parameter that
    /// collects all extra arguments into an Array value.
    /// E.g. fn sum(...args) — args receives all call arguments as an array.
    pub is_vararg: bool,
    /// The compiled bytecode of this function.
    pub chunk: Chunk,
    /// Optional per-parameter type hints for JIT compilation.
    /// `Some(vec![JitType::I64, JitType::F64, ...])` — one per parameter.
    /// `None` means all parameters default to f64 (backward compatible).
    pub param_types: Option<Vec<ParamType>>,
}

/// JIT type hint for a function parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamType {
    F64,
    I64,
    Bool,
}

#[derive(Debug)]
pub struct Closure {
    pub function: Rc<Function>,
    pub upvalues: Vec<Rc<RefCell<Upvalue>>>,
    pub module: Rc<RefCell<Module>>,
}

#[derive(Debug)]
pub enum Upvalue {
    Open(usize), // stack index
    Closed(Value),
}

#[derive(Debug, Default)]
pub struct Module {
    pub globals: HashMap<String, Value>,
    pub parent: Option<Rc<RefCell<Module>>>,
    pub exports: HashMap<String, Value>,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;
        match (self, other) {
            (Number(a), Number(b)) => a == b,
            (Obj(a), Obj(b)) => match (a.as_ref(), b.as_ref()) {
                (crate::Obj::String(sa), crate::Obj::String(sb)) => sa == sb,
            },
            (Bool(a), Bool(b)) => a == b,
            (Nil, Nil) => true,
            (Function(a), Function(b)) => Rc::ptr_eq(a, b),
            (Closure(a), Closure(b)) => Rc::ptr_eq(a, b),
            (Builtin(a), Builtin(b)) => (a.fun as usize) == (b.fun as usize),
            (Module(a), Module(b)) => Rc::ptr_eq(a, b),
            (Array(a), Array(b)) => a.borrow().as_slice() == b.borrow().as_slice(),
            (Map(a), Map(b)) => *a.borrow() == *b.borrow(),
            _ => false,
        }
    }
}

impl Value {
    pub fn as_str(&self) -> Result<&str, String> {
        match self {
            Value::Obj(o) => match o.as_ref() {
                Obj::String(s) => Ok(s.as_str()),
            },
            _ => Err("expected string".into()),
        }
    }

    /// Concatenate two values: number+number gives sum, string+string gives concatenation.
    /// Named `concat` (not `add`) to avoid confusion with `std::ops::Add`.
    pub fn concat(self, other: Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
            (Value::Obj(a), Value::Obj(b)) => match (a.as_ref(), b.as_ref()) {
                (Obj::String(sa), Obj::String(sb)) => {
                    let mut out = String::with_capacity(sa.len() + sb.len());
                    out.push_str(sa);
                    out.push_str(sb);
                    Ok(alloc_string(out))
                }
            },
            _ => Err("type error: add expects (number, number) or (string, string)".into()),
        }
    }
}

/// Serialize a compiled [`Chunk`] to the **PBCK** on-disk / embed format.
///
/// Layout: magic `PBCK`, instruction count, constant count, serialized constants,
/// then serialized opcodes. Used by the CLI Cranelift AOT path and
/// `pblang-cranelift` object emission.
pub fn serialize_chunk_pbck(chunk: &Chunk) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 + chunk.instructions.len() + chunk.constants.len() * 8);
    out.extend_from_slice(b"PBCK");
    out.extend_from_slice(&(chunk.instructions.len() as u32).to_le_bytes());
    out.extend_from_slice(&(chunk.constants.len() as u32).to_le_bytes());

    for c in &chunk.constants {
        match c {
            Value::Number(v) => {
                out.push(0);
                out.extend_from_slice(&v.to_le_bytes());
            }
            Value::Bool(b) => {
                out.push(1);
                out.push(u8::from(*b));
            }
            Value::Nil => out.push(2),
            Value::Obj(o) => {
                out.push(3);
                let Obj::String(s) = o.as_ref();
                let bytes = s.as_bytes();
                out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                out.extend_from_slice(bytes);
            }
            _ => out.push(2),
        }
    }

    for op in &chunk.instructions {
        match op {
            OpCode::OpConstant(idx) => {
                out.push(0);
                out.extend_from_slice(&(*idx as u32).to_le_bytes());
            }
            OpCode::OpReturn => out.push(1),
            OpCode::OpAdd => out.push(2),
            OpCode::OpSub => out.push(3),
            OpCode::OpMul => out.push(4),
            OpCode::OpDiv => out.push(5),
            OpCode::OpGetLocal(idx) => {
                out.push(6);
                out.extend_from_slice(&(*idx as u32).to_le_bytes());
            }
            OpCode::OpSetLocal(idx) => {
                out.push(7);
                out.extend_from_slice(&(*idx as u32).to_le_bytes());
            }
            OpCode::OpJump(off) => {
                out.push(8);
                out.extend_from_slice(&(*off as u32).to_le_bytes());
            }
            OpCode::OpJumpIfFalse(off) => {
                out.push(9);
                out.extend_from_slice(&(*off as u32).to_le_bytes());
            }
            OpCode::OpJumpIfTrue(off) => {
                out.push(10);
                out.extend_from_slice(&(*off as u32).to_le_bytes());
            }
            OpCode::OpEq => out.push(11),
            OpCode::OpNotEq => out.push(12),
            OpCode::OpGt => out.push(13),
            OpCode::OpGe => out.push(14),
            OpCode::OpLt => out.push(15),
            OpCode::OpLe => out.push(16),
            OpCode::OpNot => out.push(17),
            OpCode::OpPop => out.push(18),
            OpCode::OpCall(n) => {
                out.push(19);
                out.extend_from_slice(&(*n as u32).to_le_bytes());
            }
            OpCode::OpGetGlobal(idx) => {
                out.push(20);
                out.extend_from_slice(&(*idx as u32).to_le_bytes());
            }
            OpCode::OpPrint => out.push(21),
            OpCode::OpDup => out.push(22),
            _ => out.push(255),
        }
    }
    out
}

#[cfg(test)]
mod serialize_tests {
    use super::*;
    use chunk::Span;

    #[test]
    fn serialize_chunk_pbck_smoke() {
        let mut chunk = Chunk::default();
        let sp = Span::default();
        let c = chunk.add_const(Value::Number(42.0));
        chunk.write(OpCode::OpConstant(c), sp);
        chunk.write(OpCode::OpReturn, sp);

        let bytes = serialize_chunk_pbck(&chunk);
        assert!(bytes.starts_with(b"PBCK"));
        assert!(bytes.len() > 8);
    }
}
