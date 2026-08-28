//! Intermediate representation shared between the pblang compiler, VM, and
//! out-of-process consumers (FFI, AOT, JIT).
//!
//! This crate is the **interface boundary** between the front end (parser and
//! typechecker, in `pblang-core`) and the runtime (`pblang-vm`, plus the optional
//! `pblang-cranelift` JIT/AOT and the `pblang-ffi` embedding C ABI).
//! Anything that crosses that boundary lives here.
//!
//! # Type hierarchy
//!
//! - [`chunk::Chunk`] — a flat block of [`chunk::OpCode`] instructions plus
//!   a constant pool. The compiler emits one chunk per function body.
//! - [`Value`] — the runtime tagged-union value. Numbers and bools are
//!   unboxed; strings, arrays, maps, functions, closures, builtins, and
//!   modules are reference-counted via `Rc` / `Rc<RefCell<_>>`.
//! - [`Function`] / [`Closure`] / [`Upvalue`] / [`Module`] — the closure
//!   model. `Function` is the compiled-code template; `Closure` pairs it
//!   with captured upvalues and a parent module.
//! - [`BuiltinFn`] — a Rust function pointer registered into the VM's
//!   builtin table (`vm.rs` builds the full table from each submodule).
//! - [`VmHost`] — the abstraction the IR/builtins call back into for
//!   sandbox capability checks (`allow_*`), memory-safety tracking,
//!   module imports, and error construction. The runtime `pblang-vm`
//!   `VM` struct implements this; embedders can wrap or replace it.
//! - [`RuntimeError`] / [`FrameTrace`] — propagated through `?` from
//!   builtins back into the dispatch loop and on to the caller.
//!
//! # On-disk format
//!
//! [`serialize_chunk_pbck`] writes a compiled chunk to the **PBCK**
//! binary format used by `pblang compile` / `pblang bundle` and by the
//! Cranelift AOT path. Only a subset of opcodes is round-tripped today
//! (see the function body); the rest fall through to opcode `255` and
//! are filled in as the AOT path needs them.

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// Storage for a module's global namespace and its exports. Uses FxHash (rustc-hash) instead of the
/// std default SipHash: global names are short (`"i"`, `"h"`, `"print"`) and read on every `OpGetGlobal`,
/// which is the interpreter's hottest op in loops that touch top-level variables. FxHash has no random
/// seed and a far cheaper mix, so short-key lookups get materially faster with no API change (it is a
/// `HashMap` with a different hasher — get/insert/contains_key/iter all identical).
pub type GlobalMap = FxHashMap<String, Value>;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A binary arithmetic operation, used by [`Value::arith`] to share one
/// numeric-dispatch path (f64 fast path + arbitrary-precision `BigInt` path)
/// across all the VM's arithmetic opcodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[inline(always)]
fn apply_f64(op: ArithOp, a: f64, b: f64) -> f64 {
    match op {
        ArithOp::Add => a + b,
        ArithOp::Sub => a - b,
        ArithOp::Mul => a * b,
        ArithOp::Div => a / b,
        ArithOp::Mod => a % b,
        ArithOp::Pow => a.powf(b),
    }
}

/// Number of fractional digits a decimal *division* produces (the result is
/// rounded half-away-from-zero to this scale). Unlike a rational, a decimal has
/// a finite base-10 scale, so `decimal("1")/decimal("3")` terminates here.
const DECIMAL_DIV_SCALE: i64 = 50;

fn pow10(n: u32) -> BigInt {
    BigInt::from(10u8).pow(n)
}

/// Integer division rounded half-away-from-zero.
fn round_div(num: &BigInt, den: &BigInt) -> BigInt {
    let q = num / den;
    let r = num - &q * den;
    if r.is_zero() {
        return q;
    }
    // round away from zero when the remainder is at least half the divisor
    if (&r.abs() * 2) >= den.abs() {
        if (num.is_negative()) ^ (den.is_negative()) {
            q - 1
        } else {
            q + 1
        }
    } else {
        q
    }
}

/// Arbitrary-precision base-10 decimal: value = `mantissa × 10^(−scale)`.
/// Exact for sums/products; division rounds to [`DECIMAL_DIV_SCALE`] digits.
/// This is the money/display-friendly companion to the exact-fraction rational
/// (`1/3` displays as `0.333…` to a fixed scale, not as a fraction).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decimal {
    mantissa: BigInt,
    scale: i64,
}

impl Decimal {
    pub fn from_bigint(n: BigInt) -> Self {
        Decimal {
            mantissa: n,
            scale: 0,
        }
        .normalized()
    }

    /// Parse a plain decimal literal: optional sign, digits, optional `.frac`.
    pub fn parse(s: &str) -> Result<Self, String> {
        let t = s.trim();
        let neg = t.starts_with('-');
        let body = t.trim_start_matches(['+', '-']);
        let (int_part, frac_part) = match body.split_once('.') {
            Some((i, f)) => (i, f),
            None => (body, ""),
        };
        let digits = format!("{int_part}{frac_part}");
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("decimal(): cannot parse {s:?}"));
        }
        let mut mantissa = digits
            .parse::<BigInt>()
            .map_err(|_| format!("decimal(): cannot parse {s:?}"))?;
        if neg {
            mantissa = -mantissa;
        }
        Ok(Decimal {
            mantissa,
            scale: frac_part.len() as i64,
        }
        .normalized())
    }

    /// Drop trailing fractional zeros (so `0.30` is `0.3`); zero has scale 0.
    fn normalized(mut self) -> Self {
        let ten = BigInt::from(10u8);
        while self.scale > 0 && !self.mantissa.is_zero() && (&self.mantissa % &ten).is_zero() {
            self.mantissa /= &ten;
            self.scale -= 1;
        }
        if self.mantissa.is_zero() {
            self.scale = 0;
        }
        self
    }

    fn aligned(&self, o: &Decimal) -> (BigInt, BigInt, i64) {
        let s = self.scale.max(o.scale);
        let a = &self.mantissa * pow10((s - self.scale) as u32);
        let b = &o.mantissa * pow10((s - o.scale) as u32);
        (a, b, s)
    }

    pub fn add(&self, o: &Decimal) -> Decimal {
        let (a, b, s) = self.aligned(o);
        Decimal {
            mantissa: a + b,
            scale: s,
        }
        .normalized()
    }

    pub fn sub(&self, o: &Decimal) -> Decimal {
        let (a, b, s) = self.aligned(o);
        Decimal {
            mantissa: a - b,
            scale: s,
        }
        .normalized()
    }

    pub fn mul(&self, o: &Decimal) -> Decimal {
        Decimal {
            mantissa: &self.mantissa * &o.mantissa,
            scale: self.scale + o.scale,
        }
        .normalized()
    }

    pub fn div(&self, o: &Decimal) -> Result<Decimal, String> {
        if o.mantissa.is_zero() {
            return Err("decimal division by zero".into());
        }
        let ds = DECIMAL_DIV_SCALE;
        let k = ds - self.scale + o.scale;
        let (num, den) = if k >= 0 {
            (&self.mantissa * pow10(k as u32), o.mantissa.clone())
        } else {
            (self.mantissa.clone(), &o.mantissa * pow10((-k) as u32))
        };
        Ok(Decimal {
            mantissa: round_div(&num, &den),
            scale: ds,
        }
        .normalized())
    }

    pub fn pow(&self, exp: i64) -> Result<Decimal, String> {
        if exp.unsigned_abs() > 1_000_000 {
            return Err("decimal ** exponent too large".into());
        }
        if exp >= 0 {
            Ok(Decimal {
                mantissa: self.mantissa.pow(exp as u32),
                scale: self.scale * exp,
            }
            .normalized())
        } else {
            Decimal::from_bigint(BigInt::from(1)).div(&self.pow(-exp)?)
        }
    }

    pub fn compare(&self, o: &Decimal) -> std::cmp::Ordering {
        let (a, b, _) = self.aligned(o);
        a.cmp(&b)
    }

    pub fn to_f64(&self) -> f64 {
        self.mantissa.to_f64().unwrap_or(f64::NAN) / 10f64.powi(self.scale as i32)
    }

    pub fn is_zero(&self) -> bool {
        self.mantissa.is_zero()
    }

    pub fn negate(&self) -> Decimal {
        Decimal {
            mantissa: -self.mantissa.clone(),
            scale: self.scale,
        }
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let neg = self.mantissa.is_negative();
        let digits = self.mantissa.abs().to_string();
        let body = if self.scale <= 0 {
            let zeros = (-self.scale) as usize;
            format!("{digits}{}", "0".repeat(zeros))
        } else {
            let s = self.scale as usize;
            if digits.len() <= s {
                format!("0.{}{}", "0".repeat(s - digits.len()), digits)
            } else {
                let (int_p, frac_p) = digits.split_at(digits.len() - s);
                format!("{int_p}.{frac_p}")
            }
        };
        write!(f, "{}{}", if neg { "-" } else { "" }, body)
    }
}

