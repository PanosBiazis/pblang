use pblang_ir::chunk::{Chunk, OpCode, Span};
use pblang_ir::{
    alloc_string, string_alloc_count, BuiltinFn, Closure, Function, Module, Obj, Upvalue, Value,
    VmHost, VmLimits as IrVmLimits,
};
pub use pblang_ir::{FrameTrace, RuntimeError};

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use std::{
    io::Read,
    io::Write,
    net::{TcpListener, TcpStream},
};

// Import our new modules
use crate::concurrency::{
    ThreadPool, VmChannelBuf, VmHandleRegistry, VmSyncAtomic, VmSyncBarrier, VmSyncMutex, VmSyncOnce,
    VmSyncRwLock,
};
use crate::memory_safety::{BorrowId, BorrowType, MemoryId, MemorySafetyError, MemoryTracker};

// TLS support via rustls
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned,
};
use rustls_native_certs;
use webpki_roots;

use libloading::Library;
use rand::Rng; // for random_bytes builtin
use regex; // for regex_* builtins
use rusqlite::Connection;
use serde_json as sj;
use sha2::Digest;
static ARRAY_ALLOCS: AtomicUsize = AtomicUsize::new(0);
static MAP_ALLOCS: AtomicUsize = AtomicUsize::new(0);
static MODULE_ALLOCS: AtomicUsize = AtomicUsize::new(0);

// --- Tracing / cycle collection (mark & sweep over RC containers) ---
//
// Sapphire's runtime objects are mostly reference-counted. RC cycles can leak
// (e.g. `let a=[]; a[0]=a;`). To address this without rewriting the runtime,
// we keep weak registries of container objects and run a tracing pass from VM
// roots, then *clear* unreachable containers to break cycles.
type GcArrayRegistry = HashMap<usize, Weak<RefCell<Vec<Value>>>>;
type GcMapRegistry = HashMap<usize, Weak<RefCell<HashMap<String, Value>>>>;
type GcModuleRegistry = HashMap<usize, Weak<RefCell<Module>>>;
thread_local! {
    static GC_ARRAYS: RefCell<GcArrayRegistry> = RefCell::new(HashMap::new());
    static GC_MAPS: RefCell<GcMapRegistry> = RefCell::new(HashMap::new());
    static GC_MODULES: RefCell<GcModuleRegistry> = RefCell::new(HashMap::new());
}

fn gc_register_array(a: &Rc<RefCell<Vec<Value>>>) {
    let id = a.as_ptr() as usize;
    GC_ARRAYS.with(|t| {
        t.borrow_mut().insert(id, Rc::downgrade(a));
    });
}

fn gc_register_map(m: &Rc<RefCell<HashMap<String, Value>>>) {
    let id = m.as_ptr() as usize;
    GC_MAPS.with(|t| {
        t.borrow_mut().insert(id, Rc::downgrade(m));
    });
}

fn gc_register_module(m: &Rc<RefCell<Module>>) {
    let id = m.as_ptr() as usize;
    GC_MODULES.with(|t| {
        t.borrow_mut().insert(id, Rc::downgrade(m));
    });
}

fn alloc_array(items: Vec<Value>) -> Value {
    ARRAY_ALLOCS.fetch_add(1, Ordering::Relaxed);
    let rc = Rc::new(RefCell::new(items));
    gc_register_array(&rc);
    Value::Array(rc)
}

fn alloc_map(items: HashMap<String, Value>) -> Value {
    MAP_ALLOCS.fetch_add(1, Ordering::Relaxed);
    let rc = Rc::new(RefCell::new(items));
    gc_register_map(&rc);
    Value::Map(rc)
}

#[derive(Debug)]
enum NetResource {
    Listener(TcpListener),
    TlsListener(TcpListener, Arc<ServerConfig>),
    Stream(NetStream),
}

// StreamKind abstracts over plain and TLS streams for transparent tcp_read/tcp_write
enum StreamKind {
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

#[derive(Debug)]
struct NetStream {
    inner: StreamKind,
    buf: Vec<u8>,
}

static NET_TABLE: OnceLock<Mutex<HashMap<usize, NetResource>>> = OnceLock::new();
static NET_NEXT: AtomicUsize = AtomicUsize::new(1);

fn net_table() -> &'static Mutex<HashMap<usize, NetResource>> {
    NET_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug)]
enum FfiResource {
    Library(Library),
}

static FFI_TABLE: OnceLock<Mutex<HashMap<usize, FfiResource>>> = OnceLock::new();
static FFI_NEXT: AtomicUsize = AtomicUsize::new(1);

fn ffi_table() -> &'static Mutex<HashMap<usize, FfiResource>> {
    FFI_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug)]
enum DbResource {
    Sqlite(Connection),
}

static DB_TABLE: OnceLock<Mutex<HashMap<usize, DbResource>>> = OnceLock::new();
static DB_NEXT: AtomicUsize = AtomicUsize::new(1);

fn db_table() -> &'static Mutex<HashMap<usize, DbResource>> {
    DB_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn vm_handles(host: &mut dyn VmHost) -> Result<Arc<VmHandleRegistry>, RuntimeError> {
    host.as_any_mut()
        .downcast_mut::<VM>()
        .map(|vm| Arc::clone(&vm.handle_registry))
        .ok_or_else(|| host.err("concurrency: internal host error".into()))
}

/// Serialize a Value to JSON for sending to a spawn child (transferable types only).
fn spawn_value_to_json(v: &Value) -> Result<sj::Value, String> {
    match v {
        Value::Nil => Ok(sj::Value::Null),
        Value::Bool(b) => Ok(sj::Value::Bool(*b)),
        Value::Number(n) => Ok(sj::Number::from_f64(*n)
            .map(sj::Value::Number)
            .unwrap_or(sj::Value::Null)),
        Value::Obj(o) => match o.as_ref() {
            Obj::String(s) => Ok(sj::Value::String(s.clone())),
        },
        Value::Array(a) => {
            let a = a.borrow();
            let mut out = Vec::with_capacity(a.len());
            for x in a.iter() {
                out.push(spawn_value_to_json(x)?);
            }
            Ok(sj::Value::Array(out))
        }
        Value::Map(m) => {
            let m = m.borrow();
            let mut out = sj::Map::new();
            for (k, vv) in m.iter() {
                out.insert(k.clone(), spawn_value_to_json(vv)?);
            }
            Ok(sj::Value::Object(out))
        }
        _ => Err(
            "spawn: value not transferable (use only numbers, bools, strings, arrays, maps)".into(),
        ),
    }
}

/// Deserialize JSON to Value (used in spawn child thread).
fn spawn_value_from_json(v: &sj::Value) -> Value {
    match v {
        sj::Value::Null => Value::Nil,
        sj::Value::Bool(b) => Value::Bool(*b),
        sj::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        sj::Value::String(s) => alloc_string(s.clone()),
        sj::Value::Array(items) => alloc_array(items.iter().map(spawn_value_from_json).collect()),
        sj::Value::Object(map) => {
            let mut out = HashMap::new();
            for (k, vv) in map.iter() {
                out.insert(k.clone(), spawn_value_from_json(vv));
            }
            alloc_map(out)
        }
    }
}

fn value_to_handle(host: &mut dyn VmHost, v: Value, what: &str) -> Result<usize, RuntimeError> {
    match v {
        Value::Number(n) if n.is_finite() && n.fract() == 0.0 && n >= 1.0 => Ok(n as usize),
        _ => Err(host.err(format!("{what}: expected handle number"))),
    }
}

// Safe table access: on Mutex poison return RuntimeError instead of panicking (crash resistance).
fn with_ffi_table<F, R>(host: &mut dyn VmHost, f: F) -> Result<R, RuntimeError>
where
    F: FnOnce(&mut HashMap<usize, FfiResource>) -> R,
{
    let mut t = ffi_table()
        .lock()
        .map_err(|_| host.err("internal: ffi table unavailable".into()))?;
    Ok(f(&mut t))
}

fn with_db_table<F, R>(host: &mut dyn VmHost, f: F) -> Result<R, RuntimeError>
where
    F: FnOnce(&mut HashMap<usize, DbResource>) -> R,
{
    let mut t = db_table()
        .lock()
        .map_err(|_| host.err("internal: db table unavailable".into()))?;
    Ok(f(&mut t))
}

fn with_net_table<F, R>(host: &mut dyn VmHost, f: F) -> Result<R, RuntimeError>
where
    F: FnOnce(&mut HashMap<usize, NetResource>) -> R,
{
    let mut t = net_table()
        .lock()
        .map_err(|_| host.err("internal: net table unavailable".into()))?;
    Ok(f(&mut t))
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
    pub exec: bool,
    pub gfx: bool,
    /// If true, spawn/join builtins are available; if false, spawn returns "spawn not allowed".
    pub spawn: bool,
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
            exec: false,
            gfx: true,
            spawn: true,
        }
    }
}

