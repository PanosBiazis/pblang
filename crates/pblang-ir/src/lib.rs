//! Intermediate representation shared between the pblang compiler, VM, and
//! out-of-process consumers (FFI, AOT, JIT).
//!
//! This crate is the **interface boundary** between the front end (parser
//! + typecker, in `pblang-core`) and the runtime (`pblang-vm`, plus the
//! optional `pblang-cranelift` JIT/AOT and the `pblang-ffi` embedding C ABI).
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
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
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
/// tracking. The default trait impls grant full access — the
/// **`pblang-ffi`** embedding crate provides a sandbox-by-default
/// wrapper. See `docs/Capability_model.md` for the host-contract.
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
    /// Whether reading process environment variables (`env_get`) is allowed.
    /// Defaults to true (env reads are benign — PATH/HOME/etc.), but a sandbox
    /// can disable it to prevent secret exfiltration (e.g. `OPENAI_API_KEY`).
    fn allow_env(&self) -> bool {
        true
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
                    ArithOp::Mod => {
                        return Err("modulo (%) is not defined for rationals".into())
                    }
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