/// Bytecode chunk + opcode definitions.
///
/// A `Chunk` is the unit the compiler emits for one function body: a
/// vector of `OpCode` instructions, a parallel vector of source `Span`s
/// for diagnostics, and a pool of [`Value`] constants referenced by
/// `OpConstant(idx)` and the various `OpGetGlobal(idx)` / `OpGetAttr(idx)`
/// instructions that take a constant-pool string index.
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
        // Tail call: like OpCall, but when the callee is a user closure and the
        // current frame has no pending defers, the VM REUSES the current frame
        // instead of pushing a new one (constant stack depth for tail recursion).
        // Emitted by the compiler only in tail position (`return f(args)`), and
        // always followed by OpReturn (used by the non-reuse fallback path).
        OpTailCall(usize), // argc
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

/// One frame in the call stack of a [`RuntimeError`].
///
/// Populated by the VM as it unwinds: each frame carries the function
/// name (when known), the bytecode-mapped source [`chunk::Span`], and
/// the source file label so the error can be rendered with caret
/// pointers in the diagnostic format used by `pblang run`.
#[derive(Debug, Clone)]
pub struct FrameTrace {
    pub function: Option<String>,
    pub span: Option<chunk::Span>,
    pub source_name: Option<String>,
}

/// Runtime error propagated through the VM via `?`.
///
/// Constructed by [`VmHost::err`]; the host attaches the current
/// instruction's span and source name when available. The `source`
/// field is the full module text (kept as `Rc<String>` so multiple
/// frames can share it); the renderer slices it with each frame's
/// span to print the offending line.
#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub message: String,
    pub span: Option<chunk::Span>,
    pub source_name: Option<String>,
    pub source: Option<Rc<String>>,
    pub stack: Vec<FrameTrace>,
}

