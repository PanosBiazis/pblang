use pblang_ir::chunk::{Chunk, OpCode, Span};
use pblang_ir::{
    alloc_string, ArithOp, BuiltinFn, Closure, Function, Module, Obj, Upvalue, Value, VmHost,
    VmLimits as IrVmLimits,
};
pub use pblang_ir::{FrameTrace, RuntimeError};

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{
    io::Read,
    io::Write,
    net::{TcpListener, TcpStream},
};

// Import our new modules
use crate::concurrency::{ThreadPool, VmHandleRegistry};
// SpawnWireFunction / SpawnWireClosure / SpawnWireValue / VmChannelBuf are
// reached only from inside vm/spawn_wire.rs and vm/spawn.rs.
use crate::memory_safety::{BorrowId, BorrowType, MemoryId, MemorySafetyError, MemoryTracker};

// TLS types still referenced by `StreamKind` / `NetResource` defs in this
// file. The TCP/TLS builtin closures moved to vm/net.rs and bring their own
// rustls / webpki-roots / rustls-native-certs imports.
use rustls::{ClientConnection, ServerConfig, ServerConnection, StreamOwned};

use libloading::Library;
use rusqlite::Connection;

// Submodules of the VM. Each holds a self-contained slice of vm.rs that
// could be reviewed independently — extracted to keep this file readable.
mod ai;
mod arrays;
mod atomics;
mod bitwise;
mod builtins_table;
mod crypto;
mod db;
mod encoding;
mod env;
mod errors;
mod ffi;
mod ffi_dispatch;
mod fs;
mod gc;
#[cfg(feature = "gpu")]
mod gpu;
mod handles;
mod http;
mod json;
mod maps;
mod math;
mod memory;
mod net;
mod os;
mod parse;
mod polyglot;
mod predicates;
mod quantum;
mod re;
mod runtime;
mod spawn;
mod spawn_wire;
mod strings;
mod sync;
mod tensor;
mod time;

use gc::{gc_register_array, gc_register_map, gc_register_module};
// handles::{value_to_handle, vm_handles}, spawn_wire::*, dispatch_ffi /
// parse_ffi_sig — all consumed only inside their sibling vm/* submodules
// now (handles by atomics/sync/memory/ffi/spawn, spawn_wire by spawn).
// db_table / ffi_table / net_table accessors are reached only via the
// with_*_table wrappers; no need to import the raw fns here.
// DB_NEXT/with_db_table → used by vm/db.rs directly.
// NET_NEXT/with_net_table → used by vm/net.rs directly.
// Realm-owned FFI accessors are used by vm/ffi.rs and vm/polyglot.rs.
// spawn_function_from_wire / spawn_closure_to_wire are only called from inside
// spawn_wire.rs (by spawn_value_from_wire / spawn_value_to_wire); no need to
// re-import them here.
pub(super) static ARRAY_ALLOCS: AtomicUsize = AtomicUsize::new(0);
pub(super) static MAP_ALLOCS: AtomicUsize = AtomicUsize::new(0);
pub(super) static MODULE_ALLOCS: AtomicUsize = AtomicUsize::new(0);

// Container tracing / cycle collection moved to vm/gc.rs (imported above).
// `alloc_array` / `alloc_map` / `import_module` register their new container
// with the weak registries via `gc_register_*`. The mark-and-sweep entry
// point is `VM::gc_collect_cycles_impl`, an `impl VM` block in vm/gc.rs.

pub(super) fn alloc_array(items: Vec<Value>) -> Value {
    ARRAY_ALLOCS.fetch_add(1, Ordering::Relaxed);
    let rc = Rc::new(RefCell::new(items));
    gc_register_array(&rc);
    Value::Array(rc)
}

pub(super) fn alloc_map(items: HashMap<String, Value>) -> Value {
    MAP_ALLOCS.fetch_add(1, Ordering::Relaxed);
    let rc = Rc::new(RefCell::new(items));
    gc_register_map(&rc);
    Value::Map(rc)
}

#[derive(Debug)]
pub(super) enum NetResource {
    Listener(TcpListener),
    TlsListener(TcpListener, Arc<ServerConfig>),
    Stream(Box<NetStream>),
}

// StreamKind abstracts over plain and TLS streams for transparent tcp_read/tcp_write.
// `pub(super)` so the net-builtins submodule can name the variants when it
// constructs / destructures Stream values.
pub(super) enum StreamKind {
    Plain(TcpStream),
    TlsServer(StreamOwned<ServerConnection, TcpStream>),
    TlsClient(StreamOwned<ClientConnection, TcpStream>),
}

impl std::fmt::Debug for StreamKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamKind::Plain(_) => write!(f, "StreamKind::Plain(...)"),
            StreamKind::TlsServer(_) => write!(f, "StreamKind::TlsServer(...)"),
            StreamKind::TlsClient(_) => write!(f, "StreamKind::TlsClient(...)"),
        }
    }
}

impl Read for StreamKind {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            StreamKind::Plain(s) => s.read(buf),
            StreamKind::TlsServer(s) => s.read(buf),
            StreamKind::TlsClient(s) => s.read(buf),
        }
    }
}

impl Write for StreamKind {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            StreamKind::Plain(s) => s.write(buf),
            StreamKind::TlsServer(s) => s.write(buf),
            StreamKind::TlsClient(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            StreamKind::Plain(s) => s.flush(),
            StreamKind::TlsServer(s) => s.flush(),
            StreamKind::TlsClient(s) => s.flush(),
        }
    }
}

impl StreamKind {
    fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        match self {
            StreamKind::Plain(s) => s.local_addr(),
            StreamKind::TlsServer(s) => s.get_ref().local_addr(),
            StreamKind::TlsClient(s) => s.get_ref().local_addr(),
        }
    }

    #[allow(dead_code)]
    fn set_read_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
        match self {
            StreamKind::Plain(s) => s.set_read_timeout(dur),
            StreamKind::TlsServer(s) => s.get_ref().set_read_timeout(dur),
            StreamKind::TlsClient(s) => s.get_ref().set_read_timeout(dur),
        }
    }

    #[allow(dead_code)]
    fn set_write_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
        match self {
            StreamKind::Plain(s) => s.set_write_timeout(dur),
            StreamKind::TlsServer(s) => s.get_ref().set_write_timeout(dur),
            StreamKind::TlsClient(s) => s.get_ref().set_write_timeout(dur),
        }
    }
}

// `pub(super)` so the `NetResource::Stream(Box<NetStream>)` variant — which
// is itself `pub(super)` — doesn't trip the "type is more private than the
// item that references it" lint after the handles.rs extraction.
#[derive(Debug)]
pub(super) struct NetStream {
    inner: StreamKind,
    buf: Vec<u8>,
}

// Resource enums live here because their variants reference platform /
// library types (TcpListener, rustls config, Library, Connection) — those
// imports stay with vm.rs. The static OnceLock tables, NEXT counters,
// with_*_table helpers, value_to_handle, and vm_handles all moved to
// vm/handles.rs (imported above).
//
// parse_ffi_sig + dispatch_ffi + c_str_to_value moved to vm/ffi_dispatch.rs.

#[derive(Debug, Clone)]
pub(super) enum FfiResource {
    Library(Rc<Library>),
}

#[derive(Debug)]
pub(super) enum DbResource {
    Sqlite(Connection),
}

#[derive(Debug)]
struct CallFrame {
    closure: Rc<Closure>,
    ip: usize,
    stack_base: usize,
    module_result: Option<Value>,
    defers: Vec<Value>, // closures to run on scope exit
    is_returning: bool,
    is_defer_thunk: bool,
    /// Defer closure running while an exception unwinds (see `UnwindState`).
    is_unwind_thunk: bool,
}

/// An exception that is unwinding frames while their deferred closures run.
///
/// Frames between the failure point and the target handler (or every frame,
/// when the exception is uncaught) are popped first and their defers are
/// collected in execution order (innermost frame first, LIFO within a frame).
/// Each defer then runs as an ordinary call frame marked `is_unwind_thunk`.
/// An exception escaping a defer is discarded after that defer's own frames
/// have unwound, so the original error is preserved and older defers still run.
struct UnwindState {
    error: RuntimeError,
    handler: Option<TryHandler>,
    /// Remaining defers; `pop()` yields the next one to run.
    pending: Vec<Value>,
    /// Index of the currently running unwind-thunk frame, if any.
    thunk_frame: Option<usize>,
}

#[derive(Debug, Clone)]
struct TryHandler {
    frame_index: usize,
    target_ip: usize,
    stack_len: usize,
}

/// Configuration for which stdlib builtins to register. Used for embedding and sandboxing.
#[derive(Debug, Clone)]
pub struct StdlibConfig {
    pub core: bool,
    pub fs: bool,
    pub net: bool,
    pub http: bool,
    pub json: bool,
    pub crypto: bool,
    pub sqlite: bool,
    pub ffi: bool,
    /// Explicitly bypass the FFI library/symbol allowlists. This is never
    /// implied by `ffi: true`; only fully trusted embedding profiles set it.
    pub ffi_allow_all: bool,
    /// Exact/canonical library paths allowed when FFI is enabled.
    pub ffi_allowed_libs: HashSet<String>,
    /// Exact exported symbols allowed when FFI is enabled.
    pub ffi_allowed_symbols: HashSet<String>,
    pub exec: bool,
    pub gfx: bool,
    /// If true, source modules may be loaded from configured host paths.
    pub import: bool,
    /// If true, spawn/join builtins are available; if false, spawn returns "spawn not allowed".
    pub spawn: bool,
    /// If true, `env_get` may read process environment variables; if false it is
    /// denied (prevents secret exfiltration in a sandbox). Defaults to true.
    pub env: bool,
    /// If true, `os_exit()` may call `std::process::exit` and terminate the
    /// process. Defaults to true (correct for the CLI). Embedders set it false
    /// so an untrusted script cannot kill the host; `os_exit` then errors.
    pub process_exit: bool,
}

impl Default for StdlibConfig {
    fn default() -> Self {
        Self {
            core: true,
            fs: true,
            net: true,
            http: true,
            json: true,
            crypto: true,
            sqlite: true,
            ffi: false,
            ffi_allow_all: false,
            ffi_allowed_libs: HashSet::new(),
            ffi_allowed_symbols: HashSet::new(),
            exec: false,
            gfx: true,
            import: true,
            spawn: true,
            env: true,
            process_exit: true,
        }
    }
}

impl StdlibConfig {
    /// Returns true if the builtin with this name is in an enabled category.
    ///
    /// AUDIT NOTE #14: This is a *future hook* for strict-mode registration (where
    /// disabled builtins are not even visible as globals). It is **not currently
    /// wired** into `install_builtins_into` — the live sandbox is the per-builtin
    /// **call-time** capability check (`allow_fs`/`allow_net`/`allow_exec`/`allow_env`/…),
    /// plus registration-time gating for the `gfx` builtins (which lack a call-time
    /// check). All other builtins stay registered so stdlib modules that import
    /// them internally keep working; the call-time check denies access when the
    /// capability is off. See `install_builtins_into` for the authoritative policy.
    #[allow(dead_code)]
    pub fn allows(&self, name: &str) -> bool {
        match name {
            "clock"
            | "now_ms"
            | "now_iso"
            | "type"
            | "str"
            | "num_sqrt"
            | "num_pow"
            | "num_ln"
            | "num_exp"
            | "num_atan2"
            | "num_mod"
            | "num_floor"
            | "num_ceil"
            | "parse_number"
            | "parse_bool"
            | "read_line"
            | "len"
            | "array_push"
            | "array_pop"
            | "map_keys"
            | "map_has"
            | "map_remove"
            | "gc_stats"
            | "gc_collect"
            | "str_contains"
            | "str_starts_with"
            | "str_trim"
            | "str_split"
            | "regex_match"
            | "regex_find"
            | "regex_find_all"
            | "regex_replace"
            | "regex_replace_all"
            | "regex_split"
            | "regex_capture"
            | "str_format"
            | "str_repeat"
            | "str_pad_left"
            | "str_pad_right"
            | "str_upper"
            | "str_lower"
            | "str_join"
            | "platform"
            | "array_unique"
            | "array_zip"
            | "array_chunk"
            | "array_flatten"
            | "array_take"
            | "array_drop"
            | "range"
            | "enumerate"
            | "sum"
            | "min_of"
            | "max_of"
            | "product"
            | "clamp"
            | "is_number"
            | "is_string"
            | "is_array"
            | "is_map"
            | "is_bool"
            | "is_fn"
            | "is_nil"
            | "env_get"
            | "env_set"
            | "str_replace_all"
            | "tensor_add"
            | "tensor_sub"
            | "tensor_mul"
            | "tensor_scale"
            | "tensor_matmul"
            | "tensor_backend"
            | "tensor_dot"
            | "tensor_relu"
            | "tensor_sigmoid"
            | "tensor_tanh"
            | "tensor_softmax"
            | "tensor_sum"
            | "tensor_mean"
            | "tensor_argmax"
            | "tensor_transpose2d"
            | "tensor_gelu"
            | "tensor_leaky_relu"
            | "tensor_log_softmax"
            | "tensor_layer_norm"
            | "tensor_batch_norm"
            | "tensor_embedding_lookup"
            | "tensor_cross_entropy"
            | "tensor_conv2d"
            | "tensor_maxpool2d"
            | "tensor_avgpool2d"
            | "quantum_simulate" => self.core,
            "fs_read" | "fs_write" | "fs_append" | "fs_remove" | "fs_rename" | "fs_copy"
            | "fs_read_bytes" | "fs_write_bytes" | "fs_exists" | "fs_mkdirs" | "fs_listdir"
            | "path_join" | "path_dirname" | "path_basename" | "cwd" => self.fs,
            "sleep_ms"
            | "exec"
            | "proc_spawn_wait"
            | "os_which"
            | "os_signal_trap"
            | "os_signal_pending"
            | "os_signal_poll"
            | "os_signal_handler_get"
            | "quantum_run"
            | "quantum_statevector"
            | "quantum_backends"
            | "polyglot_exec"
            | "polyglot_exec_with_input"
            | "polyglot_exec_compiled"
            | "polyglot_detect"
            | "polyglot_compile_cached"
            | "polyglot_cache_clear"
            | "polyglot_cache_info" => self.exec,
            "sha256" | "random_bytes" | "random_float" | "constant_time_eq" | "base64_encode"
            | "base64_decode" | "hmac_sha256" | "hex_encode" | "hex_decode" | "url_encode"
            | "url_decode" => self.crypto,
            "json_parse" | "json_stringify" => self.json,
            "http_get" | "http_post" | "ai_chat" | "ai_embed" => self.http,
            "tcp_listen" | "tcp_connect" | "tcp_accept" | "tcp_read" | "tcp_read_until" | "tcp_write"
            | "tcp_close" | "tcp_local_addr" | "tcp_listen_tls" | "tcp_connect_tls"
            | "tcp_wrap_tls" | "http_parse_request" | "http_response" | "http_response_ct"
            | "http_response_json" | "ai_token_count" => self.net,
            "onnx_infer" => self.exec,
            "ffi_load" | "ffi_call" | "ffi_close" | "ffi_call_typed" => self.ffi,
            "sqlite_open"
            | "sqlite_close"
            | "sqlite_exec"
            | "sqlite_query"
            | "sqlite_last_insert_rowid" => self.sqlite,
            "spawn" | "join" => self.spawn,
            "import" => self.import,
            _ => self.gfx, // gfx builtins from pblang_graphics use their own names
        }
    }
}

