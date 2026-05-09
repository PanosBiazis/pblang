use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::fmt;
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
        OpTry(usize),  // push handler (absolute IP)
        OpEndTry,      // pop handler

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
    pub arity: usize,
    pub chunk: Chunk,
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