/// The abstraction the IR and builtin functions call back into.
///
/// Implemented by `pblang_vm::VM` in the canonical runtime, but the
/// trait shape is what makes embedding possible: a host wrapper can
/// override capability gates (`allow_fs`, `allow_net`, …) to sandbox
/// untrusted pblang code, override `io_timeout_ms` to bound network
/// stalls, override `max_external_json_bytes` to cap untrusted-JSON
/// ingress, or override the `mem_*` hooks to plug in memory-safety
/// tracking. Effectful default trait implementations deny access; concrete
/// hosts must opt in explicitly. See `docs/Capability_model.md` for the
/// host-contract.
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
        false
    }
    fn allow_net(&self) -> bool {
        false
    }
    fn allow_http(&self) -> bool {
        false
    }
    fn allow_sqlite(&self) -> bool {
        false
    }
    fn allow_ffi(&self) -> bool {
        false
    }
    fn allow_exec(&self) -> bool {
        false
    }
    /// Whether reading process environment variables (`env_get`) is allowed.
    /// Defaults to false because process environments commonly contain secrets.
    fn allow_env(&self) -> bool {
        false
    }

    /// Whether `os_exit()` may call `std::process::exit` and terminate the whole
    /// process. Defaults to false; the trusted CLI opts in explicitly.
    fn allow_process_exit(&self) -> bool {
        false
    }

    /// Whether source modules may be loaded from host paths.
    fn allow_import(&self) -> bool {
        false
    }

    /// FFI guardrails are fail-closed unless a concrete host supplies policy.
    fn ffi_allowed_library(&self, _path: &str) -> bool {
        false
    }
    fn ffi_allowed_symbol(&self, _symbol: &str) -> bool {
        false
    }

    /// IO timeout in milliseconds for network (and optionally fs) builtins. When set, http_get/http_post
    /// and tcp streams use this timeout; default None means no timeout.
    fn io_timeout_ms(&self) -> Option<u64> {
        None
    }

    /// Whether spawn() is allowed (sandbox). When false, spawn builtin returns a runtime error.
    fn allow_spawn(&self) -> bool {
        false
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

/// Entry in the VM's builtin dispatch table.
///
/// `arity = Some(n)` means strict arity (the dispatch loop rejects the
/// call before entering `fun`); `arity = None` means variadic — `fun`
/// must check `args.len()` itself, typically via the `vm_arity!` macro
/// in `pblang-vm`. The function pointer is plain `fn`, not `Fn`, so
/// builtins cannot close over state (state lives in `&mut dyn VmHost`).
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

/// Heap-allocated, reference-counted object payload.
///
/// Today only `String` is boxed this way (numbers/bools are inline;
/// arrays and maps are `Rc<RefCell<_>>` directly on [`Value`]). The
/// enum is kept extensible so new heap-allocated kinds (interned
/// symbols, byte buffers, …) can land without rewriting every match.
#[derive(Clone, Debug)]
pub enum Obj {
    String(String),
    /// Arbitrary-precision signed integer (opt-in "highest precision" type,
    /// created from pblang via `bigint(x)`). Heap-boxed like strings, so adding
    /// it doesn't disturb the unboxed `Value::Number(f64)` fast path.
    BigInt(BigInt),
    /// Arbitrary-precision **exact rational** (a `BigInt` numerator/denominator,
    /// always reduced). Created via `rational(x)` / `rational(n, d)`. This is the
    /// highest-precision representation for non-integers — `1/3` is exact, not a
    /// rounded float.
    Rational(BigRational),
    /// Arbitrary-precision **base-10 decimal** (`mantissa × 10^−scale`). Created
    /// via `decimal(x)`. Money/display-friendly: sums/products are exact, division
    /// rounds to a fixed scale (so `1/3` is `0.333…`, not a fraction).
    Decimal(Decimal),
}

/// Shared, immutable handle to an [`Obj`].
pub type ObjRef = Rc<Obj>;

static STRING_ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BIGINT_ALLOCS: AtomicUsize = AtomicUsize::new(0);
static RATIONAL_ALLOCS: AtomicUsize = AtomicUsize::new(0);
static DECIMAL_ALLOCS: AtomicUsize = AtomicUsize::new(0);

/// Allocate a decimal [`Value`]. A whole-valued decimal stays a decimal (scale 0)
/// so `decimal("5")` prints `5` but remains the decimal type.
pub fn alloc_decimal(d: Decimal) -> Value {
    DECIMAL_ALLOCS.fetch_add(1, Ordering::Relaxed);
    Value::Obj(Rc::new(Obj::Decimal(d)))
}

/// `decimal(x)` from a string ("3.14", "5", "-0.001"), exact for what's written.
pub fn decimal_from_str(s: &str) -> Result<Value, String> {
    Decimal::parse(s).map(alloc_decimal)
}

/// `decimal(x)` from an f64 — uses the f64's shortest decimal form (so
/// `decimal(0.1)` is exactly `0.1` in base 10, not the binary approximation).
pub fn decimal_from_f64(f: f64) -> Result<Value, String> {
    if !f.is_finite() {
        return Err(format!("decimal(): {f} is not finite"));
    }
    Decimal::parse(&format!("{f}")).map(alloc_decimal)
}

/// `decimal(x)` from an integer.
pub fn decimal_from_bigint(n: BigInt) -> Value {
    alloc_decimal(Decimal::from_bigint(n))
}

/// `decimal(x)` from any value: a string is parsed; a number/bigint/rational/
/// decimal is converted (a rational divides to the default scale).
pub fn decimal_from_value(v: &Value) -> Result<Value, String> {
    if let Value::Obj(o) = v {
        if let Obj::String(s) = o.as_ref() {
            return decimal_from_str(s);
        }
    }
    v.as_decimal_exact().map(alloc_decimal).ok_or_else(|| {
        "decimal(): expected a number, string, bigint, rational, or decimal".to_string()
    })
}

/// Allocate an arbitrary-precision integer [`Value`].
pub fn alloc_bigint(n: BigInt) -> Value {
    BIGINT_ALLOCS.fetch_add(1, Ordering::Relaxed);
    Value::Obj(Rc::new(Obj::BigInt(n)))
}

/// Allocate an exact rational [`Value`]. A rational whose denominator reduces to
/// 1 is normalized to a [`Obj::BigInt`] so integer results stay integers.
pub fn alloc_rational(r: BigRational) -> Value {
    if r.is_integer() {
        return alloc_bigint(r.to_integer());
    }
    RATIONAL_ALLOCS.fetch_add(1, Ordering::Relaxed);
    Value::Obj(Rc::new(Obj::Rational(r)))
}

/// Cumulative count of bigints allocated through [`alloc_bigint`].
pub fn bigint_alloc_count() -> usize {
    BIGINT_ALLOCS.load(Ordering::Relaxed)
}

/// Build a rational from a numerator/denominator pair (`rational(n, d)`).
pub fn rational_from_parts(num: BigInt, den: BigInt) -> Result<Value, String> {
    if den.is_zero() {
        return Err("rational(): denominator is zero".into());
    }
    Ok(alloc_rational(BigRational::new(num, den)))
}

/// Parse a string into an exact rational: accepts `"a/b"`, a decimal `"X.Y"`
/// (turned into an exact fraction, so `"0.1"` is precisely 1/10), or an integer.
pub fn rational_from_str(s: &str) -> Result<Value, String> {
    let t = s.trim();
    if let Some((n, d)) = t.split_once('/') {
        let num = n
            .trim()
            .parse::<BigInt>()
            .map_err(|_| format!("rational(): bad numerator in {s:?}"))?;
        let den = d
            .trim()
            .parse::<BigInt>()
            .map_err(|_| format!("rational(): bad denominator in {s:?}"))?;
        return rational_from_parts(num, den);
    }
    if let Some(dot) = t.find('.') {
        // Decimal literal "X.Y" → (XY) / 10^len(Y), exact.
        let (int_part, frac_part) = (&t[..dot], &t[dot + 1..]);
        let neg = int_part.starts_with('-');
        let digits = format!("{}{}", int_part.replace('-', ""), frac_part);
        let num = digits
            .parse::<BigInt>()
            .map_err(|_| format!("rational(): cannot parse decimal {s:?}"))?;
        let den = BigInt::from(10u8).pow(frac_part.len() as u32);
        let num = if neg { -num } else { num };
        return rational_from_parts(num, den);
    }
    let n = t
        .parse::<BigInt>()
        .map_err(|_| format!("rational(): cannot parse {s:?}"))?;
    Ok(alloc_rational(BigRational::from(n)))
}

/// Build a rational from an f64. A finite value is converted to its **exact**
/// binary rational (so `rational(0.5)` is 1/2; `rational(0.1)` is the exact
/// dyadic value of the f64, not 1/10 — use `rational("1/10")` for that).
pub fn rational_from_f64(f: f64) -> Result<Value, String> {
    BigRational::from_float(f)
        .map(alloc_rational)
        .ok_or_else(|| format!("rational(): {f} is not finite"))
}

/// Parse a decimal string into an arbitrary-precision integer [`Value`].
/// This is the lossless path for literals bigger than f64 can hold exactly
/// (e.g. `bigint("123456789012345678901234567890")`).
pub fn bigint_from_str(s: &str) -> Result<Value, String> {
    s.trim()
        .parse::<BigInt>()
        .map(alloc_bigint)
        .map_err(|_| format!("bigint(): cannot parse {s:?} as an integer"))
}

/// Convert an integral, finite f64 into an arbitrary-precision integer.
/// (Values above 2^53 are already imprecise as f64 — use the string form for
/// exactness beyond that.)
pub fn bigint_from_f64(f: f64) -> Result<Value, String> {
    if !f.is_finite() || f.fract() != 0.0 {
        return Err(format!("bigint(): {f} is not an integer"));
    }
    Ok(alloc_bigint(BigInt::from(f as i128)))
}

/// Allocate a string [`Value`] and bump the per-process counter.
///
/// Use this instead of constructing `Value::Obj(Rc::new(Obj::String(...)))`
/// inline — the counter feeds [`string_alloc_count`], which the `gc_stats`
/// builtin returns to user code.
pub fn alloc_string<S: Into<String>>(s: S) -> Value {
    STRING_ALLOCS.fetch_add(1, Ordering::Relaxed);
    Value::Obj(Rc::new(Obj::String(s.into())))
}

/// Cumulative count of strings allocated through [`alloc_string`]
/// since process start. Surfaced to pblang code via `gc_stats()`.
pub fn string_alloc_count() -> usize {
    STRING_ALLOCS.load(Ordering::Relaxed)
}

/// The runtime tagged-union value.
///
/// Numbers and bools are unboxed `f64` / `bool`. Strings live behind an
/// [`Rc`]/[`Obj`] (immutable + shareable). Arrays and maps are
/// `Rc<RefCell<_>>` so they can be mutated through shared references —
/// the VM uses `RefCell::borrow_mut` at each write site; mis-nested
/// borrows panic at runtime (caught by tests, not exposed to pblang code).
/// Functions/closures/builtins/modules are all `Rc<_>` for cheap cloning.
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
                Obj::BigInt(n) => write!(f, "{n}"),
                Obj::Rational(r) => write!(f, "{r}"),
                Obj::Decimal(d) => write!(f, "{d}"),
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

/// A compiled function: its bytecode + parameter shape + optional JIT hints.
///
/// `Function` is the **template**; it holds no captured state. The VM
/// pairs it with captured upvalues into a [`Closure`] at the
/// `OpClosure` instruction site. One `Function` instance can back many
/// closures (e.g. a recursive closure that captures fresh upvalues per
/// invocation).
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

/// A callable function value: code + captured environment + module.
///
/// `module` is the lexical parent that hosts the function's globals
/// (so `OpGetGlobal`/`OpSetGlobal` resolve against the right scope when
/// the closure is later called from a different module).
#[derive(Debug)]
pub struct Closure {
    pub function: Rc<Function>,
    pub upvalues: Vec<Rc<RefCell<Upvalue>>>,
    pub module: Rc<RefCell<Module>>,
}

/// A captured variable referenced by a [`Closure`].
///
/// `Open(stack_idx)` points at a slot still alive on the parent frame's
/// value stack (cheap, no copy). When that frame returns, the VM
/// **closes** the upvalue by moving the slot's value into
/// `Closed(value)`, so the closure can keep using it after the frame
/// is gone. Multiple closures capturing the same slot share a single
/// `Rc<RefCell<Upvalue>>` — that's how `let x = 0; ...` shared mutation
/// between closures works.
#[derive(Debug)]
pub enum Upvalue {
    Open(usize),
    Closed(Value),
}

/// One pblang module: its `globals` map (mutable, used by
/// `OpDefineGlobal` / `OpGetGlobal` / `OpSetGlobal`), its `exports` map
/// (populated by `OpExport`), and an optional `parent` pointer used by
/// nested module scopes. The VM imports a module by evaluating the
/// file's top-level instructions inside a fresh `Module` and then
/// publishing the `exports` map to the caller.
#[derive(Debug, Default)]
pub struct Module {
    pub globals: GlobalMap,
    pub parent: Option<Rc<RefCell<Module>>>,
    pub exports: GlobalMap,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;
        match (self, other) {
            (Number(a), Number(b)) => a == b,
            (Obj(a), Obj(b)) => match (a.as_ref(), b.as_ref()) {
                (crate::Obj::String(sa), crate::Obj::String(sb)) => sa == sb,
                // Numeric equality across exact types (bigint/rational/decimal):
                // compare by value, so `bigint(2) == rational("4/2")` is true.
                (crate::Obj::BigInt(_), _)
                | (crate::Obj::Rational(_), _)
                | (crate::Obj::Decimal(_), _)
                | (_, crate::Obj::BigInt(_))
                | (_, crate::Obj::Rational(_))
                | (_, crate::Obj::Decimal(_)) => {
                    self.num_cmp(other) == Some(std::cmp::Ordering::Equal)
                }
            },
            // Cross number/exact equality: `bigint(5) == 5`, `decimal("0.5") == 0.5`.
            (Obj(o), Number(_)) | (Number(_), Obj(o))
                if matches!(
                    o.as_ref(),
                    crate::Obj::BigInt(_) | crate::Obj::Rational(_) | crate::Obj::Decimal(_)
                ) =>
            {
                self.num_cmp(other) == Some(std::cmp::Ordering::Equal)
            }
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
                _ => Err("expected string".into()),
            },
            _ => Err("expected string".into()),
        }
    }

    /// True if this value is an arbitrary-precision integer.
    pub fn is_bigint(&self) -> bool {
        matches!(self, Value::Obj(o) if matches!(o.as_ref(), Obj::BigInt(_)))
    }

    /// View this value as an *exact* integer (`BigInt`): a `BigInt` directly, or
    /// an integral, finite `Number`. Returns `None` for fractional/NaN numbers
    /// and non-numeric values (so a fractional float forces an f64 fallback).
    pub fn as_bigint_exact(&self) -> Option<BigInt> {
        match self {
            Value::Obj(o) => match o.as_ref() {
                Obj::BigInt(n) => Some(n.clone()),
                _ => None,
            },
            Value::Number(f) if f.is_finite() && f.fract() == 0.0 => {
                // f64 only represents integers exactly up to 2^53; beyond that an
                // integral f64 is already imprecise, so the i128 cast is the best
                // available. Typical mixing is with small literals (1, 2, …).
                Some(BigInt::from(*f as i128))
            }
            _ => None,
        }
    }

    /// True if this value is an exact rational.
    pub fn is_rational(&self) -> bool {
        matches!(self, Value::Obj(o) if matches!(o.as_ref(), Obj::Rational(_)))
    }

    /// True if this value is a base-10 decimal.
    pub fn is_decimal(&self) -> bool {
        matches!(self, Value::Obj(o) if matches!(o.as_ref(), Obj::Decimal(_)))
    }

    /// View this value as a [`Decimal`]: a decimal directly, a `BigInt`, a finite
    /// `Number` (via its shortest base-10 form), or a rational (divided to the
    /// default scale). `None` for non-finite/non-numeric values.
    fn as_decimal_exact(&self) -> Option<Decimal> {
        match self {
            Value::Obj(o) => match o.as_ref() {
                Obj::Decimal(d) => Some(d.clone()),
                Obj::BigInt(n) => Some(Decimal::from_bigint(n.clone())),
                Obj::Rational(r) => {
                    let num = Decimal::from_bigint(r.numer().clone());
                    let den = Decimal::from_bigint(r.denom().clone());
                    num.div(&den).ok()
                }
                _ => None,
            },
            Value::Number(f) if f.is_finite() => Decimal::parse(&format!("{f}")).ok(),
            _ => None,
        }
    }

    /// View this value as an exact rational: a rational directly, a `BigInt`
    /// (as n/1), or a finite `Number` (as its exact binary rational). `None` for
    /// non-finite/non-numeric values.
    fn as_rational_exact(&self) -> Option<BigRational> {
        match self {
            Value::Obj(o) => match o.as_ref() {
                Obj::Rational(r) => Some(r.clone()),
                Obj::BigInt(n) => Some(BigRational::from(n.clone())),
                _ => None,
            },
            Value::Number(f) => BigRational::from_float(*f),
            _ => None,
        }
    }

    /// Lossy conversion to f64 for the "mixed exact + fractional float" path.
    fn to_f64_lossy(&self) -> Option<f64> {
        match self {
            Value::Number(f) => Some(*f),
            Value::Obj(o) => match o.as_ref() {
                Obj::BigInt(n) => n.to_f64(),
                Obj::Rational(r) => r.to_f64(),
                Obj::Decimal(d) => Some(d.to_f64()),
                _ => None,
            },
            _ => None,
        }
    }

    /// Unified numeric arithmetic shared by every VM arithmetic opcode.
    ///
    /// - string `+` string → concatenation (Add only);
    /// - if either operand is a `BigInt` (or both are exact integers and one is
    ///   a `BigInt`), compute exactly in arbitrary precision;
    /// - otherwise the unboxed `f64` fast path (one mismatched fractional float
    ///   demotes a bigint operation to f64).
    #[inline]
    pub fn arith(&self, other: &Value, op: ArithOp) -> Result<Value, String> {
        // Fast path: both unboxed f64 (overwhelmingly the common case). Checked
        // first so the hot loop never touches the bigint/string branches.
        if let (Value::Number(a), Value::Number(b)) = (self, other) {
            return Ok(Value::Number(apply_f64(op, *a, *b)));
        }
        // string concatenation
        if op == ArithOp::Add {
            if let (Value::Obj(a), Value::Obj(b)) = (self, other) {
                if let (Obj::String(sa), Obj::String(sb)) = (a.as_ref(), b.as_ref()) {
                    let mut out = String::with_capacity(sa.len() + sb.len());
                    out.push_str(sa);
                    out.push_str(sb);
                    return Ok(alloc_string(out));
                }
            }
        }
        // base-10 decimal path (highest priority among the exact types: if the
        // programmer asked for a decimal, keep decimal semantics/rounding).
        if self.is_decimal() || other.is_decimal() {
            if let (Some(x), Some(y)) = (self.as_decimal_exact(), other.as_decimal_exact()) {
                let r = match op {
                    ArithOp::Add => x.add(&y),
                    ArithOp::Sub => x.sub(&y),
                    ArithOp::Mul => x.mul(&y),
                    ArithOp::Div => x.div(&y)?,
                    ArithOp::Mod => return Err("modulo (%) is not defined for decimals".into()),
                    ArithOp::Pow => {
                        let exp = other
                            .as_bigint_exact()
                            .and_then(|e| e.to_i64())
                            .ok_or_else(|| "decimal ** exponent must be an integer".to_string())?;
                        x.pow(exp)?
                    }
                };
                return Ok(alloc_decimal(r));
            }
            let x = self
                .to_f64_lossy()
                .ok_or_else(|| "type error: arithmetic on non-number".to_string())?;
            let y = other
                .to_f64_lossy()
                .ok_or_else(|| "type error: arithmetic on non-number".to_string())?;
            return Ok(Value::Number(apply_f64(op, x, y)));
        }
        // exact-rational path (takes priority over bigint: rationals are a
        // superset, and a bigint operand promotes to n/1).
        if self.is_rational() || other.is_rational() {
            if let (Some(x), Some(y)) = (self.as_rational_exact(), other.as_rational_exact()) {
                let r = match op {
                    ArithOp::Add => x + y,
                    ArithOp::Sub => x - y,
                    ArithOp::Mul => x * y,
                    ArithOp::Div => {
                        if y.is_zero() {
                            return Err("rational division by zero".into());
                        }
                        x / y
                    }
                    ArithOp::Mod => return Err("modulo (%) is not defined for rationals".into()),
                    ArithOp::Pow => {
                        // exponent must be an integer (a^(p/q) is generally irrational)
                        let exp = other
                            .as_bigint_exact()
                            .and_then(|e| e.to_i32())
                            .ok_or_else(|| {
                                "rational ** exponent must be an integer that fits i32".to_string()
                            })?;
                        x.pow(exp)
                    }
                };
                return Ok(alloc_rational(r));
            }
            let x = self
                .to_f64_lossy()
                .ok_or_else(|| "type error: arithmetic on non-number".to_string())?;
            let y = other
                .to_f64_lossy()
                .ok_or_else(|| "type error: arithmetic on non-number".to_string())?;
            return Ok(Value::Number(apply_f64(op, x, y)));
        }
        // arbitrary-precision integer path
        if self.is_bigint() || other.is_bigint() {
            if let (Some(x), Some(y)) = (self.as_bigint_exact(), other.as_bigint_exact()) {
                let r = match op {
                    ArithOp::Add => x + y,
                    ArithOp::Sub => x - y,
                    ArithOp::Mul => x * y,
                    ArithOp::Div => {
                        if y.is_zero() {
                            return Err("bigint division by zero".into());
                        }
                        x / y // truncated integer division
                    }
                    ArithOp::Mod => {
                        if y.is_zero() {
                            return Err("bigint modulo by zero".into());
                        }
                        x % y
                    }
                    ArithOp::Pow => {
                        if y.is_negative() {
                            return Err("bigint ** negative exponent is not an integer".into());
                        }
                        let exp = y
                            .to_u32()
                            .ok_or_else(|| "bigint ** exponent too large".to_string())?;
                        x.pow(exp)
                    }
                };
                return Ok(alloc_bigint(r));
            }
            // mixed with a fractional float → demote to f64
            let x = self
                .to_f64_lossy()
                .ok_or_else(|| "type error: arithmetic on non-number".to_string())?;
            let y = other
                .to_f64_lossy()
                .ok_or_else(|| "type error: arithmetic on non-number".to_string())?;
            return Ok(Value::Number(apply_f64(op, x, y)));
        }
        // unboxed f64 fast path
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(apply_f64(op, *a, *b))),
            _ if op == ArithOp::Add => {
                Err("type error: + expects (number, number), (string, string), or bigint".into())
            }
            _ => Err("type error: arithmetic expects numbers or bigints".into()),
        }
    }

    /// Numeric ordering for `<`/`>`/`<=`/`>=`, bigint-aware. Returns `None` for
    /// non-numeric operands (the VM turns that into a type error).
    #[inline]
    pub fn num_cmp(&self, other: &Value) -> Option<std::cmp::Ordering> {
        // Fast path first.
        if let (Value::Number(a), Value::Number(b)) = (self, other) {
            return a.partial_cmp(b);
        }
        if self.is_decimal() || other.is_decimal() {
            if let (Some(x), Some(y)) = (self.as_decimal_exact(), other.as_decimal_exact()) {
                return Some(x.compare(&y));
            }
            return self.to_f64_lossy()?.partial_cmp(&other.to_f64_lossy()?);
        }
        if self.is_rational() || other.is_rational() {
            if let (Some(x), Some(y)) = (self.as_rational_exact(), other.as_rational_exact()) {
                return Some(x.cmp(&y));
            }
            return self.to_f64_lossy()?.partial_cmp(&other.to_f64_lossy()?);
        }
        if self.is_bigint() || other.is_bigint() {
            if let (Some(x), Some(y)) = (self.as_bigint_exact(), other.as_bigint_exact()) {
                return Some(x.cmp(&y));
            }
            let x = self.to_f64_lossy()?;
            let y = other.to_f64_lossy()?;
            return x.partial_cmp(&y);
        }
        None
    }

    /// Arithmetic negation, bigint-aware.
    pub fn negate(&self) -> Result<Value, String> {
        match self {
            Value::Number(n) => Ok(Value::Number(-n)),
            Value::Obj(o) => match o.as_ref() {
                Obj::BigInt(n) => Ok(alloc_bigint(-n.clone())),
                Obj::Rational(r) => Ok(alloc_rational(-r.clone())),
                Obj::Decimal(d) => Ok(alloc_decimal(d.negate())),
                _ => Err("type error: cannot negate non-number".into()),
            },
            _ => Err("type error: cannot negate non-number".into()),
        }
    }

    /// `+` retained for existing callers; delegates to [`arith`].
    pub fn concat(self, other: Value) -> Result<Value, String> {
        self.arith(&other, ArithOp::Add)
    }
}

