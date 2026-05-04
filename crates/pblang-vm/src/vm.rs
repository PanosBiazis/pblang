use pblang_ir::chunk::{Chunk, OpCode, Span};
use pblang_ir::{
    alloc_string, string_alloc_count, BuiltinFn, Closure, Function, Module, Obj, Upvalue, Value,
    VmHost, VmLimits as IrVmLimits,
};
pub use pblang_ir::{FrameTrace, RuntimeError};

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use std::{
    io::Read,
    io::Write,
    net::{TcpListener, TcpStream},
};

use libloading::Library;
use rand::Rng; // for random_bytes builtin
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
thread_local! {
    static GC_ARRAYS: RefCell<HashMap<usize, Weak<RefCell<Vec<Value>>>>> = RefCell::new(HashMap::new());
    static GC_MAPS: RefCell<HashMap<usize, Weak<RefCell<HashMap<String, Value>>>>> = RefCell::new(HashMap::new());
    static GC_MODULES: RefCell<HashMap<usize, Weak<RefCell<Module>>>> = RefCell::new(HashMap::new());
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
    Stream(NetStream),
}

#[derive(Debug)]
struct NetStream {
    stream: TcpStream,
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

// Spawn/join: handle table and next id for VM-per-thread spawn.
// We send Result<String, String>: Ok = JSON of return Value (Value is !Send), Err = error message.
static SPAWN_TABLE: OnceLock<Mutex<HashMap<usize, mpsc::Receiver<Result<String, String>>>>> =
    OnceLock::new();
static SPAWN_NEXT_ID: AtomicUsize = AtomicUsize::new(1);

fn spawn_table() -> &'static Mutex<HashMap<usize, mpsc::Receiver<Result<String, String>>>> {
    SPAWN_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

// Channel infrastructure for cross-thread channel builtins.
#[derive(Clone)]
struct ChanBuf {
    buf: Arc<Mutex<std::collections::VecDeque<String>>>,
    closed: Arc<AtomicBool>,
}

static CHAN_TABLE: OnceLock<Mutex<HashMap<usize, ChanBuf>>> = OnceLock::new();
static CHAN_NEXT_ID: AtomicUsize = AtomicUsize::new(1);

fn chan_table() -> &'static Mutex<HashMap<usize, ChanBuf>> {
    CHAN_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
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
    /// Used for optional "strict" registration (only expose allowed builtins); currently we register all and check at call time.
    #[allow(dead_code)]
    fn allows(&self, name: &str) -> bool {
        match name {
            "clock" | "type" | "str" | "num_sqrt" | "num_pow" | "num_ln" | "num_exp"
            | "num_atan2" | "num_mod" | "num_floor" | "num_ceil" | "parse_number"
            | "parse_bool" | "read_line" | "len" | "array_push" | "array_pop" | "map_keys"
            | "map_has" | "map_remove" | "import" | "gc_stats" | "gc_collect" | "str_contains"
            | "str_starts_with" | "str_trim" | "str_split" => self.core,
            "fs_read" | "fs_write" | "fs_append" | "fs_remove" | "fs_rename" | "fs_copy"
            | "fs_read_bytes" | "fs_write_bytes" | "fs_exists" | "fs_mkdirs" | "fs_listdir"
            | "path_join" | "path_dirname" | "path_basename" | "cwd" => self.fs,
            "env_get" | "env_set" | "sleep_ms" | "exec" => self.exec,
            "sha256" | "random_bytes" | "constant_time_eq" | "base64_encode" | "base64_decode" => {
                self.crypto
            }
            "json_parse" | "json_stringify" => self.json,
            "http_get" | "http_post" => self.http,
            "tcp_listen" | "tcp_accept" | "tcp_read" | "tcp_read_until" | "tcp_write"
            | "tcp_close" | "tcp_local_addr" | "http_parse_request" | "http_response"
            | "http_response_ct" | "http_response_json" | "onnx_infer" => self.net,
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

/// FFI policy for sandboxing: allowlist libraries and/or symbols. Forbid by default when set.
#[derive(Debug, Clone)]
pub struct FfiPolicy {
    /// If true, allow any library path and any symbol (no allowlist check).
    pub allow_all: bool,
    /// Allowed library paths (e.g. "./safe.dll"). Checked on ffi_load when allow_all is false.
    pub allowed_libs: HashSet<String>,
    /// Allowed symbol names (e.g. "add", "multiply"). Checked on ffi_call when allow_all is false.
    pub allowed_symbols: HashSet<String>,
}

impl Default for FfiPolicy {
    fn default() -> Self {
        Self {
            allow_all: true,
            allowed_libs: HashSet::new(),
            allowed_symbols: HashSet::new(),
        }
    }
}

/// Resource limits for sandboxing (anti-infinite-loop, stack overflow, IO, memory).
#[derive(Debug, Clone)]
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

impl Default for VmLimits {
    fn default() -> Self {
        Self {
            max_instructions: None,
            max_stack_depth: None,
            max_memory_bytes: None,
            io_timeout_ms: None,
        }
    }
}

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
            keys.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
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
            if !host.allow_exec() {
                return Err(host.err("exec/env access denied".into()));
            }
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

        fn env_set(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_exec() {
                return Err(host.err("exec/env access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!("env_set() expects 2 arguments, got {}", args.len())));
            }
            let k = args[0]
                .as_str()
                .map_err(|m| host.err(format!("env_set(): {m}")))?;
            let v = args[1]
                .as_str()
                .map_err(|m| host.err(format!("env_set(): {m}")))?;
            std::env::set_var(k, v);
            Ok(Value::Nil)
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
            if args.len() != 1 {
                return Err(host.err(format!("exec() expects 1 argument, got {}", args.len())));
            }
            let cmd = args[0]
                .as_str()
                .map_err(|m| host.err(format!("exec(): {m}")))?;

            #[cfg(windows)]
            let status = std::process::Command::new("cmd").args(["/C", cmd]).status();

            #[cfg(not(windows))]
            let status = std::process::Command::new("sh").args(["-c", cmd]).status();

            let status = status.map_err(|e| host.err(format!("exec(): {e}")))?;
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

        fn gc_collect(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !args.is_empty() {
                return Err(host.err(format!(
                    "gc_collect() takes no arguments, got {}",
                    args.len()
                )));
            }

            // Force-close native handle tables (tcp/ffi/sqlite) by dropping all entries,
            // then run a tracing / cycle collection pass to break RC cycles in containers.
            let net_closed = with_net_table(host, |t| {
                let n = t.len();
                t.clear();
                n
            })?;
            let ffi_closed = with_ffi_table(host, |t| {
                let n = t.len();
                t.clear();
                n
            })?;
            let db_closed = with_db_table(host, |t| {
                let n = t.len();
                t.clear();
                n
            })?;

            let (arrays_cleared, maps_cleared, modules_cleared) = host.gc_collect_cycles();

            Ok(alloc_string(format!(
                "gc_collect: closed net={} ffi={} db={} cycles arrays={} maps={} modules={}",
                net_closed, ffi_closed, db_closed, arrays_cleared, maps_cleared, modules_cleared
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

        fn http_post(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_http() {
                return Err(host.err("http access denied".into()));
            }
            if args.len() != 2 {
                return Err(host.err(format!(
                    "http_post() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let url = args[0]
                .as_str()
                .map_err(|m| host.err(format!("http_post(): {m}")))?;
            let body = args[1]
                .as_str()
                .map_err(|m| host.err(format!("http_post(): {m}")))?;
            let req = ureq::post(url).set("content-type", "application/json");
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
            names.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
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
            let listener = match r {
                NetResource::Listener(l) => l,
                other => {
                    let _ = with_net_table(host, |t| t.insert(id, other));
                    return Err(host.err("tcp_accept(): handle is not a listener".into()));
                }
            };

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
                        stream: s,
                        buf: vec![],
                    }),
                );
                sid
            })?;
            Ok(Value::Number(sid as f64))
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
                    .stream
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
                    .stream
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
                .stream
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
                NetResource::Stream(s) => {
                    let addr = s
                        .stream
                        .local_addr()
                        .map_err(|e| host.err(format!("tcp_local_addr(): {e}")))?;
                    let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(s)));
                    Ok(alloc_string(addr.to_string()))
                }
            }
        }

        fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
            if needle.is_empty() {
                return Some(0);
            }
            hay.windows(needle.len()).position(|w| w == needle)
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
            let reason = match status {
                200 => "OK",
                400 => "Bad Request",
                404 => "Not Found",
                500 => "Internal Server Error",
                _ => "OK",
            };
            let resp = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.as_bytes().len(),
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
            let reason = match status {
                200 => "OK",
                400 => "Bad Request",
                404 => "Not Found",
                500 => "Internal Server Error",
                _ => "OK",
            };
            let resp = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.as_bytes().len(),
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
            let reason = match status {
                200 => "OK",
                400 => "Bad Request",
                404 => "Not Found",
                500 => "Internal Server Error",
                _ => "OK",
            };
            let resp = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.as_bytes().len(),
                body
            );
            Ok(alloc_string(resp))
        }

        fn onnx_infer(host: &mut dyn VmHost, _args: Vec<Value>) -> Result<Value, RuntimeError> {
            if !host.allow_net() {
                return Err(host.err("net access denied".into()));
            }
            Err(host.err(
                "onnx_infer() not built-in yet; use host embedding via pblang-ffi for now".into(),
            ))
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
                    let out = conn
                        .execute(sql, rusqlite::params_from_iter(tosql.iter()))
                        .map_err(|e| host.err(format!("sqlite_exec(): {e}")))?;
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
                            for i in 0..col_count {
                                let name = col_names[i].clone();
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
                        return Err(
                            host.err("spawn: open upvalue cannot be transferred".into()),
                        )
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

            let id = SPAWN_NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = mpsc::channel();
            spawn_table()
                .lock()
                .map_err(|_| host.err("internal: spawn table unavailable".into()))?
                .insert(id, rx);

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
                                Rc::new(RefCell::new(Upvalue::Closed(
                                    spawn_value_from_json(j),
                                )))
                            })
                            .collect(),
                        Err(e) => {
                            let _ = tx.send(Err(e.to_string()));
                            return;
                        }
                    },
                    module,
                });
                let wrapper = Chunk {
                    instructions: vec![OpCode::OpConstant(0), OpCode::OpCall(0), OpCode::OpReturn],
                    constants: vec![Value::Closure(child_closure)],
                    spans: vec![Span::default(), Span::default(), Span::default()],
                    source_name: None,
                    source: None,
                };
                let mut vm = VM::with_stdlib(config);
                if let Some(lim) = spawn_limits {
                    vm.set_limits(Some(VmLimits {
                        max_instructions: lim.max_instructions,
                        max_stack_depth: lim.max_stack_depth,
                        max_memory_bytes: lim.max_memory_bytes,
                        io_timeout_ms: lim.io_timeout_ms,
                    }));
                }
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
            if args.len() != 1 {
                return Err(host.err(format!("join() expects 1 argument, got {}", args.len())));
            }
            let id = value_to_handle(host, args[0].clone(), "join")?;
            let rx = {
                let mut t = spawn_table()
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
            let id = CHAN_NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let cb = ChanBuf {
                buf: Arc::new(Mutex::new(std::collections::VecDeque::new())),
                closed: Arc::new(AtomicBool::new(false)),
            };
            chan_table()
                .lock()
                .map_err(|_| host.err("channel: table unavailable".into()))?
                .insert(id, cb);
            let mut m = HashMap::new();
            m.insert("sender".into(), Value::Number(id as f64));
            m.insert("receiver".into(), Value::Number(id as f64));
            Ok(alloc_map(m))
        }