impl StdlibConfig {
    /// Returns true if the builtin with this name is in an enabled category.
    ///
    /// AUDIT FIX #14: Previously this method was dead code (never called) while all
    /// builtins were unconditionally registered, making them visible as globals even
    /// when a capability was disabled. `install_builtins_into()` now calls this to
    /// skip registration of builtins whose category is disabled, so disabled builtins
    /// are invisible (not just call-time-denied).
    pub fn allows(&self, name: &str) -> bool {
        match name {
            "clock" | "now_ms" | "now_iso" | "type" | "str" | "num_sqrt" | "num_pow" | "num_ln"
            | "num_exp" | "num_atan2" | "num_mod" | "num_floor" | "num_ceil" | "parse_number"
            | "parse_bool" | "read_line" | "len" | "array_push" | "array_pop" | "map_keys"
            | "map_has" | "map_remove" | "import" | "gc_stats" | "gc_collect" | "str_contains"
            | "str_starts_with" | "str_trim" | "str_split" | "regex_match" | "regex_find"
            | "regex_find_all" | "regex_replace" | "regex_replace_all" | "regex_split"
            | "regex_capture" | "str_format" | "str_repeat" | "str_pad_left" | "str_pad_right"
            | "str_upper" | "str_lower" | "array_unique" | "array_zip" | "array_chunk"
            | "array_flatten" | "array_take" | "array_drop" | "range" | "enumerate" | "sum"
            | "min_of" | "max_of" | "product" | "clamp" | "is_number" | "is_string"
            | "is_array" | "is_map" | "is_bool" | "is_fn" | "is_nil" | "env_get" | "env_set"
            | "str_replace_all" => self.core,
            "fs_read" | "fs_write" | "fs_append" | "fs_remove" | "fs_rename" | "fs_copy"
            | "fs_read_bytes" | "fs_write_bytes" | "fs_exists" | "fs_mkdirs" | "fs_listdir"
            | "path_join" | "path_dirname" | "path_basename" | "cwd" => self.fs,
            "sleep_ms"
            | "exec"
            | "proc_spawn_wait"
            | "os_which"
            | "quantum_run"
            | "quantum_statevector"
            | "quantum_backends"
            | "polyglot_exec" => self.exec,
            "sha256" | "random_bytes" | "random_float" | "constant_time_eq" | "base64_encode"
            | "base64_decode" | "hmac_sha256" => self.crypto,
            "json_parse" | "json_stringify" => self.json,
            "http_get" | "http_post" | "ai_chat" | "ai_embed" => self.http,
            "tcp_listen" | "tcp_accept" | "tcp_read" | "tcp_read_until" | "tcp_write"
            | "tcp_close" | "tcp_local_addr" | "tcp_listen_tls" | "tcp_connect_tls"
            | "tcp_wrap_tls" | "http_parse_request" | "http_response" | "http_response_ct"
            | "http_response_json" | "onnx_infer" | "ai_token_count" => self.net,
            "ffi_load" | "ffi_call" | "ffi_close" => self.ffi,
            "sqlite_open"
            | "sqlite_close"
            | "sqlite_exec"
            | "sqlite_query"
            | "sqlite_last_insert_rowid" => self.sqlite,
            "spawn" | "join" => self.spawn,
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
    /// Max heap memory in bytes (optional). Enforcement is best-effort when set.
    pub max_memory_bytes: Option<usize>,
    /// Timeout in ms for IO builtins (e.g. HTTP, TCP read, fs read). Not yet enforced by all builtins.
    pub io_timeout_ms: Option<u64>,
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
    /// Capability config for sandboxing; None = minimal VM (no builtins).
    stdlib_config: Option<StdlibConfig>,
    /// Optional resource limits (instruction count, stack depth).
    limits: Option<VmLimits>,
    /// Optional FFI allowlist; when Some and allow_all is false, only listed libs/symbols are allowed.
    ffi_policy: Option<FfiPolicy>,
    /// Memory safety tracker for Rust-like ownership and borrowing
    memory_tracker: MemoryTracker,
    /// Concurrency primitives for thread-safe operations
    thread_pool: Option<ThreadPool>,
    /// Spawn/channel/sync handle tables (shared with child VMs from `spawn`).
    handle_registry: Arc<VmHandleRegistry>,
}

#[derive(Default)]
struct GcMarks {
    arrays: HashSet<usize>,
    maps: HashSet<usize>,
    modules: HashSet<usize>,
    closures: HashSet<usize>,
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

    fn gc_collect_cycles_impl(&mut self) -> (usize, usize, usize) {
        let mut marks = GcMarks::default();

        // --- roots: VM stack values ---
        for v in self.stack.iter() {
            self.gc_mark_value(v, &mut marks);
        }

        // --- roots: active call frames / closures ---
        for f in self.frames.iter() {
            self.gc_mark_closure(&f.closure, &mut marks);
        }

        // --- roots: closed upvalues (open upvalues point into stack) ---
        for uv in self.open_upvalues.iter() {
            if let Upvalue::Closed(v) = &*uv.borrow() {
                self.gc_mark_value(v, &mut marks);
            }
        }

        // --- roots: module chain and module cache ---
        self.gc_mark_module(&self.module, &mut marks);
        self.gc_mark_module(&self.builtins, &mut marks);
        for m in self.module_cache.borrow().values() {
            self.gc_mark_module(m, &mut marks);
        }

        // --- sweep: clear unreachable containers to break RC cycles ---
        let arrays_cleared = self.gc_sweep_arrays(&marks.arrays);
        let maps_cleared = self.gc_sweep_maps(&marks.maps);
        let modules_cleared = self.gc_sweep_modules(&marks.modules);

        (arrays_cleared, maps_cleared, modules_cleared)
    }

    fn gc_mark_closure(&self, c: &Rc<Closure>, marks: &mut GcMarks) {
        let id = Rc::as_ptr(c) as usize;
        if !marks.closures.insert(id) {
            return;
        }
        self.gc_mark_module(&c.module, marks);
        for uv in c.upvalues.iter() {
            if let Upvalue::Closed(v) = &*uv.borrow() {
                self.gc_mark_value(v, marks);
            }
        }
    }

    fn gc_mark_module(&self, m: &Rc<RefCell<Module>>, marks: &mut GcMarks) {
        let id = Rc::as_ptr(m) as usize;
        if !marks.modules.insert(id) {
            return;
        }
        let mb = m.borrow();
        for v in mb.globals.values() {
            self.gc_mark_value(v, marks);
        }
        for v in mb.exports.values() {
            self.gc_mark_value(v, marks);
        }
        if let Some(p) = mb.parent.clone() {
            drop(mb);
            self.gc_mark_module(&p, marks);
        }
    }

    fn gc_mark_value(&self, v: &Value, marks: &mut GcMarks) {
        match v {
            Value::Array(a) => {
                let id = Rc::as_ptr(a) as usize;
                if !marks.arrays.insert(id) {
                    return;
                }
                for it in a.borrow().iter() {
                    self.gc_mark_value(it, marks);
                }
            }
            Value::Map(m) => {
                let id = Rc::as_ptr(m) as usize;
                if !marks.maps.insert(id) {
                    return;
                }
                for it in m.borrow().values() {
                    self.gc_mark_value(it, marks);
                }
            }
            Value::Module(m) => self.gc_mark_module(m, marks),
            Value::Closure(c) => self.gc_mark_closure(c, marks),
            // Functions don't directly reference runtime values; closures cover upvalues/module.
            _ => {}
        }
    }

    fn gc_sweep_arrays(&self, live: &HashSet<usize>) -> usize {
        GC_ARRAYS.with(|tbl| {
            let mut cleared = 0usize;
            let mut t = tbl.borrow_mut();
            let keys = t.keys().cloned().collect::<Vec<_>>();
            for id in keys {
                let Some(w) = t.get(&id).cloned() else {
                    continue;
                };
                match w.upgrade() {
                    Some(rc) => {
                        if !live.contains(&id) {
                            rc.borrow_mut().clear();
                            cleared += 1;
                        }
                    }
                    None => {
                        t.remove(&id);
                    }
                }
            }
            cleared
        })
    }

    fn gc_sweep_maps(&self, live: &HashSet<usize>) -> usize {
        GC_MAPS.with(|tbl| {
            let mut cleared = 0usize;
            let mut t = tbl.borrow_mut();
            let keys = t.keys().cloned().collect::<Vec<_>>();
            for id in keys {
                let Some(w) = t.get(&id).cloned() else {
                    continue;
                };
                match w.upgrade() {
                    Some(rc) => {
                        if !live.contains(&id) {
                            rc.borrow_mut().clear();
                            cleared += 1;
                        }
                    }
                    None => {
                        t.remove(&id);
                    }
                }
            }
            cleared
        })
    }

    fn gc_sweep_modules(&self, live: &HashSet<usize>) -> usize {
        GC_MODULES.with(|tbl| {
            let mut cleared = 0usize;
            let mut t = tbl.borrow_mut();
            let keys = t.keys().cloned().collect::<Vec<_>>();
            for id in keys {
                let Some(w) = t.get(&id).cloned() else {
                    continue;
                };
                match w.upgrade() {
                    Some(rc) => {
                        if !live.contains(&id) {
                            let mut m = rc.borrow_mut();
                            m.globals.clear();
                            m.exports.clear();
                            m.parent = None;
                            cleared += 1;
                        }
                    }
                    None => {
                        t.remove(&id);
                    }
                }
            }
            cleared
        })
    }

    fn get_global_from(module: &Rc<RefCell<Module>>, name: &str) -> Option<Value> {
        let mut cur = Some(module.clone());
        while let Some(m) = cur {
            if let Some(v) = m.borrow().globals.get(name).cloned() {
                return Some(v);
            }
            cur = m.borrow().parent.clone();
        }
        None
    }

    fn set_global_in_chain(module: &Rc<RefCell<Module>>, name: &str, v: Value) -> bool {
        let mut cur = Some(module.clone());
        while let Some(m) = cur {
            if m.borrow().globals.contains_key(name) {
                m.borrow_mut().globals.insert(name.to_string(), v);
                return true;
            }
            cur = m.borrow().parent.clone();
        }
        false
    }

    fn builtins() -> Vec<BuiltinFn> {
        fn clock(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !args.is_empty() {
                return Err(host.err("clock() takes no arguments".into()));
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| host.err("clock(): system time before UNIX_EPOCH".into()))?;
            Ok(Value::Number(now.as_secs_f64()))
        }

        fn now_ms(_host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !args.is_empty() {
                return Err(_host.err("now_ms() takes no arguments".into()));
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| _host.err("now_ms(): system time before UNIX_EPOCH".into()))?;
            // BUG FIX #16: as_millis() returns u128; casting to f64 loses precision
            // beyond 2^53 ms (~year 2255). Use secs_f64 * 1000 for lossless conversion.
            Ok(Value::Number(now.as_secs_f64() * 1000.0))
        }

        fn now_iso(_host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !args.is_empty() {
                return Err(_host.err("now_iso() takes no arguments".into()));
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| _host.err("now_iso(): system time before UNIX_EPOCH".into()))?;
            let secs = now.as_secs();
            // FIX: Replaced O(years) year-by-year loop with O(1) Gregorian formula.
            // The old implementation would be slow for timestamps far in the future.
            let s_in_day = secs % 86400;
            let hours = s_in_day / 3600;
            let minutes = (s_in_day % 3600) / 60;
            let seconds = s_in_day % 60;

            // Algorithm: civil_from_days (Howard Hinnant, public domain).
            // Computes (year, month, day) from a Unix day count in O(1).
            let z: i64 = (secs / 86400) as i64 + 719468;
            let era: i64 = if z >= 0 { z } else { z - 146096 } / 146097;
            let doe = (z - era * 146097) as u64; // day of era [0, 146096]
            let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
            let y = yoe as i64 + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
            let mp = (5 * doy + 2) / 153; // month of year [0, 11]
            let day = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let y = if month <= 2 { y + 1 } else { y };
            let (y, month, day) = (y as u64, month, day);
            let iso = format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                y, month, day, hours, minutes, seconds
            );
            Ok(alloc_string(iso))
        }

        fn proc_spawn_wait(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_exec() {
                return Err(host.err("proc_spawn_wait access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!(
                    "proc_spawn_wait() expects 2 arguments (cmd, args), got {}",
                    args.len()
                )));
            }
            let cmd = args[0]
                .as_str()
                .map_err(|m| host.err(format!("proc_spawn_wait(): {m}")))?;
            let argv = match &args[1] {
                Value::Array(arr) => {
                    let mut v: Vec<String> = Vec::new();
                    for item in arr.borrow().iter() {
                        let s = item
                            .as_str()
                            .map_err(|m| host.err(format!("proc_spawn_wait(): arg: {m}")))?;
                        v.push(s.to_string());
                    }
                    v
                }
                _ => {
                    return Err(
                        host.err("proc_spawn_wait(): second argument must be an array".into())
                    )
                }
            };
            let output = std::process::Command::new(cmd)
                .args(&argv)
                .output()
                .map_err(|e| host.err(format!("proc_spawn_wait(): {e}")))?;
            let exit_code = output.status.code().unwrap_or(1) as f64;
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let mut m = std::collections::HashMap::new();
            m.insert("exit_code".to_string(), Value::Number(exit_code));
            m.insert("stdout".to_string(), alloc_string(stdout));
            m.insert("stderr".to_string(), alloc_string(stderr));
            Ok(alloc_map(m))
        }

        fn type_of(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("type() expects 1 argument, got {}", args.len())));
            }
            let s = match &args[0] {
                Value::Number(_) => "number",
                Value::Bool(_) => "bool",
                Value::Nil => "nil",
                Value::Obj(o) => match o.as_ref() {
                    Obj::String(_) => "string",
                },
                Value::Function(_) => "fn",
                Value::Closure(_) => "closure",
                Value::Builtin(_) => "builtin",
                Value::Module(_) => "module",
                Value::Array(_) => "array",
                Value::Map(_) => "map",
            };
            Ok(alloc_string(s.to_string()))
        }

        fn str_(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("str() expects 1 argument, got {}", args.len())));
            }
            Ok(alloc_string(args[0].to_string()))
        }

        fn num_sqrt(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("num_sqrt() expects 1 argument, got {}", args.len())));
            }
            let n = match args[0] {
                Value::Number(n) => n,
                _ => return Err(host.err("num_sqrt(): expected number".into())),
            };
            Ok(Value::Number(n.sqrt()))
        }

        fn num_pow(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!("num_pow() expects 2 arguments, got {}", args.len())));
            }
            let a = match args[0] {
                Value::Number(n) => n,
                _ => return Err(host.err("num_pow(): expected number base".into())),
            };
            let b = match args[1] {
                Value::Number(n) => n,
                _ => return Err(host.err("num_pow(): expected number exponent".into())),
            };
            Ok(Value::Number(a.powf(b)))
        }

        fn num_ln(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("num_ln() expects 1 argument, got {}", args.len())));
            }
            let n = match args[0] {
                Value::Number(n) => n,
                _ => return Err(host.err("num_ln(): expected number".into())),
            };
            Ok(Value::Number(n.ln()))
        }

        fn num_exp(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("num_exp() expects 1 argument, got {}", args.len())));
            }
            let n = match args[0] {
                Value::Number(n) => n,
                _ => return Err(host.err("num_exp(): expected number".into())),
            };
            Ok(Value::Number(n.exp()))
        }

        fn num_atan2(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "num_atan2() expects 2 arguments (y,x), got {}",
                    args.len()
                )));
            }
            let y = match args[0] {
                Value::Number(n) => n,
                _ => return Err(host.err("num_atan2(): expected number y".into())),
            };
            let x = match args[1] {
                Value::Number(n) => n,
                _ => return Err(host.err("num_atan2(): expected number x".into())),
            };
            Ok(Value::Number(y.atan2(x)))
        }

        fn num_mod(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!("num_mod() expects 2 arguments, got {}", args.len())));
            }
            let a = match args[0] {
                Value::Number(n) => n,
                _ => return Err(host.err("num_mod(): expected number a".into())),
            };
            let b = match args[1] {
                Value::Number(n) => n,
                _ => return Err(host.err("num_mod(): expected number b".into())),
            };
            Ok(Value::Number(a % b))
        }

        fn num_floor(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "num_floor() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let n = match args[0] {
                Value::Number(x) => x,
                _ => return Err(host.err("num_floor(): expected number".into())),
            };
            Ok(Value::Number(n.floor()))
        }

        fn num_ceil(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("num_ceil() expects 1 argument, got {}", args.len())));
            }
            let n = match args[0] {
                Value::Number(x) => x,
                _ => return Err(host.err("num_ceil(): expected number".into())),
            };
            Ok(Value::Number(n.ceil()))
        }

        fn parse_number(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "parse_number() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("parse_number(): {m}")))?;
            let n: f64 = s
                .trim()
                .parse()
                .map_err(|e| host.err(format!("parse_number(): {e}")))?;
            Ok(Value::Number(n))
        }

        fn parse_bool(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "parse_bool() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("parse_bool(): {m}")))?;
            let t = s.trim().to_ascii_lowercase();
            match t.as_str() {
                "true" => Ok(Value::Bool(true)),
                "false" => Ok(Value::Bool(false)),
                _ => Err(host.err("parse_bool(): expected 'true' or 'false'".into())),
            }
        }

        fn read_line(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !args.is_empty() {
                return Err(host.err(format!(
                    "read_line() takes no arguments, got {}",
                    args.len()
                )));
            }
            let mut s = String::new();
            let n = std::io::stdin()
                .read_line(&mut s)
                .map_err(|e| host.err(format!("read_line(): {e}")))?;
            if n == 0 {
                return Ok(Value::Nil);
            }
            // BUG FIX #15: Strip trailing '\n'/'\r\n' so comparisons like `if input == "yes"` work
            // as expected (same as Python/Ruby readline behavior).
            let s = s.trim_end_matches(['\n', '\r']).to_string();
            Ok(alloc_string(s))
        }

        fn len(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("len() expects 1 argument, got {}", args.len())));
            }
            match &args[0] {
                Value::Obj(o) => match o.as_ref() {
                    Obj::String(s) => Ok(Value::Number(s.chars().count() as f64)),
                },
                Value::Array(a) => Ok(Value::Number(a.borrow().len() as f64)),
                Value::Map(m) => Ok(Value::Number(m.borrow().len() as f64)),
                _ => Err(host.err("len() currently supports strings/arrays/maps only".into())),
            }
        }

        fn array_push(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "array_push() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let a = match &args[0] {
                Value::Array(a) => a.clone(),
                _ => return Err(host.err("array_push(): expected array".into())),
            };
            a.borrow_mut().push(args[1].clone());
            let n = { a.borrow().len() };
            Ok(Value::Number(n as f64))
        }

        fn array_pop(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "array_pop() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let a = match &args[0] {
                Value::Array(a) => a.clone(),
                _ => return Err(host.err("array_pop(): expected array".into())),
            };
            let v = a.borrow_mut().pop().unwrap_or(Value::Nil);
            Ok(v)
        }

        fn map_keys(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("map_keys() expects 1 argument, got {}", args.len())));
            }
            let m = match &args[0] {
                Value::Map(m) => m.clone(),
                _ => return Err(host.err("map_keys(): expected map".into())),
            };
            let mut keys = m
                .borrow()
                .keys()
                .map(|k| alloc_string(k.clone()))
                .collect::<Vec<_>>();
            keys.sort_by_key(|a| a.to_string());
            Ok(alloc_array(keys))
        }

        fn map_has(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!("map_has() expects 2 arguments, got {}", args.len())));
            }
            let m = match &args[0] {
                Value::Map(m) => m.clone(),
                _ => return Err(host.err("map_has(): expected map".into())),
            };
            let k = args[1]
                .as_str()
                .map_err(|mm| host.err(format!("map_has(): {mm}")))?;
            let has = { m.borrow().contains_key(k) };
            Ok(Value::Bool(has))
        }

        fn map_remove(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "map_remove() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let m = match &args[0] {
                Value::Map(m) => m.clone(),
                _ => return Err(host.err("map_remove(): expected map".into())),
            };
            let k = args[1]
                .as_str()
                .map_err(|mm| host.err(format!("map_remove(): {mm}")))?;
            let v = m.borrow_mut().remove(k).unwrap_or(Value::Nil);
            Ok(v)
        }

        fn fs_read(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!("fs_read() expects 1 argument, got {}", args.len())));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("fs_read(): {m}")))?
                .to_string();
            let s = if let Some(ms) = host.io_timeout_ms() {
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    let _ = tx.send(std::fs::read_to_string(&path));
                });
                rx.recv_timeout(Duration::from_millis(ms))
                    .map_err(|_| host.err("fs_read(): timeout".into()))?
                    .map_err(|e| host.err(format!("fs_read(): {e}")))?
            } else {
                std::fs::read_to_string(&path).map_err(|e| host.err(format!("fs_read(): {e}")))?
            };
            Ok(alloc_string(s))
        }

        fn fs_write(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!(
                    "fs_write() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("fs_write(): {m}")))?
                .to_string();
            let contents = args[1]
                .as_str()
                .map_err(|m| host.err(format!("fs_write(): {m}")))?
                .to_string();
            if let Some(ms) = host.io_timeout_ms() {
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    let _ = tx.send(std::fs::write(&path, &contents));
                });
                rx.recv_timeout(Duration::from_millis(ms))
                    .map_err(|_| host.err("fs_write(): timeout".into()))?
                    .map_err(|e| host.err(format!("fs_write(): {e}")))?
            } else {
                std::fs::write(&path, &contents)
                    .map_err(|e| host.err(format!("fs_write(): {e}")))?
            };
            Ok(Value::Nil)
        }

        fn fs_append(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!(
                    "fs_append() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("fs_append(): {m}")))?;
            let contents = args[1]
                .as_str()
                .map_err(|m| host.err(format!("fs_append(): {m}")))?;
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| host.err(format!("fs_append(): {e}")))?;
            f.write_all(contents.as_bytes())
                .map_err(|e| host.err(format!("fs_append(): {e}")))?;
            Ok(Value::Nil)
        }

        fn fs_remove(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "fs_remove() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("fs_remove(): {m}")))?;
            // try file, then dir
            if std::fs::remove_file(path).is_ok() {
                return Ok(Value::Nil);
            }
            std::fs::remove_dir_all(path).map_err(|e| host.err(format!("fs_remove(): {e}")))?;
            Ok(Value::Nil)
        }

        fn fs_rename(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!(
                    "fs_rename() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let from = args[0]
                .as_str()
                .map_err(|m| host.err(format!("fs_rename(): {m}")))?;
            let to = args[1]
                .as_str()
                .map_err(|m| host.err(format!("fs_rename(): {m}")))?;
            std::fs::rename(from, to).map_err(|e| host.err(format!("fs_rename(): {e}")))?;
            Ok(Value::Nil)
        }

        fn fs_copy(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!("fs_copy() expects 2 arguments, got {}", args.len())));
            }
            let from = args[0]
                .as_str()
                .map_err(|m| host.err(format!("fs_copy(): {m}")))?;
            let to = args[1]
                .as_str()
                .map_err(|m| host.err(format!("fs_copy(): {m}")))?;
            std::fs::copy(from, to).map_err(|e| host.err(format!("fs_copy(): {e}")))?;
            Ok(Value::Nil)
        }

        fn fs_read_bytes(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "fs_read_bytes() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("fs_read_bytes(): {m}")))?
                .to_string();
            let bytes = if let Some(ms) = host.io_timeout_ms() {
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    let _ = tx.send(std::fs::read(&path));
                });
                rx.recv_timeout(Duration::from_millis(ms))
                    .map_err(|_| host.err("fs_read_bytes(): timeout".into()))?
                    .map_err(|e| host.err(format!("fs_read_bytes(): {e}")))?
            } else {
                std::fs::read(&path).map_err(|e| host.err(format!("fs_read_bytes(): {e}")))?
            };
            let mut out: Vec<Value> = Vec::with_capacity(bytes.len());
            for b in bytes {
                out.push(Value::Number(b as f64));
            }
            Ok(alloc_array(out))
        }

        fn fs_write_bytes(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!(
                    "fs_write_bytes() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("fs_write_bytes(): {m}")))?;
            let arr = match &args[1] {
                Value::Array(a) => a.clone(),
                _ => return Err(host.err("fs_write_bytes(): expected byte array".into())),
            };
            let mut bytes: Vec<u8> = Vec::with_capacity(arr.borrow().len());
            for v in arr.borrow().iter() {
                match v {
                    Value::Number(n)
                        if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= 255.0 =>
                    {
                        bytes.push(*n as u8);
                    }
                    _ => {
                        return Err(
                            host.err("fs_write_bytes(): expected byte numbers 0..255".into())
                        )
                    }
                }
            }
            let path = path.to_string();
            if let Some(ms) = host.io_timeout_ms() {
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    let _ = tx.send(std::fs::write(&path, &bytes));
                });
                rx.recv_timeout(Duration::from_millis(ms))
                    .map_err(|_| host.err("fs_write_bytes(): timeout".into()))?
                    .map_err(|e| host.err(format!("fs_write_bytes(): {e}")))?
            } else {
                std::fs::write(&path, &bytes)
                    .map_err(|e| host.err(format!("fs_write_bytes(): {e}")))?
            };
            Ok(Value::Nil)
        }

        fn env_get(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            // env_get reads environment variables — always safe (read-only, no exec required).
            if args.len() != 1 {
                return Err(host.err(format!("env_get() expects 1 argument, got {}", args.len())));
            }
            let k = args[0]
                .as_str()
                .map_err(|m| host.err(format!("env_get(): {m}")))?;
            match std::env::var(k) {
                Ok(v) => Ok(alloc_string(v)),
                Err(_) => Ok(Value::Nil),
            }
        }

        fn env_set(host: &mut dyn VmHost, _args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_exec() {
                return Err(host.err("exec/env access denied".into()));
            }
            // SECURITY: std::env::set_var is unsound in multi-threaded programs (undefined
            // behaviour when called concurrently with env reads on Linux). Since pblang
            // supports VM-per-thread spawning, this builtin is permanently disabled.
            // Use os.sapp's env utilities or pass values via channel arguments instead.
            Err(host.err(
                "env_set() is disabled: mutating the process environment is not thread-safe.                  Pass values via channel arguments or use per-VM state.".into()
            ))
        }

        fn sleep_ms(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_exec() {
                return Err(host.err("exec access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!("sleep_ms() expects 1 argument, got {}", args.len())));
            }
            let ms = match args[0] {
                Value::Number(n) if n.is_finite() && n >= 0.0 => n as u64,
                _ => return Err(host.err("sleep_ms(): expected non-negative number".into())),
            };
            std::thread::sleep(std::time::Duration::from_millis(ms));
            Ok(Value::Nil)
        }

        fn exec(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_exec() {
                return Err(host.err("exec access denied".into()));
            }
            // SECURITY: exec(cmd_string) was vulnerable to shell injection because
            // the entire string was passed to `sh -c`. It is now deprecated in favour
            // of proc_spawn_wait(cmd, [args...]) which separates command from arguments.
            // Passing a single-token command (no spaces, no shell metacharacters) is
            // still allowed for backward compatibility, but any whitespace causes an error.
            if args.len() != 1 {
                return Err(host.err(format!("exec() expects 1 argument, got {}", args.len())));
            }
            let cmd = args[0]
                .as_str()
                .map_err(|m| host.err(format!("exec(): {m}")))?;
            // Reject strings containing shell metacharacters or whitespace (injection prevention):
            let blocked: &[char] = &[
                ';', '&', '|', '$', '`', '>', '<', '!', '{', '}', '(', ')', '\'', '"', '\\', '*',
                '?', '[', ']', '#', '~',
            ];
            if cmd
                .chars()
                .any(|c| c.is_whitespace() || blocked.contains(&c))
            {
                return Err(host.err(
                    "exec(): command contains shell metacharacters or whitespace — use proc_spawn_wait(cmd, args_array) instead.".into()
                ));
            }
            let status = std::process::Command::new(cmd)
                .status()
                .map_err(|e| host.err(format!("exec(): {e}")))?;
            Ok(Value::Number(status.code().unwrap_or(1) as f64))
        }

        fn import(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("import() expects 1 argument, got {}", args.len())));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("import(): {m}")))?;
            host.import_module(path)
        }

        fn gc_stats(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !args.is_empty() {
                return Err(host.err(format!("gc_stats() takes no arguments, got {}", args.len())));
            }
            Ok(alloc_string(format!(
                "string_allocs={} array_allocs={} map_allocs={} module_allocs={}",
                string_alloc_count(),
                ARRAY_ALLOCS.load(Ordering::Relaxed),
                MAP_ALLOCS.load(Ordering::Relaxed),
                MODULE_ALLOCS.load(Ordering::Relaxed)
            )))
        }

        // ---- Safety builtins ----

        /// assert(cond) or assert(cond, msg) — Rust-style debug assertion.
        /// Throws RuntimeError if cond is falsy.
        fn assert_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.is_empty() {
                return Err(host.err("assert() requires at least 1 argument".into()));
            }
            let cond = &args[0];
            if !VM::is_truthy(cond) {
                let msg = if args.len() >= 2 {
                    args[1].to_string()
                } else {
                    "assertion failed".to_string()
                };
                return Err(host.err(format!("AssertionError: {msg}")));
            }
            Ok(Value::Nil)
        }

        /// type_assert(value, type_str) — assert that value has the expected type.
        /// type_str: "number", "string", "bool", "nil", "array", "map", "closure", "fn".
        fn type_assert_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() < 2 {
                return Err(host.err("type_assert(value, type_str) requires 2 arguments".into()));
            }
            let expected = args[1].as_str().map_err(|_| {
                host.err("type_assert: second argument must be a string".to_string())
            })?;
            // BUG FIX #13: Must match type_of() exactly so type_assert(f, "fn") works.
            let actual = match &args[0] {
                Value::Number(_) => "number",
                Value::Bool(_) => "bool",
                Value::Nil => "nil",
                Value::Obj(o) => match o.as_ref() {
                    pblang_ir::Obj::String(_) => "string",
                },
                Value::Array(_) => "array",
                Value::Map(_) => "map",
                Value::Function(_) => "fn", // matches type_of() → "fn"
                Value::Closure(_) => "closure", // matches type_of() → "closure"
                Value::Builtin(_) => "builtin",
                Value::Module(_) => "module",
            };
            if actual != expected {
                return Err(host.err(format!("TypeError: expected {expected}, got {actual}")));
            }
            Ok(Value::Nil)
        }

        /// freeze(val) — mark a map/array as frozen (immutable).
        /// Returns the value with __frozen: true inserted.
        /// Frozen maps reject attribute assignment at runtime.
        fn freeze_builtin(_host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(_host.err("freeze(val) takes 1 argument".into()));
            }
            match &args[0] {
                Value::Map(m) => {
                    m.borrow_mut()
                        .insert("__frozen".to_string(), Value::Bool(true));
                    Ok(args.into_iter().next().unwrap())
                }
                _ => Ok(args.into_iter().next().unwrap()), // non-maps pass through
            }
        }

        /// is_frozen(val) — returns true if val has __frozen: true.
        fn is_frozen_builtin(
            _host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(_host.err("is_frozen(val) takes 1 argument".into()));
            }
            match &args[0] {
                Value::Map(m) => {
                    let frozen = m.borrow().get("__frozen").cloned();
                    Ok(Value::Bool(matches!(frozen, Some(Value::Bool(true)))))
                }
                _ => Ok(Value::Bool(false)),
            }
        }

        /// checked_add(a, b) — add two numbers; returns Err result map if overflow.
        fn checked_add_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err("checked_add(a, b) takes 2 arguments".into()));
            }
            match (&args[0], &args[1]) {
                (Value::Number(a), Value::Number(b)) => {
                    let result = a + b;
                    if result.is_infinite() && a.is_finite() && b.is_finite() {
                        Ok(alloc_map({
                            let mut m = std::collections::HashMap::new();
                            m.insert("ok".to_string(), Value::Bool(false));
                            m.insert(
                                "error".to_string(),
                                alloc_string("arithmetic overflow in checked_add"),
                            );
                            m
                        }))
                    } else {
                        Ok(alloc_map({
                            let mut m = std::collections::HashMap::new();
                            m.insert("ok".to_string(), Value::Bool(true));
                            m.insert("value".to_string(), Value::Number(result));
                            m
                        }))
                    }
                }
                _ => Err(host.err("checked_add: both arguments must be numbers".into())),
            }
        }

        fn checked_sub_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err("checked_sub(a, b) takes 2 arguments".into()));
            }
            match (&args[0], &args[1]) {
                (Value::Number(a), Value::Number(b)) => Ok(alloc_map({
                    let mut m = std::collections::HashMap::new();
                    m.insert("ok".to_string(), Value::Bool(true));
                    m.insert("value".to_string(), Value::Number(a - b));
                    m
                })),
                _ => Err(host.err("checked_sub: both arguments must be numbers".into())),
            }
        }

        fn checked_mul_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err("checked_mul(a, b) takes 2 arguments".into()));
            }
            match (&args[0], &args[1]) {
                (Value::Number(a), Value::Number(b)) => {
                    let result = a * b;
                    if result.is_infinite() && a.is_finite() && b.is_finite() {
                        Ok(alloc_map({
                            let mut m = std::collections::HashMap::new();
                            m.insert("ok".to_string(), Value::Bool(false));
                            m.insert(
                                "error".to_string(),
                                alloc_string("arithmetic overflow in checked_mul"),
                            );
                            m
                        }))
                    } else {
                        Ok(alloc_map({
                            let mut m = std::collections::HashMap::new();
                            m.insert("ok".to_string(), Value::Bool(true));
                            m.insert("value".to_string(), Value::Number(result));
                            m
                        }))
                    }
                }
                _ => Err(host.err("checked_mul: both arguments must be numbers".into())),
            }
        }

        fn checked_div_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err("checked_div(a, b) takes 2 arguments".into()));
            }
            match (&args[0], &args[1]) {
                (Value::Number(a), Value::Number(b)) => {
                    if *b == 0.0 {
                        Ok(alloc_map({
                            let mut m = std::collections::HashMap::new();
                            m.insert("ok".to_string(), Value::Bool(false));
                            m.insert(
                                "error".to_string(),
                                alloc_string("division by zero in checked_div"),
                            );
                            m
                        }))
                    } else {
                        Ok(alloc_map({
                            let mut m = std::collections::HashMap::new();
                            m.insert("ok".to_string(), Value::Bool(true));
                            m.insert("value".to_string(), Value::Number(a / b));
                            m
                        }))
                    }
                }
                _ => Err(host.err("checked_div: both arguments must be numbers".into())),
            }
        }

        /// is_nil(val) — returns true if val is nil (functional form of `val == nil`).
        fn is_nil_builtin(_host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            Ok(Value::Bool(matches!(args.first(), Some(Value::Nil) | None)))
        }

        /// require_type(val, type_str) — like type_assert but returns the value on success.
        fn require_type_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() < 2 {
                return Err(host.err("require_type(val, type_str) requires 2 arguments".into()));
            }
            let expected = args[1].as_str().map_err(|_| {
                host.err("require_type: second argument must be a string".to_string())
            })?;
            // BUG FIX #13: Must match type_of() exactly so type_assert(f, "fn") works.
            let actual = match &args[0] {
                Value::Number(_) => "number",
                Value::Bool(_) => "bool",
                Value::Nil => "nil",
                Value::Obj(o) => match o.as_ref() {
                    pblang_ir::Obj::String(_) => "string",
                },
                Value::Array(_) => "array",
                Value::Map(_) => "map",
                Value::Function(_) => "fn", // matches type_of() → "fn"
                Value::Closure(_) => "closure", // matches type_of() → "closure"
                Value::Builtin(_) => "builtin",
                Value::Module(_) => "module",
            };
            if actual != expected {
                return Err(host.err(format!(
                    "TypeError: require_type expected {expected}, got {actual}"
                )));
            }
            Ok(args.into_iter().next().unwrap())
        }

        fn gc_collect(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !args.is_empty() {
                return Err(host.err(format!(
                    "gc_collect() takes no arguments, got {}",
                    args.len()
                )));
            }

            // BUG FIX #4: Previously this function forcefully cleared ALL open TCP/FFI/SQLite
            // handles, destroying any live connections. GC should only collect *unreachable*
            // resources. Native handle tables are intentionally NOT cleared here — their
            // entries are ref-counted by user-held handle numbers. Only the cycle-breaker
            // pass runs, which traces from GC roots and clears unreachable RC containers.
            let (arrays_cleared, maps_cleared, modules_cleared) = host.gc_collect_cycles();

            Ok(alloc_string(format!(
                "gc_collect: cycles arrays={} maps={} modules={}",
                arrays_cleared, maps_cleared, modules_cleared
            )))
        }

        fn sha256(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("sha256() expects 1 argument, got {}", args.len())));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("sha256(): {m}")))?;
            let digest = sha2::Sha256::digest(s.as_bytes());
            Ok(alloc_string(hex::encode(digest)))
        }

        fn random_bytes(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "random_bytes() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let n = match args[0] {
                Value::Number(x) if x >= 0.0 && x <= (usize::MAX as f64) => x as usize,
                _ => return Err(host.err("random_bytes(): expected non-negative integer".into())),
            };
            let mut rng = rand::thread_rng();
            let bytes: Vec<u8> = (0..n).map(|_| rng.gen()).collect();
            let arr = alloc_array(bytes.into_iter().map(|b| Value::Number(b as f64)).collect());
            Ok(arr)
        }

        fn random_float_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if !args.is_empty() {
                return Err(host.err(format!(
                    "random_float() expects 0 arguments, got {}",
                    args.len()
                )));
            }
            let mut rng = rand::thread_rng();
            Ok(Value::Number(rng.gen::<f64>()))
        }

        fn constant_time_eq(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "constant_time_eq() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let a = args[0]
                .as_str()
                .map_err(|m| host.err(format!("constant_time_eq(): first arg {m}")))?;
            let b = args[1]
                .as_str()
                .map_err(|m| host.err(format!("constant_time_eq(): second arg {m}")))?;
            let aa = a.as_bytes();
            let bb = b.as_bytes();
            let ok = aa.len() == bb.len()
                && aa
                    .iter()
                    .zip(bb.iter())
                    .fold(0u8, |acc, (x, y)| acc | (x ^ y))
                    == 0;
            Ok(Value::Bool(ok))
        }

        fn base64_encode(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "base64_encode() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("base64_encode(): {m}")))?;
            Ok(alloc_string(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                s.as_bytes(),
            )))
        }

        fn base64_decode(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "base64_decode() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("base64_decode(): {m}")))?;
            let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
                .map_err(|e| host.err(format!("base64_decode(): {e}")))?;
            let s = String::from_utf8(bytes)
                .map_err(|e| host.err(format!("base64_decode(): invalid UTF-8: {e}")))?;
            Ok(alloc_string(s))
        }

        /// base64_decode_bytes(s) → array<number 0-255>
        /// BUG FIX #22: base64_decode() fails on binary data; this variant returns raw bytes.
        fn base64_decode_bytes(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "base64_decode_bytes() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("base64_decode_bytes(): {m}")))?;
            let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
                .map_err(|e| host.err(format!("base64_decode_bytes(): {e}")))?;
            let arr = alloc_array(bytes.into_iter().map(|b| Value::Number(b as f64)).collect());
            Ok(arr)
        }

        fn json_to_value(v: &sj::Value) -> Value {
            match v {
                sj::Value::Null => Value::Nil,
                sj::Value::Bool(b) => Value::Bool(*b),
                sj::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
                sj::Value::String(s) => alloc_string(s.clone()),
                sj::Value::Array(items) => {
                    let out = items.iter().map(json_to_value).collect::<Vec<_>>();
                    alloc_array(out)
                }
                sj::Value::Object(map) => {
                    let mut out = HashMap::<String, Value>::new();
                    for (k, vv) in map.iter() {
                        out.insert(k.clone(), json_to_value(vv));
                    }
                    alloc_map(out)
                }
            }
        }

        fn value_to_json(v: &Value) -> Result<sj::Value, String> {
            Ok(match v {
                Value::Nil => sj::Value::Null,
                Value::Bool(b) => sj::Value::Bool(*b),
                Value::Number(n) => sj::Number::from_f64(*n)
                    .map(sj::Value::Number)
                    .unwrap_or(sj::Value::Null),
                Value::Obj(o) => match o.as_ref() {
                    Obj::String(s) => sj::Value::String(s.clone()),
                },
                Value::Array(a) => {
                    let a = a.borrow();
                    sj::Value::Array(
                        a.iter()
                            .map(|x| value_to_json(x).unwrap_or(sj::Value::Null))
                            .collect(),
                    )
                }
                Value::Map(m) => {
                    let m = m.borrow();
                    let mut out = sj::Map::new();
                    for (k, vv) in m.iter() {
                        out.insert(k.clone(), value_to_json(vv).unwrap_or(sj::Value::Null));
                    }
                    sj::Value::Object(out)
                }
                _ => return Err("json_stringify: unsupported value type".into()),
            })
        }

        fn json_parse(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "json_parse() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("json_parse(): {m}")))?;
            let v: sj::Value = sj::from_str(s).map_err(|e| {
                // serde_json provides line/column info which is very useful.
                host.err(format!(
                    "json_parse(): {} at line {} column {}",
                    e,
                    e.line(),
                    e.column()
                ))
            })?;
            Ok(json_to_value(&v))
        }

        fn json_stringify(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "json_stringify() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let j = value_to_json(&args[0]).map_err(|m| host.err(m))?;
            let s = sj::to_string(&j).map_err(|e| host.err(format!("json_stringify(): {e}")))?;
            Ok(alloc_string(s))
        }

        fn http_get(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_http() {
                return Err(host.err("http access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!("http_get() expects 1 argument, got {}", args.len())));
            }
            let url = args[0]
                .as_str()
                .map_err(|m| host.err(format!("http_get(): {m}")))?;
            let req = ureq::get(url);
            let req = if let Some(ms) = host.io_timeout_ms() {
                req.timeout(Duration::from_millis(ms))
            } else {
                req
            };
            let resp = req
                .call()
                .map_err(|e| host.err(format!("http_get(): {e}")))?;
            let body = resp
                .into_string()
                .map_err(|e| host.err(format!("http_get(): {e}")))?;
            Ok(alloc_string(body))
        }

        /// http_post(url, body) or http_post(url, body, content_type)
        /// BUG FIX #23: No longer hardcodes Content-Type: application/json.
        /// If 3rd argument (content_type) is omitted, defaults to application/json
        /// for backward compatibility. Pass e.g. "text/plain" for non-JSON bodies.
        fn http_post(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_http() {
                return Err(host.err("http access denied".into()));
            }
            if args.len() < 2 || args.len() > 3 {
                return Err(host.err(format!(
                    "http_post() expects 2 or 3 arguments (url, body[, content_type]), got {}",
                    args.len()
                )));
            }
            let url = args[0]
                .as_str()
                .map_err(|m| host.err(format!("http_post(): {m}")))?;
            let body = args[1]
                .as_str()
                .map_err(|m| host.err(format!("http_post(): {m}")))?;
            let ct = if args.len() == 3 {
                args[2]
                    .as_str()
                    .map_err(|m| host.err(format!("http_post(): {m}")))?
                    .to_string()
            } else {
                "application/json".to_string()
            };
            let req = ureq::post(url).set("content-type", &ct);
            let req = if let Some(ms) = host.io_timeout_ms() {
                req.timeout(Duration::from_millis(ms))
            } else {
                req
            };
            let resp = req
                .send_string(body)
                .map_err(|e| host.err(format!("http_post(): {e}")))?;
            let out = resp
                .into_string()
                .map_err(|e| host.err(format!("http_post(): {e}")))?;
            Ok(alloc_string(out))
        }

        fn str_contains(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "str_contains() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("str_contains(): {m}")))?;
            let sub = args[1]
                .as_str()
                .map_err(|m| host.err(format!("str_contains(): {m}")))?;
            Ok(Value::Bool(s.contains(sub)))
        }

        fn str_starts_with(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "str_starts_with() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("str_starts_with(): {m}")))?;
            let pre = args[1]
                .as_str()
                .map_err(|m| host.err(format!("str_starts_with(): {m}")))?;
            Ok(Value::Bool(s.starts_with(pre)))
        }

        fn str_trim(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("str_trim() expects 1 argument, got {}", args.len())));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("str_trim(): {m}")))?;
            Ok(alloc_string(s.trim().to_string()))
        }

        fn str_split(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "str_split() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("str_split(): {m}")))?;
            let delim = args[1]
                .as_str()
                .map_err(|m| host.err(format!("str_split(): {m}")))?;
            let parts = if delim.is_empty() {
                s.chars()
                    .map(|c| alloc_string(c.to_string()))
                    .collect::<Vec<_>>()
            } else {
                s.split(delim)
                    .map(|p| alloc_string(p.to_string()))
                    .collect()
            };
            Ok(alloc_array(parts))
        }

        fn to_sapp_path(p: &std::path::Path) -> String {
            p.to_string_lossy().replace('\\', "/")
        }

        fn cwd(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if !args.is_empty() {
                return Err(host.err(format!("cwd() takes no arguments, got {}", args.len())));
            }
            let p = std::env::current_dir().map_err(|e| host.err(format!("cwd(): {e}")))?;
            Ok(alloc_string(to_sapp_path(&p)))
        }

        fn fs_exists(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "fs_exists() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("fs_exists(): {m}")))?;
            Ok(Value::Bool(std::path::Path::new(path).exists()))
        }

        fn fs_mkdirs(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "fs_mkdirs() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("fs_mkdirs(): {m}")))?;
            std::fs::create_dir_all(path).map_err(|e| host.err(format!("fs_mkdirs(): {e}")))?;
            Ok(Value::Nil)
        }

        fn fs_listdir(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "fs_listdir() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("fs_listdir(): {m}")))?;
            let mut names = vec![];
            let rd = std::fs::read_dir(path).map_err(|e| host.err(format!("fs_listdir(): {e}")))?;
            for ent in rd {
                let ent = ent.map_err(|e| host.err(format!("fs_listdir(): {e}")))?;
                let name = ent.file_name().to_string_lossy().to_string();
                names.push(alloc_string(name));
            }
            // stable order for deterministic programs/tests
            names.sort_by_key(|a| a.to_string());
            Ok(alloc_array(names))
        }

        fn path_join(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() < 2 {
                return Err(host.err(format!(
                    "path_join() expects at least 2 arguments, got {}",
                    args.len()
                )));
            }
            let mut pb = std::path::PathBuf::new();
            for a in args {
                let s = a
                    .as_str()
                    .map_err(|m| host.err(format!("path_join(): {m}")))?;
                pb.push(s);
            }
            Ok(alloc_string(to_sapp_path(&pb)))
        }

        fn path_dirname(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "path_dirname() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("path_dirname(): {m}")))?;
            let p = std::path::Path::new(s);
            let out = p
                .parent()
                .unwrap_or_else(|| std::path::Path::new(""))
                .to_path_buf();
            Ok(alloc_string(to_sapp_path(&out)))
        }

        fn path_basename(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_fs() {
                return Err(host.err("fs access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "path_basename() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("path_basename(): {m}")))?;
            let p = std::path::Path::new(s);
            let out = p
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_string();
            Ok(alloc_string(out))
        }

        fn tcp_listen(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "tcp_listen() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let addr = args[0]
                .as_str()
                .map_err(|m| host.err(format!("tcp_listen(): {m}")))?;
            let l = TcpListener::bind(addr).map_err(|e| host.err(format!("tcp_listen(): {e}")))?;
            let id = with_net_table(host, |t| {
                let id = NET_NEXT.fetch_add(1, Ordering::Relaxed);
                t.insert(id, NetResource::Listener(l));
                id
            })?;
            Ok(Value::Number(id as f64))
        }

        fn tcp_accept(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "tcp_accept() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "tcp_accept")?;
            let r = with_net_table(host, |t| t.remove(&id))?
                .ok_or_else(|| host.err("tcp_accept(): invalid handle".into()))?;

            match r {
                NetResource::Listener(listener) => {
                    // Plain TCP accept
                    let (s, _addr) = listener
                        .accept()
                        .map_err(|e| host.err(format!("tcp_accept(): {e}")))?;
                    if let Some(ms) = host.io_timeout_ms() {
                        let d = Duration::from_millis(ms);
                        let _ = s.set_read_timeout(Some(d));
                        let _ = s.set_write_timeout(Some(d));
                    }
                    let _ = with_net_table(host, |t| t.insert(id, NetResource::Listener(listener)));
                    let sid = with_net_table(host, |t| {
                        let sid = NET_NEXT.fetch_add(1, Ordering::Relaxed);
                        t.insert(
                            sid,
                            NetResource::Stream(NetStream {
                                inner: StreamKind::Plain(s),
                                buf: vec![],
                            }),
                        );
                        sid
                    })?;
                    Ok(Value::Number(sid as f64))
                }
                NetResource::TlsListener(listener, tls_config) => {
                    // TLS accept: accept TCP, then perform TLS handshake
                    let (tcp_stream, _addr) = listener
                        .accept()
                        .map_err(|e| host.err(format!("tcp_accept(): {e}")))?;
                    if let Some(ms) = host.io_timeout_ms() {
                        let d = Duration::from_millis(ms);
                        let _ = tcp_stream.set_read_timeout(Some(d));
                        let _ = tcp_stream.set_write_timeout(Some(d));
                    }
                    // Create TLS server connection
                    let server_conn = ServerConnection::new(tls_config.clone())
                        .map_err(|e| host.err(format!("tcp_accept() TLS: {e}")))?;
                    let tls_stream = StreamOwned::new(server_conn, tcp_stream);

                    let _ = with_net_table(host, |t| {
                        t.insert(id, NetResource::TlsListener(listener, tls_config))
                    });
                    let sid = with_net_table(host, |t| {
                        let sid = NET_NEXT.fetch_add(1, Ordering::Relaxed);
                        t.insert(
                            sid,
                            NetResource::Stream(NetStream {
                                inner: StreamKind::TlsServer(tls_stream),
                                buf: vec![],
                            }),
                        );
                        sid
                    })?;
                    Ok(Value::Number(sid as f64))
                }
                other => {
                    let _ = with_net_table(host, |t| t.insert(id, other));
                    Err(host.err("tcp_accept(): handle is not a listener".into()))
                }
            }
        }

        fn tcp_read(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!(
                    "tcp_read() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "tcp_read")?;
            let n = match &args[1] {
                Value::Number(x) if x.is_finite() && *x >= 0.0 => *x as usize,
                _ => return Err(host.err("tcp_read(): expected byte count number".into())),
            };

            let r = with_net_table(host, |t| t.remove(&id))?
                .ok_or_else(|| host.err("tcp_read(): invalid handle".into()))?;
            let mut st = match r {
                NetResource::Stream(s) => s,
                other => {
                    let _ = with_net_table(host, |t| t.insert(id, other));
                    return Err(host.err("tcp_read(): handle is not a stream".into()));
                }
            };

            // serve from buffer first
            let mut out = Vec::<u8>::new();
            if !st.buf.is_empty() {
                let take = st.buf.len().min(n);
                out.extend_from_slice(&st.buf[..take]);
                st.buf.drain(..take);
            }

            while out.len() < n {
                let mut tmp = vec![0u8; (n - out.len()).min(4096)];
                let read = st
                    .inner
                    .read(&mut tmp)
                    .map_err(|e| host.err(format!("tcp_read(): {e}")))?;
                if read == 0 {
                    break;
                }
                out.extend_from_slice(&tmp[..read]);
            }

            let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(st)));
            Ok(alloc_string(String::from_utf8_lossy(&out).to_string()))
        }

        fn tcp_read_until(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 3 {
                return Err(host.err(format!(
                    "tcp_read_until() expects 3 arguments, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "tcp_read_until")?;
            let delim = args[1]
                .as_str()
                .map_err(|m| host.err(format!("tcp_read_until(): {m}")))?
                .as_bytes()
                .to_vec();
            let max = match &args[2] {
                Value::Number(x) if x.is_finite() && *x >= 0.0 => *x as usize,
                _ => return Err(host.err("tcp_read_until(): expected max_bytes number".into())),
            };

            let r = with_net_table(host, |t| t.remove(&id))?
                .ok_or_else(|| host.err("tcp_read_until(): invalid handle".into()))?;
            let mut st = match r {
                NetResource::Stream(s) => s,
                other => {
                    let _ = with_net_table(host, |t| t.insert(id, other));
                    return Err(host.err("tcp_read_until(): handle is not a stream".into()));
                }
            };

            let mut total = 0usize;
            loop {
                if let Some(pos) = find_subslice(&st.buf, &delim) {
                    let end = (pos + delim.len()).min(st.buf.len());
                    let out = st.buf.drain(..end).collect::<Vec<u8>>();
                    let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(st)));
                    return Ok(alloc_string(String::from_utf8_lossy(&out).to_string()));
                }
                if total >= max {
                    let out = st.buf.drain(..).collect::<Vec<u8>>();
                    let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(st)));
                    return Ok(alloc_string(String::from_utf8_lossy(&out).to_string()));
                }
                let mut tmp = [0u8; 4096];
                let read = st
                    .inner
                    .read(&mut tmp)
                    .map_err(|e| host.err(format!("tcp_read_until(): {e}")))?;
                if read == 0 {
                    let out = st.buf.drain(..).collect::<Vec<u8>>();
                    let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(st)));
                    return Ok(alloc_string(String::from_utf8_lossy(&out).to_string()));
                }
                st.buf.extend_from_slice(&tmp[..read]);
                total += read;
            }
        }

        fn tcp_write(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!(
                    "tcp_write() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "tcp_write")?;
            let s = args[1]
                .as_str()
                .map_err(|m| host.err(format!("tcp_write(): {m}")))?;

            let r = with_net_table(host, |t| t.remove(&id))?
                .ok_or_else(|| host.err("tcp_write(): invalid handle".into()))?;
            let mut st = match r {
                NetResource::Stream(s) => s,
                other => {
                    let _ = with_net_table(host, |t| t.insert(id, other));
                    return Err(host.err("tcp_write(): handle is not a stream".into()));
                }
            };

            let n = st
                .inner
                .write(s.as_bytes())
                .map_err(|e| host.err(format!("tcp_write(): {e}")))?;
            let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(st)));
            Ok(Value::Number(n as f64))
        }

        fn tcp_close(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "tcp_close() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "tcp_close")?;
            let _ = with_net_table(host, |t| t.remove(&id));
            Ok(Value::Nil)
        }

        fn tcp_local_addr(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "tcp_local_addr() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "tcp_local_addr")?;
            let r = with_net_table(host, |t| t.remove(&id))?
                .ok_or_else(|| host.err("tcp_local_addr(): invalid handle".into()))?;
            match r {
                NetResource::Listener(l) => {
                    let addr = l
                        .local_addr()
                        .map_err(|e| host.err(format!("tcp_local_addr(): {e}")))?;
                    let _ = with_net_table(host, |t| t.insert(id, NetResource::Listener(l)));
                    Ok(alloc_string(addr.to_string()))
                }
                NetResource::TlsListener(l, cfg) => {
                    let addr = l
                        .local_addr()
                        .map_err(|e| host.err(format!("tcp_local_addr(): {e}")))?;
                    let _ =
                        with_net_table(host, |t| t.insert(id, NetResource::TlsListener(l, cfg)));
                    Ok(alloc_string(addr.to_string()))
                }
                NetResource::Stream(s) => {
                    let addr = s
                        .inner
                        .local_addr()
                        .map_err(|e| host.err(format!("tcp_local_addr(): {e}")))?;
                    let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(s)));
                    Ok(alloc_string(addr.to_string()))
                }
            }
        }

        // ---- TLS builtins (W4-gamma) ----

        /// tcp_listen_tls(addr, cert_pem_path, key_pem_path) → listener handle
        /// Creates a TLS-wrapped TCP listener using rustls.
        fn tcp_listen_tls(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 3 {
                return Err(host.err(format!(
                    "tcp_listen_tls() expects 3 arguments (addr, cert_path, key_path), got {}",
                    args.len()
                )));
            }
            let addr = args[0]
                .as_str()
                .map_err(|m| host.err(format!("tcp_listen_tls(): {m}")))?;
            let cert_path = args[1]
                .as_str()
                .map_err(|m| host.err(format!("tcp_listen_tls(): cert_path: {m}")))?;
            let key_path = args[2]
                .as_str()
                .map_err(|m| host.err(format!("tcp_listen_tls(): key_path: {m}")))?;

            // Read certificate chain from PEM file
            let cert_file = std::fs::File::open(cert_path)
                .map_err(|e| host.err(format!("tcp_listen_tls(): open cert: {e}")))?;
            let mut cert_reader = std::io::BufReader::new(cert_file);
            let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
                .filter_map(|r| r.ok())
                .collect();
            if certs.is_empty() {
                return Err(host.err("tcp_listen_tls(): no certificates found in PEM file".into()));
            }

            // Read private key from PEM file
            let key_file = std::fs::File::open(key_path)
                .map_err(|e| host.err(format!("tcp_listen_tls(): open key: {e}")))?;
            let mut key_reader = std::io::BufReader::new(key_file);
            let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_reader)
                .map_err(|e| host.err(format!("tcp_listen_tls(): read key: {e}")))?
                .ok_or_else(|| {
                    host.err("tcp_listen_tls(): no private key found in PEM file".into())
                })?;

            // Build ServerConfig
            let server_config = ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|e| host.err(format!("tcp_listen_tls(): config: {e}")))?;

            // Bind TCP listener
            let listener = TcpListener::bind(addr)
                .map_err(|e| host.err(format!("tcp_listen_tls(): bind: {e}")))?;

            // Store as TlsListener
            let id = with_net_table(host, |t| {
                let id = NET_NEXT.fetch_add(1, Ordering::Relaxed);
                t.insert(
                    id,
                    NetResource::TlsListener(listener, Arc::new(server_config)),
                );
                id
            })?;

            Ok(Value::Number(id as f64))
        }

        /// tcp_connect_tls(host, port) → stream handle
        /// Connects to a TLS server with certificate verification enabled.
        /// Trust roots come from webpki-roots plus best-effort native/system roots.
        fn tcp_connect_tls(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!(
                    "tcp_connect_tls() expects 2 arguments (host, port), got {}",
                    args.len()
                )));
            }
            let host_str = args[0]
                .as_str()
                .map_err(|m| host.err(format!("tcp_connect_tls(): host: {m}")))?;
            let port = match &args[1] {
                Value::Number(n) if n.is_finite() && *n >= 0.0 && *n <= 65535.0 => *n as u16,
                _ => {
                    return Err(host.err("tcp_connect_tls(): port must be a number 0-65535".into()))
                }
            };

            // Connect TCP
            let addr = format!("{}:{}", host_str, port);
            let tcp_stream = TcpStream::connect(&addr)
                .map_err(|e| host.err(format!("tcp_connect_tls(): connect: {e}")))?;
            if let Some(ms) = host.io_timeout_ms() {
                let d = Duration::from_millis(ms);
                let _ = tcp_stream.set_read_timeout(Some(d));
                let _ = tcp_stream.set_write_timeout(Some(d));
            }

            // Build ClientConfig with proper system root certificates (webpki-roots + native certs).
            // This replaces the old DangerousAllowAllVerifier that accepted ANY certificate.
            let mut root_store = RootCertStore::empty();
            // Add Mozilla's curated WebPKI root CAs (always available, no network needed)
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            // Also attempt to add system/native root certs (optional, best-effort)
            if let Ok(native_certs) = rustls_native_certs::load_native_certs() {
                for cert in native_certs {
                    let _ = root_store.add(cert);
                }
            }
            let client_config = ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth();

            // Create server name for SNI
            let server_name: ServerName<'static> = host_str
                .to_string()
                .try_into()
                .map_err(|_| host.err("tcp_connect_tls(): invalid server name for SNI".into()))?;

            // Perform TLS handshake
            let client_conn = ClientConnection::new(Arc::new(client_config), server_name)
                .map_err(|e| host.err(format!("tcp_connect_tls(): TLS: {e}")))?;
            let tls_stream = StreamOwned::new(client_conn, tcp_stream);

            // Store as Stream
            let id = with_net_table(host, |t| {
                let id = NET_NEXT.fetch_add(1, Ordering::Relaxed);
                t.insert(
                    id,
                    NetResource::Stream(NetStream {
                        inner: StreamKind::TlsClient(tls_stream),
                        buf: vec![],
                    }),
                );
                id
            })?;

            Ok(Value::Number(id as f64))
        }

        /// tcp_wrap_tls(handle, cert_path, key_path) → new stream handle
        /// Upgrades an existing plain TCP stream to TLS server mode.
        fn tcp_wrap_tls(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 3 {
                return Err(host.err(format!(
                    "tcp_wrap_tls() expects 3 arguments (handle, cert_path, key_path), got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "tcp_wrap_tls")?;
            let cert_path = args[1]
                .as_str()
                .map_err(|m| host.err(format!("tcp_wrap_tls(): cert_path: {m}")))?;
            let key_path = args[2]
                .as_str()
                .map_err(|m| host.err(format!("tcp_wrap_tls(): key_path: {m}")))?;

            // Remove the stream from table
            let r = with_net_table(host, |t| t.remove(&id))?
                .ok_or_else(|| host.err("tcp_wrap_tls(): invalid handle".into()))?;

            let tcp_stream = match r {
                NetResource::Stream(NetStream {
                    inner: StreamKind::Plain(s),
                    buf: _,
                }) => s,
                other => {
                    let _ = with_net_table(host, |t| t.insert(id, other));
                    return Err(host.err(
                        "tcp_wrap_tls(): handle must be a plain (non-TLS) TCP stream".into(),
                    ));
                }
            };

            // Read certificate chain from PEM file
            let cert_file = std::fs::File::open(cert_path)
                .map_err(|e| host.err(format!("tcp_wrap_tls(): open cert: {e}")))?;
            let mut cert_reader = std::io::BufReader::new(cert_file);
            let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
                .filter_map(|r| r.ok())
                .collect();
            if certs.is_empty() {
                return Err(host.err("tcp_wrap_tls(): no certificates found in PEM file".into()));
            }

            // Read private key from PEM file
            let key_file = std::fs::File::open(key_path)
                .map_err(|e| host.err(format!("tcp_wrap_tls(): open key: {e}")))?;
            let mut key_reader = std::io::BufReader::new(key_file);
            let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_reader)
                .map_err(|e| host.err(format!("tcp_wrap_tls(): read key: {e}")))?
                .ok_or_else(|| {
                    host.err("tcp_wrap_tls(): no private key found in PEM file".into())
                })?;

            // Build ServerConfig
            let server_config = ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|e| host.err(format!("tcp_wrap_tls(): config: {e}")))?;

            // Create TLS server connection and wrap stream
            let server_conn = ServerConnection::new(Arc::new(server_config))
                .map_err(|e| host.err(format!("tcp_wrap_tls(): TLS: {e}")))?;
            let tls_stream = StreamOwned::new(server_conn, tcp_stream);

            // Store as new Stream (reuse the same id)
            let _ = with_net_table(host, |t| {
                t.insert(
                    id,
                    NetResource::Stream(NetStream {
                        inner: StreamKind::TlsServer(tls_stream),
                        buf: vec![],
                    }),
                )
            });

            Ok(Value::Number(id as f64))
        }

        fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
            if needle.is_empty() {
                return Some(0);
            }
            hay.windows(needle.len()).position(|w| w == needle)
        }

        fn http_status_reason(status: i64) -> &'static str {
            match status {
                100 => "Continue",
                101 => "Switching Protocols",
                102 => "Processing",
                103 => "Early Hints",
                200 => "OK",
                201 => "Created",
                202 => "Accepted",
                203 => "Non-Authoritative Information",
                204 => "No Content",
                205 => "Reset Content",
                206 => "Partial Content",
                207 => "Multi-Status",
                208 => "Already Reported",
                226 => "IM Used",
                300 => "Multiple Choices",
                301 => "Moved Permanently",
                302 => "Found",
                303 => "See Other",
                304 => "Not Modified",
                307 => "Temporary Redirect",
                308 => "Permanent Redirect",
                400 => "Bad Request",
                401 => "Unauthorized",
                402 => "Payment Required",
                403 => "Forbidden",
                404 => "Not Found",
                405 => "Method Not Allowed",
                406 => "Not Acceptable",
                407 => "Proxy Authentication Required",
                408 => "Request Timeout",
                409 => "Conflict",
                410 => "Gone",
                411 => "Length Required",
                412 => "Precondition Failed",
                413 => "Content Too Large",
                414 => "URI Too Long",
                415 => "Unsupported Media Type",
                416 => "Range Not Satisfiable",
                417 => "Expectation Failed",
                418 => "I'm a teapot",
                421 => "Misdirected Request",
                422 => "Unprocessable Content",
                423 => "Locked",
                424 => "Failed Dependency",
                425 => "Too Early",
                426 => "Upgrade Required",
                428 => "Precondition Required",
                429 => "Too Many Requests",
                431 => "Request Header Fields Too Large",
                451 => "Unavailable For Legal Reasons",
                500 => "Internal Server Error",
                501 => "Not Implemented",
                502 => "Bad Gateway",
                503 => "Service Unavailable",
                504 => "Gateway Timeout",
                505 => "HTTP Version Not Supported",
                506 => "Variant Also Negotiates",
                507 => "Insufficient Storage",
                508 => "Loop Detected",
                510 => "Not Extended",
                511 => "Network Authentication Required",
                _ => "Unknown",
            }
        }

        fn http_parse_request(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "http_parse_request() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let raw = args[0]
                .as_str()
                .map_err(|m| host.err(format!("http_parse_request(): {m}")))?;
            let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
            let mut lines = head.split("\r\n");
            let req_line = lines.next().unwrap_or("");
            let mut parts = req_line.split_whitespace();
            let method = parts.next().unwrap_or("").to_string();
            let path = parts.next().unwrap_or("").to_string();
            let mut headers = HashMap::<String, Value>::new();
            for l in lines {
                if let Some((k, v)) = l.split_once(':') {
                    headers.insert(k.trim().to_string(), alloc_string(v.trim().to_string()));
                }
            }
            let headers_v = alloc_map(headers);

            let mut out = HashMap::<String, Value>::new();
            out.insert("method".into(), alloc_string(method));
            out.insert("path".into(), alloc_string(path));
            out.insert("headers".into(), headers_v);
            out.insert("body".into(), alloc_string(body.to_string()));
            Ok(alloc_map(out))
        }

        fn http_response(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "http_response() expects 2 arguments (status, body), got {}",
                    args.len()
                )));
            }
            let status = match &args[0] {
                Value::Number(n) if n.is_finite() => *n as i64,
                _ => return Err(host.err("http_response(): expected status number".into())),
            };
            let body = args[1]
                .as_str()
                .map_err(|m| host.err(format!("http_response(): {m}")))?;
            let reason = http_status_reason(status);
            let resp = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            Ok(alloc_string(resp))
        }

        fn http_response_ct(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 3 {
                return Err(host.err(format!(
                    "http_response_ct() expects 3 arguments (status, content_type, body), got {}",
                    args.len()
                )));
            }
            let status = match &args[0] {
                Value::Number(n) if n.is_finite() => *n as i64,
                _ => return Err(host.err("http_response_ct(): expected status number".into())),
            };
            let ct = args[1]
                .as_str()
                .map_err(|m| host.err(format!("http_response_ct(): {m}")))?;
            let body = args[2]
                .as_str()
                .map_err(|m| host.err(format!("http_response_ct(): {m}")))?;
            let reason = http_status_reason(status);
            let resp = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            Ok(alloc_string(resp))
        }

        fn http_response_json(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!(
                    "http_response_json() expects 2 arguments (status, value), got {}",
                    args.len()
                )));
            }
            let status = match &args[0] {
                Value::Number(n) if n.is_finite() => *n as i64,
                _ => return Err(host.err("http_response_json(): expected status number".into())),
            };
            let j = value_to_json(&args[1]).map_err(|m| host.err(m))?;
            let body =
                sj::to_string(&j).map_err(|e| host.err(format!("http_response_json(): {e}")))?;
            let ct = "application/json";
            let reason = http_status_reason(status);
            let resp = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            Ok(alloc_string(resp))
        }

        fn onnx_infer(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            if args.is_empty() {
                return Err(
                    host.err("onnx_infer() expects at least 1 argument (model_path)".into())
                );
            }
            let model_path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("onnx_infer(): {m}")))?;
            let input_json = if args.len() >= 2 {
                args[1]
                    .as_str()
                    .map_err(|m| host.err(format!("onnx_infer(): {m}")))?
                    .to_string()
            } else {
                "{}".to_string()
            };
            // ONNX Runtime inference is not available as a pure-Rust builtin.
            // Use the ai_chat / ai_embed builtins for HTTP-based AI inference,
            // or load ONNX models via the FFI layer (pblang-ffi).
            Err(host.err(format!(
                "onnx_infer(): ONNX Runtime not embedded. Use ai_chat/ai_embed for HTTP AI, or load ONNX models via ffi_load/ffi_call. Model path: {model_path}, input: {input_json}"
            )))
        }

        /// ai_chat(prompt, options_json) — Chat completion via registered AI providers.
        /// options_json: JSON string with optional { provider, model, system_prompt, temperature, max_tokens }
        fn ai_chat(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_http() {
                return Err(host.err("http access denied".into()));
            }
            if args.is_empty() {
                return Err(host.err("ai_chat() expects at least 1 argument (prompt)".into()));
            }
            let prompt = args[0]
                .as_str()
                .map_err(|m| host.err(format!("ai_chat(): {m}")))?;
            let options_str = if args.len() >= 2 {
                args[1]
                    .as_str()
                    .map_err(|m| host.err(format!("ai_chat(): {m}")))?
            } else {
                "{}"
            };

            let mut registry = pblang_ai::ProviderRegistry::new();
            registry.auto_register();

            let options: pblang_ai::ChatOptions =
                if options_str.trim().is_empty() || options_str == "{}" {
                    pblang_ai::ChatOptions::default()
                } else {
                    let v: serde_json::Value = serde_json::from_str(options_str)
                        .map_err(|e| host.err(format!("ai_chat(): invalid options JSON: {e}")))?;
                    let mut opts = pblang_ai::ChatOptions::default();
                    if let Some(p) = v.get("provider").and_then(|x| x.as_str()) {
                        opts.provider = Some(p.to_string());
                    }
                    if let Some(m) = v.get("model").and_then(|x| x.as_str()) {
                        opts.model = Some(m.to_string());
                    }
                    if let Some(s) = v.get("system_prompt").and_then(|x| x.as_str()) {
                        opts.system_prompt = Some(s.to_string());
                    }
                    if let Some(t) = v.get("temperature").and_then(|x| x.as_f64()) {
                        opts.temperature = Some(t);
                    }
                    if let Some(mt) = v.get("max_tokens").and_then(|x| x.as_u64()) {
                        opts.max_tokens = Some(mt as u32);
                    }
                    opts
                };

            match pblang_ai::chat(prompt, &options, &registry) {
                Ok(resp) => {
                    let json = pblang_ai::chat_response_to_json(&resp);
                    Ok(alloc_string(json))
                }
                Err(e) => Err(host.err(format!("ai_chat(): {e}"))),
            }
        }

        /// ai_embed(text, options_json) — Generate embedding vector from text.
        fn ai_embed(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_http() {
                return Err(host.err("http access denied".into()));
            }
            if args.is_empty() {
                return Err(host.err("ai_embed() expects at least 1 argument (text)".into()));
            }
            let text = args[0]
                .as_str()
                .map_err(|m| host.err(format!("ai_embed(): {m}")))?;
            let options_str = if args.len() >= 2 {
                args[1]
                    .as_str()
                    .map_err(|m| host.err(format!("ai_embed(): {m}")))?
            } else {
                "{}"
            };

            let mut registry = pblang_ai::ProviderRegistry::new();
            registry.auto_register();

            let options: pblang_ai::ChatOptions =
                if options_str.trim().is_empty() || options_str == "{}" {
                    pblang_ai::ChatOptions::default()
                } else {
                    let v: serde_json::Value = serde_json::from_str(options_str)
                        .map_err(|e| host.err(format!("ai_embed(): invalid options JSON: {e}")))?;
                    let mut opts = pblang_ai::ChatOptions::default();
                    if let Some(p) = v.get("provider").and_then(|x| x.as_str()) {
                        opts.provider = Some(p.to_string());
                    }
                    if let Some(m) = v.get("model").and_then(|x| x.as_str()) {
                        opts.model = Some(m.to_string());
                    }
                    opts
                };

            match pblang_ai::embed(text, &options, &registry) {
                Ok(resp) => {
                    let json = pblang_ai::embedding_response_to_json(&resp);
                    Ok(alloc_string(json))
                }
                Err(e) => Err(host.err(format!("ai_embed(): {e}"))),
            }
        }

        /// ai_token_count(text) — Estimate token count for text.
        fn ai_token_count(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "ai_token_count() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let text = args[0]
                .as_str()
                .map_err(|m| host.err(format!("ai_token_count(): {m}")))?;
            let tc = pblang_ai::estimate_tokens(text);
            let json = pblang_ai::token_count_to_json(&tc);
            Ok(alloc_string(json))
        }

        /// quantum_run(qasm, backend_str, shots) — Execute a QASM circuit on a quantum backend.
        fn quantum_run(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_exec() {
                return Err(host.err("exec access denied".into()));
            }
            if args.is_empty() || args.len() > 3 {
                return Err(host.err(format!(
                    "quantum_run() expects 1-3 arguments (qasm[, backend, shots]), got {}",
                    args.len()
                )));
            }
            let qasm = args[0]
                .as_str()
                .map_err(|m| host.err(format!("quantum_run(): {m}")))?;
            let backend_str = if args.len() >= 2 {
                args[1]
                    .as_str()
                    .map_err(|m| host.err(format!("quantum_run(): {m}")))?
            } else {
                "qiskit"
            };
            let shots = if args.len() >= 3 {
                match &args[2] {
                    Value::Number(n) => *n as u32,
                    other => {
                        return Err(host.err(format!(
                            "quantum_run(): expected number for shots, got {:?}",
                            other
                        )));
                    }
                }
            } else {
                1024
            };

            let backend =
                pblang_quantum::QuantumBackend::from_str(backend_str).ok_or_else(|| {
                    host.err(format!(
                        "quantum_run(): unknown backend '{}'. Try: qiskit, cirq, braket",
                        backend_str
                    ))
                })?;

            match pblang_quantum::run_qasm(qasm, backend, shots) {
                Ok(result) => {
                    let json = pblang_quantum::result_to_json(&result);
                    Ok(alloc_string(json))
                }
                Err(e) => Err(host.err(format!("quantum_run(): {e}"))),
            }
        }

        /// quantum_statevector(qasm, backend_str) — Get the statevector of a QASM circuit.
        fn quantum_statevector(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if !host.allow_exec() {
                return Err(host.err("exec access denied".into()));
            }
            if args.is_empty() || args.len() > 2 {
                return Err(host.err(format!(
                    "quantum_statevector() expects 1-2 arguments (qasm[, backend]), got {}",
                    args.len()
                )));
            }
            let qasm = args[0]
                .as_str()
                .map_err(|m| host.err(format!("quantum_statevector(): {m}")))?;
            let backend_str = if args.len() >= 2 {
                args[1]
                    .as_str()
                    .map_err(|m| host.err(format!("quantum_statevector(): {m}")))?
            } else {
                "qiskit"
            };

            let backend =
                pblang_quantum::QuantumBackend::from_str(backend_str).ok_or_else(|| {
                    host.err(format!(
                        "quantum_statevector(): unknown backend '{}'. Try: qiskit, cirq",
                        backend_str
                    ))
                })?;

            match pblang_quantum::get_statevector(qasm, backend) {
                Ok(sv) => {
                    let json = pblang_quantum::statevector_to_json(&sv);
                    Ok(alloc_string(json))
                }
                Err(e) => Err(host.err(format!("quantum_statevector(): {e}"))),
            }
        }

        /// quantum_backends() — List available quantum backends with availability status.
        fn quantum_backends(
            _host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let backends = [("qiskit", pblang_quantum::QuantumBackend::QiskitAer),
                ("cirq", pblang_quantum::QuantumBackend::CirqLocal),
                ("braket", pblang_quantum::QuantumBackend::BraketLocal)];
            let mut result = String::from("[");
            for (i, (name, backend)) in backends.iter().enumerate() {
                if i > 0 {
                    result.push(',');
                }
                let available = pblang_quantum::is_backend_available(backend.clone());
                result.push_str(&format!(
                    "{{\"name\":\"{name}\",\"available\":{available}}}"
                ));
            }
            result.push(']');
            Ok(alloc_string(result))
        }

        /// polyglot_exec(lang, code) — Execute code in a foreign language via subprocess.
        /// Supported langs: python, js (node), bash (sh), ruby, perl
        /// Returns JSON: { success, stdout, stderr, exit_code }
        fn polyglot_exec(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_exec() {
                return Err(host.err("exec access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!(
                    "polyglot_exec() expects 2 arguments (lang, code), got {}",
                    args.len()
                )));
            }
            let lang = args[0]
                .as_str()
                .map_err(|m| host.err(format!("polyglot_exec(): {m}")))?;
            let code = args[1]
                .as_str()
                .map_err(|m| host.err(format!("polyglot_exec(): {m}")))?;

            let (interpreter, arg) = match lang.to_lowercase().as_str() {
                "python" | "py" => ("python3", Some("-c")),
                "js" | "javascript" | "node" => ("node", Some("-e")),
                "bash" | "sh" | "shell" => ("bash", Some("-c")),
                "ruby" | "rb" => ("ruby", Some("-e")),
                "perl" | "pl" => ("perl", Some("-e")),
                "lua" => ("lua", Some("-e")),
                "php" => ("php", Some("-r")),
                "r" | "R" => ("Rscript", Some("-e")),
                other => {
                    return Err(host.err(format!(
                        "polyglot_exec(): unsupported language '{}'. Supported: python, js, bash, ruby, perl, lua, php, r",
                        other
                    )));
                }
            };

            let mut cmd = std::process::Command::new(interpreter);
            if let Some(flag) = arg {
                cmd.arg(flag);
            }
            cmd.arg(code)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let output = match cmd.output() {
                Ok(o) => o,
                Err(e) => {
                    // Interpreter not found
                    let json = format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"polyglot_exec: {interpreter} not found: {e}\",\"exit_code\":-1}}"
                    );
                    return Ok(alloc_string(json));
                }
            };

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);
            let success = output.status.success();

            // Escape JSON strings
            let stdout_esc = stdout
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            let stderr_esc = stderr
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");

            let json = format!(
                "{{\"success\":{success},\"stdout\":\"{stdout_esc}\",\"stderr\":\"{stderr_esc}\",\"exit_code\":{exit_code}}}"
            );
            Ok(alloc_string(json))
        }

        fn ffi_load(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_ffi() {
                return Err(host.err("ffi access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "ffi_load() expects 1 argument (path), got {}",
                    args.len()
                )));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("ffi_load(): {m}")))?;
            if !host.ffi_allowed_library(path) {
                return Err(host.err("ffi: library not in allowlist".into()));
            }
            let lib =
                unsafe { Library::new(path) }.map_err(|e| host.err(format!("ffi_load(): {e}")))?;
            let id = with_ffi_table(host, |t| {
                let id = FFI_NEXT.fetch_add(1, Ordering::Relaxed);
                t.insert(id, FfiResource::Library(lib));
                id
            })?;
            Ok(Value::Number(id as f64))
        }

        fn ffi_close(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_ffi() {
                return Err(host.err("ffi access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "ffi_close() expects 1 argument (handle), got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "ffi_close")?;
            let _ = with_ffi_table(host, |t| t.remove(&id));
            Ok(Value::Nil)
        }

        fn ffi_call(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_ffi() {
                return Err(host.err("ffi access denied".into()));
            }
            // ffi_call(handle, "symbol", [args...]) -> number (f64)
            if args.len() != 3 {
                return Err(host.err(format!(
                    "ffi_call() expects 3 arguments (handle, symbol, args_array), got {}",
                    args.len()
                )));
            }
            let sym = args[1]
                .as_str()
                .map_err(|m| host.err(format!("ffi_call(): {m}")))?;
            if !host.ffi_allowed_symbol(sym) {
                return Err(host.err("ffi: symbol not in allowlist".into()));
            }
            let id = value_to_handle(host, args[0].clone(), "ffi_call")?;
            let arr = match &args[2] {
                Value::Array(a) => a.clone(),
                _ => return Err(host.err("ffi_call(): expected args array".into())),
            };

            let mut nums: Vec<f64> = vec![];
            for v in arr.borrow().iter() {
                match v {
                    Value::Number(n) => nums.push(*n),
                    _ => return Err(host.err("ffi_call(): args must be numbers for now".into())),
                }
            }

            let res = match with_ffi_table(host, |t| t.remove(&id))? {
                Some(FfiResource::Library(lib)) => {
                    let out = unsafe {
                        match nums.len() {
                            0 => {
                                let f: libloading::Symbol<unsafe extern "C" fn() -> f64> = lib
                                    .get(sym.as_bytes())
                                    .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                                f()
                            }
                            1 => {
                                let f: libloading::Symbol<unsafe extern "C" fn(f64) -> f64> = lib
                                    .get(sym.as_bytes())
                                    .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                                f(nums[0])
                            }
                            2 => {
                                let f: libloading::Symbol<unsafe extern "C" fn(f64, f64) -> f64> =
                                    lib.get(sym.as_bytes())
                                        .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                                f(nums[0], nums[1])
                            }
                            3 => {
                                let f: libloading::Symbol<
                                    unsafe extern "C" fn(f64, f64, f64) -> f64,
                                > = lib
                                    .get(sym.as_bytes())
                                    .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                                f(nums[0], nums[1], nums[2])
                            }
                            4 => {
                                let f: libloading::Symbol<
                                    unsafe extern "C" fn(f64, f64, f64, f64) -> f64,
                                > = lib
                                    .get(sym.as_bytes())
                                    .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                                f(nums[0], nums[1], nums[2], nums[3])
                            }
                            5 => {
                                let f: libloading::Symbol<
                                    unsafe extern "C" fn(f64, f64, f64, f64, f64) -> f64,
                                > = lib
                                    .get(sym.as_bytes())
                                    .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                                f(nums[0], nums[1], nums[2], nums[3], nums[4])
                            }
                            6 => {
                                let f: libloading::Symbol<
                                    unsafe extern "C" fn(f64, f64, f64, f64, f64, f64) -> f64,
                                > = lib
                                    .get(sym.as_bytes())
                                    .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                                f(nums[0], nums[1], nums[2], nums[3], nums[4], nums[5])
                            }
                            n => {
                                let _ = with_ffi_table(host, |t| {
                                    t.insert(id, FfiResource::Library(lib))
                                });
                                return Err(
                                    host.err(format!("ffi_call(): too many args ({}), max 6", n))
                                );
                            }
                        }
                    };
                    let _ = with_ffi_table(host, |t| t.insert(id, FfiResource::Library(lib)));
                    Ok(Value::Number(out))
                }
                None => Err(host.err("ffi_call(): invalid handle".into())),
            }?;

            Ok(res)
        }

        fn sqlite_open(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_sqlite() {
                return Err(host.err("sqlite access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "sqlite_open() expects 1 argument (path), got {}",
                    args.len()
                )));
            }
            let path = args[0]
                .as_str()
                .map_err(|m| host.err(format!("sqlite_open(): {m}")))?;
            let conn =
                Connection::open(path).map_err(|e| host.err(format!("sqlite_open(): {e}")))?;
            let id = with_db_table(host, |t| {
                let id = DB_NEXT.fetch_add(1, Ordering::Relaxed);
                t.insert(id, DbResource::Sqlite(conn));
                id
            })?;
            Ok(Value::Number(id as f64))
        }

        fn sqlite_close(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_sqlite() {
                return Err(host.err("sqlite access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "sqlite_close() expects 1 argument (handle), got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "sqlite_close")?;
            let _ = with_db_table(host, |t| t.remove(&id));
            Ok(Value::Nil)
        }

        fn sqlite_exec(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_sqlite() {
                return Err(host.err("sqlite access denied".into()));
            }
            // sqlite_exec(handle, sql, params_array_or_nil) -> rows_changed
            if args.len() != 3 {
                return Err(host.err(format!(
                    "sqlite_exec() expects 3 arguments (handle, sql, params), got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "sqlite_exec")?;
            let sql = args[1]
                .as_str()
                .map_err(|m| host.err(format!("sqlite_exec(): {m}")))?;

            let params_v: Vec<Value> = match &args[2] {
                Value::Nil => vec![],
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("sqlite_exec(): params must be array or nil".into())),
            };

            let mut tosql: Vec<rusqlite::types::Value> = Vec::with_capacity(params_v.len());
            for v in params_v {
                match v {
                    Value::Nil => tosql.push(rusqlite::types::Value::Null),
                    Value::Bool(b) => {
                        tosql.push(rusqlite::types::Value::Integer(if b { 1 } else { 0 }))
                    }
                    Value::Number(n) => {
                        if n.is_finite()
                            && n.fract() == 0.0
                            && n >= (i64::MIN as f64)
                            && n <= (i64::MAX as f64)
                        {
                            tosql.push(rusqlite::types::Value::Integer(n as i64));
                        } else {
                            tosql.push(rusqlite::types::Value::Real(n));
                        }
                    }
                    Value::Obj(o) => match o.as_ref() {
                        Obj::String(s) => tosql.push(rusqlite::types::Value::Text(s.clone())),
                    },
                    Value::Array(a) => {
                        // treat array<number 0..255> as BLOB
                        let mut bytes: Vec<u8> = Vec::with_capacity(a.borrow().len());
                        for it in a.borrow().iter() {
                            match it {
                                Value::Number(n)
                                    if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= 255.0 =>
                                {
                                    bytes.push(*n as u8);
                                }
                                _ => {
                                    return Err(host.err(
                                        "sqlite_exec(): blob params must be array of byte numbers 0..255"
                                            .into(),
                                    ))
                                }
                            }
                        }
                        tosql.push(rusqlite::types::Value::Blob(bytes));
                    }
                    _ => {
                        return Err(host.err(
                            "sqlite_exec(): params support number/bool/string/nil/blob-array only"
                                .into(),
                        ))
                    }
                }
            }

            let changed = match with_db_table(host, |t| t.remove(&id))? {
                Some(DbResource::Sqlite(conn)) => {
                    let out = if tosql.is_empty() {
                        // Use execute_batch so multi-statement SQL (e.g. migration scripts
                        // like "CREATE TABLE t(...); INSERT INTO t VALUES (1);") runs fully.
                        // execute_batch does not return a row count, so we report 0.
                        conn.execute_batch(sql)
                            .map_err(|e| host.err(format!("sqlite_exec(): {e}")))?;
                        0usize
                    } else {
                        conn.execute(sql, rusqlite::params_from_iter(tosql.iter()))
                            .map_err(|e| host.err(format!("sqlite_exec(): {e}")))?
                    };
                    let _ = with_db_table(host, |t| t.insert(id, DbResource::Sqlite(conn)));
                    out
                }
                None => return Err(host.err("sqlite_exec(): invalid handle".into())),
            };
            Ok(Value::Number(changed as f64))
        }

        fn sqlite_query(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_sqlite() {
                return Err(host.err("sqlite access denied".into()));
            }
            // sqlite_query(handle, sql, params_array_or_nil) -> array<map{col: value}>
            if args.len() != 3 {
                return Err(host.err(format!(
                    "sqlite_query() expects 3 arguments (handle, sql, params), got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "sqlite_query")?;
            let sql = args[1]
                .as_str()
                .map_err(|m| host.err(format!("sqlite_query(): {m}")))?;

            let params_v: Vec<Value> = match &args[2] {
                Value::Nil => vec![],
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("sqlite_query(): params must be array or nil".into())),
            };

            let mut tosql: Vec<rusqlite::types::Value> = Vec::with_capacity(params_v.len());
            for v in params_v {
                match v {
                    Value::Nil => tosql.push(rusqlite::types::Value::Null),
                    Value::Bool(b) => {
                        tosql.push(rusqlite::types::Value::Integer(if b { 1 } else { 0 }))
                    }
                    Value::Number(n) => {
                        if n.is_finite()
                            && n.fract() == 0.0
                            && n >= (i64::MIN as f64)
                            && n <= (i64::MAX as f64)
                        {
                            tosql.push(rusqlite::types::Value::Integer(n as i64));
                        } else {
                            tosql.push(rusqlite::types::Value::Real(n));
                        }
                    }
                    Value::Obj(o) => match o.as_ref() {
                        Obj::String(s) => tosql.push(rusqlite::types::Value::Text(s.clone())),
                    },
                    Value::Array(a) => {
                        let mut bytes: Vec<u8> = Vec::with_capacity(a.borrow().len());
                        for it in a.borrow().iter() {
                            match it {
                                Value::Number(n)
                                    if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= 255.0 =>
                                {
                                    bytes.push(*n as u8);
                                }
                                _ => {
                                    return Err(host.err(
                                        "sqlite_query(): blob params must be array of byte numbers 0..255"
                                            .into(),
                                    ))
                                }
                            }
                        }
                        tosql.push(rusqlite::types::Value::Blob(bytes));
                    }
                    _ => {
                        return Err(host.err(
                            "sqlite_query(): params support number/bool/string/nil/blob-array only"
                                .into(),
                        ))
                    }
                }
            }

            let rows = match with_db_table(host, |t| t.remove(&id))? {
                Some(DbResource::Sqlite(conn)) => {
                    let out_rows = {
                        let mut stmt = conn
                            .prepare(sql)
                            .map_err(|e| host.err(format!("sqlite_query(): {e}")))?;
                        let col_count = stmt.column_count();
                        let col_names = (0..col_count)
                            .map(|i| stmt.column_name(i).unwrap_or("").to_string())
                            .collect::<Vec<_>>();

                        let mut out_rows: Vec<Value> = vec![];
                        let mut rows = stmt
                            .query(rusqlite::params_from_iter(tosql.iter()))
                            .map_err(|e| host.err(format!("sqlite_query(): {e}")))?;

                        while let Some(row) = rows
                            .next()
                            .map_err(|e| host.err(format!("sqlite_query(): {e}")))?
                        {
                            let mut m = HashMap::<String, Value>::new();
                            for (i, name) in col_names.iter().enumerate() {
                                let name = name.clone();
                                let v: rusqlite::types::Value = row
                                    .get(i)
                                    .map_err(|e| host.err(format!("sqlite_query(): {e}")))?;
                                let sv = match v {
                                    rusqlite::types::Value::Null => Value::Nil,
                                    rusqlite::types::Value::Integer(n) => Value::Number(n as f64),
                                    rusqlite::types::Value::Real(n) => Value::Number(n),
                                    rusqlite::types::Value::Text(s) => alloc_string(s),
                                    rusqlite::types::Value::Blob(b) => {
                                        let mut out = Vec::<Value>::with_capacity(b.len());
                                        for byte in b {
                                            out.push(Value::Number(byte as f64));
                                        }
                                        alloc_array(out)
                                    }
                                };
                                m.insert(name, sv);
                            }
                            out_rows.push(alloc_map(m));
                        }
                        out_rows
                    };

                    let _ = with_db_table(host, |t| t.insert(id, DbResource::Sqlite(conn)));
                    out_rows
                }
                None => return Err(host.err("sqlite_query(): invalid handle".into())),
            };

            Ok(alloc_array(rows))
        }

        fn sqlite_last_insert_rowid(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if !host.allow_sqlite() {
                return Err(host.err("sqlite access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!(
                    "sqlite_last_insert_rowid() expects 1 argument (handle), got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "sqlite_last_insert_rowid")?;
            let rowid = match with_db_table(host, |t| t.remove(&id))? {
                Some(DbResource::Sqlite(conn)) => {
                    let rid = conn.last_insert_rowid();
                    let _ = with_db_table(host, |t| t.insert(id, DbResource::Sqlite(conn)));
                    rid
                }
                None => return Err(host.err("sqlite_last_insert_rowid(): invalid handle".into())),
            };
            Ok(Value::Number(rowid as f64))
        }

        fn spawn_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_spawn() {
                return Err(host.err("spawn not allowed".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!("spawn() expects 1 argument, got {}", args.len())));
            }
            let closure = match &args[0] {
                Value::Closure(c) => c.clone(),
                _ => {
                    return Err(
                        host.err("spawn: expected a closure (function with no arguments)".into())
                    )
                }
            };
            // Serialize closed upvalues; open upvalues (still on stack) cannot transfer.
            let mut upvalues_json: Vec<sj::Value> = Vec::new();
            for uv in &closure.upvalues {
                let val = match &*uv.borrow() {
                    Upvalue::Closed(v) => v.clone(),
                    Upvalue::Open(_) => {
                        return Err(host.err("spawn: open upvalue cannot be transferred".into()))
                    }
                };
                upvalues_json.push(spawn_value_to_json(&val).map_err(|e| host.err(e))?);
            }
            let upvalues_json_str =
                sj::to_string(&upvalues_json).map_err(|e| host.err(e.to_string()))?;
            let chunk = &closure.function.chunk;
            let mut constants_json = Vec::new();
            for c in &chunk.constants {
                constants_json.push(spawn_value_to_json(c).map_err(|e| host.err(e))?);
            }
            let constants_json_str =
                sj::to_string(&constants_json).map_err(|e| host.err(e.to_string()))?;
            let instructions = chunk.instructions.clone();
            let spans = chunk.spans.clone();
            let spawn_limits = host.spawn_limits();

            let handles = vm_handles(host)?;
            let id = handles.spawn_next.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = mpsc::channel();
            handles
                .spawn
                .lock()
                .map_err(|_| host.err("internal: spawn table unavailable".into()))?
                .insert(id, rx);

            let child_handles = Arc::clone(&handles);
            let config = StdlibConfig::default();
            thread::spawn(move || {
                let constants: Vec<Value> =
                    match sj::from_str::<Vec<sj::Value>>(&constants_json_str) {
                        Ok(arr) => arr.iter().map(spawn_value_from_json).collect(),
                        Err(e) => {
                            let _ = tx.send(Err(e.to_string()));
                            return;
                        }
                    };
                let child_chunk = Chunk {
                    instructions: instructions.clone(),
                    spans: spans.clone(),
                    constants,
                    source_name: None,
                    source: None,
                };
                let function = Rc::new(Function {
                    name: None,
                    arity: 0,
                    min_arity: 0,
                    defaults: vec![],
                    is_vararg: false,
                    chunk: child_chunk,
                });
                let module = Rc::new(RefCell::new(Module {
                    globals: HashMap::new(),
                    parent: None,
                    exports: HashMap::new(),
                }));
                let child_closure = Rc::new(Closure {
                    function,
                    upvalues: match sj::from_str::<Vec<sj::Value>>(&upvalues_json_str) {
                        Ok(arr) => arr
                            .iter()
                            .map(|j| {
                                Rc::new(RefCell::new(Upvalue::Closed(spawn_value_from_json(j))))
                            })
                            .collect(),
                        Err(e) => {
                            let _ = tx.send(Err(e.to_string()));
                            return;
                        }
                    },
                    module,
                });
                let mut vm = VM::with_stdlib(config);
                vm.handle_registry = child_handles;
                if let Some(lim) = spawn_limits {
                    vm.set_limits(Some(VmLimits {
                        max_instructions: lim.max_instructions,
                        max_stack_depth: lim.max_stack_depth,
                        max_memory_bytes: lim.max_memory_bytes,
                        io_timeout_ms: lim.io_timeout_ms,
                    }));
                }
                // Connect the child closure's module to the VM's builtins
                // so that builtin globals (recv, send, etc.) are accessible.
                {
                    let builtins_parent = vm.module.borrow().parent.clone();
                    child_closure.module.borrow_mut().parent = builtins_parent;
                }
                let wrapper = Chunk {
                    instructions: vec![OpCode::OpConstant(0), OpCode::OpCall(0), OpCode::OpReturn],
                    constants: vec![Value::Closure(child_closure)],
                    spans: vec![Span::default(), Span::default(), Span::default()],
                    source_name: None,
                    source: None,
                };
                let result = match vm.run_return_value(wrapper) {
                    Ok(v) => spawn_value_to_json(&v)
                        .and_then(|j| sj::to_string(&j).map_err(|e| e.to_string())),
                    Err(e) => Err(e.message),
                };
                let _ = tx.send(result);
            });
            Ok(Value::Number(id as f64))
        }

        fn join_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.len() != 1 {
                return Err(host.err(format!("join() expects 1 argument, got {}", args.len())));
            }
            let id = value_to_handle(host, args[0].clone(), "join")?;
            let rx = {
                let mut t = reg.spawn
                    .lock()
                    .map_err(|_| host.err("internal: spawn table unavailable".into()))?;
                t.remove(&id)
                    .ok_or_else(|| host.err("join: invalid or already joined handle".into()))?
            };
            let result = rx
                .recv()
                .map_err(|_| host.err("join: channel closed (spawn failed?)".into()))?;
            match result {
                Ok(json_str) => {
                    let j: sj::Value =
                        sj::from_str(&json_str).map_err(|e| host.err(e.to_string()))?;
                    Ok(spawn_value_from_json(&j))
                }
                Err(msg) => Err(host.err(msg)),
            }
        }

        // ---- Channel builtins ----

        fn channel_builtin(
            host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = reg.channel_next.fetch_add(1, Ordering::SeqCst);
            let cb = VmChannelBuf {
                buf: Arc::new(Mutex::new(std::collections::VecDeque::new())),
                closed: Arc::new(AtomicBool::new(false)),
                capacity: 0, // unbounded
                space_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
                item_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
            };
            reg.channels
                .lock()
                .map_err(|_| host.err("channel: table unavailable".into()))?
                .insert(id, cb);
            let mut m = HashMap::new();
            m.insert("sender".into(), Value::Number(id as f64));
            m.insert("receiver".into(), Value::Number(id as f64));
            Ok(alloc_map(m))
        }

        /// channel_buffered(capacity) — Go-style buffered channel.
        /// Sends don't block until the buffer is full; recv blocks when empty.
        fn channel_buffered_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let capacity = match args.first() {
                Some(Value::Number(n)) if *n >= 1.0 => *n as usize,
                _ => {
                    return Err(host.err(
                        "channel_buffered(capacity): expected positive integer capacity".into(),
                    ))
                }
            };
            let id = reg.channel_next.fetch_add(1, Ordering::SeqCst);
            let cb = VmChannelBuf {
                buf: Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(
                    capacity,
                ))),
                closed: Arc::new(AtomicBool::new(false)),
                capacity,
                space_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
                item_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
            };
            reg.channels
                .lock()
                .map_err(|_| host.err("channel_buffered: table unavailable".into()))?
                .insert(id, cb);
            let mut m = HashMap::new();
            m.insert("sender".into(), Value::Number(id as f64));
            m.insert("receiver".into(), Value::Number(id as f64));
            m.insert("capacity".into(), Value::Number(capacity as f64));
            Ok(alloc_map(m))
        }

        fn channel_send_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.len() != 2 {
                return Err(host.err(format!("send() expects 2 arguments, got {}", args.len())));
            }
            let id = value_to_handle(host, args[0].clone(), "send")?;
            let json = spawn_value_to_json(&args[1]).map_err(|e| host.err(e))?;
            let json_str = sj::to_string(&json).map_err(|e| host.err(e.to_string()))?;
            let cb = {
                let t = reg.channels
                    .lock()
                    .map_err(|_| host.err("send: table unavailable".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("send: invalid channel id".into()))?
            };
            if cb.closed.load(Ordering::SeqCst) {
                return Err(host.err("send: channel is closed".into()));
            }
            // For bounded channels: block until there is space
            if cb.capacity > 0 {
                let deadline = std::time::Instant::now() + Duration::from_secs(30);
                loop {
                    {
                        let buf_len = cb
                            .buf
                            .lock()
                            .map_err(|_| host.err("send: buf lock unavailable".into()))?
                            .len();
                        if buf_len < cb.capacity {
                            break; // space available
                        }
                    }
                    if cb.closed.load(Ordering::SeqCst) {
                        return Err(host.err("send: channel closed while waiting for space".into()));
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err(host.err("send: timeout waiting for channel space".into()));
                    }
                    // Wait for space signal with 5ms sleep (condvar across threads is tricky here)
                    let (lock, cv) = cb.space_cv.as_ref();
                    let guard = lock
                        .lock()
                        .map_err(|_| host.err("send: condvar lock".into()))?;
                    let _ = cv.wait_timeout(guard, Duration::from_millis(5));
                }
            }
            cb.buf
                .lock()
                .map_err(|_| host.err("send: buf lock unavailable".into()))?
                .push_back(json_str);
            // Notify waiting receivers
            cb.item_cv.1.notify_all();
            Ok(Value::Nil)
        }

        fn channel_recv_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            channel_recv_impl(host, args, 30_000)
        }

        /// recv_timeout(receiver, timeout_ms) — recv with explicit timeout.
        /// Returns nil if the channel closed OR timed out; use channel_is_closed() to distinguish.
        fn channel_recv_timeout_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() < 2 {
                return Err(host.err("recv_timeout(receiver, ms) expects 2 arguments".into()));
            }
            let ms = match args[1] {
                Value::Number(n) if n >= 0.0 => n as u64,
                _ => {
                    return Err(
                        host.err("recv_timeout: timeout_ms must be a non-negative number".into())
                    )
                }
            };
            channel_recv_impl(host, vec![args[0].clone()], ms)
        }

        fn channel_recv_impl(
            host: &mut dyn VmHost,
            args: Vec<Value>,
            timeout_ms: u64,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.is_empty() {
                return Err(host.err("recv() expects 1 argument, got 0".into()));
            }
            let id = value_to_handle(host, args[0].clone(), "recv")?;
            let cb = {
                let t = reg.channels
                    .lock()
                    .map_err(|_| host.err("recv: table unavailable".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("recv: invalid channel id".into()))?
            };
            let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
            loop {
                // Try to dequeue
                {
                    let mut buf = cb
                        .buf
                        .lock()
                        .map_err(|_| host.err("recv: buf lock unavailable".into()))?;
                    if let Some(json_str) = buf.pop_front() {
                        // Notify blocked senders that space is available
                        cb.space_cv.1.notify_all();
                        let j: sj::Value =
                            sj::from_str(&json_str).map_err(|e| host.err(e.to_string()))?;
                        return Ok(spawn_value_from_json(&j));
                    }
                }
                if cb.closed.load(Ordering::SeqCst) {
                    return Ok(Value::Nil); // channel closed + empty
                }
                if std::time::Instant::now() >= deadline {
                    return Ok(Value::Nil); // timeout
                }
                // Sleep briefly then retry (condvar across threads requires Arc<Condvar>)
                let (lock, cv) = cb.item_cv.as_ref();
                let guard = lock
                    .lock()
                    .map_err(|_| host.err("recv: condvar lock".into()))?;
                let _ = cv.wait_timeout(guard, Duration::from_millis(1));
            }
        }

        fn channel_close_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.len() != 1 {
                return Err(host.err(format!(
                    "channel_close() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "channel_close")?;
            let cb = {
                let t = reg.channels
                    .lock()
                    .map_err(|_| host.err("channel_close: table unavailable".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("channel_close: invalid channel id".into()))?
            };
            cb.closed.store(true, Ordering::SeqCst);
            // Wake all blocked receivers so they can return nil
            cb.item_cv.1.notify_all();
            // Wake all blocked senders so they can error out
            cb.space_cv.1.notify_all();
            Ok(Value::Nil)
        }

        fn channel_is_closed_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.len() != 1 {
                return Err(host.err(format!(
                    "channel_is_closed() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "channel_is_closed")?;
            let cb = {
                let t = reg.channels
                    .lock()
                    .map_err(|_| host.err("channel_is_closed: table unavailable".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("channel_is_closed: invalid channel id".into()))?
            };
            Ok(Value::Bool(cb.closed.load(Ordering::SeqCst)))
        }

        fn channel_select_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.len() != 1 {
                return Err(host.err(format!("select() expects 1 argument, got {}", args.len())));
            }
            let ids: Vec<usize> = match &args[0] {
                Value::Array(a) => {
                    let a = a.borrow();
                    let mut out = Vec::with_capacity(a.len());
                    for v in a.iter() {
                        out.push(value_to_handle(host, v.clone(), "select")?);
                    }
                    out
                }
                _ => return Err(host.err("select(): expected array of channel ids".into())),
            };
            // Snapshot VmChannelBuf handles upfront (VmChannelBuf is Clone + Arc-backed, cheap).
            let cbs: Vec<Option<VmChannelBuf>> = {
                let t = reg.channels
                    .lock()
                    .map_err(|_| host.err("select: table unavailable".into()))?;
                ids.iter().map(|id| t.get(id).cloned()).collect()
            };
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                let mut all_closed_empty = true;
                for (i, cb_opt) in cbs.iter().enumerate() {
                    if let Some(cb) = cb_opt {
                        let is_closed = cb.closed.load(Ordering::SeqCst);
                        if let Ok(mut buf) = cb.buf.try_lock() {
                            if !buf.is_empty() {
                                // Dequeue the value and return {index, value}
                                let json_str = buf.pop_front().unwrap_or_default();
                                let v = sj::from_str::<sj::Value>(&json_str)
                                    .map(|j| spawn_value_from_json(&j))
                                    .unwrap_or(Value::Nil);
                                let mut m = std::collections::HashMap::new();
                                m.insert("index".into(), Value::Number(i as f64));
                                m.insert("value".into(), v);
                                return Ok(alloc_map(m));
                            }
                            // channel has data or could still receive
                            if !is_closed {
                                all_closed_empty = false;
                            }
                        } else {
                            all_closed_empty = false; // locked = possibly active
                        }
                    }
                }
                // If all channels are closed and empty, return nil
                if all_closed_empty {
                    return Ok(Value::Nil);
                }
                if std::time::Instant::now() >= deadline {
                    return Ok(Value::Nil); // timeout → nil (not an error)
                }
                thread::sleep(Duration::from_millis(1));
            }
        }

        // ---- Enhanced channel/threading builtins ----

        /// select_with_timeout(receivers_array, timeout_ms)
        /// Like select() but returns nil after timeout_ms milliseconds instead of nil on closed.
        /// Returns {index, value, timed_out: bool} or nil if all closed.
        fn channel_select_timeout_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.len() < 2 {
                return Err(
                    host.err("select_with_timeout(channels, ms) expects 2 arguments".into())
                );
            }
            let ms = match args[1] {
                Value::Number(n) if n >= 0.0 => n as u64,
                _ => {
                    return Err(
                        host.err("select_with_timeout: ms must be non-negative number".into())
                    )
                }
            };
            let ids: Vec<usize> = match &args[0] {
                Value::Array(a) => {
                    let a = a.borrow();
                    let mut out = Vec::with_capacity(a.len());
                    for v in a.iter() {
                        out.push(value_to_handle(host, v.clone(), "select_with_timeout")?);
                    }
                    out
                }
                _ => return Err(host.err("select_with_timeout: first arg must be array".into())),
            };
            let cbs: Vec<Option<VmChannelBuf>> = {
                let t = reg.channels
                    .lock()
                    .map_err(|_| host.err("select_with_timeout: table".into()))?;
                ids.iter().map(|id| t.get(id).cloned()).collect()
            };
            let deadline = std::time::Instant::now() + Duration::from_millis(ms);
            loop {
                let mut all_closed_empty = true;
                for (i, cb_opt) in cbs.iter().enumerate() {
                    if let Some(cb) = cb_opt {
                        let is_closed = cb.closed.load(Ordering::SeqCst);
                        if let Ok(mut buf) = cb.buf.try_lock() {
                            if !buf.is_empty() {
                                let json_str = buf.pop_front().unwrap_or_default();
                                cb.space_cv.1.notify_all();
                                let v = sj::from_str::<sj::Value>(&json_str)
                                    .map(|j| spawn_value_from_json(&j))
                                    .unwrap_or(Value::Nil);
                                let mut m = std::collections::HashMap::new();
                                m.insert("index".into(), Value::Number(i as f64));
                                m.insert("value".into(), v);
                                m.insert("timed_out".into(), Value::Bool(false));
                                return Ok(alloc_map(m));
                            }
                            if !is_closed {
                                all_closed_empty = false;
                            }
                        } else {
                            all_closed_empty = false;
                        }
                    }
                }
                if all_closed_empty {
                    return Ok(Value::Nil);
                }
                if std::time::Instant::now() >= deadline {
                    let mut m = std::collections::HashMap::new();
                    m.insert("timed_out".into(), Value::Bool(true));
                    return Ok(alloc_map(m));
                }
                thread::sleep(Duration::from_millis(1));
            }
        }

        /// channel_size(receiver) — number of items currently buffered in the channel.
        fn channel_size_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = value_to_handle(
                host,
                args.into_iter().next().unwrap_or(Value::Nil),
                "channel_size",
            )?;
            let t = reg.channels
                .lock()
                .map_err(|_| host.err("channel_size: table".into()))?;
            let cb = t
                .get(&id)
                .ok_or_else(|| host.err("channel_size: invalid channel id".into()))?;
            let n = cb
                .buf
                .lock()
                .map_err(|_| host.err("channel_size: buf lock".into()))?
                .len();
            Ok(Value::Number(n as f64))
        }

        /// join_all(handles_array) — join all thread handles, return array of results.
        /// Like Go's WaitGroup or Java's CompletableFuture.allOf().
        fn join_all_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let handles = match args.first() {
                Some(Value::Array(a)) => a.borrow().clone(),
                _ => return Err(host.err("join_all(handles): expected array of handles".into())),
            };
            let mut results = Vec::with_capacity(handles.len());
            for h in handles {
                let id = value_to_handle(host, h, "join_all")?;
                let rx = {
                    let mut t = reg.spawn
                        .lock()
                        .map_err(|_| host.err("join_all: spawn table unavailable".into()))?;
                    t.remove(&id).ok_or_else(|| {
                        host.err("join_all: invalid or already joined handle".into())
                    })?
                };
                let result = rx
                    .recv()
                    .map_err(|_| host.err("join_all: channel closed".into()))?;
                let val = match result {
                    Ok(json_str) => {
                        let j: sj::Value =
                            sj::from_str(&json_str).map_err(|e| host.err(e.to_string()))?;
                        spawn_value_from_json(&j)
                    }
                    Err(msg) => {
                        // Propagate error as {ok:false, error:msg}
                        let mut m = std::collections::HashMap::new();
                        m.insert("ok".to_string(), Value::Bool(false));
                        m.insert("error".to_string(), alloc_string(msg));
                        alloc_map(m)
                    }
                };
                results.push(val);
            }
            Ok(alloc_array(results))
        }

        /// spawn_n(n, fn) — spawn n threads each running fn(), return array of handles.
        /// Like Go's fan-out pattern: for i := 0; i < n; i++ { go fn() }.
        fn spawn_n_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() < 2 {
                return Err(host.err("spawn_n(n, fn) expects 2 arguments".into()));
            }
            let n = match args[0] {
                Value::Number(x) if x >= 1.0 => x as usize,
                _ => return Err(host.err("spawn_n: n must be a positive integer".into())),
            };
            let fn_val = args[1].clone();
            let mut handles = Vec::with_capacity(n);
            for _ in 0..n {
                let h = spawn_builtin(host, vec![fn_val.clone()])?;
                handles.push(h);
            }
            Ok(alloc_array(handles))
        }

        /// ticker(interval_ms) — create a channel that receives a tick every interval_ms.
        /// Like Go's time.NewTicker(). Close the channel to stop it.
        /// Returns the receiver handle (a channel).
        fn ticker_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let ms = match args.first() {
                Some(Value::Number(n)) if *n >= 1.0 => *n as u64,
                _ => return Err(host.err("ticker(interval_ms): expected positive ms".into())),
            };
            // Create an unbounded channel for tick events
            let tick_ch_id = reg.channel_next.fetch_add(1, Ordering::SeqCst);
            let tick_cb = VmChannelBuf {
                buf: Arc::new(Mutex::new(std::collections::VecDeque::new())),
                closed: Arc::new(AtomicBool::new(false)),
                capacity: 0,
                space_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
                item_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
            };
            reg.channels
                .lock()
                .map_err(|_| host.err("ticker: table unavailable".into()))?
                .insert(tick_ch_id, tick_cb.clone());
            // Spawn background ticker thread
            thread::spawn(move || {
                let mut tick_count = 0u64;
                loop {
                    thread::sleep(Duration::from_millis(ms));
                    if tick_cb.closed.load(Ordering::SeqCst) {
                        break;
                    }
                    tick_count += 1;
                    let json_str = format!("{}", tick_count);
                    if let Ok(mut buf) = tick_cb.buf.lock() {
                        buf.push_back(json_str);
                    }
                    tick_cb.item_cv.1.notify_all();
                }
            });
            // Return {sender, receiver} for compatibility
            let mut m = std::collections::HashMap::new();
            m.insert("sender".into(), Value::Number(tick_ch_id as f64));
            m.insert("receiver".into(), Value::Number(tick_ch_id as f64));
            Ok(alloc_map(m))
        }

        /// timeout_channel(ms) — returns a channel that sends exactly once after ms, then closes.
        /// Like Go's time.After(). Use with select to implement timeouts.
        fn timeout_channel_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let ms = match args.first() {
                Some(Value::Number(n)) if *n >= 0.0 => *n as u64,
                _ => return Err(host.err("timeout_channel(ms): expected non-negative ms".into())),
            };
            let ch_id = reg.channel_next.fetch_add(1, Ordering::SeqCst);
            let cb = VmChannelBuf {
                buf: Arc::new(Mutex::new(std::collections::VecDeque::new())),
                closed: Arc::new(AtomicBool::new(false)),
                capacity: 1, // buffered(1) so the fire-and-forget doesn't block
                space_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
                item_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
            };
            reg.channels
                .lock()
                .map_err(|_| host.err("timeout_channel: table unavailable".into()))?
                .insert(ch_id, cb.clone());
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(ms));
                if !cb.closed.load(Ordering::SeqCst) {
                    if let Ok(mut buf) = cb.buf.lock() {
                        buf.push_back("true".to_string());
                    }
                    cb.item_cv.1.notify_all();
                    cb.closed.store(true, Ordering::SeqCst);
                }
            });
            let mut m = std::collections::HashMap::new();
            m.insert("sender".into(), Value::Number(ch_id as f64));
            m.insert("receiver".into(), Value::Number(ch_id as f64));
            Ok(alloc_map(m))
        }

        // ---- Mutex builtins (Go sync.Mutex / Rust Mutex) ----

        /// mutex_new() — create a mutex. Returns a handle.
        fn mutex_new_builtin(
            host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = reg.mutex_next.fetch_add(1, Ordering::SeqCst);
            let m = Arc::new(VmSyncMutex {
                locked: Mutex::new(false),
                cv: std::sync::Condvar::new(),
            });
            reg.mutexes
                .lock()
                .map_err(|_| host.err("mutex_new: table unavailable".into()))?
                .insert(id, m);
            Ok(Value::Number(id as f64))
        }

        /// mutex_lock(handle) — acquire mutex; blocks until available.
        fn mutex_lock_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = value_to_handle(
                host,
                args.into_iter().next().unwrap_or(Value::Nil),
                "mutex_lock",
            )?;
            let entry = {
                let t = reg.mutexes
                    .lock()
                    .map_err(|_| host.err("mutex_lock: table".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("mutex_lock: invalid handle".into()))?
            };
            let mut locked = entry
                .locked
                .lock()
                .map_err(|_| host.err("mutex_lock: lock".into()))?;
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            while *locked {
                if std::time::Instant::now() >= deadline {
                    return Err(host.err("mutex_lock: deadlock timeout (30s)".into()));
                }
                let result = entry
                    .cv
                    .wait_timeout(locked, Duration::from_millis(10))
                    .map_err(|_| host.err("mutex_lock: condvar".into()))?;
                locked = result.0;
            }
            *locked = true;
            Ok(Value::Nil)
        }

        /// mutex_try_lock(handle) — non-blocking acquire. Returns true if locked, false if busy.
        fn mutex_try_lock_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = value_to_handle(
                host,
                args.into_iter().next().unwrap_or(Value::Nil),
                "mutex_try_lock",
            )?;
            let entry = {
                let t = reg.mutexes
                    .lock()
                    .map_err(|_| host.err("mutex_try_lock: table".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("mutex_try_lock: invalid handle".into()))?
            };
            let mut locked = entry
                .locked
                .lock()
                .map_err(|_| host.err("mutex_try_lock: lock".into()))?;
            if *locked {
                return Ok(Value::Bool(false));
            }
            *locked = true;
            Ok(Value::Bool(true))
        }

        /// mutex_unlock(handle) — release mutex.
        fn mutex_unlock_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = value_to_handle(
                host,
                args.into_iter().next().unwrap_or(Value::Nil),
                "mutex_unlock",
            )?;
            let entry = {
                let t = reg.mutexes
                    .lock()
                    .map_err(|_| host.err("mutex_unlock: table".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("mutex_unlock: invalid handle".into()))?
            };
            let mut locked = entry
                .locked
                .lock()
                .map_err(|_| host.err("mutex_unlock: lock".into()))?;
            if !*locked {
                return Err(host.err("mutex_unlock: mutex is not locked".into()));
            }
            *locked = false;
            entry.cv.notify_one();
            Ok(Value::Nil)
        }

        // ---- RwLock builtins (sync.RWMutex in Go / Rust RwLock) ----

        fn rwlock_new_builtin(
            host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = reg.rwlock_next.fetch_add(1, Ordering::SeqCst);
            reg.rwlocks
                .lock()
                .map_err(|_| host.err("rwlock_new: table".into()))?
                .insert(
                    id,
                    Arc::new(VmSyncRwLock {
                        state: Mutex::new(0),
                        cv: std::sync::Condvar::new(),
                    }),
                );
            Ok(Value::Number(id as f64))
        }

        /// rwlock_read(handle) — acquire shared (read) lock.
        fn rwlock_read_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = value_to_handle(
                host,
                args.into_iter().next().unwrap_or(Value::Nil),
                "rwlock_read",
            )?;
            let entry = {
                let t = reg.rwlocks
                    .lock()
                    .map_err(|_| host.err("rwlock_read: table".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("rwlock_read: invalid handle".into()))?
            };
            let mut state = entry
                .state
                .lock()
                .map_err(|_| host.err("rwlock_read: state".into()))?;
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            while *state < 0 {
                // write-locked
                if std::time::Instant::now() >= deadline {
                    return Err(host.err("rwlock_read: timeout".into()));
                }
                let r = entry
                    .cv
                    .wait_timeout(state, Duration::from_millis(5))
                    .map_err(|_| host.err("rwlock_read: condvar".into()))?;
                state = r.0;
            }
            *state += 1;
            Ok(Value::Nil)
        }

        /// rwlock_write(handle) — acquire exclusive (write) lock.
        fn rwlock_write_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = value_to_handle(
                host,
                args.into_iter().next().unwrap_or(Value::Nil),
                "rwlock_write",
            )?;
            let entry = {
                let t = reg.rwlocks
                    .lock()
                    .map_err(|_| host.err("rwlock_write: table".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("rwlock_write: invalid handle".into()))?
            };
            let mut state = entry
                .state
                .lock()
                .map_err(|_| host.err("rwlock_write: state".into()))?;
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            while *state != 0 {
                // either read(s) or write in progress
                if std::time::Instant::now() >= deadline {
                    return Err(host.err("rwlock_write: timeout".into()));
                }
                let r = entry
                    .cv
                    .wait_timeout(state, Duration::from_millis(5))
                    .map_err(|_| host.err("rwlock_write: condvar".into()))?;
                state = r.0;
            }
            *state = -1; // write-locked
            Ok(Value::Nil)
        }

        /// rwlock_unlock(handle) — release read OR write lock.
        fn rwlock_unlock_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = value_to_handle(
                host,
                args.into_iter().next().unwrap_or(Value::Nil),
                "rwlock_unlock",
            )?;
            let entry = {
                let t = reg.rwlocks
                    .lock()
                    .map_err(|_| host.err("rwlock_unlock: table".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("rwlock_unlock: invalid handle".into()))?
            };
            let mut state = entry
                .state
                .lock()
                .map_err(|_| host.err("rwlock_unlock: state".into()))?;
            if *state == -1 {
                *state = 0;
            }
            // write unlock
            else if *state > 0 {
                *state -= 1;
            }
            // read unlock
            else {
                return Err(host.err("rwlock_unlock: lock not held".into()));
            }
            entry.cv.notify_all();
            Ok(Value::Nil)
        }

        // ---- Barrier (CyclicBarrier in Java / sync.WaitGroup N threads) ----

        /// barrier_new(n) — create a barrier that releases all n threads when all have arrived.
        fn barrier_new_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let n = match args.first() {
                Some(Value::Number(x)) if *x >= 1.0 => *x as usize,
                _ => return Err(host.err("barrier_new(n): expected positive integer n".into())),
            };
            let id = reg.barrier_next.fetch_add(1, Ordering::SeqCst);
            reg.barriers
                .lock()
                .map_err(|_| host.err("barrier_new: table".into()))?
                .insert(
                    id,
                    Arc::new(VmSyncBarrier {
                        n,
                        arrived: Mutex::new(0),
                        cv: std::sync::Condvar::new(),
                    }),
                );
            Ok(Value::Number(id as f64))
        }

        /// barrier_wait(handle) — wait at barrier until all n threads have arrived.
        fn barrier_wait_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = value_to_handle(
                host,
                args.into_iter().next().unwrap_or(Value::Nil),
                "barrier_wait",
            )?;
            let entry = {
                let t = reg.barriers
                    .lock()
                    .map_err(|_| host.err("barrier_wait: table".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("barrier_wait: invalid handle".into()))?
            };
            let mut arrived = entry
                .arrived
                .lock()
                .map_err(|_| host.err("barrier_wait: lock".into()))?;
            *arrived += 1;
            if *arrived >= entry.n {
                *arrived = 0; // reset for reuse (CyclicBarrier semantics)
                entry.cv.notify_all();
                return Ok(Value::Bool(true)); // was the releasing thread
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(60);
            loop {
                let r = entry
                    .cv
                    .wait_timeout(arrived, Duration::from_millis(5))
                    .map_err(|_| host.err("barrier_wait: condvar".into()))?;
                arrived = r.0;
                if *arrived == 0 {
                    return Ok(Value::Bool(false));
                } // released, not the last
                if std::time::Instant::now() >= deadline {
                    return Err(host.err("barrier_wait: timeout (60s) — possible deadlock".into()));
                }
            }
        }

        // ---- Once (sync.Once in Go / lazy by-name in Kotlin) ----

        /// once_new() — create a Once handle.
        fn once_new_builtin(
            host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = reg.once_next.fetch_add(1, Ordering::SeqCst);
            reg.once_flags
                .lock()
                .map_err(|_| host.err("once_new: table".into()))?
                .insert(
                    id,
                    Arc::new(VmSyncOnce {
                        done: AtomicBool::new(false),
                        running: Mutex::new(false),
                        cv: std::sync::Condvar::new(),
                    }),
                );
            Ok(Value::Number(id as f64))
        }

        /// once_is_done(handle) — true if the once has already run.
        fn once_is_done_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = value_to_handle(
                host,
                args.into_iter().next().unwrap_or(Value::Nil),
                "once_is_done",
            )?;
            let t = reg.once_flags
                .lock()
                .map_err(|_| host.err("once_is_done: table".into()))?;
            let entry = t
                .get(&id)
                .ok_or_else(|| host.err("once_is_done: invalid handle".into()))?;
            Ok(Value::Bool(entry.done.load(Ordering::SeqCst)))
        }

        // ---- Runtime memory safety (ownership / borrowing) ----

        fn allocate_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.is_empty() {
                return Err(host.err("allocate(value, is_mutable?) expects at least 1 arg".into()));
            }
            let value = args[0].clone();
            let is_mutable = match args.get(1) {
                None => true,
                Some(Value::Bool(b)) => *b,
                Some(Value::Number(n)) => *n != 0.0,
                _ => {
                    return Err(host.err("allocate: is_mutable must be bool or number".into()));
                }
            };
            let h = host.mem_allocate(value, is_mutable)?;
            Ok(Value::Number(h))
        }

        fn borrow_immutable_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let handle = match args.first() {
                Some(Value::Number(n)) => *n,
                _ => return Err(host.err("borrow_immutable(handle) expects a number".into())),
            };
            Ok(Value::Number(host.mem_borrow_immutable(handle)?))
        }

        fn borrow_mutable_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let handle = match args.first() {
                Some(Value::Number(n)) => *n,
                _ => return Err(host.err("borrow_mutable(handle) expects a number".into())),
            };
            Ok(Value::Number(host.mem_borrow_mutable(handle)?))
        }

        fn borrow_unique_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let handle = match args.first() {
                Some(Value::Number(n)) => *n,
                _ => return Err(host.err("borrow_unique(handle) expects a number".into())),
            };
            Ok(Value::Number(host.mem_borrow_unique(handle)?))
        }

        fn release_borrow_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.is_empty() {
                return Err(host.err("release_borrow(borrow_id) or release_borrow(handle, type)".into()));
            }
            if args.len() == 1 {
                let borrow_id = match &args[0] {
                    Value::Number(n) => *n,
                    _ => return Err(host.err("release_borrow: expected number".into())),
                };
                host.mem_release_borrow_id(borrow_id)?;
            } else {
                let handle = match &args[0] {
                    Value::Number(n) => *n,
                    _ => return Err(host.err("release_borrow: handle must be a number".into())),
                };
                let ty = match &args[1] {
                    Value::Number(n) => *n as u8,
                    _ => return Err(host.err("release_borrow: type must be 0, 1, or 2".into())),
                };
                host.mem_release_borrow_type(handle, ty)?;
            }
            Ok(Value::Nil)
        }

        fn mem_load_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let handle = match args.first() {
                Some(Value::Number(n)) => *n,
                _ => return Err(host.err("load(handle) expects a number".into())),
            };
            host.mem_load(handle)
        }

        fn mem_store_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() < 2 {
                return Err(host.err("store(handle, value) expects 2 args".into()));
            }
            let handle = match &args[0] {
                Value::Number(n) => *n,
                _ => return Err(host.err("store: handle must be a number".into())),
            };
            host.mem_store(handle, args[1].clone())?;
            Ok(Value::Nil)
        }

        fn deallocate_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let handle = match args.first() {
                Some(Value::Number(n)) => *n,
                _ => return Err(host.err("deallocate(handle) expects a number".into())),
            };
            host.mem_deallocate(handle)?;
            Ok(Value::Nil)
        }

        fn push_lifetime_builtin(
            host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            Ok(Value::Number(host.mem_push_lifetime()?))
        }

        fn pop_lifetime_builtin(
            host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            Ok(Value::Number(host.mem_pop_lifetime()?))
        }

        fn memory_stats_builtin(
            host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            host.mem_stats()
        }

        // ---- Atomic i64 (Java AtomicInteger / Go atomic.Int64 / Rust AtomicI64) ----

        /// atomic_new(initial) — create an atomic integer.
        fn atomic_new_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let init = match args.first() {
                Some(Value::Number(n)) => *n as i64,
                None => 0,
                _ => return Err(host.err("atomic_new(val): expected number or no argument".into())),
            };
            let id = reg.atomic_next.fetch_add(1, Ordering::SeqCst);
            reg.atomics
                .lock()
                .map_err(|_| host.err("atomic_new: table".into()))?
                .insert(
                    id,
                    Arc::new(VmSyncAtomic {
                        val: AtomicI64::new(init),
                    }),
                );
            Ok(Value::Number(id as f64))
        }

        /// atomic_get(handle) — read the current value.
        fn atomic_get_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            let id = value_to_handle(
                host,
                args.into_iter().next().unwrap_or(Value::Nil),
                "atomic_get",
            )?;
            let t = reg.atomics
                .lock()
                .map_err(|_| host.err("atomic_get: table".into()))?;
            let entry = t
                .get(&id)
                .ok_or_else(|| host.err("atomic_get: invalid handle".into()))?;
            Ok(Value::Number(entry.val.load(Ordering::SeqCst) as f64))
        }

        /// atomic_set(handle, val) — store a value.
        fn atomic_set_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.len() < 2 {
                return Err(host.err("atomic_set(handle, val) expects 2 args".into()));
            }
            let id = value_to_handle(host, args[0].clone(), "atomic_set")?;
            let val = match args[1] {
                Value::Number(n) => n as i64,
                _ => return Err(host.err("atomic_set: value must be a number".into())),
            };
            let t = reg.atomics
                .lock()
                .map_err(|_| host.err("atomic_set: table".into()))?;
            let entry = t
                .get(&id)
                .ok_or_else(|| host.err("atomic_set: invalid handle".into()))?;
            entry.val.store(val, Ordering::SeqCst);
            Ok(Value::Nil)
        }

        /// atomic_add(handle, delta) — atomically add delta, return NEW value.
        fn atomic_add_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.len() < 2 {
                return Err(host.err("atomic_add(handle, delta) expects 2 args".into()));
            }
            let id = value_to_handle(host, args[0].clone(), "atomic_add")?;
            let delta = match args[1] {
                Value::Number(n) => n as i64,
                _ => return Err(host.err("atomic_add: delta must be a number".into())),
            };
            let t = reg.atomics
                .lock()
                .map_err(|_| host.err("atomic_add: table".into()))?;
            let entry = t
                .get(&id)
                .ok_or_else(|| host.err("atomic_add: invalid handle".into()))?;
            let new_val = entry.val.fetch_add(delta, Ordering::SeqCst) + delta;
            Ok(Value::Number(new_val as f64))
        }

        /// atomic_sub(handle, delta) — atomically subtract delta, return NEW value.
        fn atomic_sub_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.len() < 2 {
                return Err(host.err("atomic_sub(handle, delta) expects 2 args".into()));
            }
            let id = value_to_handle(host, args[0].clone(), "atomic_sub")?;
            let delta = match args[1] {
                Value::Number(n) => n as i64,
                _ => return Err(host.err("atomic_sub: delta must be a number".into())),
            };
            let t = reg.atomics
                .lock()
                .map_err(|_| host.err("atomic_sub: table".into()))?;
            let entry = t
                .get(&id)
                .ok_or_else(|| host.err("atomic_sub: invalid handle".into()))?;
            let new_val = entry.val.fetch_sub(delta, Ordering::SeqCst) - delta;
            Ok(Value::Number(new_val as f64))
        }

        /// atomic_cas(handle, expected, new) — compare-and-swap.
        /// Returns true if swap happened, false if current value != expected.
        fn atomic_cas_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.len() < 3 {
                return Err(host.err("atomic_cas(handle, expected, new) expects 3 args".into()));
            }
            let id = value_to_handle(host, args[0].clone(), "atomic_cas")?;
            let expected = match args[1] {
                Value::Number(n) => n as i64,
                _ => return Err(host.err("atomic_cas: expected must be a number".into())),
            };
            let new_val = match args[2] {
                Value::Number(n) => n as i64,
                _ => return Err(host.err("atomic_cas: new_val must be a number".into())),
            };
            let t = reg.atomics
                .lock()
                .map_err(|_| host.err("atomic_cas: table".into()))?;
            let entry = t
                .get(&id)
                .ok_or_else(|| host.err("atomic_cas: invalid handle".into()))?;
            let ok = entry
                .val
                .compare_exchange(expected, new_val, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok();
            Ok(Value::Bool(ok))
        }

        /// atomic_swap(handle, new_val) — atomically swap, return OLD value.
        fn atomic_swap_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let reg = vm_handles(host)?;
            if args.len() < 2 {
                return Err(host.err("atomic_swap(handle, val) expects 2 args".into()));
            }
            let id = value_to_handle(host, args[0].clone(), "atomic_swap")?;
            let val = match args[1] {
                Value::Number(n) => n as i64,
                _ => return Err(host.err("atomic_swap: val must be a number".into())),
            };
            let t = reg.atomics
                .lock()
                .map_err(|_| host.err("atomic_swap: table".into()))?;
            let entry = t
                .get(&id)
                .ok_or_else(|| host.err("atomic_swap: invalid handle".into()))?;
            let old = entry.val.swap(val, Ordering::SeqCst);
            Ok(Value::Number(old as f64))
        }

        // ---- String helper builtins ----

        fn str_slice_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 3 {
                return Err(host.err(format!(
                    "str_slice() expects 3 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("str_slice(): {m}")))?;
            let start = match args[1] {
                Value::Number(n) if n >= 0.0 => n as usize,
                _ => {
                    return Err(host.err("str_slice(): start must be a non-negative number".into()))
                }
            };
            let end = match args[2] {
                Value::Number(n) if n >= 0.0 => n as usize,
                _ => return Err(host.err("str_slice(): end must be a non-negative number".into())),
            };
            let chars: Vec<char> = s.chars().collect();
            let start = start.min(chars.len());
            let end = end.min(chars.len());
            if start >= end {
                return Ok(alloc_string(""));
            }
            let result: String = chars[start..end].iter().collect();
            Ok(alloc_string(result))
        }

        fn str_index_of_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "str_index_of() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("str_index_of(): {m}")))?;
            let sub = args[1]
                .as_str()
                .map_err(|m| host.err(format!("str_index_of(): {m}")))?;
            match s.find(sub) {
                Some(byte_idx) => {
                    // Convert byte offset to Unicode char index.
                    let char_idx = s[..byte_idx].chars().count();
                    Ok(Value::Number(char_idx as f64))
                }
                None => Ok(Value::Number(-1.0)),
            }
        }

        fn str_ends_with_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "str_ends_with() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("str_ends_with(): {m}")))?;
            let suffix = args[1]
                .as_str()
                .map_err(|m| host.err(format!("str_ends_with(): {m}")))?;
            Ok(Value::Bool(s.ends_with(suffix)))
        }

        fn str_lower_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "str_lower() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("str_lower(): {m}")))?;
            Ok(alloc_string(s.to_lowercase()))
        }

        fn str_upper_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "str_upper() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("str_upper(): {m}")))?;
            Ok(alloc_string(s.to_uppercase()))
        }

        fn strfromcharcode_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "strfromcharcode() expects 1 argument, got {}",
                    args.len()
                )));
            }
            match &args[0] {
                Value::Number(n) => {
                    let cp = *n as u32;
                    match char::from_u32(cp) {
                        Some(c) => Ok(alloc_string(c.to_string())),
                        None => Err(host.err(format!(
                            "strfromcharcode(): {} is not a valid unicode code point",
                            cp
                        ))),
                    }
                }
                _ => Err(host.err("strfromcharcode(): expected number".into())),
            }
        }

        fn strcharcodeat_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "strcharcodeat() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("strcharcodeat(): {m}")))?;
            let idx = match &args[1] {
                Value::Number(n) => *n as usize,
                _ => return Err(host.err("strcharcodeat(): index must be a number".into())),
            };
            let bytes = s.as_bytes();
            if idx >= bytes.len() {
                return Err(host.err(format!(
                    "strcharcodeat(): index {} out of bounds (len {})",
                    idx,
                    bytes.len()
                )));
            }
            Ok(Value::Number(bytes[idx] as f64))
        }

        // ---- Sort / numeric helpers ----

        fn array_sort_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "array_sort() expects 1 argument, got {}",
                    args.len()
                )));
            }
            match &args[0] {
                Value::Array(a) => {
                    let mut items = a.borrow().clone();
                    items.sort_by(|x, y| match (x, y) {
                        (Value::Number(a), Value::Number(b)) => {
                            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        (Value::Obj(a), Value::Obj(b)) => {
                            let sa = match a.as_ref() {
                                pblang_ir::Obj::String(s) => s.clone(),
                            };
                            let sb = match b.as_ref() {
                                pblang_ir::Obj::String(s) => s.clone(),
                            };
                            sa.cmp(&sb)
                        }
                        _ => std::cmp::Ordering::Equal,
                    });
                    Ok(alloc_array(items))
                }
                _ => Err(host.err("array_sort(): expected array".into())),
            }
        }

        // ── Higher-order array builtins (native, faster than pblang stdlib) ─────────────────

        #[allow(dead_code)]
        fn call_value_1(
            _host: &mut dyn VmHost,
            _f: Value,
            _arg: Value,
        ) -> Result<Value, RuntimeError> {
            // Placeholder: cannot call pblang closures from within a builtin without
            // VM re-entry. Use stdlib/array.sapp or for-in loop instead.
            unreachable!("call_value_1 should never be called directly")
        }

        /// array_map(arr, fn) — like arr.map() but as a builtin name
        /// Falls back gracefully: just calls the pblang array:map helper.
        /// The real implementation is in stdlib/array.sapp; this is a thin alias
        /// so code can call `array_map(arr, fn)` without importing array.sapp.
        #[allow(dead_code)]
        fn array_map_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "array_map() expects 2 arguments (array, fn), got {}",
                    args.len()
                )));
            }
            // We use OpCall internally via the VM — but since we're inside a builtin
            // we cannot re-enter the main dispatch loop easily.
            // Approach: use host.import_module to get the array module, then call .map.
            // Simpler approach for prototype: implement the loop manually.
            let _arr = match &args[0] {
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("array_map(): first argument must be an array".into())),
            };
            let _ = &args[1]; // fn — cannot call from builtin without VM re-entry
                              // Since we can't call pblang functions from builtins without VM re-entry,
                              // we signal the caller to use `import("stdlib:array.sapp").map(arr, fn)` instead.
                              // However, we can at least verify types and return an error with a good message.
            match &args[1] {
                Value::Function(_) | Value::Closure(_) | Value::Builtin(_) => {}
                _ => return Err(host.err("array_map(): second argument must be a function".into())),
            }
            Err(host.err("array_map(): use import('stdlib:array.sapp').map(arr, fn) or the for-in loop; direct builtin call not supported".into()))
        }

        /// str_replace_all(s, from, to) — replace all occurrences of `from` with `to` in `s`.
        /// Faster and safer than regex_replace_all for literal strings.
        fn str_replace_all_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 3 {
                return Err(host.err(format!(
                    "str_replace_all() expects 3 arguments (string, from, to), got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|m| host.err(format!("str_replace_all(): {m}")))?;
            let from = args[1]
                .as_str()
                .map_err(|m| host.err(format!("str_replace_all(): {m}")))?;
            let to = args[2]
                .as_str()
                .map_err(|m| host.err(format!("str_replace_all(): {m}")))?;
            Ok(alloc_string(s.replace(from, to)))
        }

        /// hmac_sha256(data, key) — compute HMAC-SHA256 of data with key.
        /// Returns a hex-encoded string. Requires the crypto category.
        fn hmac_sha256_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            // Crypto ops are always available — they are pure computation, no I/O.
            if args.len() != 2 {
                return Err(host.err(format!(
                    "hmac_sha256() expects 2 arguments (data, key), got {}",
                    args.len()
                )));
            }
            let data = args[0]
                .as_str()
                .map_err(|m| host.err(format!("hmac_sha256(): {m}")))?;
            let key = args[1]
                .as_str()
                .map_err(|m| host.err(format!("hmac_sha256(): {m}")))?;
            // HMAC-SHA256: K XOR opad || SHA256(K XOR ipad || data)
            // RFC 2104 implementation using only sha2 crate (already a dep)
            use sha2::{Digest, Sha256};
            const BLOCK_SIZE: usize = 64;
            let key_bytes = key.as_bytes();
            // Derive block-sized key
            let mut k: Vec<u8> = if key_bytes.len() > BLOCK_SIZE {
                let mut h = Sha256::new();
                h.update(key_bytes);
                h.finalize().to_vec()
            } else {
                key_bytes.to_vec()
            };
            k.resize(BLOCK_SIZE, 0);
            // Inner hash: SHA256((K XOR ipad) || data)
            let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
            let mut inner = Sha256::new();
            inner.update(&ipad);
            inner.update(data.as_bytes());
            let inner_hash = inner.finalize();
            // Outer hash: SHA256((K XOR opad) || inner_hash)
            let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
            let mut outer = Sha256::new();
            outer.update(&opad);
            outer.update(inner_hash);
            let result = outer.finalize();
            Ok(alloc_string(hex::encode(result)))
        }

        /// str_format(fmt, ...args) — Python-style %s/%d/%f formatting
        /// Example: str_format("Hello %s, you are %d years old!", name, age)
        fn str_format_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.is_empty() {
                return Err(
                    host.err("str_format() expects at least 1 argument (format string)".into())
                );
            }
            let fmt = args[0]
                .as_str()
                .map_err(|_| host.err("str_format(): first argument must be a string".into()))?;

            let mut result = String::with_capacity(fmt.len() * 2);
            let mut arg_idx = 1usize;
            let mut chars = fmt.chars().peekable();

            while let Some(ch) = chars.next() {
                if ch == '%' {
                    if let Some(&spec) = chars.peek() {
                        chars.next();
                        match spec {
                            's' => {
                                if arg_idx >= args.len() {
                                    return Err(
                                        host.err("str_format(): not enough arguments".into())
                                    );
                                }
                                result.push_str(&format!("{}", args[arg_idx]));
                                arg_idx += 1;
                            }
                            'd' | 'i' => {
                                if arg_idx >= args.len() {
                                    return Err(
                                        host.err("str_format(): not enough arguments".into())
                                    );
                                }
                                match &args[arg_idx] {
                                    Value::Number(n) => result.push_str(&format!("{}", *n as i64)),
                                    v => result.push_str(&format!("{v}")),
                                }
                                arg_idx += 1;
                            }
                            'f' => {
                                if arg_idx >= args.len() {
                                    return Err(
                                        host.err("str_format(): not enough arguments".into())
                                    );
                                }
                                match &args[arg_idx] {
                                    Value::Number(n) => result.push_str(&format!("{:.6}", n)),
                                    v => result.push_str(&format!("{v}")),
                                }
                                arg_idx += 1;
                            }
                            'v' => {
                                // %v = any value (Go-style)
                                if arg_idx >= args.len() {
                                    return Err(
                                        host.err("str_format(): not enough arguments".into())
                                    );
                                }
                                result.push_str(&format!("{}", args[arg_idx]));
                                arg_idx += 1;
                            }
                            '%' => result.push('%'),
                            other => {
                                result.push('%');
                                result.push(other);
                            }
                        }
                    } else {
                        result.push('%');
                    }
                } else {
                    result.push(ch);
                }
            }
            Ok(alloc_string(result))
        }

        // ── Regex builtins ─────────────────────────────────────────────────────

        /// regex_match(str, pattern) → bool
        /// Returns true if str contains a match for the regex pattern.
        fn regex_match_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "regex_match() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|_| host.err("regex_match(): first arg must be string".into()))?;
            let pat = args[1]
                .as_str()
                .map_err(|_| host.err("regex_match(): second arg must be string".into()))?;
            let re = regex::Regex::new(pat)
                .map_err(|e| host.err(format!("regex_match(): invalid pattern: {e}")))?;
            Ok(Value::Bool(re.is_match(s)))
        }

        /// regex_find(str, pattern) → string | nil
        /// Returns the first match, or nil if no match.
        fn regex_find_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "regex_find() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|_| host.err("regex_find(): first arg must be string".into()))?;
            let pat = args[1]
                .as_str()
                .map_err(|_| host.err("regex_find(): second arg must be string".into()))?;
            let re = regex::Regex::new(pat)
                .map_err(|e| host.err(format!("regex_find(): invalid pattern: {e}")))?;
            match re.find(s) {
                Some(m) => Ok(alloc_string(m.as_str())),
                None => Ok(Value::Nil),
            }
        }

        /// regex_find_all(str, pattern) → array of strings
        /// Returns all non-overlapping matches.
        fn regex_find_all_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "regex_find_all() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|_| host.err("regex_find_all(): first arg must be string".into()))?;
            let pat = args[1]
                .as_str()
                .map_err(|_| host.err("regex_find_all(): second arg must be string".into()))?;
            let re = regex::Regex::new(pat)
                .map_err(|e| host.err(format!("regex_find_all(): invalid pattern: {e}")))?;
            let matches: Vec<Value> = re.find_iter(s).map(|m| alloc_string(m.as_str())).collect();
            Ok(alloc_array(matches))
        }

        /// regex_replace(str, pattern, replacement) → string
        /// Replaces the first match with replacement.
        fn regex_replace_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 3 {
                return Err(host.err(format!(
                    "regex_replace() expects 3 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|_| host.err("regex_replace(): first arg must be string".into()))?;
            let pat = args[1]
                .as_str()
                .map_err(|_| host.err("regex_replace(): second arg must be string".into()))?;
            let rep = args[2]
                .as_str()
                .map_err(|_| host.err("regex_replace(): third arg must be string".into()))?;
            let re = regex::Regex::new(pat)
                .map_err(|e| host.err(format!("regex_replace(): invalid pattern: {e}")))?;
            Ok(alloc_string(re.replacen(s, 1, rep).to_string()))
        }

        /// regex_replace_all(str, pattern, replacement) → string
        /// Replaces all matches with replacement.
        fn regex_replace_all_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 3 {
                return Err(host.err(format!(
                    "regex_replace_all() expects 3 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|_| host.err("regex_replace_all(): first arg must be string".into()))?;
            let pat = args[1]
                .as_str()
                .map_err(|_| host.err("regex_replace_all(): second arg must be string".into()))?;
            let rep = args[2]
                .as_str()
                .map_err(|_| host.err("regex_replace_all(): third arg must be string".into()))?;
            let re = regex::Regex::new(pat)
                .map_err(|e| host.err(format!("regex_replace_all(): invalid pattern: {e}")))?;
            Ok(alloc_string(re.replace_all(s, rep).to_string()))
        }

        /// regex_split(str, pattern) → array of strings
        /// Splits str on every match of pattern.
        fn regex_split_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "regex_split() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|_| host.err("regex_split(): first arg must be string".into()))?;
            let pat = args[1]
                .as_str()
                .map_err(|_| host.err("regex_split(): second arg must be string".into()))?;
            let re = regex::Regex::new(pat)
                .map_err(|e| host.err(format!("regex_split(): invalid pattern: {e}")))?;
            let parts: Vec<Value> = re.split(s).map(alloc_string).collect();
            Ok(alloc_array(parts))
        }

        /// regex_capture(str, pattern) → array of captured group strings | nil
        /// Returns an array of capture groups for the first match, or nil if no match.
        /// Group 0 is the entire match; groups 1+ are parenthesized captures.
        fn regex_capture_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "regex_capture() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let s = args[0]
                .as_str()
                .map_err(|_| host.err("regex_capture(): first arg must be string".into()))?;
            let pat = args[1]
                .as_str()
                .map_err(|_| host.err("regex_capture(): second arg must be string".into()))?;
            let re = regex::Regex::new(pat)
                .map_err(|e| host.err(format!("regex_capture(): invalid pattern: {e}")))?;
            match re.captures(s) {
                None => Ok(Value::Nil),
                Some(caps) => {
                    let groups: Vec<Value> = caps
                        .iter()
                        .map(|m| match m {
                            Some(m) => alloc_string(m.as_str()),
                            None => Value::Nil,
                        })
                        .collect();
                    Ok(alloc_array(groups))
                }
            }
        }

        // ── str_format is a core utility ────────────────────────────────────────────────

        // ── Fundamental sequence/collection builtins ──────────────────────────────────

        /// range(n) → [0,1,...,n-1]
        /// range(start, end) → [start,...,end-1]
        /// range(start, end, step) → array with step
        fn range_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            let (start, end, step) = match args.len() {
                1 => {
                    let n = match &args[0] {
                        Value::Number(x) => *x,
                        _ => return Err(host.err("range(): argument must be number".into())),
                    };
                    (0.0, n, 1.0)
                }
                2 => {
                    let s = match &args[0] {
                        Value::Number(x) => *x,
                        _ => return Err(host.err("range(): arguments must be numbers".into())),
                    };
                    let e = match &args[1] {
                        Value::Number(x) => *x,
                        _ => return Err(host.err("range(): arguments must be numbers".into())),
                    };
                    (s, e, 1.0)
                }
                3 => {
                    let s = match &args[0] {
                        Value::Number(x) => *x,
                        _ => return Err(host.err("range(): arguments must be numbers".into())),
                    };
                    let e = match &args[1] {
                        Value::Number(x) => *x,
                        _ => return Err(host.err("range(): arguments must be numbers".into())),
                    };
                    let st = match &args[2] {
                        Value::Number(x) if *x != 0.0 => *x,
                        _ => return Err(host.err("range(): step must be non-zero number".into())),
                    };
                    (s, e, st)
                }
                _ => {
                    return Err(
                        host.err(format!("range() expects 1-3 arguments, got {}", args.len()))
                    )
                }
            };
            let mut result = Vec::new();
            let mut i = start;
            if step > 0.0 {
                while i < end {
                    result.push(Value::Number(i));
                    i += step;
                }
            } else {
                while i > end {
                    result.push(Value::Number(i));
                    i += step;
                }
            }
            Ok(alloc_array(result))
        }

        /// enumerate(arr) → array of [index, value] pairs
        fn enumerate_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "enumerate() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let items = match &args[0] {
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("enumerate(): argument must be array".into())),
            };
            let result: Vec<Value> = items
                .into_iter()
                .enumerate()
                .map(|(i, v)| alloc_array(vec![Value::Number(i as f64), v]))
                .collect();
            Ok(alloc_array(result))
        }

        /// sum(arr) → numeric sum of array (nil/non-numeric elements are skipped)
        fn sum_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("sum() expects 1 argument, got {}", args.len())));
            }
            let items = match &args[0] {
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("sum(): argument must be array".into())),
            };
            let s: f64 = items
                .iter()
                .filter_map(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .sum();
            Ok(Value::Number(s))
        }

        /// min_of(arr) → minimum numeric value, or nil if empty
        fn min_of_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("min_of() expects 1 argument, got {}", args.len())));
            }
            let items = match &args[0] {
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("min_of(): argument must be array".into())),
            };
            let nums: Vec<f64> = items
                .iter()
                .filter_map(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .collect();
            if nums.is_empty() {
                return Ok(Value::Nil);
            }
            Ok(Value::Number(
                nums.iter().cloned().fold(f64::INFINITY, f64::min),
            ))
        }

        /// max_of(arr) → maximum numeric value, or nil if empty
        fn max_of_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("max_of() expects 1 argument, got {}", args.len())));
            }
            let items = match &args[0] {
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("max_of(): argument must be array".into())),
            };
            let nums: Vec<f64> = items
                .iter()
                .filter_map(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .collect();
            if nums.is_empty() {
                return Ok(Value::Nil);
            }
            Ok(Value::Number(
                nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            ))
        }

        /// product(arr) → product of all numeric elements
        fn product_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("product() expects 1 argument, got {}", args.len())));
            }
            let items = match &args[0] {
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("product(): argument must be array".into())),
            };
            let p: f64 = items
                .iter()
                .filter_map(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .product();
            Ok(Value::Number(p))
        }

        /// clamp(val, min, max) → value clamped to [min, max]
        fn clamp_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 3 {
                return Err(host.err("clamp(val, min, max) expects 3 arguments".into()));
            }
            let v = match &args[0] {
                Value::Number(x) => *x,
                _ => return Err(host.err("clamp(): arguments must be numbers".into())),
            };
            let mn = match &args[1] {
                Value::Number(x) => *x,
                _ => return Err(host.err("clamp(): arguments must be numbers".into())),
            };
            let mx = match &args[2] {
                Value::Number(x) => *x,
                _ => return Err(host.err("clamp(): arguments must be numbers".into())),
            };
            Ok(Value::Number(v.max(mn).min(mx)))
        }

        // ── Type-checking predicates (additional) ────────────────────────────────────────────────

        fn is_number_builtin(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            Ok(Value::Bool(matches!(args.first(), Some(Value::Number(_)))))
        }
        fn is_string_builtin(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            Ok(Value::Bool(matches!(args.first(), Some(Value::Obj(_)))))
        }
        fn is_array_builtin(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            Ok(Value::Bool(matches!(args.first(), Some(Value::Array(_)))))
        }
        fn is_map_builtin(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            Ok(Value::Bool(matches!(args.first(), Some(Value::Map(_)))))
        }
        fn is_bool_builtin(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            Ok(Value::Bool(matches!(args.first(), Some(Value::Bool(_)))))
        }
        fn is_fn_builtin(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            Ok(Value::Bool(matches!(
                args.first(),
                Some(Value::Function(_) | Value::Closure(_) | Value::Builtin(_))
            )))
        }
        // is_nil_builtin already defined above (line ~1694)

        // ── New string helpers ───────────────────────────────────────────────────────

        /// str_repeat(s, n) → string repeated n times
        fn str_repeat_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err("str_repeat(s, n) expects 2 arguments".into()));
            }
            let s = args[0]
                .as_str()
                .map_err(|_| host.err("str_repeat(): first arg must be string".into()))?;
            let n = match &args[1] {
                Value::Number(x) if *x >= 0.0 => *x as usize,
                _ => {
                    return Err(
                        host.err("str_repeat(): second arg must be non-negative number".into())
                    )
                }
            };
            Ok(alloc_string(s.repeat(n)))
        }

        /// str_pad_left(s, width, pad_char) → left-pads s to width with pad_char
        fn str_pad_left_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 3 {
                return Err(host.err("str_pad_left(s, width, pad) expects 3 arguments".into()));
            }
            let s = args[0]
                .as_str()
                .map_err(|_| host.err("str_pad_left(): first arg must be string".into()))?;
            let w = match &args[1] {
                Value::Number(x) => *x as usize,
                _ => return Err(host.err("str_pad_left(): width must be number".into())),
            };
            let pad = args[2]
                .as_str()
                .map_err(|_| host.err("str_pad_left(): pad must be string".into()))?;
            let pad_ch = pad.chars().next().unwrap_or(' ');
            let cur = s.chars().count();
            if cur >= w {
                return Ok(alloc_string(s));
            }
            let padding: String = std::iter::repeat_n(pad_ch, w - cur).collect();
            Ok(alloc_string(format!("{padding}{s}")))
        }

        /// str_pad_right(s, width, pad_char) → right-pads s to width with pad_char
        fn str_pad_right_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 3 {
                return Err(host.err("str_pad_right(s, width, pad) expects 3 arguments".into()));
            }
            let s = args[0]
                .as_str()
                .map_err(|_| host.err("str_pad_right(): first arg must be string".into()))?;
            let w = match &args[1] {
                Value::Number(x) => *x as usize,
                _ => return Err(host.err("str_pad_right(): width must be number".into())),
            };
            let pad = args[2]
                .as_str()
                .map_err(|_| host.err("str_pad_right(): pad must be string".into()))?;
            let pad_ch = pad.chars().next().unwrap_or(' ');
            let cur = s.chars().count();
            if cur >= w {
                return Ok(alloc_string(s));
            }
            let padding: String = std::iter::repeat_n(pad_ch, w - cur).collect();
            Ok(alloc_string(format!("{s}{padding}")))
        }

        // ── New array helpers ───────────────────────────────────────────────────────

        /// array_unique(arr) → new array with duplicate values removed (preserves order, first occurrence)
        fn array_unique_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err("array_unique(arr) expects 1 argument".into()));
            }
            let items = match &args[0] {
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("array_unique(): argument must be an array".into())),
            };
            let mut seen: Vec<Value> = vec![];
            let mut result: Vec<Value> = vec![];
            for item in items {
                if !seen.iter().any(|s| s == &item) {
                    seen.push(item.clone());
                    result.push(item);
                }
            }
            Ok(alloc_array(result))
        }

        /// array_zip(a, b) → array of [a_i, b_i] pairs (stops at shorter length)
        fn array_zip_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err("array_zip(a, b) expects 2 arguments".into()));
            }
            let a = match &args[0] {
                Value::Array(x) => x.borrow().clone(),
                _ => return Err(host.err("array_zip(): first arg must be array".into())),
            };
            let b = match &args[1] {
                Value::Array(x) => x.borrow().clone(),
                _ => return Err(host.err("array_zip(): second arg must be array".into())),
            };
            let n = a.len().min(b.len());
            let result: Vec<Value> = (0..n)
                .map(|i| alloc_array(vec![a[i].clone(), b[i].clone()]))
                .collect();
            Ok(alloc_array(result))
        }

        /// array_chunk(arr, size) → array of arrays of at most `size` elements
        fn array_chunk_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err("array_chunk(arr, size) expects 2 arguments".into()));
            }
            let items = match &args[0] {
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("array_chunk(): first arg must be array".into())),
            };
            let size = match &args[1] {
                Value::Number(n) if *n >= 1.0 => *n as usize,
                _ => return Err(host.err("array_chunk(): size must be positive number".into())),
            };
            let result: Vec<Value> = items
                .chunks(size)
                .map(|chunk| alloc_array(chunk.to_vec()))
                .collect();
            Ok(alloc_array(result))
        }

        /// array_flatten(arr) → flattens one level of nesting
        fn array_flatten_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err("array_flatten(arr) expects 1 argument".into()));
            }
            let items = match &args[0] {
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("array_flatten(): argument must be array".into())),
            };
            let mut result: Vec<Value> = vec![];
            for item in items {
                match item {
                    Value::Array(sub) => result.extend(sub.borrow().clone()),
                    other => result.push(other),
                }
            }
            Ok(alloc_array(result))
        }

        /// array_take(arr, n) → first n elements
        fn array_take_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err("array_take(arr, n) expects 2 arguments".into()));
            }
            let items = match &args[0] {
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("array_take(): first arg must be array".into())),
            };
            let n = match &args[1] {
                Value::Number(x) if *x >= 0.0 => *x as usize,
                _ => return Err(host.err("array_take(): n must be non-negative".into())),
            };
            Ok(alloc_array(items.into_iter().take(n).collect()))
        }

        /// array_drop(arr, n) → all elements after the first n
        fn array_drop_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err("array_drop(arr, n) expects 2 arguments".into()));
            }
            let items = match &args[0] {
                Value::Array(a) => a.borrow().clone(),
                _ => return Err(host.err("array_drop(): first arg must be array".into())),
            };
            let n = match &args[1] {
                Value::Number(x) if *x >= 0.0 => *x as usize,
                _ => return Err(host.err("array_drop(): n must be non-negative".into())),
            };
            Ok(alloc_array(items.into_iter().skip(n).collect()))
        }

        fn num_abs_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("num_abs() expects 1 argument, got {}", args.len())));
            }
            match &args[0] {
                Value::Number(n) => Ok(Value::Number(n.abs())),
                _ => Err(host.err("num_abs(): expected number".into())),
            }
        }

        fn num_max_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!("num_max() expects 2 arguments, got {}", args.len())));
            }
            match (&args[0], &args[1]) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.max(*b))),
                _ => Err(host.err("num_max(): expected numbers".into())),
            }
        }

        fn num_min_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!("num_min() expects 2 arguments, got {}", args.len())));
            }
            match (&args[0], &args[1]) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.min(*b))),
                _ => Err(host.err("num_min(): expected numbers".into())),
            }
        }

        // ---- OS builtins ----

        fn os_args_builtin(
            _host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let argv: Vec<Value> = std::env::args().skip(1).map(alloc_string).collect();
            Ok(alloc_array(argv))
        }

        fn os_exit_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!("os_exit() expects 1 argument, got {}", args.len())));
            }
            let code = match args[0] {
                Value::Number(n) => n as i32,
                _ => return Err(host.err("os_exit(): expected number".into())),
            };
            std::process::exit(code);
        }

        fn os_signal_trap_builtin(
            host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            Err(host.err(
                "os_signal_trap() is not implemented on this runtime target yet".into(),
            ))
        }

        /// os_which(cmd) — safely find an executable in PATH.
        /// Uses std::process::Command to check each PATH entry directly (no shell injection).
        fn os_which_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if !host.allow_exec() {
                return Err(host.err("exec access denied".into()));
            }
            if args.len() != 1 {
                return Err(host.err(format!("os_which() expects 1 argument, got {}", args.len())));
            }
            let cmd = args[0]
                .as_str()
                .map_err(|m| host.err(format!("os_which(): {m}")))?;
            // Validate the command name (no path separators, no metacharacters)
            let is_dangerous: bool = cmd.chars().any(|c| "'/\\;&| \t\n><`$(){}!#~\"".contains(c));
            if is_dangerous || cmd.is_empty() {
                return Err(host.err(format!("os_which(): unsafe command name: {}", cmd)));
            }
            // Search PATH entries directly (no shell)
            let path_var = std::env::var("PATH").unwrap_or_default();
            let separator = if cfg!(windows) { ';' } else { ':' };
            for dir in path_var.split(separator) {
                let candidate = std::path::Path::new(dir).join(cmd);
                if candidate.is_file() {
                    if let Some(s) = candidate.to_str() {
                        return Ok(alloc_string(s.to_string()));
                    }
                }
                // Windows: also try with .exe extension
                #[cfg(windows)]
                {
                    let candidate_exe = std::path::Path::new(dir).join(format!("{}.exe", cmd));
                    if candidate_exe.is_file() {
                        if let Some(s) = candidate_exe.to_str() {
                            return Ok(alloc_string(s.to_string()));
                        }
                    }
                }
            }
            Ok(Value::Nil)
        }

        // ---- WebSocket accept key (RFC 6455) ----

        fn websocket_accept_key_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "websocket_accept_key() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let key = args[0]
                .as_str()
                .map_err(|m| host.err(format!("websocket_accept_key(): {m}")))?;
            const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
            let combined = format!("{key}{WS_GUID}");
            use sha1::Digest as _;
            let hash = sha1::Sha1::digest(combined.as_bytes());
            let encoded =
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, hash.as_slice());
            Ok(alloc_string(encoded))
        }

        // ---- HTTP post_auth (with header forwarding) ----

        fn http_post_auth_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() < 3 {
                return Err(host.err(format!(
                    "http_post_auth() expects 3 arguments, got {}",
                    args.len()
                )));
            }
            if !host.allow_http() {
                return Err(host.err("http access denied".into()));
            }
            let url = args[0]
                .as_str()
                .map_err(|m| host.err(format!("http_post_auth(): {m}")))?;
            let body_str = args[2]
                .as_str()
                .map_err(|m| host.err(format!("http_post_auth(): body: {m}")))?;
            // Collect headers first to avoid borrow conflicts.
            let headers: Vec<(String, String)> = if let Value::Map(ref hm) = args[1] {
                hm.borrow()
                    .iter()
                    .filter_map(|(k, v)| v.as_str().ok().map(|s| (k.clone(), s.to_string())))
                    .collect()
            } else {
                vec![]
            };
            let mut req = ureq::post(url);
            for (k, v) in &headers {
                req = req.set(k.as_str(), v.as_str());
            }
            let req = if let Some(ms) = host.io_timeout_ms() {
                req.timeout(Duration::from_millis(ms))
            } else {
                req
            };
            match req.send_string(body_str) {
                Ok(resp) => Ok(alloc_string(resp.into_string().unwrap_or_default())),
                Err(e) => Ok(alloc_string(format!("error: {e}"))),
            }
        }

        fn http_get_auth_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() < 2 {
                return Err(host.err(format!(
                    "http_get_auth() expects 2 arguments (url, token), got {}",
                    args.len()
                )));
            }
            if !host.allow_http() {
                return Err(host.err("http access denied".into()));
            }
            let url = args[0]
                .as_str()
                .map_err(|m| host.err(format!("http_get_auth(): {m}")))?;
            let token = args[1]
                .as_str()
                .map_err(|m| host.err(format!("http_get_auth(): token: {m}")))?;
            let mut req = ureq::get(url).set("Authorization", &format!("Bearer {token}"));
            if let Some(ms) = host.io_timeout_ms() {
                req = req.timeout(Duration::from_millis(ms));
            }
            match req.call() {
                Ok(resp) => Ok(alloc_string(resp.into_string().unwrap_or_default())),
                Err(e) => Ok(alloc_string(format!("error: {e}"))),
            }
        }

        // ---- parse_sse_line ----

        fn parse_sse_line_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "parse_sse_line() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let line = args[0]
                .as_str()
                .map_err(|m| host.err(format!("parse_sse_line(): {m}")))?;
            let (field, value) = if let Some(v) = line.strip_prefix("data: ") {
                ("data", v)
            } else if let Some(v) = line.strip_prefix("event: ") {
                ("event", v)
            } else if let Some(v) = line.strip_prefix("id: ") {
                ("id", v)
            } else if let Some(v) = line.strip_prefix("retry: ") {
                ("retry", v)
            } else {
                ("", line)
            };
            let mut m = HashMap::new();
            m.insert("field".into(), alloc_string(field));
            m.insert("value".into(), alloc_string(value));
            Ok(alloc_map(m))
        }

        // ---- WebSocket frame builtins ----

        fn ws_parse_frame_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "ws_parse_frame() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let frame = args[0]
                .as_str()
                .map_err(|m| host.err(format!("ws_parse_frame(): {m}")))?;
            let codes: Vec<u32> = frame.chars().map(|c| c as u32).collect();
            if codes.len() < 2 {
                return Err(host.err("ws_parse_frame(): frame too short".into()));
            }

            let b0 = codes[0] as u8;
            let b1 = codes[1] as u8;
            let fin = if (b0 & 0x80) != 0 { 1.0 } else { 0.0 };
            let opcode = (b0 & 0x0f) as f64;
            let masked = (b1 & 0x80) != 0;
            let payload_len = (b1 & 0x7f) as usize;
            let mut idx = 2usize;

            let mut masking_key = [0u8; 4];
            if masked {
                if codes.len() < idx + 4 {
                    return Err(host.err("ws_parse_frame(): masked frame missing key".into()));
                }
                for i in 0..4 {
                    masking_key[i] = codes[idx + i] as u8;
                }
                idx += 4;
            }
            if codes.len() < idx + payload_len {
                return Err(host.err("ws_parse_frame(): truncated payload".into()));
            }

            let mut payload = String::with_capacity(payload_len);
            for i in 0..payload_len {
                let mut b = codes[idx + i] as u8;
                if masked {
                    b ^= masking_key[i % 4];
                }
                let ch = std::char::from_u32(b as u32)
                    .ok_or_else(|| host.err("ws_parse_frame(): invalid payload byte".into()))?;
                payload.push(ch);
            }

            let mut m = HashMap::new();
            m.insert("opcode".into(), Value::Number(opcode));
            m.insert("payload".into(), alloc_string(payload));
            m.insert("fin".into(), Value::Number(fin));
            m.insert("masked".into(), Value::Number(if masked { 1.0 } else { 0.0 }));
            Ok(alloc_map(m))
        }

        fn ws_write_frame_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "ws_write_frame() expects 2 arguments (opcode, payload), got {}",
                    args.len()
                )));
            }
            let opcode = match args[0] {
                Value::Number(n) if n.is_finite() && n >= 0.0 && n <= 15.0 => n as u8,
                _ => return Err(host.err("ws_write_frame(): opcode must be number 0-15".into())),
            };
            let payload = args[1]
                .as_str()
                .map_err(|m| host.err(format!("ws_write_frame(): payload: {m}")))?;
            let payload_bytes: Vec<u8> = payload.chars().map(|c| (c as u32).min(255) as u8).collect();
            if payload_bytes.len() > 125 {
                return Err(host.err(
                    "ws_write_frame(): payload >125 bytes is not implemented yet".into(),
                ));
            }

            let mut rng = rand::thread_rng();
            let mask_key = [
                rand::Rng::gen::<u8>(&mut rng),
                rand::Rng::gen::<u8>(&mut rng),
                rand::Rng::gen::<u8>(&mut rng),
                rand::Rng::gen::<u8>(&mut rng),
            ];
            let mut out = String::new();
            let b0 = 0x80u8 | (opcode & 0x0f); // FIN=1
            let b1 = 0x80u8 | (payload_bytes.len() as u8); // MASK=1
            out.push(
                std::char::from_u32(b0 as u32)
                    .ok_or_else(|| host.err("ws_write_frame(): invalid header byte".into()))?,
            );
            out.push(
                std::char::from_u32(b1 as u32)
                    .ok_or_else(|| host.err("ws_write_frame(): invalid header byte".into()))?,
            );
            for b in mask_key {
                out.push(
                    std::char::from_u32(b as u32)
                        .ok_or_else(|| host.err("ws_write_frame(): invalid mask byte".into()))?,
                );
            }
            for (i, b) in payload_bytes.iter().enumerate() {
                let masked_b = *b ^ mask_key[i % 4];
                out.push(
                    std::char::from_u32(masked_b as u32).ok_or_else(|| {
                        host.err("ws_write_frame(): invalid masked payload byte".into())
                    })?,
                );
            }
            Ok(alloc_string(out))
        }

        let mut v: Vec<BuiltinFn> = vec![
            BuiltinFn {
                name: "clock",
                arity: Some(0),
                fun: clock,
            },
            BuiltinFn {
                name: "now_ms",
                arity: Some(0),
                fun: now_ms,
            },
            BuiltinFn {
                name: "now_iso",
                arity: Some(0),
                fun: now_iso,
            },
            BuiltinFn {
                name: "type",
                arity: Some(1),
                fun: type_of,
            },
            BuiltinFn {
                name: "str",
                arity: Some(1),
                fun: str_,
            },
            BuiltinFn {
                name: "num_sqrt",
                arity: Some(1),
                fun: num_sqrt,
            },
            BuiltinFn {
                name: "num_pow",
                arity: Some(2),
                fun: num_pow,
            },
            BuiltinFn {
                name: "num_ln",
                arity: Some(1),
                fun: num_ln,
            },
            BuiltinFn {
                name: "num_exp",
                arity: Some(1),
                fun: num_exp,
            },
            BuiltinFn {
                name: "num_atan2",
                arity: Some(2),
                fun: num_atan2,
            },
            BuiltinFn {
                name: "num_mod",
                arity: Some(2),
                fun: num_mod,
            },
            BuiltinFn {
                name: "num_floor",
                arity: Some(1),
                fun: num_floor,
            },
            BuiltinFn {
                name: "num_ceil",
                arity: Some(1),
                fun: num_ceil,
            },
            BuiltinFn {
                name: "parse_number",
                arity: Some(1),
                fun: parse_number,
            },
            BuiltinFn {
                name: "parse_bool",
                arity: Some(1),
                fun: parse_bool,
            },
            BuiltinFn {
                name: "read_line",
                arity: Some(0),
                fun: read_line,
            },
            BuiltinFn {
                name: "len",
                arity: Some(1),
                fun: len,
            },
            BuiltinFn {
                name: "array_push",
                arity: Some(2),
                fun: array_push,
            },
            BuiltinFn {
                name: "array_pop",
                arity: Some(1),
                fun: array_pop,
            },
            BuiltinFn {
                name: "map_keys",
                arity: Some(1),
                fun: map_keys,
            },
            BuiltinFn {
                name: "map_has",
                arity: Some(2),
                fun: map_has,
            },
            BuiltinFn {
                name: "map_remove",
                arity: Some(2),
                fun: map_remove,
            },
            BuiltinFn {
                name: "fs_read",
                arity: Some(1),
                fun: fs_read,
            },
            BuiltinFn {
                name: "fs_write",
                arity: Some(2),
                fun: fs_write,
            },
            BuiltinFn {
                name: "fs_append",
                arity: Some(2),
                fun: fs_append,
            },
            BuiltinFn {
                name: "fs_remove",
                arity: Some(1),
                fun: fs_remove,
            },
            BuiltinFn {
                name: "fs_rename",
                arity: Some(2),
                fun: fs_rename,
            },
            BuiltinFn {
                name: "fs_copy",
                arity: Some(2),
                fun: fs_copy,
            },
            BuiltinFn {
                name: "fs_read_bytes",
                arity: Some(1),
                fun: fs_read_bytes,
            },
            BuiltinFn {
                name: "fs_write_bytes",
                arity: Some(2),
                fun: fs_write_bytes,
            },
            BuiltinFn {
                name: "env_get",
                arity: Some(1),
                fun: env_get,
            },
            BuiltinFn {
                name: "env_set",
                arity: Some(2),
                fun: env_set,
            },
            BuiltinFn {
                name: "sleep_ms",
                arity: Some(1),
                fun: sleep_ms,
            },
            BuiltinFn {
                name: "exec",
                arity: Some(1),
                fun: exec,
            },
            BuiltinFn {
                name: "proc_spawn_wait",
                arity: Some(2),
                fun: proc_spawn_wait,
            },
            BuiltinFn {
                name: "import",
                arity: Some(1),
                fun: import,
            },
            BuiltinFn {
                name: "gc_stats",
                arity: Some(0),
                fun: gc_stats,
            },
            BuiltinFn {
                name: "gc_collect",
                arity: Some(0),
                fun: gc_collect,
            },
            // --- Safety builtins (Rust-style guarantees for pblang code) ---
            BuiltinFn {
                name: "assert",
                arity: None,
                fun: assert_builtin,
            },
            BuiltinFn {
                name: "type_assert",
                arity: Some(2),
                fun: type_assert_builtin,
            },
            BuiltinFn {
                name: "freeze",
                arity: Some(1),
                fun: freeze_builtin,
            },
            BuiltinFn {
                name: "is_frozen",
                arity: Some(1),
                fun: is_frozen_builtin,
            },
            BuiltinFn {
                name: "checked_add",
                arity: Some(2),
                fun: checked_add_builtin,
            },
            BuiltinFn {
                name: "checked_mul",
                arity: Some(2),
                fun: checked_mul_builtin,
            },
            BuiltinFn {
                name: "checked_sub",
                arity: Some(2),
                fun: checked_sub_builtin,
            },
            BuiltinFn {
                name: "checked_div",
                arity: Some(2),
                fun: checked_div_builtin,
            },
            BuiltinFn {
                name: "is_nil",
                arity: Some(1),
                fun: is_nil_builtin,
            },
            BuiltinFn {
                name: "require_type",
                arity: Some(2),
                fun: require_type_builtin,
            },
            BuiltinFn {
                name: "sha256",
                arity: Some(1),
                fun: sha256,
            },
            BuiltinFn {
                name: "random_bytes",
                arity: Some(1),
                fun: random_bytes,
            },
            BuiltinFn {
                name: "random_float",
                arity: Some(0),
                fun: random_float_builtin,
            },
            BuiltinFn {
                name: "constant_time_eq",
                arity: Some(2),
                fun: constant_time_eq,
            },
            BuiltinFn {
                name: "base64_encode",
                arity: Some(1),
                fun: base64_encode,
            },
            BuiltinFn {
                name: "base64_decode",
                arity: Some(1),
                fun: base64_decode,
            },
            // FIX #22: binary-safe base64 decode
            BuiltinFn {
                name: "base64_decode_bytes",
                arity: Some(1),
                fun: base64_decode_bytes,
            },
            BuiltinFn {
                name: "json_parse",
                arity: Some(1),
                fun: json_parse,
            },
            BuiltinFn {
                name: "json_stringify",
                arity: Some(1),
                fun: json_stringify,
            },
            BuiltinFn {
                name: "http_get",
                arity: Some(1),
                fun: http_get,
            },
            BuiltinFn {
                name: "http_post",
                arity: Some(2),
                fun: http_post,
            },
            BuiltinFn {
                name: "str_contains",
                arity: Some(2),
                fun: str_contains,
            },
            BuiltinFn {
                name: "str_starts_with",
                arity: Some(2),
                fun: str_starts_with,
            },
            BuiltinFn {
                name: "str_trim",
                arity: Some(1),
                fun: str_trim,
            },
            BuiltinFn {
                name: "str_split",
                arity: Some(2),
                fun: str_split,
            },
            BuiltinFn {
                name: "cwd",
                arity: Some(0),
                fun: cwd,
            },
            BuiltinFn {
                name: "fs_exists",
                arity: Some(1),
                fun: fs_exists,
            },
            BuiltinFn {
                name: "fs_mkdirs",
                arity: Some(1),
                fun: fs_mkdirs,
            },
            BuiltinFn {
                name: "fs_listdir",
                arity: Some(1),
                fun: fs_listdir,
            },
            BuiltinFn {
                name: "path_join",
                arity: None,
                fun: path_join,
            },
            BuiltinFn {
                name: "path_dirname",
                arity: Some(1),
                fun: path_dirname,
            },
            BuiltinFn {
                name: "path_basename",
                arity: Some(1),
                fun: path_basename,
            },
            BuiltinFn {
                name: "tcp_listen",
                arity: Some(1),
                fun: tcp_listen,
            },
            BuiltinFn {
                name: "tcp_accept",
                arity: Some(1),
                fun: tcp_accept,
            },
            BuiltinFn {
                name: "tcp_read",
                arity: Some(2),
                fun: tcp_read,
            },
            BuiltinFn {
                name: "tcp_read_until",
                arity: Some(3),
                fun: tcp_read_until,
            },
            BuiltinFn {
                name: "tcp_write",
                arity: Some(2),
                fun: tcp_write,
            },
            BuiltinFn {
                name: "tcp_close",
                arity: Some(1),
                fun: tcp_close,
            },
            BuiltinFn {
                name: "tcp_local_addr",
                arity: Some(1),
                fun: tcp_local_addr,
            },
            // TLS builtins (W4-gamma)
            BuiltinFn {
                name: "tcp_listen_tls",
                arity: Some(3),
                fun: tcp_listen_tls,
            },
            BuiltinFn {
                name: "tcp_connect_tls",
                arity: Some(2),
                fun: tcp_connect_tls,
            },
            BuiltinFn {
                name: "tcp_wrap_tls",
                arity: Some(3),
                fun: tcp_wrap_tls,
            },
            BuiltinFn {
                name: "http_parse_request",
                arity: Some(1),
                fun: http_parse_request,
            },
            BuiltinFn {
                name: "http_response",
                arity: Some(2),
                fun: http_response,
            },
            BuiltinFn {
                name: "http_response_ct",
                arity: Some(3),
                fun: http_response_ct,
            },
            BuiltinFn {
                name: "http_response_json",
                arity: Some(2),
                fun: http_response_json,
            },
            BuiltinFn {
                name: "onnx_infer",
                arity: None,
                fun: onnx_infer,
            },
            BuiltinFn {
                name: "ai_chat",
                arity: Some(1),
                fun: ai_chat,
            },
            BuiltinFn {
                name: "ai_embed",
                arity: Some(1),
                fun: ai_embed,
            },
            BuiltinFn {
                name: "ai_token_count",
                arity: Some(1),
                fun: ai_token_count,
            },
            BuiltinFn {
                name: "quantum_run",
                arity: None,
                fun: quantum_run,
            },
            BuiltinFn {
                name: "quantum_statevector",
                arity: None,
                fun: quantum_statevector,
            },
            BuiltinFn {
                name: "quantum_backends",
                arity: Some(0),
                fun: quantum_backends,
            },
            BuiltinFn {
                name: "polyglot_exec",
                arity: Some(2),
                fun: polyglot_exec,
            },
            BuiltinFn {
                name: "ffi_load",
                arity: Some(1),
                fun: ffi_load,
            },
            BuiltinFn {
                name: "ffi_call",
                arity: Some(3),
                fun: ffi_call,
            },
            BuiltinFn {
                name: "ffi_close",
                arity: Some(1),
                fun: ffi_close,
            },
            BuiltinFn {
                name: "sqlite_open",
                arity: Some(1),
                fun: sqlite_open,
            },
            BuiltinFn {
                name: "sqlite_close",
                arity: Some(1),
                fun: sqlite_close,
            },
            BuiltinFn {
                name: "sqlite_exec",
                arity: Some(3),
                fun: sqlite_exec,
            },
            BuiltinFn {
                name: "sqlite_query",
                arity: Some(3),
                fun: sqlite_query,
            },
            BuiltinFn {
                name: "sqlite_last_insert_rowid",
                arity: Some(1),
                fun: sqlite_last_insert_rowid,
            },
            BuiltinFn {
                name: "spawn",
                arity: Some(1),
                fun: spawn_builtin,
            },
            BuiltinFn {
                name: "join",
                arity: Some(1),
                fun: join_builtin,
            },
            // Channel builtins
            BuiltinFn {
                name: "channel",
                arity: Some(0),
                fun: channel_builtin,
            },
            BuiltinFn {
                name: "send",
                arity: Some(2),
                fun: channel_send_builtin,
            },
            BuiltinFn {
                name: "recv",
                arity: Some(1),
                fun: channel_recv_builtin,
            },
            BuiltinFn {
                name: "channel_close",
                arity: Some(1),
                fun: channel_close_builtin,
            },
            BuiltinFn {
                name: "channel_is_closed",
                arity: Some(1),
                fun: channel_is_closed_builtin,
            },
            BuiltinFn {
                name: "select",
                arity: Some(1),
                fun: channel_select_builtin,
            },
            // ---- Enhanced threading builtins ----
            BuiltinFn {
                name: "channel_buffered",
                arity: Some(1),
                fun: channel_buffered_builtin,
            },
            BuiltinFn {
                name: "recv_timeout",
                arity: Some(2),
                fun: channel_recv_timeout_builtin,
            },
            BuiltinFn {
                name: "select_with_timeout",
                arity: Some(2),
                fun: channel_select_timeout_builtin,
            },
            BuiltinFn {
                name: "channel_size",
                arity: Some(1),
                fun: channel_size_builtin,
            },
            BuiltinFn {
                name: "join_all",
                arity: Some(1),
                fun: join_all_builtin,
            },
            BuiltinFn {
                name: "spawn_n",
                arity: Some(2),
                fun: spawn_n_builtin,
            },
            BuiltinFn {
                name: "ticker",
                arity: Some(1),
                fun: ticker_builtin,
            },
            BuiltinFn {
                name: "timeout_channel",
                arity: Some(1),
                fun: timeout_channel_builtin,
            },
            // ---- Mutex (Go sync.Mutex / Rust Mutex) ----
            BuiltinFn {
                name: "mutex_new",
                arity: Some(0),
                fun: mutex_new_builtin,
            },
            BuiltinFn {
                name: "mutex_lock",
                arity: Some(1),
                fun: mutex_lock_builtin,
            },
            BuiltinFn {
                name: "mutex_try_lock",
                arity: Some(1),
                fun: mutex_try_lock_builtin,
            },
            BuiltinFn {
                name: "mutex_unlock",
                arity: Some(1),
                fun: mutex_unlock_builtin,
            },
            // ---- RwLock (Go sync.RWMutex / Rust RwLock) ----
            BuiltinFn {
                name: "rwlock_new",
                arity: Some(0),
                fun: rwlock_new_builtin,
            },
            BuiltinFn {
                name: "rwlock_read",
                arity: Some(1),
                fun: rwlock_read_builtin,
            },
            BuiltinFn {
                name: "rwlock_write",
                arity: Some(1),
                fun: rwlock_write_builtin,
            },
            BuiltinFn {
                name: "rwlock_unlock",
                arity: Some(1),
                fun: rwlock_unlock_builtin,
            },
            // ---- Barrier (Java CyclicBarrier / Go WaitGroup) ----
            BuiltinFn {
                name: "barrier_new",
                arity: Some(1),
                fun: barrier_new_builtin,
            },
            BuiltinFn {
                name: "barrier_wait",
                arity: Some(1),
                fun: barrier_wait_builtin,
            },
            // ---- Once (Go sync.Once / Kotlin by lazy) ----
            BuiltinFn {
                name: "once_new",
                arity: Some(0),
                fun: once_new_builtin,
            },
            BuiltinFn {
                name: "once_is_done",
                arity: Some(1),
                fun: once_is_done_builtin,
            },
            // ---- Runtime memory safety ----
            BuiltinFn {
                name: "allocate",
                arity: None,
                fun: allocate_builtin,
            },
            BuiltinFn {
                name: "borrow_immutable",
                arity: Some(1),
                fun: borrow_immutable_builtin,
            },
            BuiltinFn {
                name: "borrow_mutable",
                arity: Some(1),
                fun: borrow_mutable_builtin,
            },
            BuiltinFn {
                name: "borrow_unique",
                arity: Some(1),
                fun: borrow_unique_builtin,
            },
            BuiltinFn {
                name: "release_borrow",
                arity: None,
                fun: release_borrow_builtin,
            },
            BuiltinFn {
                name: "load",
                arity: Some(1),
                fun: mem_load_builtin,
            },
            BuiltinFn {
                name: "store",
                arity: Some(2),
                fun: mem_store_builtin,
            },
            BuiltinFn {
                name: "deallocate",
                arity: Some(1),
                fun: deallocate_builtin,
            },
            BuiltinFn {
                name: "push_lifetime",
                arity: Some(0),
                fun: push_lifetime_builtin,
            },
            BuiltinFn {
                name: "pop_lifetime",
                arity: Some(0),
                fun: pop_lifetime_builtin,
            },
            BuiltinFn {
                name: "memory_stats",
                arity: Some(0),
                fun: memory_stats_builtin,
            },
            // ---- Atomic i64 (Java AtomicInteger / Go atomic.Int64 / Rust AtomicI64) ----
            BuiltinFn {
                name: "atomic_new",
                arity: None,
                fun: atomic_new_builtin,
            },
            BuiltinFn {
                name: "atomic_get",
                arity: Some(1),
                fun: atomic_get_builtin,
            },
            BuiltinFn {
                name: "atomic_set",
                arity: Some(2),
                fun: atomic_set_builtin,
            },
            BuiltinFn {
                name: "atomic_add",
                arity: Some(2),
                fun: atomic_add_builtin,
            },
            BuiltinFn {
                name: "atomic_sub",
                arity: Some(2),
                fun: atomic_sub_builtin,
            },
            BuiltinFn {
                name: "atomic_cas",
                arity: Some(3),
                fun: atomic_cas_builtin,
            },
            BuiltinFn {
                name: "atomic_swap",
                arity: Some(2),
                fun: atomic_swap_builtin,
            },
            // String helpers
            BuiltinFn {
                name: "str_slice",
                arity: Some(3),
                fun: str_slice_builtin,
            },
            BuiltinFn {
                name: "str_index_of",
                arity: Some(2),
                fun: str_index_of_builtin,
            },
            BuiltinFn {
                name: "str_replace_all",
                arity: Some(3),
                fun: str_replace_all_builtin,
            },
            BuiltinFn {
                name: "hmac_sha256",
                arity: Some(2),
                fun: hmac_sha256_builtin,
            },
            BuiltinFn {
                name: "str_ends_with",
                arity: Some(2),
                fun: str_ends_with_builtin,
            },
            BuiltinFn {
                name: "str_lower",
                arity: Some(1),
                fun: str_lower_builtin,
            },
            BuiltinFn {
                name: "str_upper",
                arity: Some(1),
                fun: str_upper_builtin,
            },
            BuiltinFn {
                name: "strfromcharcode",
                arity: Some(1),
                fun: strfromcharcode_builtin,
            },
            BuiltinFn {
                name: "strcharcodeat",
                arity: Some(2),
                fun: strcharcodeat_builtin,
            },
            // Fundamental sequence/collection builtins
            BuiltinFn {
                name: "range",
                arity: None,
                fun: range_builtin,
            },
            BuiltinFn {
                name: "enumerate",
                arity: Some(1),
                fun: enumerate_builtin,
            },
            BuiltinFn {
                name: "sum",
                arity: Some(1),
                fun: sum_builtin,
            },
            BuiltinFn {
                name: "min_of",
                arity: Some(1),
                fun: min_of_builtin,
            },
            BuiltinFn {
                name: "max_of",
                arity: Some(1),
                fun: max_of_builtin,
            },
            BuiltinFn {
                name: "product",
                arity: Some(1),
                fun: product_builtin,
            },
            BuiltinFn {
                name: "clamp",
                arity: Some(3),
                fun: clamp_builtin,
            },
            // Type-checking predicates
            BuiltinFn {
                name: "is_number",
                arity: Some(1),
                fun: is_number_builtin,
            },
            BuiltinFn {
                name: "is_string",
                arity: Some(1),
                fun: is_string_builtin,
            },
            BuiltinFn {
                name: "is_array",
                arity: Some(1),
                fun: is_array_builtin,
            },
            BuiltinFn {
                name: "is_map",
                arity: Some(1),
                fun: is_map_builtin,
            },
            BuiltinFn {
                name: "is_bool",
                arity: Some(1),
                fun: is_bool_builtin,
            },
            BuiltinFn {
                name: "is_fn",
                arity: Some(1),
                fun: is_fn_builtin,
            },
            BuiltinFn {
                name: "is_nil",
                arity: Some(1),
                fun: is_nil_builtin,
            },
            // String helpers
            BuiltinFn {
                name: "str_repeat",
                arity: Some(2),
                fun: str_repeat_builtin,
            },
            BuiltinFn {
                name: "str_pad_left",
                arity: Some(3),
                fun: str_pad_left_builtin,
            },
            BuiltinFn {
                name: "str_pad_right",
                arity: Some(3),
                fun: str_pad_right_builtin,
            },
            // New array helpers
            BuiltinFn {
                name: "array_unique",
                arity: Some(1),
                fun: array_unique_builtin,
            },
            BuiltinFn {
                name: "array_zip",
                arity: Some(2),
                fun: array_zip_builtin,
            },
            BuiltinFn {
                name: "array_chunk",
                arity: Some(2),
                fun: array_chunk_builtin,
            },
            BuiltinFn {
                name: "array_flatten",
                arity: Some(1),
                fun: array_flatten_builtin,
            },
            BuiltinFn {
                name: "array_take",
                arity: Some(2),
                fun: array_take_builtin,
            },
            BuiltinFn {
                name: "array_drop",
                arity: Some(2),
                fun: array_drop_builtin,
            },
            BuiltinFn {
                name: "array_sort",
                arity: Some(1),
                fun: array_sort_builtin,
            },
            // Regex builtins
            BuiltinFn {
                name: "regex_match",
                arity: Some(2),
                fun: regex_match_builtin,
            },
            BuiltinFn {
                name: "regex_find",
                arity: Some(2),
                fun: regex_find_builtin,
            },
            BuiltinFn {
                name: "regex_find_all",
                arity: Some(2),
                fun: regex_find_all_builtin,
            },
            BuiltinFn {
                name: "regex_replace",
                arity: Some(3),
                fun: regex_replace_builtin,
            },
            BuiltinFn {
                name: "regex_replace_all",
                arity: Some(3),
                fun: regex_replace_all_builtin,
            },
            BuiltinFn {
                name: "regex_split",
                arity: Some(2),
                fun: regex_split_builtin,
            },
            BuiltinFn {
                name: "regex_capture",
                arity: Some(2),
                fun: regex_capture_builtin,
            },
            // String formatting
            BuiltinFn {
                name: "str_format",
                arity: None,
                fun: str_format_builtin,
            },
            BuiltinFn {
                name: "num_abs",
                arity: Some(1),
                fun: num_abs_builtin,
            },
            BuiltinFn {
                name: "num_max",
                arity: Some(2),
                fun: num_max_builtin,
            },
            BuiltinFn {
                name: "num_min",
                arity: Some(2),
                fun: num_min_builtin,
            },
            // OS builtins
            BuiltinFn {
                name: "os_args",
                arity: Some(0),
                fun: os_args_builtin,
            },
            BuiltinFn {
                name: "os_exit",
                arity: Some(1),
                fun: os_exit_builtin,
            },
            BuiltinFn {
                name: "os_signal_trap",
                arity: Some(2),
                fun: os_signal_trap_builtin,
            },
            BuiltinFn {
                name: "os_which",
                arity: Some(1),
                fun: os_which_builtin,
            },
            // str_from_char_code alias for strfromcharcode (used in stdlib)
            BuiltinFn {
                name: "str_from_char_code",
                arity: Some(1),
                fun: strfromcharcode_builtin,
            },
            // WebSocket / HTTP / SSE builtins
            BuiltinFn {
                name: "websocket_accept_key",
                arity: Some(1),
                fun: websocket_accept_key_builtin,
            },
            BuiltinFn {
                name: "http_post_auth",
                arity: Some(3),
                fun: http_post_auth_builtin,
            },
            BuiltinFn {
                name: "http_get_auth",
                arity: Some(2),
                fun: http_get_auth_builtin,
            },
            BuiltinFn {
                name: "parse_sse_line",
                arity: Some(1),
                fun: parse_sse_line_builtin,
            },
            BuiltinFn {
                name: "websocket_parse_frame",
                arity: Some(1),
                fun: ws_parse_frame_builtin,
            },
            BuiltinFn {
                name: "websocket_write_frame",
                arity: Some(3),
                fun: ws_write_frame_builtin,
            },
        ];
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
        for b in VM::builtins() {
            if config.map(|_| true).unwrap_or(false) {
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
                let closed_val = self.stack[match *uv.borrow() {
                    crate::Upvalue::Open(idx) => idx,
                    _ => unreachable!(),
                }]
                .clone();
                *uv.borrow_mut() = crate::Upvalue::Closed(closed_val);
            } else {
                i += 1;
            }
        }
    }

    fn upvalue_get(&self, uv: &std::rc::Rc<std::cell::RefCell<crate::Upvalue>>) -> Value {
        match &*uv.borrow() {
            crate::Upvalue::Open(i) => self.stack[*i].clone(),
            crate::Upvalue::Closed(v) => v.clone(),
        }
    }

    fn upvalue_set(&mut self, uv: &std::rc::Rc<std::cell::RefCell<crate::Upvalue>>, v: Value) {
        match &mut *uv.borrow_mut() {
            crate::Upvalue::Open(i) => self.stack[*i] = v,
            crate::Upvalue::Closed(slot) => *slot = v,
        }
    }

    fn new_with(
        module: Rc<RefCell<Module>>,
        builtins: Rc<RefCell<Module>>,
        module_cache: Rc<RefCell<HashMap<String, Rc<RefCell<Module>>>>>,
        import_stack: Rc<RefCell<Vec<String>>>,
        stdlib_config: Option<StdlibConfig>,
        limits: Option<VmLimits>,
        ffi_policy: Option<FfiPolicy>,
    ) -> Self {
        Self {
            stack: vec![],
            module,
            builtins,
            module_cache,
            import_stack,
            frames: vec![],
            open_upvalues: vec![],
            handlers: vec![],
            stdlib_config,
            limits,
            ffi_policy,
            memory_tracker: MemoryTracker::new(),
            thread_pool: None,
            handle_registry: VmHandleRegistry::new(),
        }
    }

    /// Set resource limits (instruction count, stack depth). Used for sandboxing untrusted code.
    pub fn set_limits(&mut self, limits: Option<VmLimits>) {
        self.limits = limits;
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
            globals: HashMap::new(),
            parent: Some(builtins.clone()),
            exports: HashMap::new(),
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
            stdlib_config: None,
            limits: None,
            ffi_policy: None,
            memory_tracker: MemoryTracker::new(),
            thread_pool: None,
            handle_registry: VmHandleRegistry::new(),
        }
    }

    /// Creates a VM with stdlib builtins enabled according to `config`. Use this for scripting and CLI.
    pub fn with_stdlib(config: StdlibConfig) -> Self {
        let builtins = Rc::new(RefCell::new(Module::default()));
        gc_register_module(&builtins);
        VM::install_builtins_into(&builtins, Some(&config));
        MODULE_ALLOCS.fetch_add(1, Ordering::Relaxed);
        let module = Rc::new(RefCell::new(Module {
            globals: HashMap::new(),
            parent: Some(builtins.clone()),
            exports: HashMap::new(),
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
            stdlib_config: Some(config),
            memory_tracker: MemoryTracker::new(),
            thread_pool: None,
            handle_registry: VmHandleRegistry::new(),
            limits: None,
            ffi_policy: None,
        }
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

            let op = {
                let frame = match self.frames.last_mut() {
                    Some(f) => f,
                    None => return Err(self.err("no current frame")),
                };
                let code = &frame.closure.function.chunk.instructions;

                if frame.ip >= code.len() {
                    return Err(self.err("instruction pointer out of bounds"));
                }

                //let op = code[frame.ip];
                let op = code[frame.ip].clone();
                frame.ip += 1;
                op
            };

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
                    OpCode::OpAdd => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        let v = a.concat(b).map_err(|m| self.err(m))?;
                        self.stack.push(v);
                    }
                    OpCode::OpSub => {
                        let b = self.pop_number()?;
                        let a = self.pop_number()?;
                        self.stack.push(Value::Number(a - b));
                    }
                    OpCode::OpMul => {
                        let b = self.pop_number()?;
                        let a = self.pop_number()?;
                        self.stack.push(Value::Number(a * b));
                    }
                    OpCode::OpDiv => {
                        let b = self.pop_number()?;
                        let a = self.pop_number()?;
                        self.stack.push(Value::Number(a / b));
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
                            match &chunk.constants[name_idx] {
                                Value::Obj(o) => match o.as_ref() {
                                    pblang_ir::Obj::String(s) => s.clone(),
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
                    OpCode::OpEq => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        self.stack.push(Value::Bool(a == b));
                    }
                    OpCode::OpNotEq => {
                        let b = self.pop()?;
                        let a = self.pop()?;
                        self.stack.push(Value::Bool(a != b));
                    }
                    OpCode::OpGt => {
                        let b = self.pop_number()?;
                        let a = self.pop_number()?;
                        self.stack.push(Value::Bool(a > b));
                    }
                    OpCode::OpGe => {
                        let b = self.pop_number()?;
                        let a = self.pop_number()?;
                        self.stack.push(Value::Bool(a >= b));
                    }
                    OpCode::OpLt => {
                        let b = self.pop_number()?;
                        let a = self.pop_number()?;
                        self.stack.push(Value::Bool(a < b));
                    }
                    OpCode::OpLe => {
                        let b = self.pop_number()?;
                        let a = self.pop_number()?;
                        self.stack.push(Value::Bool(a <= b));
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
                        let v = Self::get_global_from(&frame.closure.module, &name)
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
                            .map_err(|m| self.err(m))?
                            .to_string();

                        let frame = self
                            .frames
                            .last()
                            .ok_or_else(|| self.err("no current frame"))?;
                        if !Self::set_global_in_chain(&frame.closure.module, &name, v) {
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
                            .map_err(|m| self.err(m))?
                            .to_string();

                        match obj {
                            Value::Module(m) => {
                                let v =
                                    m.borrow().exports.get(&attr).cloned().ok_or_else(|| {
                                        self.err(format!("undefined export '{attr}'"))
                                    })?;
                                self.stack.push(v);
                            }
                            Value::Map(m) => {
                                let v =
                                    m.borrow().get(&attr).cloned().ok_or_else(|| {
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
                            _ => return Err(self.err("indexing only supported on arrays/maps")),
                        }
                    }

                    OpCode::OpSetIndex => {
                        let value = self.pop()?;
                        let index = self.pop()?;
                        let obj = self.pop()?;
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
                if let Some(h) = self.handlers.pop() {
                    // Unwind frames above handler frame (avoid unwrap: pop only when len > h.frame_index+1)
                    while self.frames.len() > h.frame_index + 1 {
                        if let Some(frame) = self.frames.pop() {
                            self.close_upvalues_from(frame.stack_base + 1);
                        } else {
                            break;
                        }
                    }

                    // Restore stack
                    self.close_upvalues_from(h.stack_len);
                    if self.stack.len() > h.stack_len {
                        self.stack.truncate(h.stack_len);
                    }

                    // Jump to handler
                    if let Some(frame) = self.frames.get_mut(h.frame_index) {
                        frame.ip = h.target_ip;
                    } else {
                        return Err(e);
                    }

                    // Provide the error as a value (message string)
                    self.stack.push(alloc_string(e.message.clone()));
                    continue;
                } else {
                    return Err(e);
                }
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

    fn pop_number(&mut self) -> Result<f64, RuntimeError> {
        match self.pop()? {
            Value::Number(n) => Ok(n),
            _ => Err(self.err("type error: expected number")),
        }
    }

    fn current_chunk(&self) -> Result<&Chunk, RuntimeError> {
        let frame = self
            .frames
            .last()
            .ok_or_else(|| self.err("no current frame"))?;
        Ok(&frame.closure.function.chunk)
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
        });

        Ok(())
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

fn borrow_type_from_code(host: &dyn VmHost, code: f64, what: &str) -> Result<BorrowType, RuntimeError> {
    match code as i64 {
        0 => Ok(BorrowType::Immutable),
        1 => Ok(BorrowType::Mutable),
        2 => Ok(BorrowType::Unique),
        _ => Err(host.err(format!("{what}: borrow type must be 0, 1, or 2"))),
    }
}

impl VM {
    fn mem_allocate_impl(&mut self, value: Value, is_mutable: bool) -> Result<f64, RuntimeError> {
        let id = self.memory_tracker.allocate(value, is_mutable);
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
        self.memory_tracker
            .set(mid, value)
            .map_err(|e| memory_safety_err(self, e))
    }

    fn mem_deallocate_impl(&mut self, handle: f64) -> Result<(), RuntimeError> {
        let mid = memory_handle_from_f64(self, handle, "deallocate")?;
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

    fn mem_release_borrow_type(&mut self, handle: f64, borrow_type: u8) -> Result<(), RuntimeError> {
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
                globals: HashMap::new(),
                parent: Some(self.builtins.clone()),
                exports: HashMap::new(),
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

            let mut vm = VM::new_with(
                module_rc.clone(),
                self.builtins.clone(),
                self.module_cache.clone(),
                self.import_stack.clone(),
                self.stdlib_config.clone(),
                self.limits.clone(),
                self.ffi_policy.clone(),
            );
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