/// Compare an f64 to a BigInt for equality (`bigint(5) == 5`). True only when
/// `m` is a finite integer equal to `n`.
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
            Value::Obj(o) => match o.as_ref() {
                Obj::String(s) => {
                    out.push(3);
                    let bytes = s.as_bytes();
                    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    out.extend_from_slice(bytes);
                }
                // Bigints aren't produced as compile-time constants today (they
                // come from the runtime `bigint()` builtin), but serialize their
                // decimal form under tag 4 so the format is forward-compatible.
                Obj::BigInt(n) => {
                    out.push(4);
                    let bytes = n.to_string().into_bytes();
                    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    out.extend_from_slice(&bytes);
                }
                // Rationals (like bigints) aren't compile-time constants today;
                // serialize the reduced "n/d" form under tag 5 for forward-compat.
                Obj::Rational(r) => {
                    out.push(5);
                    let bytes = r.to_string().into_bytes();
                    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    out.extend_from_slice(&bytes);
                }
                // Decimals: their plain decimal form under tag 6.
                Obj::Decimal(d) => {
                    out.push(6);
                    let bytes = d.to_string().into_bytes();
                    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    out.extend_from_slice(&bytes);
                }
            },
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

// ============================================================================
// PBCK v1 — lossless bytecode container (compile once, run without re-parsing)
//
// Unlike the write-only `serialize_chunk_pbck` above (used by the Cranelift AOT
// path, lossy by design), this is a complete, reversible serializer + a matching
// deserializer covering EVERY opcode, every compile-time constant type (Number,
// Bool, Nil, String — plus BigInt/Rational forward-compat), and nested function
// constants (recursively, the whole closure template tree) with spans. This is
// the basis for regenerating a runnable program from pblang without re-running
// the Rust parser/compiler — `pblang run file.pbck`.
//
// Format: magic b"PBCK", u8 version=1, then one serialized <chunk>. All integers
// little-endian u32; strings are u32 length + UTF-8 bytes.
// ============================================================================