        fn channel_send_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 2 {
                return Err(host.err(format!(
                    "send() expects 2 arguments, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "send")?;
            let json = spawn_value_to_json(&args[1]).map_err(|e| host.err(e))?;
            let json_str = sj::to_string(&json).map_err(|e| host.err(e.to_string()))?;
            let cb = {
                let t = chan_table()
                    .lock()
                    .map_err(|_| host.err("send: table unavailable".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("send: invalid channel id".into()))?
            };
            if cb.closed.load(Ordering::SeqCst) {
                return Err(host.err("send: channel is closed".into()));
            }
            cb.buf
                .lock()
                .map_err(|_| host.err("send: buf lock unavailable".into()))?
                .push_back(json_str);
            Ok(Value::Nil)
        }

        fn channel_recv_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "recv() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "recv")?;
            let cb = {
                let t = chan_table()
                    .lock()
                    .map_err(|_| host.err("recv: table unavailable".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("recv: invalid channel id".into()))?
            };
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            loop {
                {
                    let mut buf = cb
                        .buf
                        .lock()
                        .map_err(|_| host.err("recv: buf lock unavailable".into()))?;
                    if let Some(json_str) = buf.pop_front() {
                        let j: sj::Value =
                            sj::from_str(&json_str).map_err(|e| host.err(e.to_string()))?;
                        return Ok(spawn_value_from_json(&j));
                    }
                }
                if cb.closed.load(Ordering::SeqCst) {
                    return Ok(Value::Nil);
                }
                if std::time::Instant::now() >= deadline {
                    return Err(host.err("recv: timeout".into()));
                }
                thread::sleep(Duration::from_millis(1));
            }
        }

        fn channel_close_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "channel_close() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "channel_close")?;
            let cb = {
                let t = chan_table()
                    .lock()
                    .map_err(|_| host.err("channel_close: table unavailable".into()))?;
                t.get(&id)
                    .cloned()
                    .ok_or_else(|| host.err("channel_close: invalid channel id".into()))?
            };
            cb.closed.store(true, Ordering::SeqCst);
            Ok(Value::Nil)
        }

        fn channel_is_closed_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "channel_is_closed() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let id = value_to_handle(host, args[0].clone(), "channel_is_closed")?;
            let cb = {
                let t = chan_table()
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
            if args.len() != 1 {
                return Err(host.err(format!(
                    "select() expects 1 argument, got {}",
                    args.len()
                )));
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
                _ => {
                    return Err(host.err("select(): expected array of channel ids".into()))
                }
            };
            // Snapshot ChanBuf handles upfront (ChanBuf is Clone + Arc-backed, cheap).
            let cbs: Vec<Option<ChanBuf>> = {
                let t = chan_table()
                    .lock()
                    .map_err(|_| host.err("select: table unavailable".into()))?;
                ids.iter().map(|id| t.get(id).cloned()).collect()
            };
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                for (i, cb_opt) in cbs.iter().enumerate() {
                    if let Some(cb) = cb_opt {
                        if let Ok(buf) = cb.buf.try_lock() {
                            if !buf.is_empty() {
                                return Ok(Value::Number(i as f64));
                            }
                        }
                    }
                }
                if std::time::Instant::now() >= deadline {
                    return Err(host.err("select: timeout".into()));
                }
                thread::sleep(Duration::from_millis(1));
            }
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
                    return Err(
                        host.err("str_slice(): start must be a non-negative number".into()),
                    )
                }
            };
            let end = match args[2] {
                Value::Number(n) if n >= 0.0 => n as usize,
                _ => {
                    return Err(
                        host.err("str_slice(): end must be a non-negative number".into()),
                    )
                }
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

        // ---- OS builtins ----

        fn os_args_builtin(
            _host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let argv: Vec<Value> =
                std::env::args().skip(1).map(|a| alloc_string(a)).collect();
            Ok(alloc_array(argv))
        }

        fn os_exit_builtin(
            host: &mut dyn VmHost,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            if args.len() != 1 {
                return Err(host.err(format!(
                    "os_exit() expects 1 argument, got {}",
                    args.len()
                )));
            }
            let code = match args[0] {
                Value::Number(n) => n as i32,
                _ => return Err(host.err("os_exit(): expected number".into())),
            };
            std::process::exit(code);
        }

        fn os_signal_trap_builtin(
            _host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            Ok(alloc_string("stub"))
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
            let encoded = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                hash.as_slice(),
            );
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
            let (field, value) = if line.starts_with("data: ") {
                ("data", &line[6..])
            } else if line.starts_with("event: ") {
                ("event", &line[7..])
            } else if line.starts_with("id: ") {
                ("id", &line[4..])
            } else if line.starts_with("retry: ") {
                ("retry", &line[7..])
            } else {
                ("", line)
            };
            let mut m = HashMap::new();
            m.insert("field".into(), alloc_string(field));
            m.insert("value".into(), alloc_string(value));
            Ok(alloc_map(m))
        }

        // ---- WebSocket frame stubs ----

        fn ws_parse_frame_builtin(
            _host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            let mut m = HashMap::new();
            m.insert("opcode".into(), Value::Number(1.0));
            m.insert("payload".into(), alloc_string(""));
            m.insert("fin".into(), Value::Bool(true));
            m.insert("masked".into(), Value::Bool(false));
            Ok(alloc_map(m))
        }

        fn ws_write_frame_builtin(
            _host: &mut dyn VmHost,
            _args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            Ok(alloc_array(vec![]))
        }

        let mut v: Vec<BuiltinFn> = vec![
            BuiltinFn {
                name: "clock",
                arity: Some(0),
                fun: clock,
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
                        let v = a.add(b).map_err(|m| self.err(m))?;
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
                            .map_err(|m| self.err(m))?
                            .to_string();

                        match obj {
                            Value::Map(m) => {
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

        if argc != closure_rc.function.arity {
            return Err(self.err(format!(
                "arity mismatch: expected {}, got {}",
                closure_rc.function.arity, argc
            )));
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

            let frame = self.frames.last_mut().unwrap();

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
                self.frames.last_mut().unwrap().is_defer_thunk = true;
                return Ok(());
            }

            // No more defers for this frame. Pop it.
            let frame = self.frames.pop().unwrap();

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

impl VmHost for VM {
    fn err(&self, msg: String) -> RuntimeError {
        self.make_err(msg)
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
            let mut chunk = pblang_compiler::compile(stmts);
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