/// FFI policy for sandboxing: allowlist libraries and/or symbols.
///
/// Default is `allow_all: false` — all FFI calls are denied unless explicitly permitted
/// via `allowed_libs` and `allowed_symbols`. This is the secure default (principle of
/// least privilege). Set `allow_all: true` only for trusted embedding contexts.
#[derive(Debug, Clone, Default)]
pub struct FfiPolicy {
    /// If true, allow any library path and any symbol (no allowlist check).
    /// Defaults to `false` — secure by default.
    pub allow_all: bool,
    /// Allowed library paths (e.g. "./safe.dll"). Checked on ffi_load when allow_all is false.
    pub allowed_libs: HashSet<String>,
    /// Allowed symbol names (e.g. "add", "multiply"). Checked on ffi_call when allow_all is false.
    pub allowed_symbols: HashSet<String>,
}

/// Resource limits for sandboxing (anti-infinite-loop, stack overflow, IO, memory).
#[derive(Debug, Clone, Default)]
pub struct VmLimits {
    /// Max VM instructions per run. Prevents infinite loops.
    pub max_instructions: Option<u64>,
    /// Max call stack depth (number of frames).
    pub max_stack_depth: Option<usize>,
    /// Reserved heap budget; intrinsic VM allocations are not hard-accounted by this field.
    pub max_memory_bytes: Option<usize>,
    /// Timeout in ms for IO builtins (e.g. HTTP, TCP read, fs read). Not yet enforced by all builtins.
    pub io_timeout_ms: Option<u64>,
    /// Maximum byte length of JSON accepted from **external** ingress points
    /// (FFI calls, AI provider responses, package registry index). The
    /// `json_parse` builtin called from pblang code is *not* gated by this
    /// — only external bytes are. Default applied by helpers when None: 10 MB.
    pub max_external_json_bytes: Option<usize>,
}

/// Hardcoded fallback when `VmLimits.max_external_json_bytes` is None.
/// 10 MB matches the IDE-server body cap and is large enough for any
/// realistic package index / AI completion / FFI argument set.
pub const DEFAULT_MAX_EXTERNAL_JSON_BYTES: usize = 10 * 1024 * 1024;

/// Helper for JSON parsing at trust boundaries. Returns Err with a descriptive
/// message when input exceeds the cap; caller maps to its own error type.
/// `source` is a short label (e.g. "ai_chat options") that appears in the
/// error message.
pub fn bounded_json_parse<T: serde::de::DeserializeOwned>(
    source: &str,
    raw: &[u8],
    limits: Option<&VmLimits>,
) -> Result<T, String> {
    let cap = limits
        .and_then(|l| l.max_external_json_bytes)
        .unwrap_or(DEFAULT_MAX_EXTERNAL_JSON_BYTES);
    if raw.len() > cap {
        return Err(format!(
            "{source}: JSON payload of {} bytes exceeds cap of {} bytes (set VmLimits.max_external_json_bytes to override)",
            raw.len(),
            cap
        ));
    }
    serde_json::from_slice::<T>(raw).map_err(|e| format!("{source}: invalid JSON: {e}"))
}

#[allow(dead_code)]
pub struct VM {
    stack: Vec<Value>,
    //globals: HashMap<String, Value>,
    pub module: Rc<RefCell<Module>>,
    builtins: Rc<RefCell<Module>>,
    module_cache: Rc<RefCell<HashMap<String, Rc<RefCell<Module>>>>>,
    import_stack: Rc<RefCell<Vec<String>>>,
    frames: Vec<CallFrame>,
    open_upvalues: Vec<Rc<RefCell<Upvalue>>>,
    handlers: Vec<TryHandler>,
    /// Exceptions currently unwinding through deferred cleanup (innermost last).
    unwinds: Vec<UnwindState>,
    /// Set by `do_return` when an unwind thunk finished; the run loop resumes the unwind.
    unwind_resume: bool,
    /// Capability config for sandboxing; None = minimal VM (no builtins).
    stdlib_config: Option<StdlibConfig>,
    /// Optional resource limits (instruction count, stack depth).
    limits: Option<VmLimits>,
    /// Optional FFI allowlist; when Some and allow_all is false, only listed libs/symbols are allowed.
    ffi_policy: Option<FfiPolicy>,
    /// Shared with import initialization VMs only; final owner drop unloads libraries.
    ffi_resources: Rc<RefCell<HashMap<u64, FfiResource>>>,
    /// Memory safety tracker for Rust-like ownership and borrowing
    memory_tracker: MemoryTracker,
    /// Maps raw Rc pointers of tracked arrays/maps to their MemoryId for mutation checks.
    object_memory_map: RefCell<HashMap<usize, MemoryId>>,
    /// Concurrency primitives for thread-safe operations
    thread_pool: Option<ThreadPool>,
    /// Spawn/channel/sync handle tables (shared with child VMs from `spawn`).
    handle_registry: Arc<VmHandleRegistry>,
    /// Optional pre-seeded stdin lines (used by IDE / embeddings).
    stdin_lines: std::collections::VecDeque<String>,
    /// Per-VM registry of OS signal handler closures, keyed by canonical name
    /// (e.g. `"SIGINT"`). Populated by `os_signal_trap()` and consulted by
    /// `os_signal_handler_get()`. Handlers are *not* invoked from the libc
    /// signal handler — pblang programs explicitly drain pending counters via
    /// `os_signal_poll()` / `os_signal_pending()` and call the handler from
    /// regular bytecode context (see `stdlib/signal.sapp`).
    signal_handlers: Rc<RefCell<HashMap<String, Value>>>,
    /// Cranelift JIT for numeric functions (opt-in via the `PBLANG_JIT` env var). Hot,
    /// eligible functions (numeric arithmetic/loops/recursion) are compiled to native
    /// code; anything else falls back to the bytecode interpreter. Off by default.
    jit: Option<pblang_cranelift::JitCache>,
    jit_state: HashMap<usize, JitSlot>,
    jit_enabled: bool,
}

/// Per-function JIT status, keyed by the function's `Rc` pointer in `VM::jit_state`.
#[derive(Clone, Copy)]
enum JitStatus {
    Unknown,
    Compiled,
    Ineligible,
}
struct JitSlot {
    count: u32,
    status: JitStatus,
    func: Option<pblang_cranelift::JitFn>,
}

/// Compile a function after it has been called this many times (amortizes the Cranelift
/// compile cost over a hot function's many invocations).
const JIT_THRESHOLD: u32 = 8;

// `GcMarks` moved to vm/gc.rs alongside the mark-and-sweep methods.

/// Get a raw pointer-based hash key for array/map Values to track in object_memory_map.
fn object_addr(v: &Value) -> Option<usize> {
    match v {
        Value::Array(arr) => Some(Rc::as_ptr(arr) as usize),
        Value::Map(m) => Some(Rc::as_ptr(m) as usize),
        _ => None,
    }
}

/// Track an array/map value in the object_memory_map for mutation safety checks.
fn register_object_memory(vm: &VM, value: &Value, memory_id: MemoryId) {
    if let Some(addr) = object_addr(value) {
        vm.object_memory_map.borrow_mut().insert(addr, memory_id);
    }
}

/// Unregister an object from the memory map when deallocated.
fn unregister_object_memory(vm: &VM, value: &Value) {
    if let Some(addr) = object_addr(value) {
        vm.object_memory_map.borrow_mut().remove(&addr);
    }
}

impl VM {
    fn polyglot_blocks_to_values(blocks: Vec<pblang_core::PolyglotBlock>) -> (Value, Value) {
        let mut out: Vec<Value> = Vec::with_capacity(blocks.len());
        let mut by_name = HashMap::<String, Value>::new();
        let mut by_lang = HashMap::<String, Vec<Value>>::new();

        for b in blocks {
            let mut m = HashMap::<String, Value>::new();
            let lang_s = b.lang;
            m.insert("lang".into(), alloc_string(lang_s.clone()));
            m.insert(
                "name".into(),
                b.name.clone().map(alloc_string).unwrap_or(Value::Nil),
            );
            m.insert(
                "args".into(),
                b.args.clone().map(alloc_string).unwrap_or(Value::Nil),
            );
            let mut attrs = HashMap::<String, Value>::new();
            for (k, v) in b.attrs.into_iter() {
                attrs.insert(k, alloc_string(v));
            }
            m.insert("attrs".into(), alloc_map(attrs));
            m.insert("code".into(), alloc_string(b.code));
            m.insert("start".into(), Value::Number(b.span.start as f64));
            m.insert("end".into(), Value::Number(b.span.end as f64));
            let block_v = alloc_map(m);
            if let Some(n) = b.name {
                // Keep the first block with that name (deterministic).
                by_name.entry(n).or_insert_with(|| block_v.clone());
            }
            by_lang.entry(lang_s).or_default().push(block_v.clone());
            out.push(block_v);
        }

        let blocks_v = alloc_array(out);

        let mut by_lang_v = HashMap::<String, Value>::new();
        for (k, vv) in by_lang {
            by_lang_v.insert(k, alloc_array(vv));
        }

        let mut poly = HashMap::<String, Value>::new();
        poly.insert("blocks".into(), blocks_v.clone());
        poly.insert("by_name".into(), alloc_map(by_name));
        poly.insert("by_lang".into(), alloc_map(by_lang_v));

        (blocks_v, alloc_map(poly))
    }

    // GC methods (gc_collect_cycles_impl, gc_mark_*, gc_sweep_*) moved to vm/gc.rs

    fn get_global_from(module: &Rc<RefCell<Module>>, name: &str) -> Option<Value> {
        // Fast path: resolve in THIS module without cloning the Rc — one borrow, one (FxHash) probe.
        // The vast majority of hot global reads (loop variables, local top-level fns) hit here and never
        // touch the parent chain. Only builtins/imported names fall through to the Rc-walking slow path.
        if let Some(v) = module.borrow().globals.get(name).cloned() {
            return Some(v);
        }
        let mut cur = module.borrow().parent.clone();
        while let Some(m) = cur {
            if let Some(v) = m.borrow().globals.get(name).cloned() {
                return Some(v);
            }
            cur = m.borrow().parent.clone();
        }
        None
    }

    fn set_global_in_chain(module: &Rc<RefCell<Module>>, name: &str, v: Value) -> bool {
        // Update the existing slot IN PLACE via get_mut: one hash probe, and crucially NO
        // `name.to_string()` key allocation (the old contains_key + insert path allocated a fresh
        // String on every assignment). OpSetGlobal is the hottest op in loops that write globals —
        // `i = i + 1` fired this per iteration, so this removes an allocation from the inner loop.
        // Fast path: assign in THIS module without cloning the Rc (the common case for loop writes).
        {
            let mut mb = module.borrow_mut();
            if let Some(slot) = mb.globals.get_mut(name) {
                *slot = v;
                return true;
            }
        }
        let mut cur = module.borrow().parent.clone();
        while let Some(m) = cur {
            {
                let mut mb = m.borrow_mut();
                if let Some(slot) = mb.globals.get_mut(name) {
                    *slot = v;
                    return true;
                }
            }
            cur = m.borrow().parent.clone();
        }
        false
    }

    fn builtins() -> Vec<BuiltinFn> {
        // clock / now_ms / now_iso moved to vm/time.rs.

        // proc_spawn_wait / str_ / read_line / len moved to vm/runtime.rs.

        // array_push / array_pop moved to vm/arrays.rs.
        // map_keys / map_has / map_remove moved to vm/maps.rs.

        // env_get / env_set moved to vm/env.rs.

        // sleep_ms moved to vm/time.rs.

        // exec / import / gc_stats / assert moved to vm/runtime.rs.

        // type_assert / freeze / is_frozen / is_nil / require_type moved to
        // vm/predicates.rs.

        // checked_add / checked_sub / checked_mul / checked_div moved to
        // vm/math.rs.

        // gc_collect moved to vm/runtime.rs.

        // sha256 / constant_time_eq moved to vm/crypto.rs.
        // random_bytes / random_float moved to vm/math.rs.

        // base64_encode / base64_decode / base64_decode_bytes moved to vm/encoding.rs.

        // json / http / onnx_infer / ai / quantum all moved to their own
        // submodules under `vm/`. See dispatch entries below for `fun: X::Y`
        // references.

        // str_contains / str_starts_with / str_trim / str_split moved to vm/strings.rs.

        // tcp_* + tls + find_subslice moved to vm/net.rs (imported above).

        // http_status_reason / http_parse_request / http_response /
        // http_response_ct / http_response_json moved to vm/http.rs.

        // ── polyglot builtins extracted ────────────────────────────
        //
        // Implementations live in vm/polyglot.rs and are registered in the
        // BuiltinFn table below as `fun: polyglot::polyglot_*`.

        // ---- String helper builtins ----
        //
        // str_slice / str_index_of / str_ends_with / str_lower / str_upper / str_join
        // moved to vm/strings.rs.

        // platform moved to vm/env.rs.

        // strfromcharcode / strcharcodeat moved to vm/strings.rs.

        // array_sort moved to vm/arrays.rs.
        // array_map: deleted (dead code — could not VM-reenter from a builtin).

        // str_replace_all moved to vm/strings.rs.

        // hmac_sha256 moved to vm/crypto.rs.

        // str_format moved to vm/strings.rs.

        // ── str_format is a core utility ────────────────────────────────────────────────

        // ── Fundamental sequence/collection builtins ──────────────────────────────────

        // range / enumerate moved to vm/arrays.rs.

        // sum / min_of / max_of / product / clamp moved to vm/math.rs.

        // is_number / is_string / is_array / is_map / is_bool / is_fn moved to
        // vm/predicates.rs.

        // ── New string helpers ───────────────────────────────────────────────────────
        //
        // str_repeat / str_pad_left / str_pad_right moved to vm/strings.rs.

        // ── New array helpers (moved to vm/arrays.rs) ───────────────────────────────

        // array_unique / array_zip / array_chunk / array_flatten / array_take /
        // array_drop moved to vm/arrays.rs.

        // num_abs / num_max / num_min moved to vm/math.rs.

        // ---- OS builtins ----

        // ── Native quantum statevector simulator ─────────────────────────────
        // `quantum_simulate(circuit)` runs a circuit produced by
        // `stdlib/crypto_quantum.sapp::new_circuit()/apply_gate()/...` against
        // an actual complex statevector and returns the probability vector

        // ── Native encoding builtins (hex / url) moved to vm/encoding.rs ──────────────────────

        // websocket_accept_key / http_post_auth / http_get_auth moved to vm/http.rs.

        // parse_sse_line moved to vm/parse.rs.

        // The ~1.3k-line BuiltinFn registration table lives in
        // vm/builtins_table.rs. This call returns the core builtins; we
        // extend below with the optional pblang-graphics surface.
        let mut v = builtins_table::build();
        v.extend(pblang_graphics::gfx_builtins());
        v
    }