/// Container format version. Bump on any breaking layout change.
pub const PBCK_V1_VERSION: u8 = 1;

/// Max function-constant nesting the `.pbck` reader will follow before erroring.
/// Bounds recursion (function → chunk → value → function) so a hostile bytecode
/// file can't overflow the native stack. Far above any realistic compiler output.
pub const PBCK_MAX_DEPTH: u32 = 256;

#[inline]
fn pbck_w_u32(out: &mut Vec<u8>, v: usize) {
    out.extend_from_slice(&(v as u32).to_le_bytes());
}
#[inline]
fn pbck_w_str(out: &mut Vec<u8>, s: &str) {
    pbck_w_u32(out, s.len());
    out.extend_from_slice(s.as_bytes());
}

fn pbck_w_opcode(out: &mut Vec<u8>, op: &OpCode) {
    match op {
        OpCode::OpConstant(i) => { out.push(0); pbck_w_u32(out, *i); }
        OpCode::OpAdd => out.push(1),
        OpCode::OpSub => out.push(2),
        OpCode::OpMul => out.push(3),
        OpCode::OpDiv => out.push(4),
        OpCode::OpEq => out.push(5),
        OpCode::OpNotEq => out.push(6),
        OpCode::OpGt => out.push(7),
        OpCode::OpGe => out.push(8),
        OpCode::OpLt => out.push(9),
        OpCode::OpLe => out.push(10),
        OpCode::OpPop => out.push(11),
        OpCode::OpPrint => out.push(12),
        OpCode::OpDefineGlobal(i) => { out.push(13); pbck_w_u32(out, *i); }
        OpCode::OpGetGlobal(i) => { out.push(14); pbck_w_u32(out, *i); }
        OpCode::OpSetGlobal(i) => { out.push(15); pbck_w_u32(out, *i); }
        OpCode::OpGetAttr(i) => { out.push(16); pbck_w_u32(out, *i); }
        OpCode::OpSetAttr(i) => { out.push(17); pbck_w_u32(out, *i); }
        OpCode::OpExport(i) => { out.push(18); pbck_w_u32(out, *i); }
        OpCode::OpMakeArray(i) => { out.push(19); pbck_w_u32(out, *i); }
        OpCode::OpMakeMap(i) => { out.push(20); pbck_w_u32(out, *i); }
        OpCode::OpGetIndex => out.push(21),
        OpCode::OpSetIndex => out.push(22),
        OpCode::OpTry(i) => { out.push(23); pbck_w_u32(out, *i); }
        OpCode::OpEndTry => out.push(24),
        OpCode::OpGetLocal(i) => { out.push(25); pbck_w_u32(out, *i); }
        OpCode::OpSetLocal(i) => { out.push(26); pbck_w_u32(out, *i); }
        OpCode::OpClosure { fn_const, captures } => {
            out.push(27);
            pbck_w_u32(out, *fn_const);
            pbck_w_u32(out, captures.len());
            for (is_local, idx) in captures {
                out.push(u8::from(*is_local));
                pbck_w_u32(out, *idx);
            }
        }
        OpCode::OpGetUpvalue(i) => { out.push(28); pbck_w_u32(out, *i); }
        OpCode::OpSetUpvalue(i) => { out.push(29); pbck_w_u32(out, *i); }
        OpCode::OpCloseUpvalue => out.push(30),
        OpCode::OpCall(i) => { out.push(31); pbck_w_u32(out, *i); }
        OpCode::OpTailCall(i) => { out.push(32); pbck_w_u32(out, *i); }
        OpCode::OpReturn => out.push(33),
        OpCode::OpDefer => out.push(34),
        OpCode::OpJump(i) => { out.push(35); pbck_w_u32(out, *i); }
        OpCode::OpJumpIfFalse(i) => { out.push(36); pbck_w_u32(out, *i); }
        OpCode::OpJumpIfTrue(i) => { out.push(37); pbck_w_u32(out, *i); }
        OpCode::OpNot => out.push(38),
        OpCode::OpNegate => out.push(39),
        OpCode::OpMod => out.push(40),
        OpCode::OpPow => out.push(41),
        OpCode::OpSafeGetAttr(i) => { out.push(42); pbck_w_u32(out, *i); }
        OpCode::OpSafeIndex => out.push(43),
        OpCode::OpResultUnwrap => out.push(44),
        OpCode::OpIsNil => out.push(45),
        OpCode::OpDup => out.push(46),
        OpCode::OpThrow => out.push(47),
    }
}

fn pbck_w_value(out: &mut Vec<u8>, v: &Value) {
    match v {
        Value::Number(n) => { out.push(0); out.extend_from_slice(&n.to_le_bytes()); }
        Value::Bool(b) => { out.push(1); out.push(u8::from(*b)); }
        Value::Nil => out.push(2),
        Value::Obj(o) => match o.as_ref() {
            Obj::String(s) => { out.push(3); pbck_w_str(out, s); }
            Obj::BigInt(n) => { out.push(4); pbck_w_str(out, &n.to_string()); }
            Obj::Rational(r) => { out.push(5); pbck_w_str(out, &r.to_string()); }
            Obj::Decimal(d) => { out.push(6); pbck_w_str(out, &d.to_string()); }
        },
        Value::Function(f) => { out.push(7); pbck_w_function(out, f); }
        // Closures/Builtins/Modules/Array/Map are runtime-only and never appear
        // as compile-time constants; encode as Nil to keep the format total.
        _ => out.push(2),
    }
}

fn pbck_w_function(out: &mut Vec<u8>, f: &Function) {
    match &f.name {
        Some(n) => { out.push(1); pbck_w_str(out, n); }
        None => out.push(0),
    }
    pbck_w_u32(out, f.arity);
    pbck_w_u32(out, f.min_arity);
    pbck_w_u32(out, f.defaults.len());
    for d in &f.defaults {
        pbck_w_value(out, d);
    }
    out.push(u8::from(f.is_vararg));
    match &f.param_types {
        Some(pts) => {
            out.push(1);
            pbck_w_u32(out, pts.len());
            for pt in pts {
                out.push(match pt {
                    ParamType::F64 => 0,
                    ParamType::I64 => 1,
                    ParamType::Bool => 2,
                });
            }
        }
        None => out.push(0),
    }
    pbck_w_chunk(out, &f.chunk);
}

fn pbck_w_chunk(out: &mut Vec<u8>, chunk: &Chunk) {
    pbck_w_u32(out, chunk.instructions.len());
    for op in &chunk.instructions {
        pbck_w_opcode(out, op);
    }
    pbck_w_u32(out, chunk.constants.len());
    for c in &chunk.constants {
        pbck_w_value(out, c);
    }
    pbck_w_u32(out, chunk.spans.len());
    for sp in &chunk.spans {
        pbck_w_u32(out, sp.start);
        pbck_w_u32(out, sp.end);
    }
    match &chunk.source_name {
        Some(n) => { out.push(1); pbck_w_str(out, n); }
        None => out.push(0),
    }
}

/// Serialize a compiled [`Chunk`] (with nested function constants) to the
/// lossless PBCK v1 container.
pub fn pbck_serialize(chunk: &Chunk) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(b"PBCK");
    out.push(PBCK_V1_VERSION);
    pbck_w_chunk(&mut out, chunk);
    out
}