    /// When config is None (VM::new), no builtins are registered.
    /// When config is Some (VM::with_stdlib), all builtins are registered; capability checks
    /// run at call time via VmHost::allow_fs() etc., so sandboxing still applies.
    // AUDIT NOTE #14: `allows()` is retained as a future hook for strict-mode
    // registration (where disabled builtins are not even visible as globals). For now
    // the real sandbox is per-builtin call-time capability checks (allow_fs, allow_net,
    // etc.). We intentionally keep ALL builtins registered so that stdlib modules (which
    // import exec/net/fs builtins internally) continue to work — the call-time check
    // denies access when the capability is disabled. Strict-mode registration is tracked
    // as a future improvement.
    fn install_builtins_into(module: &Rc<RefCell<Module>>, config: Option<&StdlibConfig>) {
        let mut m = module.borrow_mut();
        let gfx_allowed = config.map(|c| c.gfx).unwrap_or(false);
        for b in VM::builtins() {
            if config.is_some() {
                // Most builtins are sandboxed by call-time capability checks
                // (allow_fs/net/exec/…), so they stay registered. The graphics
                // (`gfx*`) builtins are the exception — they have no call-time
                // check — so the `gfx` capability is enforced at registration:
                // when disabled, they are not visible at all.
                if b.name.starts_with("gfx") && !gfx_allowed {
                    continue;
                }
                m.globals.insert(b.name.to_string(), Value::Builtin(b));
            }
        }
        // Optional stdlib-style json API: global `json` map with parse/stringify (json.parse(s), json.stringify(x))
        if config.map(|c| c.json).unwrap_or(false) {
            let blist = VM::builtins();
            if let (Some(json_parse), Some(json_stringify)) = (
                blist.iter().find(|b| b.name == "json_parse").copied(),
                blist.iter().find(|b| b.name == "json_stringify").copied(),
            ) {
                let mut map = HashMap::new();
                map.insert("parse".to_string(), Value::Builtin(json_parse));
                map.insert("stringify".to_string(), Value::Builtin(json_stringify));
                m.globals
                    .insert("json".to_string(), Value::Map(Rc::new(RefCell::new(map))));
            }
        }
    }

    fn current_ip_span(&self) -> Option<Span> {
        let frame = self.frames.last()?;
        let ip = frame.ip.saturating_sub(1); // ip += 1 before match
        frame.closure.function.chunk.spans.get(ip).copied()
    }

    fn current_source(&self) -> Option<Rc<String>> {
        let frame = self.frames.last()?;
        frame.closure.function.chunk.source.clone()
    }

    fn current_source_name(&self) -> Option<String> {
        let frame = self.frames.last()?;
        frame.closure.function.chunk.source_name.as_deref().cloned()
    }

    fn build_stack(&self) -> Vec<FrameTrace> {
        self.frames
            .iter()
            .rev()
            .map(|f| {
                let ip = f.ip.saturating_sub(1);
                let span = f.closure.function.chunk.spans.get(ip).copied();
                FrameTrace {
                    function: f.closure.function.name.clone(),
                    span,
                    source_name: f.closure.function.chunk.source_name.as_deref().cloned(),
                }
            })
            .collect()
    }

    fn make_err(&self, msg: impl Into<String>) -> RuntimeError {
        RuntimeError {
            message: msg.into(),
            span: self.current_ip_span(),
            source_name: self.current_source_name(),
            source: self.current_source(),
            stack: self.build_stack(),
        }
    }

    fn err(&self, msg: impl Into<String>) -> RuntimeError {
        self.make_err(msg)
    }

    fn capture_upvalue(
        &mut self,
        stack_index: usize,
    ) -> std::rc::Rc<std::cell::RefCell<crate::Upvalue>> {
        // Return existing if already captured
        for uv in &self.open_upvalues {
            if let crate::Upvalue::Open(i) = *uv.borrow() {
                if i == stack_index {
                    return uv.clone();
                }
            }
        }
        let uv = std::rc::Rc::new(std::cell::RefCell::new(crate::Upvalue::Open(stack_index)));
        self.open_upvalues.push(uv.clone());
        uv
    }

    fn close_upvalues_from(&mut self, stack_index: usize) {
        // Close all open upvalues pointing to >= stack_index
        let mut i = 0;
        while i < self.open_upvalues.len() {
            let should_close = match *self.open_upvalues[i].borrow() {
                crate::Upvalue::Open(idx) => idx >= stack_index,
                crate::Upvalue::Closed(_) => false,
            };
            if should_close {
                let uv = self.open_upvalues.remove(i);
                let closed_val = match *uv.borrow() {
                    crate::Upvalue::Open(idx) => self.stack.get(idx).cloned().unwrap_or(Value::Nil),
                    _ => Value::Nil,
                };
                *uv.borrow_mut() = crate::Upvalue::Closed(closed_val);
            } else {
                i += 1;
            }
        }
    }

    fn upvalue_get(&self, uv: &std::rc::Rc<std::cell::RefCell<crate::Upvalue>>) -> Value {
        match &*uv.borrow() {
            crate::Upvalue::Open(i) => self.stack.get(*i).cloned().unwrap_or(Value::Nil),
            crate::Upvalue::Closed(v) => v.clone(),
        }
    }

    fn upvalue_set(&mut self, uv: &std::rc::Rc<std::cell::RefCell<crate::Upvalue>>, v: Value) {
        match &mut *uv.borrow_mut() {
            crate::Upvalue::Open(i) => {
                if let Some(slot) = self.stack.get_mut(*i) {
                    *slot = v;
                }
            }
            crate::Upvalue::Closed(slot) => *slot = v,
        }
    }

    /// Imports execute in their caller's authority and resource realm.
    fn new_for_import(&self, module: Rc<RefCell<Module>>) -> Self {
        Self {
            stack: vec![],
            module,
            builtins: self.builtins.clone(),
            module_cache: self.module_cache.clone(),
            import_stack: self.import_stack.clone(),
            frames: vec![],
            open_upvalues: vec![],
            handlers: vec![],
            unwinds: vec![],
            unwind_resume: false,
            stdlib_config: self.stdlib_config.clone(),
            limits: self.limits.clone(),
            ffi_policy: self.ffi_policy.clone(),
            ffi_resources: self.ffi_resources.clone(),
            memory_tracker: MemoryTracker::new(),
            object_memory_map: RefCell::new(HashMap::new()),
            thread_pool: None,
            handle_registry: VmHandleRegistry::new(),
            stdin_lines: std::collections::VecDeque::new(),
            signal_handlers: Rc::new(RefCell::new(HashMap::new())),
            jit: None,
            jit_state: HashMap::new(),
            jit_enabled: std::env::var("PBLANG_JIT")
                .map(|v| v != "0")
                .unwrap_or(false),
        }
    }

    /// Set resource limits (instruction count, stack depth). Used for sandboxing untrusted code.
    pub fn set_limits(&mut self, limits: Option<VmLimits>) {
        self.limits = limits;
    }

    /// Read-only access to the configured resource limits, for ingress helpers
    /// like `bounded_json_parse` that want to consult the cap without taking
    /// ownership of the VM.
    pub fn limits(&self) -> Option<&VmLimits> {
        self.limits.as_ref()
    }

    /// Set FFI allowlist policy. When policy.allow_all is false, only allowed_libs and allowed_symbols are permitted.
    pub fn set_ffi_policy(&mut self, policy: Option<FfiPolicy>) {
        self.ffi_policy = policy;
    }
}

impl Default for VM {
    /// Creates a minimal VM with no stdlib builtins. Delegates to `VM::new()`.
    fn default() -> Self {
        VM::new()
    }
}

impl VM {
    /// Creates a minimal VM with no stdlib builtins (for embedding when you register your own).
    pub fn new() -> Self {
        let builtins = Rc::new(RefCell::new(Module::default()));
        gc_register_module(&builtins);
        VM::install_builtins_into(&builtins, None);
        MODULE_ALLOCS.fetch_add(1, Ordering::Relaxed);
        let module = Rc::new(RefCell::new(Module {
            globals: Default::default(),
            parent: Some(builtins.clone()),
            exports: Default::default(),
        }));
        gc_register_module(&module);
        VM {
            stack: vec![],
            module,
            builtins,
            module_cache: Rc::new(RefCell::new(HashMap::new())),
            import_stack: Rc::new(RefCell::new(vec![])),
            frames: vec![],
            open_upvalues: vec![],
            handlers: vec![],
            unwinds: vec![],
            unwind_resume: false,
            stdlib_config: None,
            limits: None,
            ffi_policy: None,
            ffi_resources: Rc::new(RefCell::new(HashMap::new())),
            memory_tracker: MemoryTracker::new(),
            object_memory_map: RefCell::new(HashMap::new()),
            thread_pool: None,
            handle_registry: VmHandleRegistry::new(),
            stdin_lines: std::collections::VecDeque::new(),
            signal_handlers: Rc::new(RefCell::new(HashMap::new())),
            jit: None,
            jit_state: HashMap::new(),
            jit_enabled: std::env::var("PBLANG_JIT")
                .map(|v| v != "0")
                .unwrap_or(false),
        }
    }

    /// Creates a VM with stdlib builtins enabled according to `config`. Use this for scripting and CLI.
    pub fn with_stdlib(config: StdlibConfig) -> Self {
        // Enabling FFI is an explicit trusted-host opt-in. Keep a concrete
        // policy in every VM so `None` can never silently mean allow-all.
        let ffi_policy = Some(FfiPolicy {
            allow_all: config.ffi && config.ffi_allow_all,
            allowed_libs: if config.ffi {
                config.ffi_allowed_libs.clone()
            } else {
                HashSet::new()
            },
            allowed_symbols: if config.ffi {
                config.ffi_allowed_symbols.clone()
            } else {
                HashSet::new()
            },
        });
        let builtins = Rc::new(RefCell::new(Module::default()));
        gc_register_module(&builtins);
        VM::install_builtins_into(&builtins, Some(&config));
        MODULE_ALLOCS.fetch_add(1, Ordering::Relaxed);
        let module = Rc::new(RefCell::new(Module {
            globals: Default::default(),
            parent: Some(builtins.clone()),
            exports: Default::default(),
        }));
        gc_register_module(&module);
        VM {
            stack: vec![],
            module,
            builtins,
            module_cache: Rc::new(RefCell::new(HashMap::new())),
            import_stack: Rc::new(RefCell::new(vec![])),
            frames: vec![],
            open_upvalues: vec![],
            handlers: vec![],
            unwinds: vec![],
            unwind_resume: false,
            stdlib_config: Some(config),
            memory_tracker: MemoryTracker::new(),
            object_memory_map: RefCell::new(HashMap::new()),
            thread_pool: None,
            handle_registry: VmHandleRegistry::new(),
            limits: None,
            ffi_policy,
            ffi_resources: Rc::new(RefCell::new(HashMap::new())),
            stdin_lines: std::collections::VecDeque::new(),
            signal_handlers: Rc::new(RefCell::new(HashMap::new())),
            jit: None,
            jit_state: HashMap::new(),
            jit_enabled: std::env::var("PBLANG_JIT")
                .map(|v| v != "0")
                .unwrap_or(false),
        }
    }

    /// Seed stdin lines consumed by `read_line()` before falling back to terminal stdin.
    pub fn set_stdin_lines<I: IntoIterator<Item = String>>(&mut self, lines: I) {
        self.stdin_lines = lines.into_iter().collect();
    }

    fn pop_stdin_line(&mut self) -> Option<String> {
        self.stdin_lines.pop_front()
    }

    /// Normal run: prints to stdout (REPL behavior).
    pub fn run(&mut self, chunk: Chunk) -> Result<(), RuntimeError> {
        self.run_inner(chunk, None)
    }

    /// Test-friendly run: collects values printed by `print`.
    pub fn run_collect(&mut self, chunk: Chunk) -> Result<Vec<Value>, RuntimeError> {
        let mut out = Vec::<Value>::new();
        self.run_inner(chunk, Some(&mut out))?;
        Ok(out)
    }

    /// Run a chunk (as script) and return the top-of-stack value after completion.
    /// Used by spawn child thread to get the closure's return value.
    pub fn run_return_value(&mut self, chunk: Chunk) -> Result<Value, RuntimeError> {
        self.run_inner(chunk, None)?;
        Ok(self.stack.pop().unwrap_or(Value::Nil))
    }