struct PbckReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> PbckReader<'a> {
    /// Bytes left to read — the only legitimate upper bound on any declared count
    /// (each element needs ≥1 byte), so it caps untrusted-length pre-allocations.
    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }
    /// A safe `Vec::with_capacity` size for a declared element count `n`: never
    /// reserve more than the buffer could possibly contain, so a hostile count
    /// (e.g. 0xFFFFFFFF) can't trigger a multi-GB allocation / OOM abort.
    fn cap(&self, n: usize) -> usize {
        n.min(self.remaining())
    }
    fn u8(&mut self) -> Result<u8, String> {
        let b = *self.buf.get(self.pos).ok_or("pbck: unexpected end of input")?;
        self.pos += 1;
        Ok(b)
    }
    fn u32(&mut self) -> Result<usize, String> {
        let end = self.pos.checked_add(4).ok_or("pbck: offset overflow")?;
        let s = self.buf.get(self.pos..end).ok_or("pbck: truncated u32")?;
        let arr: [u8; 4] = s.try_into().map_err(|_| "pbck: bad u32".to_string())?;
        self.pos = end;
        Ok(u32::from_le_bytes(arr) as usize)
    }
    fn f64(&mut self) -> Result<f64, String> {
        let end = self.pos.checked_add(8).ok_or("pbck: offset overflow")?;
        let s = self.buf.get(self.pos..end).ok_or("pbck: truncated f64")?;
        let arr: [u8; 8] = s.try_into().map_err(|_| "pbck: bad f64".to_string())?;
        self.pos = end;
        Ok(f64::from_le_bytes(arr))
    }
    fn str(&mut self) -> Result<String, String> {
        let n = self.u32()?;
        let end = self.pos.checked_add(n).ok_or("pbck: offset overflow")?;
        let s = self.buf.get(self.pos..end).ok_or("pbck: truncated string")?;
        let out = String::from_utf8(s.to_vec()).map_err(|_| "pbck: invalid utf-8".to_string())?;
        self.pos = end;
        Ok(out)
    }
}

fn pbck_r_opcode(r: &mut PbckReader) -> Result<OpCode, String> {
    let tag = r.u8()?;
    Ok(match tag {
        0 => OpCode::OpConstant(r.u32()?),
        1 => OpCode::OpAdd,
        2 => OpCode::OpSub,
        3 => OpCode::OpMul,
        4 => OpCode::OpDiv,
        5 => OpCode::OpEq,
        6 => OpCode::OpNotEq,
        7 => OpCode::OpGt,
        8 => OpCode::OpGe,
        9 => OpCode::OpLt,
        10 => OpCode::OpLe,
        11 => OpCode::OpPop,
        12 => OpCode::OpPrint,
        13 => OpCode::OpDefineGlobal(r.u32()?),
        14 => OpCode::OpGetGlobal(r.u32()?),
        15 => OpCode::OpSetGlobal(r.u32()?),
        16 => OpCode::OpGetAttr(r.u32()?),
        17 => OpCode::OpSetAttr(r.u32()?),
        18 => OpCode::OpExport(r.u32()?),
        19 => OpCode::OpMakeArray(r.u32()?),
        20 => OpCode::OpMakeMap(r.u32()?),
        21 => OpCode::OpGetIndex,
        22 => OpCode::OpSetIndex,
        23 => OpCode::OpTry(r.u32()?),
        24 => OpCode::OpEndTry,
        25 => OpCode::OpGetLocal(r.u32()?),
        26 => OpCode::OpSetLocal(r.u32()?),
        27 => {
            let fn_const = r.u32()?;
            let n = r.u32()?;
            let mut captures = Vec::with_capacity(r.cap(n));
            for _ in 0..n {
                let is_local = r.u8()? != 0;
                let idx = r.u32()?;
                captures.push((is_local, idx));
            }
            OpCode::OpClosure { fn_const, captures }
        }
        28 => OpCode::OpGetUpvalue(r.u32()?),
        29 => OpCode::OpSetUpvalue(r.u32()?),
        30 => OpCode::OpCloseUpvalue,
        31 => OpCode::OpCall(r.u32()?),
        32 => OpCode::OpTailCall(r.u32()?),
        33 => OpCode::OpReturn,
        34 => OpCode::OpDefer,
        35 => OpCode::OpJump(r.u32()?),
        36 => OpCode::OpJumpIfFalse(r.u32()?),
        37 => OpCode::OpJumpIfTrue(r.u32()?),
        38 => OpCode::OpNot,
        39 => OpCode::OpNegate,
        40 => OpCode::OpMod,
        41 => OpCode::OpPow,
        42 => OpCode::OpSafeGetAttr(r.u32()?),
        43 => OpCode::OpSafeIndex,
        44 => OpCode::OpResultUnwrap,
        45 => OpCode::OpIsNil,
        46 => OpCode::OpDup,
        47 => OpCode::OpThrow,
        other => return Err(format!("pbck: unknown opcode tag {other}")),
    })
}

fn pbck_r_value(r: &mut PbckReader, depth: u32) -> Result<Value, String> {
    use std::str::FromStr;
    let tag = r.u8()?;
    Ok(match tag {
        0 => Value::Number(r.f64()?),
        1 => Value::Bool(r.u8()? != 0),
        2 => Value::Nil,
        3 => alloc_string(r.str()?),
        4 => {
            let s = r.str()?;
            alloc_bigint(BigInt::from_str(&s).map_err(|_| "pbck: bad bigint".to_string())?)
        }
        5 => {
            let s = r.str()?;
            alloc_rational(BigRational::from_str(&s).map_err(|_| "pbck: bad rational".to_string())?)
        }
        6 => return Err("pbck: decimal constants are not supported".to_string()),
        7 => Value::Function(Rc::new(pbck_r_function(r, depth)?)),
        other => return Err(format!("pbck: unknown value tag {other}")),
    })
}

fn pbck_r_function(r: &mut PbckReader, depth: u32) -> Result<Function, String> {
    if depth >= PBCK_MAX_DEPTH {
        return Err(format!(
            "pbck: function nesting too deep (exceeds {PBCK_MAX_DEPTH}): refusing to parse to avoid a stack overflow"
        ));
    }
    let name = if r.u8()? != 0 { Some(r.str()?) } else { None };
    let arity = r.u32()?;
    let min_arity = r.u32()?;
    let n_defaults = r.u32()?;
    let mut defaults = Vec::with_capacity(r.cap(n_defaults));
    for _ in 0..n_defaults {
        defaults.push(pbck_r_value(r, depth + 1)?);
    }
    let is_vararg = r.u8()? != 0;
    let param_types = if r.u8()? != 0 {
        let n = r.u32()?;
        let mut pts = Vec::with_capacity(r.cap(n));
        for _ in 0..n {
            pts.push(match r.u8()? {
                0 => ParamType::F64,
                1 => ParamType::I64,
                2 => ParamType::Bool,
                other => return Err(format!("pbck: bad param type {other}")),
            });
        }
        Some(pts)
    } else {
        None
    };
    let chunk = pbck_r_chunk(r, depth + 1)?;
    Ok(Function {
        name,
        arity,
        min_arity,
        defaults,
        is_vararg,
        chunk,
        param_types,
    })
}

fn pbck_r_chunk(r: &mut PbckReader, depth: u32) -> Result<Chunk, String> {
    let n_ins = r.u32()?;
    let mut instructions = Vec::with_capacity(r.cap(n_ins));
    for _ in 0..n_ins {
        instructions.push(pbck_r_opcode(r)?);
    }
    let n_const = r.u32()?;
    let mut constants = Vec::with_capacity(r.cap(n_const));
    for _ in 0..n_const {
        constants.push(pbck_r_value(r, depth)?);
    }
    let n_spans = r.u32()?;
    let mut spans = Vec::with_capacity(r.cap(n_spans));
    for _ in 0..n_spans {
        let start = r.u32()?;
        let end = r.u32()?;
        spans.push(crate::chunk::Span { start, end });
    }
    let source_name = if r.u8()? != 0 {
        Some(Rc::new(r.str()?))
    } else {
        None
    };
    Ok(Chunk {
        instructions,
        constants,
        spans,
        source_name,
        source: None,
    })
}

/// Load a [`Chunk`] from a PBCK v1 container produced by [`pbck_serialize`].
pub fn pbck_deserialize(buf: &[u8]) -> Result<Chunk, String> {
    if buf.len() < 5 || &buf[0..4] != b"PBCK" {
        return Err("pbck: bad magic (not a PBCK file)".to_string());
    }
    let version = buf[4];
    if version != PBCK_V1_VERSION {
        return Err(format!(
            "pbck: unsupported version {version} (this build reads v{PBCK_V1_VERSION})"
        ));
    }
    let mut r = PbckReader { buf, pos: 5 };
    pbck_r_chunk(&mut r, 0)
}

#[cfg(test)]
mod pbck_roundtrip_tests {
    use super::*;
    use chunk::Span;

    fn sample_chunk() -> Chunk {
        // A chunk exercising several opcode shapes + a nested function constant.
        let mut inner = Chunk::default();
        let sp = Span::default();
        let c = inner.add_const(Value::Number(7.0));
        inner.write(OpCode::OpConstant(c), sp);
        inner.write(OpCode::OpReturn, sp);
        let func = Function {
            name: Some("inner".to_string()),
            arity: 1,
            min_arity: 1,
            defaults: vec![Value::Number(1.0), alloc_string("x")],
            is_vararg: false,
            chunk: inner,
            param_types: Some(vec![ParamType::F64, ParamType::I64]),
        };

        let mut top = Chunk::default();
        let fc = top.add_const(Value::Function(Rc::new(func)));
        let s = top.add_const(alloc_string("hi"));
        top.write(OpCode::OpClosure { fn_const: fc, captures: vec![(true, 0), (false, 2)] }, sp);
        top.write(OpCode::OpConstant(s), sp);
        top.write(OpCode::OpPrint, sp);
        top.write(OpCode::OpJumpIfFalse(2), sp);
        top.write(OpCode::OpReturn, sp);
        top.source_name = Some(Rc::new("sample.sapp".to_string()));
        top
    }

    #[test]
    fn pbck_roundtrip_is_byte_stable() {
        let chunk = sample_chunk();
        let b1 = pbck_serialize(&chunk);
        assert!(b1.starts_with(b"PBCK"));
        let restored = pbck_deserialize(&b1).expect("deserialize must succeed");
        let b2 = pbck_serialize(&restored);
        assert_eq!(b1, b2, "serialize∘deserialize must be byte-stable (lossless)");
    }

    #[test]
    fn pbck_preserves_structure() {
        let chunk = sample_chunk();
        let restored = pbck_deserialize(&pbck_serialize(&chunk)).unwrap();
        assert_eq!(restored.instructions.len(), chunk.instructions.len());
        assert_eq!(restored.constants.len(), chunk.constants.len());
        assert_eq!(restored.spans.len(), chunk.spans.len());
        assert_eq!(
            restored.source_name.as_deref().map(|s| s.as_str()),
            Some("sample.sapp")
        );
        // nested function constant survived with its own chunk + defaults
        match &restored.constants[0] {
            Value::Function(f) => {
                assert_eq!(f.name.as_deref(), Some("inner"));
                assert_eq!(f.arity, 1);
                assert_eq!(f.defaults.len(), 2);
                assert_eq!(f.chunk.instructions.len(), 2);
                assert!(matches!(f.param_types.as_deref(), Some([ParamType::F64, ParamType::I64])));
            }
            other => panic!("expected function constant, got {other:?}"),
        }
    }

    #[test]
    fn pbck_rejects_bad_magic() {
        assert!(pbck_deserialize(b"NOPE.....").is_err());
    }

    // ── adversarial / malformed input (untrusted .pbck is a DoS surface) ──

    fn pbck_header() -> Vec<u8> {
        // A valid 5-byte header ("PBCK" + version), reused to craft hostile bodies.
        pbck_serialize(&Chunk::default())[0..5].to_vec()
    }
    fn u32le(v: u32) -> [u8; 4] {
        v.to_le_bytes()
    }

    #[test]
    fn pbck_rejects_huge_length_without_oom() {
        // A 9-byte file claiming ~4.29 billion instructions must NOT pre-allocate
        // ~tens of GB (Vec::with_capacity on an attacker-controlled count) — it
        // must fail fast with an error, bounded by the actual buffer size.
        let mut buf = pbck_header();
        buf.extend_from_slice(&u32le(u32::MAX)); // n_ins = 0xFFFFFFFF
        assert!(
            pbck_deserialize(&buf).is_err(),
            "a huge declared count must error, not allocate gigabytes"
        );
    }

    #[test]
    fn pbck_rejects_huge_count_for_each_section() {
        // Same for constants and spans (each reached after a 0-count predecessor).
        // constants: n_ins=0, n_const=MAX
        let mut b = pbck_header();
        b.extend_from_slice(&u32le(0));
        b.extend_from_slice(&u32le(u32::MAX));
        assert!(pbck_deserialize(&b).is_err());
        // spans: n_ins=0, n_const=0, n_spans=MAX
        let mut b = pbck_header();
        b.extend_from_slice(&u32le(0));
        b.extend_from_slice(&u32le(0));
        b.extend_from_slice(&u32le(u32::MAX));
        assert!(pbck_deserialize(&b).is_err());
    }

    #[test]
    fn pbck_rejects_deeply_nested_functions_without_stack_overflow() {
        // Build a chain of ~100k nested function constants by hand. The reader
        // recurses function -> chunk -> value -> function; without a depth guard
        // this overflows the native stack (uncatchable abort). It must instead
        // return an error.
        let n = 100_000usize;
        // Per-level wrapper around an inner value:
        //   value tag 7 (function)
        //   function: name(0) arity(u32 0) min_arity(0) n_defaults(0)
        //             is_vararg(0) param_types_present(0)
        //   chunk:    n_ins(0) n_const(1)  <inner value...>  [suffix later]
        let mut prefix = vec![7u8, 0u8];
        prefix.extend_from_slice(&u32le(0)); // arity
        prefix.extend_from_slice(&u32le(0)); // min_arity
        prefix.extend_from_slice(&u32le(0)); // n_defaults
        prefix.push(0); // is_vararg
        prefix.push(0); // param_types: none
        prefix.extend_from_slice(&u32le(0)); // chunk.n_ins
        prefix.extend_from_slice(&u32le(1)); // chunk.n_const = 1 (the inner value)
        // chunk suffix after the inner constant: n_spans(0) source_name(none)
        let mut suffix = u32le(0).to_vec();
        suffix.push(0);

        let mut buf = pbck_header();
        buf.extend_from_slice(&u32le(0)); // top chunk n_ins
        buf.extend_from_slice(&u32le(1)); // top chunk n_const = 1 (the chain)
        for _ in 0..n {
            buf.extend_from_slice(&prefix);
        }
        buf.push(2); // innermost value = Nil (tag 2), terminating the chain
        for _ in 0..n {
            buf.extend_from_slice(&suffix);
        }
        buf.extend_from_slice(&u32le(0)); // top chunk n_spans
        buf.push(0); // top chunk source_name: none

        assert!(
            pbck_deserialize(&buf).is_err(),
            "deeply nested function constants must error, not overflow the stack"
        );
    }
}