    fn run_inner(
        &mut self,
        chunk: Chunk,
        mut out: Option<&mut Vec<Value>>,
    ) -> Result<(), RuntimeError> {
        self.stack.clear();
        self.frames.clear();
        self.unwinds.clear();
        self.unwind_resume = false;

        if let Some(src) = chunk.source.as_deref() {
            let blocks = pblang_core::extract_polyglot_blocks(src.as_str());
            let (blocks_v, poly_v) = VM::polyglot_blocks_to_values(blocks);
            let mut g = self.module.borrow_mut();
            g.globals.insert("__polyglot_blocks".into(), blocks_v);
            g.globals.insert("__polyglot".into(), poly_v);
        }

        // Wrap the top-level chunk as a 0-arg "script" function.
        let script_fn = Rc::new(Function {
            name: Some("script".to_string()),
            arity: 0,
            min_arity: 0,
            defaults: vec![],
            is_vararg: false,
            param_types: None,
            chunk,
        });

        let script_closure = Rc::new(Closure {
            function: script_fn,
            upvalues: vec![],
            module: self.module.clone(),
        });

        // Calling convention: [callee, arg1..argN]
        self.stack.push(Value::Closure(script_closure));
        self.call_top(0)?;

        // Optional Global Interpreter Lock (off unless PBLANG_GIL is set). Acquire for
        // the duration of this run; the guard releases on every exit path, including the
        // `?`/`return Err(..)` shortcuts below. Recursive, so re-entering run_inner via
        // import/eval on the same thread is safe.
        let gil_reg = self.handle_registry.clone();
        gil_reg.gil.acquire();
        struct GilGuard(Arc<VmHandleRegistry>);
        impl Drop for GilGuard {
            fn drop(&mut self) {
                self.0.gil.release();
            }
        }
        let _gil_guard = GilGuard(gil_reg.clone());

        let mut instruction_count: u64 = 0;
        while !self.frames.is_empty() {
            if let Some(ref lim) = self.limits {
                if let Some(max) = lim.max_instructions {
                    if instruction_count >= max {
                        return Err(self.make_err("instruction limit exceeded"));
                    }
                }
            }
            instruction_count += 1;
            // GIL cooperative yield: periodically hand the lock to another waiting
            // thread so threads interleave. No-op (one predictable branch) when off.
            if gil_reg.gil.enabled && (instruction_count & 0x1FFF) == 0 {
                gil_reg.gil.yield_point();
            }

            // Fetch WITHOUT cloning the opcode: clone the function Rc (one refcount
            // bump) so `op` can be a plain reference while the dispatch arms below
            // freely mutate self.stack / self.frames. The enum clone (it carries a
            // Vec in the OpClosure variant, so it's a 32-byte copy + branch) now
            // happens only on the slow fall-through path.
            let (cur_fn, cur_ip) = {
                let frame = match self.frames.last_mut() {
                    Some(f) => f,
                    None => return Err(self.err("no current frame")),
                };
                let ip = frame.ip;
                frame.ip += 1;
                (frame.closure.function.clone(), ip)
            };
            let op = match cur_fn.chunk.instructions.get(cur_ip) {
                Some(o) => o,
                None => return Err(self.err("instruction pointer out of bounds")),
            };

            // ---- fast-path dispatch for the hottest opcodes ----
            // Measured (fib/loop micro-benches): the dominant per-op cost is not the
            // handler work but the machinery around it — a closure returning Result,
            // plus 2-3 redundant `frames.last()` lookups per instruction. This block
            // handles the COMMON CASE of the most frequent opcodes with one frame
            // borrow and zero Result plumbing. Anything unusual (non-number operands,
            // NaN compares, missing globals/locals, stack underflow) leaves the stack
            // untouched and falls through to the general match below, which owns all
            // error paths — so try/catch and error messages behave identically.
            {
                let mut handled = true;
                match op {
                    OpCode::OpGetLocal(slot) => {
                        if let Some(frame) = self.frames.last() {
                            let idx = frame.stack_base + 1 + slot;
                            if idx < self.stack.len() {
                                let v = self.stack[idx].clone();
                                self.stack.push(v);
                            } else {
                                handled = false;
                            }
                        } else {
                            handled = false;
                        }
                    }
                    OpCode::OpSetLocal(slot) => {
                        if let Some(frame) = self.frames.last() {
                            let idx = frame.stack_base + 1 + slot;
                            let n = self.stack.len();
                            // idx must be valid AFTER the pop (mirrors the general handler)
                            if n >= 1 && idx < n - 1 {
                                if let Some(v) = self.stack.pop() {
                                    self.stack[idx] = v;
                                } else {
                                    handled = false;
                                }
                            } else {
                                handled = false;
                            }
                        } else {
                            handled = false;
                        }
                    }
                    OpCode::OpConstant(i) => {
                        let v = self
                            .frames
                            .last()
                            .and_then(|f| f.closure.function.chunk.constants.get(*i))
                            .cloned();
                        if let Some(v) = v {
                            self.stack.push(v);
                        } else {
                            handled = false;
                        }
                    }
                    OpCode::OpGetGlobal(idx) => {
                        let mut pushed = false;
                        if let Some(frame) = self.frames.last() {
                            if let Some(Ok(name)) = frame
                                .closure
                                .function
                                .chunk
                                .constants
                                .get(*idx)
                                .map(|c| c.as_str())
                            {
                                if let Some(v) =
                                    Self::get_global_from(&frame.closure.module, name)
                                {
                                    self.stack.push(v);
                                    pushed = true;
                                }
                            }
                        }
                        handled = pushed;
                    }
                    OpCode::OpJump(target) => {
                        if let Some(frame) = self.frames.last_mut() {
                            frame.ip = *target;
                        } else {
                            handled = false;
                        }
                    }
                    OpCode::OpJumpIfFalse(target) => {
                        // NOTE: condition stays on the stack (the compiler emits an
                        // explicit OpPop), exactly like the general handler.
                        if let Some(cond) = self.stack.last() {
                            if !Self::is_truthy(cond) {
                                if let Some(frame) = self.frames.last_mut() {
                                    frame.ip = *target;
                                } else {
                                    handled = false;
                                }
                            }
                        } else {
                            handled = false;
                        }
                    }
                    OpCode::OpJumpIfTrue(target) => {
                        if let Some(cond) = self.stack.last() {
                            if Self::is_truthy(cond) {
                                if let Some(frame) = self.frames.last_mut() {
                                    frame.ip = *target;
                                } else {
                                    handled = false;
                                }
                            }
                        } else {
                            handled = false;
                        }
                    }
                    OpCode::OpPop => {
                        if self.stack.pop().is_none() {
                            handled = false; // underflow: general handler raises the error
                        }
                    }
                    OpCode::OpAdd | OpCode::OpSub | OpCode::OpMul | OpCode::OpDiv => {
                        // Unboxed f64 × f64 only — identical to Value::arith's fast
                        // path (apply_f64). Everything else (strings, bigints,
                        // decimals, overloads) falls through untouched.
                        let n = self.stack.len();
                        if n >= 2 {
                            if let (Value::Number(a), Value::Number(b)) =
                                (&self.stack[n - 2], &self.stack[n - 1])
                            {
                                let r = match op {
                                    OpCode::OpAdd => a + b,
                                    OpCode::OpSub => a - b,
                                    OpCode::OpMul => a * b,
                                    _ => a / b,
                                };
                                self.stack.truncate(n - 1);
                                self.stack[n - 2] = Value::Number(r);
                            } else {
                                handled = false;
                            }
                        } else {
                            handled = false;
                        }
                    }
                    OpCode::OpLt | OpCode::OpLe | OpCode::OpGt | OpCode::OpGe => {
                        let n = self.stack.len();
                        if n >= 2 {
                            if let (Value::Number(a), Value::Number(b)) =
                                (&self.stack[n - 2], &self.stack[n - 1])
                            {
                                // partial_cmp mirrors num_cmp's Number fast path;
                                // NaN (None) falls through to the general handler.
                                if let Some(ord) = a.partial_cmp(b) {
                                    use std::cmp::Ordering::*;
                                    let r = match op {
                                        OpCode::OpLt => ord == Less,
                                        OpCode::OpLe => ord != Greater,
                                        OpCode::OpGt => ord == Greater,
                                        _ => ord != Less,
                                    };
                                    self.stack.truncate(n - 1);
                                    self.stack[n - 2] = Value::Bool(r);
                                    // NOTE: fusing the following OpJumpIfFalse into this
                                    // dispatch was tried and MEASURED SLOWER (~15-20%,
                                    // interleaved A/B) — the peek perturbed the dispatch
                                    // loop's code layout. Don't re-add without re-measuring.
                                } else {
                                    handled = false;
                                }
                            } else {
                                handled = false;
                            }
                        } else {
                            handled = false;
                        }
                    }
                    OpCode::OpEq | OpCode::OpNotEq => {
                        let n = self.stack.len();
                        if n >= 2 {
                            if let (Value::Number(a), Value::Number(b)) =
                                (&self.stack[n - 2], &self.stack[n - 1])
                            {
                                let r = if matches!(op, OpCode::OpEq) {
                                    a == b
                                } else {
                                    a != b
                                };
                                self.stack.truncate(n - 1);
                                self.stack[n - 2] = Value::Bool(r);
                            } else {
                                handled = false;
                            }
                        } else {
                            handled = false;
                        }
                    }
                    _ => handled = false,
                }
                if handled {
                    continue;
                }
            }

            // Slow path: clone the opcode so the general match below can bind
            // payloads by value (OpClosure's captures Vec etc.) exactly as before.
            let op = op.clone();

            let step: Result<(), RuntimeError> = (|| {
                match op {
                    OpCode::OpConstant(i) => {
                        let c = self.current_chunk()?;
                        let v = c
                            .constants
                            .get(i)
                            .cloned()
                            .ok_or_else(|| self.err("constant index out of bounds"))?;
                        self.stack.push(v);
                    }

                    // ---- arithmetic ----
                    // Arithmetic routes through `Value::arith`, which keeps the
                    // unboxed f64 fast path but transparently switches to
                    // arbitrary-precision `BigInt` when an operand is a bigint.
                    // Each binary arithmetic op keeps the unboxed fast path; on a
                    // type failure it falls back to operator overloading
                    // (`a.__op__(a, b)`) before surfacing the error, so user types
                    // (vectors, matrices, complex, money) use natural `+ - * /`
                    // at zero cost to numeric code.
                    OpCode::OpAdd => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        match a.arith(&b, ArithOp::Add) {
                            Ok(v) => self.stack.push(v),
                            Err(m) => {
                                if !self.try_binary_overload(a, b, "__add__", "__radd__")? {
                                    return Err(self.err(m));
                                }
                            }
                        }
                    }
                    OpCode::OpSub => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        match a.arith(&b, ArithOp::Sub) {
                            Ok(v) => self.stack.push(v),
                            Err(m) => {
                                if !self.try_binary_overload(a, b, "__sub__", "__rsub__")? {
                                    return Err(self.err(m));
                                }
                            }
                        }
                    }
                    OpCode::OpMul => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        match a.arith(&b, ArithOp::Mul) {
                            Ok(v) => self.stack.push(v),
                            Err(m) => {
                                if !self.try_binary_overload(a, b, "__mul__", "__rmul__")? {
                                    return Err(self.err(m));
                                }
                            }
                        }
                    }
                    OpCode::OpDiv => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        match a.arith(&b, ArithOp::Div) {
                            Ok(v) => self.stack.push(v),
                            Err(m) => {
                                if !self.try_binary_overload(a, b, "__div__", "__rdiv__")? {
                                    return Err(self.err(m));
                                }
                            }
                        }
                    }

                    // ---- extended arithmetic ----
                    OpCode::OpNegate => {
                        let a = self.pop()?;
                        match a.negate() {
                            Ok(v) => self.stack.push(v),
                            Err(m) => {
                                if !self.try_unary_overload(a, "__neg__")? {
                                    return Err(self.err(m));
                                }
                            }
                        }
                    }
                    OpCode::OpMod => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        match a.arith(&b, ArithOp::Mod) {
                            Ok(v) => self.stack.push(v),
                            Err(m) => {
                                if !self.try_binary_overload(a, b, "__mod__", "__rmod__")? {
                                    return Err(self.err(m));
                                }
                            }
                        }
                    }
                    OpCode::OpPow => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        match a.arith(&b, ArithOp::Pow) {
                            Ok(v) => self.stack.push(v),
                            Err(m) => {
                                if !self.try_binary_overload(a, b, "__pow__", "__rpow__")? {
                                    return Err(self.err(m));
                                }
                            }
                        }
                    }

                    // ---- logic ----
                    OpCode::OpNot => {
                        let v = self.pop()?;
                        let is_true = VM::is_truthy(&v);
                        let result = if is_true {
                            Value::Bool(false)
                        } else {
                            Value::Bool(true)
                        };
                        self.stack.push(result);
                    }

                    // ---- safety opcodes ----

                    // `OpSafeGetAttr(name_idx)` — `obj?.field`
                    // Pops obj. If nil, pushes nil. Otherwise gets named attribute.
                    OpCode::OpSafeGetAttr(name_idx) => {
                        let obj = self.pop()?;
                        let attr_name = {
                            let chunk = self.current_chunk()?;
                            // Bounds-checked: a crafted/corrupt constant index must
                            // not panic the VM (every sibling opcode uses `.get()`).
                            match chunk.constants.get(name_idx) {
                                Some(Value::Obj(o)) => match o.as_ref() {
                                    pblang_ir::Obj::String(s) => s.clone(),
                                    _ => {
                                        return Err(self
                                            .err("OpSafeGetAttr: invalid attr name".to_string()))
                                    }
                                },
                                _ => {
                                    return Err(
                                        self.err("OpSafeGetAttr: invalid attr name".to_string())
                                    )
                                }
                            }
                        };
                        match obj {
                            Value::Nil => self.stack.push(Value::Nil),
                            Value::Map(ref m) => {
                                let v = m.borrow().get(&attr_name).cloned().unwrap_or(Value::Nil);
                                self.stack.push(v);
                            }
                            other => {
                                return Err(self.err(format!(
                                    "?.{attr_name}: expected map or nil, got {other}"
                                )));
                            }
                        }
                    }

                    // `OpSafeIndex` — `obj?[idx]` (safe indexed access)
                    // Stack top: idx, below: obj
                    // If obj is nil, pop both and push nil. Else do normal OpGetIndex.
                    OpCode::OpSafeIndex => {
                        let idx = self.pop()?;
                        let obj = self.pop()?;
                        if let Value::Nil = &obj {
                            self.stack.push(Value::Nil);
                        } else {
                            // Re-push for normal index operation
                            self.stack.push(obj);
                            self.stack.push(idx);
                            // Inline OpGetIndex logic
                            let index = self.pop()?;
                            let obj2 = self.pop()?;
                            match obj2 {
                                Value::Array(a) => {
                                    let i = match index {
                                        Value::Number(n) => n as isize,
                                        _ => {
                                            return Err(self
                                                .err("array?[]: index must be number".to_string()))
                                        }
                                    };
                                    let a = a.borrow();
                                    if i < 0 || (i as usize) >= a.len() {
                                        self.stack.push(Value::Nil); // safe: out-of-bounds = nil
                                    } else {
                                        self.stack.push(a[i as usize].clone());
                                    }
                                }
                                Value::Map(m) => {
                                    let key = index.as_str().map_err(|_| {
                                        self.err("map?[]: key must be string".to_string())
                                    })?;
                                    let v = m.borrow().get(key).cloned().unwrap_or(Value::Nil);
                                    self.stack.push(v);
                                }
                                Value::Obj(o) => match o.as_ref() {
                                    Obj::String(s) => {
                                        let i = match index {
                                            Value::Number(n) => n as isize,
                                            _ => {
                                                return Err(self.err(
                                                    "string?[]: index must be number".to_string(),
                                                ))
                                            }
                                        };
                                        if i < 0 {
                                            self.stack.push(Value::Nil);
                                        } else {
                                            match s.chars().nth(i as usize) {
                                                Some(c) => {
                                                    self.stack.push(alloc_string(c.to_string()))
                                                }
                                                None => self.stack.push(Value::Nil),
                                            }
                                        }
                                    }
                                    // bigint isn't indexable; nil-safe index yields nil.
                                    _ => self.stack.push(Value::Nil),
                                },
                                _ => self.stack.push(Value::Nil),
                            }
                        }
                    }

                    // `OpResultUnwrap` — `expr?` (Result unwrap operator)
                    // If value is {ok: false, error: msg}, throw RuntimeError.
                    // If value is {ok: true, value: v}, push v.
                    // Otherwise, push as-is (non-Result values pass through unchanged).
                    OpCode::OpResultUnwrap => {
                        let v = self.pop()?;
                        match &v {
                            Value::Map(m) => {
                                let mb = m.borrow();
                                let is_ok = mb.get("ok").cloned();
                                match is_ok {
                                    Some(Value::Bool(false)) => {
                                        let msg = mb
                                            .get("error")
                                            .and_then(|e| e.as_str().ok().map(String::from))
                                            .unwrap_or_else(|| {
                                                "error (from ? operator)".to_string()
                                            });
                                        drop(mb);
                                        return Err(self.err(msg));
                                    }
                                    Some(Value::Bool(true)) => {
                                        let val = mb.get("value").cloned().unwrap_or(Value::Nil);
                                        drop(mb);
                                        self.stack.push(val);
                                    }
                                    _ => {
                                        drop(mb);
                                        self.stack.push(v);
                                    }
                                }
                            }
                            Value::Nil => {
                                return Err(self.err(
                                    "? operator: cannot unwrap nil (use if/else or unwrap_or)"
                                        .to_string(),
                                ));
                            }
                            _ => self.stack.push(v),
                        }
                    }

                    // `OpIsNil` — push true if top of stack is nil, false otherwise
                    OpCode::OpIsNil => {
                        let v = self.pop()?;
                        self.stack.push(Value::Bool(matches!(v, Value::Nil)));
                    }

                    OpCode::OpDup => {
                        // Duplicate top of stack
                        let top = self
                            .stack
                            .last()
                            .cloned()
                            .ok_or_else(|| self.err("OpDup: stack underflow".to_string()))?;
                        self.stack.push(top);
                    }

                    OpCode::OpThrow => {
                        // Pop value and raise as RuntimeError
                        let val = self.pop()?;
                        let msg = match &val {
                            Value::Obj(o) => match o.as_ref() {
                                pblang_ir::Obj::String(s) => s.clone(),
                                pblang_ir::Obj::BigInt(n) => n.to_string(),
                                pblang_ir::Obj::Rational(r) => r.to_string(),
                                pblang_ir::Obj::Decimal(d) => d.to_string(),
                            },
                            Value::Map(m) => {
                                let mb = m.borrow();
                                mb.get("message")
                                    .and_then(|v| v.as_str().ok().map(String::from))
                                    .or_else(|| {
                                        mb.get("msg")
                                            .and_then(|v| v.as_str().ok().map(String::from))
                                    })
                                    .unwrap_or_else(|| format!("{val}"))
                            }
                            _ => format!("{val}"),
                        };
                        return Err(self.err(msg));
                    }

                    // ---- comparisons ----
                    // Equality is structural by default and never errors, so
                    // overloading is opt-in by *presence*: dispatch `__eq__` only
                    // when an operand defines it, otherwise native `==`.
                    OpCode::OpEq => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        // Cheap guard keeps numeric/string equality on the pure
                        // native path; only map operands can carry `__eq__`.
                        let maybe_overload =
                            matches!(a, Value::Map(_)) || matches!(b, Value::Map(_));
                        if !maybe_overload
                            || !self.try_compare_overload(&a, &b, "__eq__", "__eq__")?
                        {
                            self.stack.push(Value::Bool(a == b));
                        }
                    }
                    OpCode::OpNotEq => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        let maybe_overload =
                            matches!(a, Value::Map(_)) || matches!(b, Value::Map(_));
                        if !maybe_overload
                            || !self.try_compare_overload(&a, &b, "__ne__", "__ne__")?
                        {
                            self.stack.push(Value::Bool(a != b));
                        }
                    }
                    // Ordering routes through `Value::num_cmp` (bigint-aware); when
                    // it returns `None` (operands not numerically comparable) fall
                    // back to a comparison overload before erroring. Reflected
                    // dunder is the mirror operator (a > b ⟺ b < a).
                    OpCode::OpGt => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        match a.num_cmp(&b) {
                            Some(ord) => self
                                .stack
                                .push(Value::Bool(ord == std::cmp::Ordering::Greater)),
                            None => {
                                if !self.try_compare_overload(&a, &b, "__gt__", "__lt__")? {
                                    return Err(self.err("comparison expects numbers"));
                                }
                            }
                        }
                    }
                    OpCode::OpGe => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        match a.num_cmp(&b) {
                            Some(ord) => self
                                .stack
                                .push(Value::Bool(ord != std::cmp::Ordering::Less)),
                            None => {
                                if !self.try_compare_overload(&a, &b, "__ge__", "__le__")? {
                                    return Err(self.err("comparison expects numbers"));
                                }
                            }
                        }
                    }
                    OpCode::OpLt => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        match a.num_cmp(&b) {
                            Some(ord) => self
                                .stack
                                .push(Value::Bool(ord == std::cmp::Ordering::Less)),
                            None => {
                                if !self.try_compare_overload(&a, &b, "__lt__", "__gt__")? {
                                    return Err(self.err("comparison expects numbers"));
                                }
                            }
                        }
                    }
                    OpCode::OpLe => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        match a.num_cmp(&b) {
                            Some(ord) => self
                                .stack
                                .push(Value::Bool(ord != std::cmp::Ordering::Greater)),
                            None => {
                                if !self.try_compare_overload(&a, &b, "__le__", "__ge__")? {
                                    return Err(self.err("comparison expects numbers"));
                                }
                            }
                        }
                    }

                    // ---- stack / io ----
                    OpCode::OpPop => {
                        let _ = self.pop()?;
                    }

                    OpCode::OpPrint => {
                        let v = self.pop()?;
                        if let Some(buf) = out.as_deref_mut() {
                            buf.push(v);
                        } else {
                            println!("{v}");
                        }
                    }

                    // ---- globals ----
                    OpCode::OpDefineGlobal(name_const_idx) => {
                        let v = self.pop()?;
                        let c = self.current_chunk()?;
                        let name = c
                            .constants
                            .get(name_const_idx)
                            .ok_or_else(|| self.err("constant index out of bounds"))?
                            .as_str()
                            .map_err(|m| self.err(m))?
                            .to_string();
                        let frame = self
                            .frames
                            .last()
                            .ok_or_else(|| self.err("no current frame"))?;
                        frame.closure.module.borrow_mut().globals.insert(name, v);
                    }

                    OpCode::OpGetGlobal(name_const_idx) => {
                        // Resolve the name as a borrowed &str — no per-access
                        // String allocation. This is one of the hottest ops
                        // (every builtin/function reference goes through it).
                        let c = self.current_chunk()?;
                        let name = c
                            .constants
                            .get(name_const_idx)
                            .ok_or_else(|| self.err("constant index out of bounds"))?
                            .as_str()
                            .map_err(|m| self.err(m))?;
                        let frame = self
                            .frames
                            .last()
                            .ok_or_else(|| self.err("no current frame"))?;
                        let v = Self::get_global_from(&frame.closure.module, name)
                            .ok_or_else(|| self.err(format!("undefined global '{name}'")))?;
                        self.stack.push(v);
                    }

                    OpCode::OpSetGlobal(name_const_idx) => {
                        let v = self.pop()?;
                        let c = self.current_chunk()?;
                        let name = c
                            .constants
                            .get(name_const_idx)
                            .ok_or_else(|| self.err("constant index out of bounds"))?
                            .as_str()
                            .map_err(|m| self.err(m))?;

                        let frame = self
                            .frames
                            .last()
                            .ok_or_else(|| self.err("no current frame"))?;
                        if !Self::set_global_in_chain(&frame.closure.module, name, v) {
                            return Err(self.err(format!("undefined global '{name}'")));
                        }
                    }

                    OpCode::OpGetAttr(attr_const_idx) => {
                        let obj = self.pop()?;
                        let c = self.current_chunk()?;
                        let attr = c
                            .constants
                            .get(attr_const_idx)
                            .ok_or_else(|| self.err("constant index out of bounds"))?
                            .as_str()
                            .map_err(|m| self.err(m))?;

                        match obj {
                            Value::Module(m) => {
                                let v = m.borrow().exports.get(attr).cloned().ok_or_else(|| {
                                    self.err(format!("undefined export '{attr}'"))
                                })?;
                                self.stack.push(v);
                            }
                            Value::Map(m) => {
                                let v =
                                    m.borrow().get(attr).cloned().ok_or_else(|| {
                                        self.err(format!("undefined key '{attr}'"))
                                    })?;
                                self.stack.push(v);
                            }
                            _ => {
                                return Err(self.err("member access only supported on modules/maps"))
                            }
                        }
                    }

                    OpCode::OpExport(name_const_idx) => {
                        let c = self.current_chunk()?;
                        let name = c
                            .constants
                            .get(name_const_idx)
                            .ok_or_else(|| self.err("constant index out of bounds"))?
                            .as_str()
                            .map_err(|m| self.err(m))?
                            .to_string();
                        let frame = self
                            .frames
                            .last()
                            .ok_or_else(|| self.err("no current frame"))?;
                        let v = frame
                            .closure
                            .module
                            .borrow()
                            .globals
                            .get(&name)
                            .cloned()
                            .ok_or_else(|| self.err(format!("cannot export undefined '{name}'")))?;
                        frame.closure.module.borrow_mut().exports.insert(name, v);
                    }

                    OpCode::OpSetAttr(attr_const_idx) => {
                        let value = self.pop()?;
                        let obj = self.pop()?;
                        let c = self.current_chunk()?;
                        let attr = c
                            .constants
                            .get(attr_const_idx)
                            .ok_or_else(|| self.err("constant index out of bounds"))?
                            .as_str()
                            .map_err(|m| self.err(m))?;
                        // Do not allow __frozen itself to be overwritten through this path
                        let attr = attr.to_string();

                        match obj {
                            Value::Map(m) => {
                                // Check memory safety: refuse mutation if map is tracked and borrowed.
                                if let Some(&mid) = self
                                    .object_memory_map
                                    .borrow()
                                    .get(&(Rc::as_ptr(&m) as usize))
                                {
                                    if let Err(e) = self.memory_tracker.check_mutable_allowed(mid) {
                                        return Err(self
                                            .make_err(format!("Memory safety violation: {}", e)));
                                    }
                                }
                                // Check freeze guard
                                if matches!(m.borrow().get("__frozen"), Some(Value::Bool(true))) {
                                    return Err(self.err(format!(
                                        "FrozenError: cannot set attribute '{}' on a frozen map",
                                        attr
                                    )));
                                }
                                m.borrow_mut().insert(attr, value.clone());
                                self.stack.push(value);
                            }
                            _ => return Err(self.err("attr set only supported on maps")),
                        }
                    }

                    // ---- collections ----
                    OpCode::OpMakeArray(n) => {
                        if n > self.stack.len() {
                            return Err(self.err("stack underflow (make array)"));
                        }
                        let start = self.stack.len() - n;
                        let items = self.stack.drain(start..).collect::<Vec<Value>>();
                        self.stack.push(alloc_array(items));
                    }

                    OpCode::OpMakeMap(n) => {
                        let need = n * 2;
                        if need > self.stack.len() {
                            return Err(self.err("stack underflow (make map)"));
                        }
                        let start = self.stack.len() - need;
                        let items = self.stack.drain(start..).collect::<Vec<Value>>();
                        let mut out = HashMap::<String, Value>::new();
                        let mut it = items.into_iter();
                        while let Some(kv) = it.next() {
                            let key = kv
                                .as_str()
                                .map_err(|m| self.err(format!("map key must be string: {m}")))?;
                            let v = it
                                .next()
                                .ok_or_else(|| self.err("map literal missing value"))?;
                            out.insert(key.to_string(), v);
                        }
                        self.stack.push(alloc_map(out));
                    }

                    // ---- error handling ----
                    OpCode::OpTry(target) => {
                        let frame_index = self
                            .frames
                            .len()
                            .checked_sub(1)
                            .ok_or_else(|| self.err("no current frame"))?;
                        self.handlers.push(TryHandler {
                            frame_index,
                            target_ip: target,
                            stack_len: self.stack.len(),
                        });
                    }

                    OpCode::OpEndTry => {
                        let _ = self.handlers.pop();
                    }

                    OpCode::OpGetIndex => {
                        let index = self.pop()?;
                        let obj = self.pop()?;
                        match obj {
                            Value::Array(a) => {
                                let i = match index {
                                    Value::Number(n) => n as isize,
                                    _ => return Err(self.err("array index must be number")),
                                };
                                let a = a.borrow();
                                if i < 0 || (i as usize) >= a.len() {
                                    return Err(self.err("array index out of bounds"));
                                }
                                self.stack.push(a[i as usize].clone());
                            }
                            Value::Map(m) => {
                                let key = index.as_str().map_err(|m| {
                                    self.err(format!("map index must be string: {m}"))
                                })?;
                                let v =
                                    m.borrow().get(key).cloned().ok_or_else(|| {
                                        self.err(format!("undefined key '{key}'"))
                                    })?;
                                self.stack.push(v);
                            }
                            // String indexing is char-indexed (consistent with
                            // `len()` and `str_slice`): `s[i]` is the i-th
                            // codepoint as a one-character string.
                            Value::Obj(o) => match o.as_ref() {
                                Obj::String(s) => {
                                    let i = match index {
                                        Value::Number(n) => n as isize,
                                        _ => return Err(self.err("string index must be number")),
                                    };
                                    if i < 0 {
                                        return Err(self.err("string index out of bounds"));
                                    }
                                    match s.chars().nth(i as usize) {
                                        Some(c) => self.stack.push(alloc_string(c.to_string())),
                                        None => return Err(self.err("string index out of bounds")),
                                    }
                                }
                                _ => {
                                    return Err(
                                        self.err("indexing only supported on strings/arrays/maps")
                                    )
                                }
                            },
                            _ => {
                                return Err(
                                    self.err("indexing only supported on strings/arrays/maps")
                                )
                            }
                        }
                    }

                    OpCode::OpSetIndex => {
                        let value = self.pop()?;
                        let index = self.pop()?;
                        let obj = self.pop()?;
                        // Check memory safety before mutation.
                        if let Some(addr) = object_addr(&obj) {
                            if let Some(&mid) = self.object_memory_map.borrow().get(&addr) {
                                if let Err(e) = self.memory_tracker.check_mutable_allowed(mid) {
                                    return Err(
                                        self.make_err(format!("Memory safety violation: {}", e))
                                    );
                                }
                            }
                        }
                        match obj {
                            Value::Array(a) => {
                                let i = match index {
                                    Value::Number(n) => n as isize,
                                    _ => return Err(self.err("array index must be number")),
                                };
                                let mut a = a.borrow_mut();
                                if i < 0 {
                                    return Err(self.err("array index out of bounds"));
                                }
                                let iu = i as usize;
                                if iu == a.len() {
                                    a.push(value.clone());
                                } else if iu < a.len() {
                                    a[iu] = value.clone();
                                } else {
                                    return Err(self.err("array index out of bounds"));
                                }
                                self.stack.push(value);
                            }
                            Value::Map(m) => {
                                // FIX #20: Enforce freeze() for index-style assignment too
                                if matches!(m.borrow().get("__frozen"), Some(Value::Bool(true))) {
                                    return Err(self.err(
                                        "FrozenError: cannot set index on a frozen map".to_string(),
                                    ));
                                }
                                let key = index.as_str().map_err(|m| {
                                    self.err(format!("map index must be string: {m}"))
                                })?;
                                m.borrow_mut().insert(key.to_string(), value.clone());
                                self.stack.push(value);
                            }
                            _ => {
                                return Err(
                                    self.err("index assignment only supported on arrays/maps")
                                )
                            }
                        }
                    }

                    // ---- locals ----
                    OpCode::OpGetLocal(slot) => {
                        let frame = self
                            .frames
                            .last()
                            .ok_or_else(|| self.err("no current frame"))?;
                        let idx = frame.stack_base + 1 + slot;
                        let v = self
                            .stack
                            .get(idx)
                            .cloned()
                            .ok_or_else(|| self.err("local slot out of bounds"))?;
                        self.stack.push(v);
                    }

                    OpCode::OpSetLocal(slot) => {
                        let v = self.pop()?;
                        let frame = self
                            .frames
                            .last()
                            .ok_or_else(|| self.err("no current frame"))?;
                        let idx = frame.stack_base + 1 + slot;

                        if idx >= self.stack.len() {
                            return Err(self.err("local slot out of bounds"));
                        }
                        self.stack[idx] = v;
                    }

                    // ---- functions ----
                    OpCode::OpClosure { fn_const, captures } => {
                        let c = self.current_chunk()?;
                        let v = c
                            .constants
                            .get(fn_const)
                            .cloned()
                            .ok_or_else(|| self.err("constant index out of bounds"))?;

                        let fun_rc = match v {
                            Value::Function(f) => f,
                            _ => return Err(self.err("OpClosure expects a Function constant")),
                        };

                        let mut upvalues: Vec<Rc<RefCell<Upvalue>>> =
                            Vec::with_capacity(captures.len());

                        let (stack_base, parent_upvalues) = {
                            let frame = self
                                .frames
                                .last()
                                .ok_or_else(|| self.err("no current frame"))?;
                            (frame.stack_base, frame.closure.upvalues.clone())
                        };

                        for (is_local, index) in captures {
                            if is_local {
                                let stack_index = stack_base + 1 + index;
                                upvalues.push(self.capture_upvalue(stack_index));
                            } else {
                                let parent_uv = parent_upvalues
                                    .get(index)
                                    .cloned()
                                    .ok_or_else(|| self.err("upvalue index out of bounds"))?;
                                upvalues.push(parent_uv);
                            }
                        }
                        let closure_rc = Rc::new(Closure {
                            function: fun_rc,
                            upvalues,
                            module: {
                                let frame = self
                                    .frames
                                    .last()
                                    .ok_or_else(|| self.err("no current frame"))?;
                                frame.closure.module.clone()
                            },
                        });
                        self.stack.push(Value::Closure(closure_rc));
                    }

                    OpCode::OpGetUpvalue(slot) => {
                        let frame = self
                            .frames
                            .last()
                            .ok_or_else(|| self.err("no current frame"))?;
                        let uv = frame
                            .closure
                            .upvalues
                            .get(slot)
                            .cloned()
                            .ok_or_else(|| self.err("upvalue slot out of bounds"))?;
                        let v = self.upvalue_get(&uv);
                        self.stack.push(v);
                    }

                    OpCode::OpSetUpvalue(slot) => {
                        let v = self.pop()?;
                        let frame = self
                            .frames
                            .last()
                            .ok_or_else(|| self.err("no current frame"))?;
                        let uv = frame
                            .closure
                            .upvalues
                            .get(slot)
                            .cloned()
                            .ok_or_else(|| self.err("upvalue slot out of bounds"))?;
                        self.upvalue_set(&uv, v);
                    }

                    OpCode::OpCloseUpvalue => {
                        // Close upvalues for the local that is about to be popped
                        let top_index = self.stack.len().saturating_sub(1);
                        self.close_upvalues_from(top_index);
                        let _ = self.pop()?;
                    }

                    OpCode::OpCall(argc) => self.call_top(argc)?,

                    OpCode::OpTailCall(argc) => self.do_tail_call(argc)?,

                    OpCode::OpReturn => self.do_return()?,

                    OpCode::OpDefer => {
                        let closure = self.pop()?;
                        if let Some(frame) = self.frames.last_mut() {
                            frame.defers.push(closure);
                        }
                    }

                    // ---- control flow ----
                    OpCode::OpJump(target) => {
                        let frame = match self.frames.last_mut() {
                            Some(f) => f,
                            None => return Err(self.err("no current frame")),
                        };
                        frame.ip = target;
                    }

                    OpCode::OpJumpIfFalse(target) => {
                        let cond = self
                            .stack
                            .last()
                            .cloned()
                            .ok_or_else(|| self.err("stack underflow (jump cond)"))?;

                        if !Self::is_truthy(&cond) {
                            let frame = match self.frames.last_mut() {
                                Some(f) => f,
                                None => return Err(self.err("no current frame")),
                            };
                            frame.ip = target;
                        }
                    }

                    OpCode::OpJumpIfTrue(target) => {
                        let cond = self
                            .stack
                            .last()
                            .cloned()
                            .ok_or_else(|| self.err("stack underflow (jump cond)"))?;
                        if Self::is_truthy(&cond) {
                            let frame = match self.frames.last_mut() {
                                Some(f) => f,
                                None => return Err(self.err("no current frame")),
                            };
                            frame.ip = target;
                        }
                    }
                }
                Ok(())
            })();

            if let Err(e) = step {
                // Exceptions run the exiting frames' defers before reaching the
                // handler (or leaving `run`), per the frozen defer contract.
                self.unwind_resume = false;
                self.begin_unwind(e)?;
                continue;
            }
            if self.unwind_resume {
                self.unwind_resume = false;
                self.continue_unwind()?;
            }
        }

        Ok(())
    }

    // fn is_truthy(v: &Value) -> bool {
    //     match v {
    //         Value::Bool(b) => *b,
    //         Value::Nil => false,
    //         _ => true,
    //     }
    // }

    fn is_truthy(v: &Value) -> bool {
        match v {
            Value::Nil => false,

            Value::Bool(b) => *b,

            Value::Number(n) => *n != 0.0,

            Value::Obj(o) => match o.as_ref() {
                Obj::String(s) => !s.is_empty(),
                // A bigint is truthy unless it is zero (Displays as exactly "0").
                Obj::BigInt(n) => n.to_string() != "0",
                // A rational object is always non-integer (zero normalizes to a
                // bigint), hence always nonzero / truthy.
                Obj::Rational(_) => true,
                // A decimal is truthy unless its value is zero.
                Obj::Decimal(d) => !d.is_zero(),
            },

            // Function / Closure values συνήθως θεωρούνται πάντα truthy
            Value::Function(_) => true,
            Value::Closure(_) => true,
            Value::Builtin(_) => true,
            Value::Module(_) => true,
            Value::Array(a) => !a.borrow().is_empty(),
            Value::Map(m) => !m.borrow().is_empty(),
        }
    }

    fn pop(&mut self) -> Result<Value, RuntimeError> {
        self.stack.pop().ok_or_else(|| self.err("stack underflow"))
    }

    fn current_chunk(&self) -> Result<&Chunk, RuntimeError> {
        let frame = self
            .frames
            .last()
            .ok_or_else(|| self.err("no current frame"))?;
        Ok(&frame.closure.function.chunk)
    }

    /// Tail call (`OpTailCall`). When the callee is a user closure/function and
    /// the current frame has no pending defers, REUSE the current frame instead
    /// of pushing a new one — so tail recursion runs in constant stack depth.
    /// Every other case (builtin, callable map, frame with defers) falls back to
    /// a normal `call_top`; the compiler always emits an `OpReturn` after the
    /// `OpTailCall`, so that fallback returns correctly (the reuse path jumps to
    /// `ip = 0`, skipping that trailing return).
    fn do_tail_call(&mut self, argc: usize) -> Result<(), RuntimeError> {
        let callee_index = match self.stack.len().checked_sub(argc + 1) {
            Some(i) => i,
            None => return self.call_top(argc),
        };

        // Only direct user closures/functions are frame-reusable, and only when
        // the current frame has nothing that must run on its return (defers /
        // defer-thunk). Anything else: behave exactly like a normal call and let
        // the trailing OpReturn finish the job.
        let reusable = matches!(
            self.stack[callee_index],
            Value::Closure(_) | Value::Function(_)
        );
        let frame_clean = self
            .frames
            .last()
            .map(|f| f.defers.is_empty() && !f.is_defer_thunk && !f.is_unwind_thunk)
            .unwrap_or(false);
        if !(reusable && frame_clean) {
            return self.call_top(argc);
        }

        let old_base = self
            .frames
            .last()
            .expect("frame_clean implies a current frame")
            .stack_base;

        // Let call_top do all the arity / defaults / vararg normalization and
        // push a fresh frame for the callee (exactly one, since the callee is a
        // closure/function — not a builtin or map). Then collapse that frame
        // down onto the current one, dropping the old frame.
        self.call_top(argc)?;
        let mut new_frame = self
            .frames
            .pop()
            .expect("call_top on a closure pushes exactly one frame");
        let new_base = new_frame.stack_base;

        // Close upvalues captured from the OLD frame's locals before we overwrite
        // them. The freshly-placed callee+args (at new_base..) hold no open
        // upvalues yet (the new function hasn't run), so this only closes the
        // discarded frame's captures.
        self.close_upvalues_from(old_base + 1);

        // Shift [new_base .. top] (new callee + normalized args) down to old_base.
        // old_base <= new_base, so a forward copy never clobbers an unread source.
        let n = self.stack.len() - new_base;
        for i in 0..n {
            self.stack[old_base + i] = self.stack[new_base + i].clone();
        }
        self.stack.truncate(old_base + n);

        // Replace the old frame with the rebased new frame (ip already 0, defers
        // empty, module_result None — fresh from call_top).
        new_frame.stack_base = old_base;
        new_frame.ip = 0;
        *self
            .frames
            .last_mut()
            .expect("old frame is still present after popping the new one") = new_frame;
        Ok(())
    }

    /// Operator overloading — look up a dunder method (`__add__`, `__neg__`, …)
    /// on a map value or anywhere up its `__class` table chain, returning the
    /// method value if present. Mirrors `has_member`'s walk but yields the value
    /// so the caller can invoke it. Associated fn (no `&self`) to keep borrows
    /// clean at the call site. Depth-bounded (32) against cycles.
    fn lookup_member_value(val: &Value, name: &str) -> Option<Value> {
        let mut cur = match val {
            Value::Map(m) => m.clone(),
            _ => return None,
        };
        for _ in 0..32 {
            let (found, parent) = {
                let b = cur.borrow();
                let found = b.get(name).cloned();
                let parent = match b.get("__class") {
                    Some(Value::Map(p)) => Some(p.clone()),
                    _ => None,
                };
                (found, parent)
            };
            if found.is_some() {
                return found;
            }
            match parent {
                Some(p) => cur = p,
                None => break,
            }
        }
        None
    }

    /// Slow-path binary operator overloading. Called only AFTER native `arith`
    /// fails (so numeric/string/bigint fast paths pay nothing).
    ///
    /// Forward dispatch: if the LEFT operand carries `dunder` (a method
    /// `fn(self, other)`), call `a.dunder(a, b)`. Otherwise **reflected**
    /// dispatch (Python-style): if the RIGHT operand carries `rdunder`, call
    /// `b.rdunder(b, a)`. Reflection makes `scalar * vec` work via `__rmul__`
    /// and — because unary `-x` desugars to `0 - x` — makes `-vec` work via
    /// `__rsub__`. The call is set up through `call_top`; the main loop then runs
    /// the method and its return value lands where the arithmetic result would
    /// have. Returns `true` if a method was dispatched, `false` if neither
    /// operand overloads it (caller then surfaces the original arithmetic error).
    fn try_binary_overload(
        &mut self,
        a: Value,
        b: Value,
        dunder: &str,
        rdunder: &str,
    ) -> Result<bool, RuntimeError> {
        if let Some(method) = Self::lookup_member_value(&a, dunder) {
            self.stack.push(method);
            self.stack.push(a);
            self.stack.push(b);
            self.call_top(2)?;
            Ok(true)
        } else if let Some(method) = Self::lookup_member_value(&b, rdunder) {
            self.stack.push(method);
            self.stack.push(b);
            self.stack.push(a);
            self.call_top(2)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Slow-path unary operator overloading (`-x` → `x.__neg__(x)`). Same
    /// contract as `try_binary_overload` but with one argument.
    fn try_unary_overload(&mut self, a: Value, dunder: &str) -> Result<bool, RuntimeError> {
        if let Some(method) = Self::lookup_member_value(&a, dunder) {
            self.stack.push(method);
            self.stack.push(a);
            self.call_top(1)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Comparison operator overloading (`==`, `!=`, `<`, `>`, `<=`, `>=`). Takes
    /// the operands by reference so callers can still run the native comparison
    /// when no overload exists. Forward dispatch `a.dunder(a, b)`, else reflected
    /// `b.rdunder(b, a)` — where the reflected dunder is the *mirror* operator
    /// (`a > b` ⟺ `b < a`, so Gt reflects through `__lt__`). Every comparison
    /// dunder returns the final bool directly (the `call_top` mechanism can't
    /// post-process a method's result), so defining `__lt__`, `__le__`, `__eq__`,
    /// `__ne__` yields all six operators via reflection. Returns `true` if a
    /// method was dispatched. Used as a pre-check for `==`/`!=` (which never error
    /// natively, so opt-in by presence) and as the `None`-path fallback for
    /// ordering.
    fn try_compare_overload(
        &mut self,
        a: &Value,
        b: &Value,
        dunder: &str,
        rdunder: &str,
    ) -> Result<bool, RuntimeError> {
        if let Some(method) = Self::lookup_member_value(a, dunder) {
            self.stack.push(method);
            self.stack.push(a.clone());
            self.stack.push(b.clone());
            self.call_top(2)?;
            Ok(true)
        } else if let Some(method) = Self::lookup_member_value(b, rdunder) {
            self.stack.push(method);
            self.stack.push(b.clone());
            self.stack.push(a.clone());
            self.call_top(2)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn call_top(&mut self, argc: usize) -> Result<(), RuntimeError> {
        let callee_index = self
            .stack
            .len()
            .checked_sub(argc + 1)
            .ok_or_else(|| self.err("stack underflow in call"))?;

        let callee = self.stack[callee_index].clone();
        // Builtins execute immediately, without creating a CallFrame.
        if let Value::Builtin(b) = callee {
            if let Some(arity) = b.arity {
                if argc != arity {
                    return Err(
                        self.err(format!("arity mismatch: expected {}, got {}", arity, argc))
                    );
                }
            }

            let args = self
                .stack
                .drain(callee_index + 1..callee_index + 1 + argc)
                .collect::<Vec<Value>>();
            // remove callee
            self.stack.pop();

            let ret = (b.fun)(self, args)?;
            self.stack.push(ret);
            return Ok(());
        }

        let closure_rc: Rc<Closure> = match callee {
            Value::Closure(c) => c,
            Value::Function(f) => {
                let module = self
                    .frames
                    .last()
                    .map(|f| f.closure.module.clone())
                    .unwrap_or_else(|| self.module.clone());
                Rc::new(Closure {
                    function: f,
                    upvalues: vec![],
                    module,
                })
            }
            // Map with a __call entry: class constructor call like `Point(x, y)`.
            // Replace the callee on the stack with the __call value and retry.
            Value::Map(m) => {
                let call_fn = m.borrow().get("__call").cloned();
                if let Some(callable) = call_fn {
                    self.stack[callee_index] = callable;
                    return self.call_top(argc);
                }
                return Err(self.err("can only call function/closure/builtin (map has no __call)"));
            }
            _ => return Err(self.err("can only call function/closure/builtin")),
        };

        let func = &closure_rc.function;
        let arity = func.arity;
        let min_arity = func.min_arity;
        let is_vararg = func.is_vararg;

        // Arity check with defaults and vararg support
        if is_vararg {
            // Vararg: must supply at least min_arity args; extra args → vararg array
            if argc < min_arity {
                return Err(self.err(format!(
                    "arity mismatch: expected at least {} arguments, got {}",
                    min_arity, argc
                )));
            }
        } else {
            // Normal function with optional defaults
            if argc < min_arity {
                return Err(self.err(format!(
                    "arity mismatch: expected at least {} argument(s), got {}",
                    min_arity, argc
                )));
            }
            if argc > arity {
                return Err(self.err(format!(
                    "arity mismatch: expected at most {} argument(s), got {}",
                    arity, argc
                )));
            }
        }

        // Push default values for missing optional parameters
        if !is_vararg && argc < arity {
            let defaults = func.defaults.clone();
            let missing = arity - argc;
            // defaults[i] covers param at index (min_arity + i)
            // We need defaults for params at index [argc, arity)
            let start_default = argc.saturating_sub(min_arity);
            for i in 0..missing {
                let default_idx = start_default + i;
                let dv = defaults.get(default_idx).cloned().unwrap_or(Value::Nil);
                self.stack.push(dv);
            }
        }

        // Handle variadic: collect extra args into array
        if is_vararg {
            let extra = argc.saturating_sub(arity);
            let vararg_start = self.stack.len() - extra;
            let vararg_vals: Vec<Value> = self.stack.drain(vararg_start..).collect();
            let vararg_arr = alloc_array(vararg_vals);
            // Push the vararg array as the last parameter
            self.stack.push(vararg_arr);
        }

        // ── JIT fast path: hot, eligible, all-numeric, non-vararg calls ──────────────
        // Compile once a function is hot; thereafter run it natively. Any miss (not
        // eligible, not yet hot, non-numeric arg, or call_jit decline) falls straight
        // through to the bytecode interpreter below — so behaviour is unchanged.
        if self.jit_enabled && !is_vararg {
            let ptr = Rc::as_ptr(&closure_rc.function) as usize;
            let args_start = callee_index + 1;
            if self.stack.len() == args_start + arity {
                let mut numeric = true;
                for v in &self.stack[args_start..args_start + arity] {
                    if !matches!(v, Value::Number(_) | Value::Bool(_)) {
                        numeric = false;
                        break;
                    }
                }
                if numeric {
                    let (cached_fp, should_compile) = {
                        let slot = self.jit_state.entry(ptr).or_insert(JitSlot {
                            count: 0,
                            status: JitStatus::Unknown,
                            func: None,
                        });
                        match slot.status {
                            JitStatus::Compiled => (slot.func, false),
                            JitStatus::Ineligible => (None, false),
                            JitStatus::Unknown => {
                                slot.count += 1;
                                (None, slot.count >= JIT_THRESHOLD)
                            }
                        }
                    };
                    let fp = if cached_fp.is_some() {
                        cached_fp
                    } else if should_compile {
                        if self.jit.is_none() {
                            self.jit = pblang_cranelift::JitCache::new().ok();
                        }
                        let name = format!("f{:x}", ptr);
                        let compiled = self.jit.as_mut().and_then(|j| {
                            match j.compile(&name, closure_rc.function.as_ref()) {
                                pblang_cranelift::JitResult::Compiled(f) => Some(f),
                                _ => None,
                            }
                        });
                        if let Some(slot) = self.jit_state.get_mut(&ptr) {
                            match compiled {
                                Some(f) => {
                                    slot.status = JitStatus::Compiled;
                                    slot.func = Some(f);
                                }
                                None => slot.status = JitStatus::Ineligible,
                            }
                        }
                        compiled
                    } else {
                        None
                    };
                    if let Some(f) = fp {
                        let args: Vec<Value> = self.stack[args_start..args_start + arity].to_vec();
                        if let Some(ret) = pblang_cranelift::call_jit(f, &args) {
                            self.stack.truncate(callee_index);
                            self.stack.push(ret);
                            return Ok(());
                        }
                    }
                }
            }
        }

        if let Some(ref lim) = self.limits {
            if let Some(max_depth) = lim.max_stack_depth {
                if self.frames.len() + 1 > max_depth {
                    return Err(self.err("stack depth limit exceeded"));
                }
            }
        }

        self.frames.push(CallFrame {
            closure: closure_rc,
            ip: 0,
            stack_base: callee_index,
            module_result: None,
            defers: vec![],
            is_returning: false,
            is_defer_thunk: false,
            is_unwind_thunk: false,
        });

        Ok(())
    }

    /// Pop frames above `target_len`, closing upvalues, and collect their
    /// defers in execution order (innermost frame first, LIFO within a frame).
    fn pop_frames_collecting_defers(&mut self, target_len: usize) -> Vec<Value> {
        let mut run_order = Vec::new();
        while self.frames.len() > target_len {
            let Some(mut frame) = self.frames.pop() else {
                break;
            };
            self.close_upvalues_from(frame.stack_base + 1);
            run_order.extend(frame.defers.drain(..).rev());
        }
        run_order
    }

    /// Start unwinding for a runtime error raised by an instruction.
    fn begin_unwind(&mut self, e: RuntimeError) -> Result<(), RuntimeError> {
        // Is the error escaping a defer that is itself running during an unwind?
        let thunk_floor = self.unwinds.last().and_then(|u| u.thunk_frame);
        let handler = match (self.handlers.last(), thunk_floor) {
            (Some(h), Some(floor)) if h.frame_index < floor => None,
            (Some(_), _) => self.handlers.pop(),
            (None, _) => None,
        };

        if handler.is_none() {
            if let Some(floor) = thunk_floor {
                // Discard the defer's exception after unwinding its own frames;
                // their defers run before the remaining outer defers.
                let base = self.frames[floor].stack_base;
                let inner = self.pop_frames_collecting_defers(floor);
                self.close_upvalues_from(base);
                self.stack.truncate(base);
                let u = self
                    .unwinds
                    .last_mut()
                    .expect("thunk_floor implies an active unwind");
                u.thunk_frame = None;
                u.pending.extend(inner.into_iter().rev());
                return self.continue_unwind();
            }
        }

        // An uncaught error keeps the bottom (script) frame as the execution
        // context for the defers (globals resolve through it), but still runs
        // that frame's own defers last. The frame is left in place, matching the
        // previous behaviour of returning the error without popping frames.
        let target_len = match &handler {
            Some(h) => h.frame_index + 1,
            None => self.frames.len().min(1),
        };
        let mut run_order = self.pop_frames_collecting_defers(target_len);
        if handler.is_none() {
            if let Some(bottom) = self.frames.first_mut() {
                run_order.extend(bottom.defers.drain(..).rev());
            }
        }
        if let Some(h) = &handler {
            self.close_upvalues_from(h.stack_len);
            if self.stack.len() > h.stack_len {
                self.stack.truncate(h.stack_len);
            }
        }
        self.unwinds.push(UnwindState {
            error: e,
            handler,
            pending: run_order.into_iter().rev().collect(),
            thunk_frame: None,
        });
        self.continue_unwind()
    }

    /// Run the next pending defer of the innermost unwind, or finish it by
    /// jumping to its handler / returning its error.
    fn continue_unwind(&mut self) -> Result<(), RuntimeError> {
        loop {
            let next = match self.unwinds.last_mut() {
                Some(u) => u.pending.pop(),
                None => return Ok(()),
            };
            if let Some(defer_closure) = next {
                let depth = self.frames.len();
                let stack_len = self.stack.len();
                self.stack.push(defer_closure);
                match self.call_top(0) {
                    Ok(()) if self.frames.len() > depth => {
                        let idx = self.frames.len() - 1;
                        self.frames[idx].is_unwind_thunk = true;
                        if let Some(u) = self.unwinds.last_mut() {
                            u.thunk_frame = Some(idx);
                        }
                        return Ok(());
                    }
                    // A native callable completed synchronously, or the call
                    // itself failed: discard its result/error and continue.
                    _ => {
                        self.stack.truncate(stack_len);
                        continue;
                    }
                }
            }

            let u = self.unwinds.pop().expect("checked above");
            match u.handler {
                Some(h) => match self.frames.get_mut(h.frame_index) {
                    Some(frame) => {
                        frame.ip = h.target_ip;
                        // Provide the error as a value (message string)
                        self.stack.push(alloc_string(u.error.message.clone()));
                        return Ok(());
                    }
                    None => return Err(u.error),
                },
                None => return Err(u.error),
            }
        }
    }

    fn do_return(&mut self) -> Result<(), RuntimeError> {
        let mut ret = self.pop()?;

        loop {
            let has_frames = !self.frames.is_empty();
            if !has_frames {
                return Err(self.err("return with no frame"));
            }

            let frame = self
                .frames
                .last_mut()
                .expect("do_return: frames non-empty confirmed by has_frames guard above");

            // If the current frame has pending defers, run the next one
            if let Some(defer_closure) = frame.defers.pop() {
                if !frame.is_returning {
                    frame.is_returning = true;
                    if frame.module_result.is_none() {
                        frame.module_result = Some(ret);
                    }
                }

                self.stack.push(defer_closure);
                self.call_top(0)?;
                self.frames
                    .last_mut()
                    .expect("do_return: call_top pushed a frame so frames is non-empty")
                    .is_defer_thunk = true;
                return Ok(());
            }

            // No more defers for this frame. Pop it.
            let frame = self
                .frames
                .pop()
                .expect("do_return: last_mut() succeeded so pop() cannot be None");

            self.close_upvalues_from(frame.stack_base + 1);
            self.stack.truncate(frame.stack_base);

            if frame.is_unwind_thunk {
                // A defer that ran during exception unwinding returned normally.
                // Its value is discarded; the run loop continues the unwind.
                if let Some(u) = self.unwinds.last_mut() {
                    u.thunk_frame = None;
                }
                self.unwind_resume = true;
                return Ok(());
            }

            let out = frame.module_result.unwrap_or(ret);

            if frame.is_defer_thunk {
                // This frame was a defer thunk that just finished.
                // We discard its return value (`out`), and the frame below it is currently returning.
                // Loop to handle the next defer for the returning frame.
                ret = Value::Nil;
                continue;
            }

            // Normal frame finishing. Push its result and we're done.
            self.stack.push(out);
            return Ok(());
        }
    }
}

fn memory_safety_err(host: &dyn VmHost, e: MemorySafetyError) -> RuntimeError {
    host.err(e.to_string())
}

fn memory_handle_from_f64(host: &dyn VmHost, h: f64, what: &str) -> Result<MemoryId, RuntimeError> {
    if !h.is_finite() || h.fract() != 0.0 || h < 1.0 {
        return Err(host.err(format!("{what}: expected memory handle >= 1")));
    }
    Ok(MemoryId(h as usize))
}

fn borrow_type_from_code(
    host: &dyn VmHost,
    code: f64,
    what: &str,
) -> Result<BorrowType, RuntimeError> {
    match code as i64 {
        0 => Ok(BorrowType::Immutable),
        1 => Ok(BorrowType::Mutable),
        2 => Ok(BorrowType::Unique),
        _ => Err(host.err(format!("{what}: borrow type must be 0, 1, or 2"))),
    }
}

impl VM {
    fn mem_allocate_impl(&mut self, value: Value, is_mutable: bool) -> Result<f64, RuntimeError> {
        let id = self.memory_tracker.allocate(value.clone(), is_mutable);
        // Track array/map objects for mutation safety checks in bytecode.
        register_object_memory(self, &value, id);
        Ok(id.0 as f64)
    }

    fn mem_borrow_immutable_impl(&mut self, handle: f64) -> Result<f64, RuntimeError> {
        let mid = memory_handle_from_f64(self, handle, "borrow_immutable")?;
        self.memory_tracker
            .borrow_immutable(mid, None)
            .map(|b| b.0 as f64)
            .map_err(|e| memory_safety_err(self, e))
    }

    fn mem_borrow_mutable_impl(&mut self, handle: f64) -> Result<f64, RuntimeError> {
        let mid = memory_handle_from_f64(self, handle, "borrow_mutable")?;
        self.memory_tracker
            .borrow_mutable(mid, None)
            .map(|b| b.0 as f64)
            .map_err(|e| memory_safety_err(self, e))
    }

    fn mem_borrow_unique_impl(&mut self, handle: f64) -> Result<f64, RuntimeError> {
        let mid = memory_handle_from_f64(self, handle, "borrow_unique")?;
        self.memory_tracker
            .borrow_unique(mid, None)
            .map(|b| b.0 as f64)
            .map_err(|e| memory_safety_err(self, e))
    }

    fn mem_release_borrow_id_impl(&mut self, borrow_id: f64) -> Result<(), RuntimeError> {
        if !borrow_id.is_finite() || borrow_id.fract() != 0.0 || borrow_id < 1.0 {
            return Err(self.make_err(String::from("release_borrow: expected borrow id >= 1")));
        }
        self.memory_tracker
            .release_borrow_by_id(BorrowId(borrow_id as usize))
            .map_err(|e| memory_safety_err(self, e))
    }

    fn mem_release_borrow_type_impl(
        &mut self,
        handle: f64,
        borrow_type: u8,
    ) -> Result<(), RuntimeError> {
        let mid = memory_handle_from_f64(self, handle, "release_borrow")?;
        let bt = borrow_type_from_code(self, borrow_type as f64, "release_borrow")?;
        self.memory_tracker
            .release_borrow(mid, bt)
            .map_err(|e| memory_safety_err(self, e))
    }

    fn mem_load_impl(&mut self, handle: f64) -> Result<Value, RuntimeError> {
        let mid = memory_handle_from_f64(self, handle, "load")?;
        self.memory_tracker
            .get(mid)
            .map_err(|e| memory_safety_err(self, e))
    }

    fn mem_store_impl(&mut self, handle: f64, value: Value) -> Result<(), RuntimeError> {
        let mid = memory_handle_from_f64(self, handle, "store")?;
        // Unregister old object mapping if it was an array/map.
        if let Ok(old_val) = self.memory_tracker.get(mid) {
            unregister_object_memory(self, &old_val);
        }
        // Register new object mapping for the replacement value.
        register_object_memory(self, &value, mid);
        self.memory_tracker
            .set(mid, value)
            .map_err(|e| memory_safety_err(self, e))
    }

    fn mem_deallocate_impl(&mut self, handle: f64) -> Result<(), RuntimeError> {
        let mid = memory_handle_from_f64(self, handle, "deallocate")?;
        // Unregister object mapping before deallocation.
        if let Ok(old_val) = self.memory_tracker.get(mid) {
            unregister_object_memory(self, &old_val);
        }
        self.memory_tracker
            .deallocate(mid)
            .map_err(|e| memory_safety_err(self, e))
    }

    fn mem_push_lifetime_impl(&mut self) -> Result<f64, RuntimeError> {
        Ok(self.memory_tracker.push_lifetime().0 as f64)
    }

    fn mem_pop_lifetime_impl(&mut self) -> Result<f64, RuntimeError> {
        self.memory_tracker
            .pop_lifetime()
            .map(|l| l.0 as f64)
            .map_err(|e| memory_safety_err(self, e))
    }

    fn mem_stats_impl(&self) -> Result<Value, RuntimeError> {
        let s = self.memory_tracker.get_stats();
        let mut m = std::collections::HashMap::new();
        m.insert(
            "total_locations".into(),
            Value::Number(s.total_locations as f64),
        );
        m.insert(
            "active_borrows".into(),
            Value::Number(s.active_borrows as f64),
        );
        m.insert(
            "lifetime_depth".into(),
            Value::Number(s.current_lifetime_depth as f64),
        );
        m.insert(
            "ref_count_total".into(),
            Value::Number(s.ref_count_total as f64),
        );
        Ok(alloc_map(m))
    }

    /// A one-line human-readable memory/resource report: cumulative allocation
    /// counters (since VM start) plus the live memory-safety tracker. Surfaced
    /// by `pblang run --mem-report`; mirrors what `stdlib/resources.sapp` builds
    /// from `gc_stats()` + `memory_stats()`.
    pub fn memory_report(&self) -> String {
        let s = self.memory_tracker.get_stats();
        let str_a = pblang_ir::string_alloc_count();
        let arr_a = ARRAY_ALLOCS.load(Ordering::Relaxed);
        let map_a = MAP_ALLOCS.load(Ordering::Relaxed);
        let mod_a = MODULE_ALLOCS.load(Ordering::Relaxed);
        format!(
            "allocs(string={} array={} map={} module={} total={}) memory(locations={} active_borrows={} lifetime_depth={} ref_count={})",
            str_a, arr_a, map_a, mod_a,
            str_a + arr_a + map_a + mod_a,
            s.total_locations, s.active_borrows, s.current_lifetime_depth, s.ref_count_total
        )
    }
}

impl VmHost for VM {
    fn err(&self, msg: String) -> RuntimeError {
        self.make_err(msg)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn mem_allocate(&mut self, value: Value, is_mutable: bool) -> Result<f64, RuntimeError> {
        self.mem_allocate_impl(value, is_mutable)
    }

    fn mem_borrow_immutable(&mut self, handle: f64) -> Result<f64, RuntimeError> {
        self.mem_borrow_immutable_impl(handle)
    }

    fn mem_borrow_mutable(&mut self, handle: f64) -> Result<f64, RuntimeError> {
        self.mem_borrow_mutable_impl(handle)
    }

    fn mem_borrow_unique(&mut self, handle: f64) -> Result<f64, RuntimeError> {
        self.mem_borrow_unique_impl(handle)
    }

    fn mem_release_borrow_id(&mut self, borrow_id: f64) -> Result<(), RuntimeError> {
        self.mem_release_borrow_id_impl(borrow_id)
    }

    fn mem_release_borrow_type(
        &mut self,
        handle: f64,
        borrow_type: u8,
    ) -> Result<(), RuntimeError> {
        self.mem_release_borrow_type_impl(handle, borrow_type)
    }

    fn mem_load(&mut self, handle: f64) -> Result<Value, RuntimeError> {
        self.mem_load_impl(handle)
    }

    fn mem_store(&mut self, handle: f64, value: Value) -> Result<(), RuntimeError> {
        self.mem_store_impl(handle, value)
    }

    fn mem_deallocate(&mut self, handle: f64) -> Result<(), RuntimeError> {
        self.mem_deallocate_impl(handle)
    }

    fn mem_push_lifetime(&mut self) -> Result<f64, RuntimeError> {
        self.mem_push_lifetime_impl()
    }

    fn mem_pop_lifetime(&mut self) -> Result<f64, RuntimeError> {
        self.mem_pop_lifetime_impl()
    }

    fn mem_stats(&self) -> Result<Value, RuntimeError> {
        self.mem_stats_impl()
    }

    fn allow_fs(&self) -> bool {
        self.stdlib_config.as_ref().map(|c| c.fs).unwrap_or(false)
    }
    fn allow_net(&self) -> bool {
        self.stdlib_config.as_ref().map(|c| c.net).unwrap_or(false)
    }
    fn allow_http(&self) -> bool {
        self.stdlib_config.as_ref().map(|c| c.http).unwrap_or(false)
    }
    fn allow_sqlite(&self) -> bool {
        self.stdlib_config
            .as_ref()
            .map(|c| c.sqlite)
            .unwrap_or(false)
    }
    fn allow_ffi(&self) -> bool {
        self.stdlib_config.as_ref().map(|c| c.ffi).unwrap_or(false)
    }
    fn allow_exec(&self) -> bool {
        self.stdlib_config.as_ref().map(|c| c.exec).unwrap_or(false)
    }
    fn allow_env(&self) -> bool {
        // Default true (benign) when no config is set, matching the trait default;
        // an explicit StdlibConfig (CLI/embedder) controls it.
        self.stdlib_config.as_ref().map(|c| c.env).unwrap_or(true)
    }
    fn allow_process_exit(&self) -> bool {
        // Default true (CLI: the script is the process); an embedder's
        // StdlibConfig sets it false so os_exit can't kill the host.
        self.stdlib_config
            .as_ref()
            .map(|c| c.process_exit)
            .unwrap_or(true)
    }

    fn allow_import(&self) -> bool {
        self.stdlib_config
            .as_ref()
            .map(|c| c.import)
            .unwrap_or(false)
    }

    /// Library allowlist check. A missing policy denies access. When a policy is
    /// set, the path is canonicalised
    /// (best-effort) so `./lib.so` and `lib.so` resolve to the same entry, and
    /// matched against `policy.allowed_libs`. `policy.allow_all = true` bypasses
    /// the check.
    fn ffi_allowed_library(&self, path: &str) -> bool {
        let policy = match self.ffi_policy.as_ref() {
            Some(p) => p,
            None => return false,
        };
        if policy.allow_all {
            return true;
        }
        if policy.allowed_libs.contains(path) {
            return true;
        }
        // Best-effort canonicalisation: try the resolved path too, so embedders
        // can list either form. Falls back gracefully if the file doesn't exist
        // yet (Library::new will give a clearer error in that case).
        if let Ok(canon) = std::fs::canonicalize(path) {
            let canon_str = canon.to_string_lossy().to_string();
            if policy.allowed_libs.contains(&canon_str) {
                return true;
            }
        }
        false
    }

    /// Symbol allowlist check. A missing policy denies access. Otherwise the
    /// symbol must appear in `policy.allowed_symbols` unless `allow_all = true`.
    fn ffi_allowed_symbol(&self, symbol: &str) -> bool {
        let policy = match self.ffi_policy.as_ref() {
            Some(p) => p,
            None => return false,
        };
        if policy.allow_all {
            return true;
        }
        policy.allowed_symbols.contains(symbol)
    }

    fn io_timeout_ms(&self) -> Option<u64> {
        self.limits.as_ref().and_then(|l| l.io_timeout_ms)
    }

    fn gc_collect_cycles(&mut self) -> (usize, usize, usize) {
        self.gc_collect_cycles_impl()
    }

    fn allow_spawn(&self) -> bool {
        self.stdlib_config
            .as_ref()
            .map(|c| c.spawn)
            .unwrap_or(false)
    }

    fn spawn_limits(&self) -> Option<IrVmLimits> {
        self.limits.as_ref().map(|l| IrVmLimits {
            max_instructions: l.max_instructions,
            max_stack_depth: l.max_stack_depth,
            max_memory_bytes: l.max_memory_bytes,
            io_timeout_ms: l.io_timeout_ms,
        })
    }

    fn max_external_json_bytes(&self) -> usize {
        self.limits
            .as_ref()
            .and_then(|l| l.max_external_json_bytes)
            .unwrap_or(DEFAULT_MAX_EXTERNAL_JSON_BYTES)
    }

    fn import_module(&mut self, path: &str) -> Result<Value, RuntimeError> {
        let base_dir: Option<std::path::PathBuf> = self
            .frames
            .last()
            .and_then(|f| f.closure.function.chunk.source_name.as_deref().cloned())
            .and_then(|p| {
                std::path::PathBuf::from(p)
                    .parent()
                    .map(|p| p.to_path_buf())
            });

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let resolved = pblang_pm::resolve_import(path, base_dir.as_deref(), &cwd)
            .map_err(|m| self.make_err(format!("import '{path}': {m}")))?;
        let key = resolved.to_string_lossy().to_string();

        {
            let stack = self.import_stack.borrow();
            if stack.iter().any(|p| p == &key) {
                let mut cycle = stack.clone();
                cycle.push(key.clone());
                return Err(self.make_err(format!("import cycle detected: {}", cycle.join(" -> "))));
            }
        }

        if let Some(m) = self.module_cache.borrow().get(&key).cloned() {
            return Ok(Value::Module(m));
        }

        self.import_stack.borrow_mut().push(key.clone());
        let result = (|| {
            let src = std::fs::read_to_string(&key)
                .map_err(|e| self.make_err(format!("import '{key}': failed to read file: {e}")))?;
            let (stmts, polyglot_blocks) = pblang_core::parse_with_polyglot(&src)
                .map_err(|e| self.make_err(format!("import '{key}': parse failed: {e:?}")))?;
            let mut chunk = pblang_compiler::compile(stmts)
                .map_err(|e| self.make_err(format!("import '{key}': compile error: {e}")))?;
            chunk.source_name = Some(Rc::new(key.clone()));
            chunk.source = Some(Rc::new(src));

            let module_rc = Rc::new(RefCell::new(Module {
                globals: Default::default(),
                parent: Some(self.builtins.clone()),
                exports: Default::default(),
            }));
            MODULE_ALLOCS.fetch_add(1, Ordering::Relaxed);
            gc_register_module(&module_rc);

            // Make polyglot blocks available inside the module.
            let (blocks_v, poly_v) = VM::polyglot_blocks_to_values(polyglot_blocks);
            {
                let mut g = module_rc.borrow_mut();
                g.globals.insert("__polyglot_blocks".into(), blocks_v);
                g.globals.insert("__polyglot".into(), poly_v);
            }

            // Cache before executing so repeated imports share identity.
            self.module_cache
                .borrow_mut()
                .insert(key.clone(), module_rc.clone());

            let mut vm = self.new_for_import(module_rc.clone());
            vm.run(chunk)?;

            Ok(Value::Module(module_rc))
        })();

        // Pop import stack even on failure.
        let _ = self.import_stack.borrow_mut().pop();

        // If import failed, remove from cache so retry works and we don't keep half-initialized modules.
        if result.is_err() {
            self.module_cache.borrow_mut().remove(&key);
        }

        result
    }
}
