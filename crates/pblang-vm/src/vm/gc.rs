//! Automatic & manual cycle-collecting garbage collector for the pblang VM.
//!
//! pblang values are reference-counted (`Rc`); reference cycles across arrays,
//! maps, modules, closures, and closed upvalues cannot be freed by `Rc` alone.
//! Every cycle-capable heap allocation (`alloc_array`, `alloc_map`, `Module`,
//! `Upvalue`, and `Closure`) registers a `Weak` pointer in thread-local
//! registries and bumps an allocation counter.
//!
//! Collection runs both:
//! - **Automatically** at safe instruction boundaries in `VM::run_inner` once
//!   allocations since the last pass reach an adaptive watermark
//!   (`max(8192, live_objects * 2)`), and
//! - **Explicitly** when user/host code invokes `gc_collect()`.
//!
//! Safety with nested VMs (`import_module` on the same thread) and temporary
//! host `Rc` clones is guaranteed via **trial deletion** (`Rc::strong_count`
//! minus internal references among tracked objects) combined with explicit
//! tracing from `self` roots (`stack`, `frames`, `defers`, `module_result`,
//! `open_upvalues`, `unwinds`, `module`, `builtins`, `module_cache`, and
//! `signal_handlers`). Only objects with zero external strong references AND
//! unreachable from all roots are swept, breaking cycles without ever touching
//! live state.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use pblang_ir::{Closure, Module, Upvalue, Value};

use super::VM;

const INITIAL_GC_THRESHOLD: usize = 8_192;

const TAG_ARRAY: u8 = 1;
const TAG_MAP: u8 = 2;
const TAG_MODULE: u8 = 3;
const TAG_UPVALUE: u8 = 4;
const TAG_CLOSURE: u8 = 5;

type GcArrayRegistry = HashMap<usize, Weak<RefCell<Vec<Value>>>>;
type GcMapRegistry = HashMap<usize, Weak<RefCell<HashMap<String, Value>>>>;
type GcModuleRegistry = HashMap<usize, Weak<RefCell<Module>>>;
type GcUpvalueRegistry = HashMap<usize, Weak<RefCell<Upvalue>>>;
type GcClosureRegistry = HashMap<usize, Weak<Closure>>;

thread_local! {
    static GC_ARRAYS: RefCell<GcArrayRegistry> = RefCell::new(HashMap::new());
    static GC_MAPS: RefCell<GcMapRegistry> = RefCell::new(HashMap::new());
    static GC_MODULES: RefCell<GcModuleRegistry> = RefCell::new(HashMap::new());
    static GC_UPVALUES: RefCell<GcUpvalueRegistry> = RefCell::new(HashMap::new());
    static GC_CLOSURES: RefCell<GcClosureRegistry> = RefCell::new(HashMap::new());

    static GC_ALLOCS_SINCE_COLLECT: Cell<usize> = const { Cell::new(0) };
    static GC_NEXT_THRESHOLD: Cell<usize> = const { Cell::new(INITIAL_GC_THRESHOLD) };
    static GC_COLLECTIONS: Cell<usize> = const { Cell::new(0) };
}

#[inline]
fn bump_gc_alloc() {
    GC_ALLOCS_SINCE_COLLECT.with(|c| {
        let next = c.get().saturating_add(1);
        c.set(next);
        // If host Rust code allocates heavily outside the VM bytecode loop,
        // prune dead Weak entries periodically so registry tables never bloat.
        let thresh = GC_NEXT_THRESHOLD.with(|t| t.get());
        if next >= thresh.saturating_mul(4) {
            gc_prune_dead_weaks();
            c.set(thresh);
        }
    });
}

#[inline]
pub(super) fn gc_should_collect() -> bool {
    let allocs = GC_ALLOCS_SINCE_COLLECT.with(|c| c.get());
    let thresh = GC_NEXT_THRESHOLD.with(|t| t.get());
    allocs >= thresh
}

pub(super) fn gc_live_tracked_count() -> usize {
    let a = GC_ARRAYS.with(|t| t.borrow().values().filter(|w| w.strong_count() > 0).count());
    let m = GC_MAPS.with(|t| t.borrow().values().filter(|w| w.strong_count() > 0).count());
    let md = GC_MODULES.with(|t| t.borrow().values().filter(|w| w.strong_count() > 0).count());
    let uv = GC_UPVALUES.with(|t| t.borrow().values().filter(|w| w.strong_count() > 0).count());
    let cl = GC_CLOSURES.with(|t| t.borrow().values().filter(|w| w.strong_count() > 0).count());
    a + m + md + uv + cl
}

pub(super) fn gc_collection_count() -> usize {
    GC_COLLECTIONS.with(|c| c.get())
}

fn compact_map<V>(map: &mut HashMap<usize, Weak<V>>) -> usize {
    map.retain(|_, w| w.strong_count() > 0);
    let len = map.len();
    let cap_limit = 1024usize.max(len.saturating_mul(4));
    if map.capacity() > cap_limit {
        map.shrink_to(1024usize.max(len.saturating_mul(2)));
    }
    len
}

pub(super) fn gc_prune_dead_weaks() -> usize {
    let a = GC_ARRAYS.with(|t| compact_map(&mut t.borrow_mut()));
    let m = GC_MAPS.with(|t| compact_map(&mut t.borrow_mut()));
    let md = GC_MODULES.with(|t| compact_map(&mut t.borrow_mut()));
    let uv = GC_UPVALUES.with(|t| compact_map(&mut t.borrow_mut()));
    let cl = GC_CLOSURES.with(|t| compact_map(&mut t.borrow_mut()));
    a + m + md + uv + cl
}

// Always key by `Rc::as_ptr(..)` (never `RefCell::as_ptr`), so registration
// addresses match pointer lookups during root & edge tracing.
pub(super) fn gc_register_array(a: &Rc<RefCell<Vec<Value>>>) {
    let id = Rc::as_ptr(a) as usize;
    GC_ARRAYS.with(|t| {
        t.borrow_mut().insert(id, Rc::downgrade(a));
    });
    bump_gc_alloc();
}

pub(super) fn gc_register_map(m: &Rc<RefCell<HashMap<String, Value>>>) {
    let id = Rc::as_ptr(m) as usize;
    GC_MAPS.with(|t| {
        t.borrow_mut().insert(id, Rc::downgrade(m));
    });
    bump_gc_alloc();
}

pub(super) fn gc_register_module(m: &Rc<RefCell<Module>>) {
    let id = Rc::as_ptr(m) as usize;
    GC_MODULES.with(|t| {
        t.borrow_mut().insert(id, Rc::downgrade(m));
    });
    bump_gc_alloc();
}

pub(super) fn gc_register_upvalue(uv: &Rc<RefCell<Upvalue>>) {
    let id = Rc::as_ptr(uv) as usize;
    GC_UPVALUES.with(|t| {
        t.borrow_mut().insert(id, Rc::downgrade(uv));
    });
    bump_gc_alloc();
}

pub(super) fn gc_register_closure(c: &Rc<Closure>) {
    let id = Rc::as_ptr(c) as usize;
    GC_CLOSURES.with(|t| {
        t.borrow_mut().insert(id, Rc::downgrade(c));
    });
    bump_gc_alloc();
}

enum GcNode {
    Array(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<HashMap<String, Value>>>),
    Module(Rc<RefCell<Module>>),
    Upvalue(Rc<RefCell<Upvalue>>),
    Closure(Rc<Closure>),
}

impl GcNode {
    #[inline]
    fn strong_count(&self) -> usize {
        match self {
            GcNode::Array(rc) => Rc::strong_count(rc),
            GcNode::Map(rc) => Rc::strong_count(rc),
            GcNode::Module(rc) => Rc::strong_count(rc),
            GcNode::Upvalue(rc) => Rc::strong_count(rc),
            GcNode::Closure(rc) => Rc::strong_count(rc),
        }
    }
}

#[inline]
fn push_value_edge(
    v: &Value,
    id_by_key: &HashMap<(u8, usize), usize>,
    out: &mut Vec<usize>,
) {
    match v {
        Value::Array(rc) => {
            if let Some(&id) = id_by_key.get(&(TAG_ARRAY, Rc::as_ptr(rc) as usize)) {
                out.push(id);
            }
        }
        Value::Map(rc) => {
            if let Some(&id) = id_by_key.get(&(TAG_MAP, Rc::as_ptr(rc) as usize)) {
                out.push(id);
            }
        }
        Value::Module(rc) => {
            if let Some(&id) = id_by_key.get(&(TAG_MODULE, Rc::as_ptr(rc) as usize)) {
                out.push(id);
            }
        }
        Value::Closure(rc) => {
            if let Some(&id) = id_by_key.get(&(TAG_CLOSURE, Rc::as_ptr(rc) as usize)) {
                out.push(id);
            }
        }
        Value::Number(_)
        | Value::Bool(_)
        | Value::Nil
        | Value::Obj(_)
        | Value::Function(_)
        | Value::Builtin(_) => {}
    }
}

#[inline]
fn mark_root_id(id: usize, live: &mut [bool], worklist: &mut Vec<usize>) {
    if !live[id] {
        live[id] = true;
        worklist.push(id);
    }
}

fn mark_root_value(
    v: &Value,
    id_by_key: &HashMap<(u8, usize), usize>,
    live: &mut [bool],
    worklist: &mut Vec<usize>,
) {
    match v {
        Value::Array(rc) => {
            if let Some(&id) = id_by_key.get(&(TAG_ARRAY, Rc::as_ptr(rc) as usize)) {
                mark_root_id(id, live, worklist);
            }
        }
        Value::Map(rc) => {
            if let Some(&id) = id_by_key.get(&(TAG_MAP, Rc::as_ptr(rc) as usize)) {
                mark_root_id(id, live, worklist);
            }
        }
        Value::Module(rc) => {
            if let Some(&id) = id_by_key.get(&(TAG_MODULE, Rc::as_ptr(rc) as usize)) {
                mark_root_id(id, live, worklist);
            }
        }
        Value::Closure(rc) => {
            if let Some(&id) = id_by_key.get(&(TAG_CLOSURE, Rc::as_ptr(rc) as usize)) {
                mark_root_id(id, live, worklist);
            } else {
                mark_root_closure_direct(rc, id_by_key, live, worklist);
            }
        }
        Value::Number(_)
        | Value::Bool(_)
        | Value::Nil
        | Value::Obj(_)
        | Value::Function(_)
        | Value::Builtin(_) => {}
    }
}

fn mark_root_closure_direct(
    c: &Rc<Closure>,
    id_by_key: &HashMap<(u8, usize), usize>,
    live: &mut [bool],
    worklist: &mut Vec<usize>,
) {
    if let Some(&cid) = id_by_key.get(&(TAG_CLOSURE, Rc::as_ptr(c) as usize)) {
        mark_root_id(cid, live, worklist);
        return;
    }
    if let Some(&mid) = id_by_key.get(&(TAG_MODULE, Rc::as_ptr(&c.module) as usize)) {
        mark_root_id(mid, live, worklist);
    }
    for uv in &c.upvalues {
        if let Some(&uid) = id_by_key.get(&(TAG_UPVALUE, Rc::as_ptr(uv) as usize)) {
            mark_root_id(uid, live, worklist);
        } else if let Ok(guard) = uv.try_borrow() {
            if let Upvalue::Closed(v) = &*guard {
                mark_root_value(v, id_by_key, live, worklist);
            }
        }
    }
}

impl VM {
    /// Return the number of currently live GC-tracked heap objects on this thread.
    pub fn gc_tracked_live_count(&self) -> usize {
        gc_live_tracked_count()
    }

    /// Return the total number of cycle-GC passes executed on this thread.
    pub fn gc_collection_count(&self) -> usize {
        gc_collection_count()
    }

    /// Run one full trial-deletion + root-tracing cycle collection pass.
    /// Returns `(arrays_cleared, maps_cleared, modules_cleared)`.
    pub(super) fn gc_collect_cycles_impl(&mut self) -> (usize, usize, usize) {
        GC_COLLECTIONS.with(|c| c.set(c.get().saturating_add(1)));

        // 1. Snapshot all live tracked objects across the 5 registries.
        let mut nodes: Vec<GcNode> = Vec::new();
        let mut id_by_key: HashMap<(u8, usize), usize> = HashMap::new();

        GC_ARRAYS.with(|tbl| {
            let map = tbl.borrow();
            nodes.reserve(map.len());
            for (&ptr, w) in map.iter() {
                if let Some(rc) = w.upgrade() {
                    let id = nodes.len();
                    nodes.push(GcNode::Array(rc));
                    id_by_key.insert((TAG_ARRAY, ptr), id);
                }
            }
        });
        GC_MAPS.with(|tbl| {
            let map = tbl.borrow();
            nodes.reserve(map.len());
            for (&ptr, w) in map.iter() {
                if let Some(rc) = w.upgrade() {
                    let id = nodes.len();
                    nodes.push(GcNode::Map(rc));
                    id_by_key.insert((TAG_MAP, ptr), id);
                }
            }
        });
        GC_MODULES.with(|tbl| {
            let map = tbl.borrow();
            nodes.reserve(map.len());
            for (&ptr, w) in map.iter() {
                if let Some(rc) = w.upgrade() {
                    let id = nodes.len();
                    nodes.push(GcNode::Module(rc));
                    id_by_key.insert((TAG_MODULE, ptr), id);
                }
            }
        });
        GC_UPVALUES.with(|tbl| {
            let map = tbl.borrow();
            nodes.reserve(map.len());
            for (&ptr, w) in map.iter() {
                if let Some(rc) = w.upgrade() {
                    let id = nodes.len();
                    nodes.push(GcNode::Upvalue(rc));
                    id_by_key.insert((TAG_UPVALUE, ptr), id);
                }
            }
        });
        GC_CLOSURES.with(|tbl| {
            let map = tbl.borrow();
            nodes.reserve(map.len());
            for (&ptr, w) in map.iter() {
                if let Some(rc) = w.upgrade() {
                    let id = nodes.len();
                    nodes.push(GcNode::Closure(rc));
                    id_by_key.insert((TAG_CLOSURE, ptr), id);
                }
            }
        });

        let n = nodes.len();
        if n == 0 {
            let live_rem = gc_prune_dead_weaks();
            GC_ALLOCS_SINCE_COLLECT.with(|c| c.set(0));
            GC_NEXT_THRESHOLD
                .with(|t| t.set(INITIAL_GC_THRESHOLD.max(live_rem.saturating_mul(2))));
            return (0, 0, 0);
        }

        // 2. Trial deletion: start with each node's real strong count (excluding
        //    our single snapshot upgrade) and subtract internal edges among `nodes`.
        let mut trial_rc: Vec<usize> = nodes
            .iter()
            .map(|node| node.strong_count().saturating_sub(1))
            .collect();
        let mut pinned = vec![false; n];
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];

        for i in 0..n {
            let mut edges = Vec::new();
            match &nodes[i] {
                GcNode::Array(rc) => match rc.try_borrow() {
                    Ok(vec) => {
                        for v in vec.iter() {
                            push_value_edge(v, &id_by_key, &mut edges);
                        }
                    }
                    Err(_) => pinned[i] = true,
                },
                GcNode::Map(rc) => match rc.try_borrow() {
                    Ok(map) => {
                        for v in map.values() {
                            push_value_edge(v, &id_by_key, &mut edges);
                        }
                    }
                    Err(_) => pinned[i] = true,
                },
                GcNode::Module(rc) => match rc.try_borrow() {
                    Ok(m) => {
                        for v in m.globals.values() {
                            push_value_edge(v, &id_by_key, &mut edges);
                        }
                        for v in m.exports.values() {
                            push_value_edge(v, &id_by_key, &mut edges);
                        }
                        if let Some(parent) = &m.parent {
                            if let Some(&pid) =
                                id_by_key.get(&(TAG_MODULE, Rc::as_ptr(parent) as usize))
                            {
                                edges.push(pid);
                            }
                        }
                    }
                    Err(_) => pinned[i] = true,
                },
                GcNode::Upvalue(rc) => match rc.try_borrow() {
                    Ok(uv) => {
                        if let Upvalue::Closed(v) = &*uv {
                            push_value_edge(v, &id_by_key, &mut edges);
                        }
                    }
                    Err(_) => pinned[i] = true,
                },
                GcNode::Closure(rc) => {
                    for uv in &rc.upvalues {
                        if let Some(&uid) =
                            id_by_key.get(&(TAG_UPVALUE, Rc::as_ptr(uv) as usize))
                        {
                            edges.push(uid);
                        }
                    }
                    if let Some(&mid) =
                        id_by_key.get(&(TAG_MODULE, Rc::as_ptr(&rc.module) as usize))
                    {
                        edges.push(mid);
                    }
                }
            }
            for &target in &edges {
                trial_rc[target] = trial_rc[target].saturating_sub(1);
            }
            children[i] = edges;
        }

        // 3. Seed roots: every node with external strong references (`trial_rc > 0`
        //    or actively borrowed) PLUS explicit `self` VM roots.
        let mut live = vec![false; n];
        let mut worklist = Vec::with_capacity(n / 4 + 16);

        for i in 0..n {
            if pinned[i] || trial_rc[i] > 0 {
                mark_root_id(i, &mut live, &mut worklist);
            }
        }

        for v in self.stack.iter() {
            mark_root_value(v, &id_by_key, &mut live, &mut worklist);
        }
        for f in self.frames.iter() {
            mark_root_closure_direct(&f.closure, &id_by_key, &mut live, &mut worklist);
            for deferred in &f.defers {
                mark_root_value(deferred, &id_by_key, &mut live, &mut worklist);
            }
            if let Some(result) = &f.module_result {
                mark_root_value(result, &id_by_key, &mut live, &mut worklist);
            }
        }
        for uv in self.open_upvalues.iter() {
            if let Some(&uid) = id_by_key.get(&(TAG_UPVALUE, Rc::as_ptr(uv) as usize)) {
                mark_root_id(uid, &mut live, &mut worklist);
            } else if let Ok(guard) = uv.try_borrow() {
                if let Upvalue::Closed(v) = &*guard {
                    mark_root_value(v, &id_by_key, &mut live, &mut worklist);
                }
            }
        }
        for unwind in self.unwinds.iter() {
            for deferred in &unwind.pending {
                mark_root_value(deferred, &id_by_key, &mut live, &mut worklist);
            }
        }
        if let Some(&mid) = id_by_key.get(&(TAG_MODULE, Rc::as_ptr(&self.module) as usize)) {
            mark_root_id(mid, &mut live, &mut worklist);
        }
        if let Some(&bid) = id_by_key.get(&(TAG_MODULE, Rc::as_ptr(&self.builtins) as usize)) {
            mark_root_id(bid, &mut live, &mut worklist);
        }
        if let Ok(cache) = self.module_cache.try_borrow() {
            for m_rc in cache.values() {
                if let Some(&mid) = id_by_key.get(&(TAG_MODULE, Rc::as_ptr(m_rc) as usize)) {
                    mark_root_id(mid, &mut live, &mut worklist);
                } else if let Ok(m) = m_rc.try_borrow() {
                    for v in m.globals.values() {
                        mark_root_value(v, &id_by_key, &mut live, &mut worklist);
                    }
                    for v in m.exports.values() {
                        mark_root_value(v, &id_by_key, &mut live, &mut worklist);
                    }
                }
            }
        }
        if let Ok(sigs) = self.signal_handlers.try_borrow() {
            for v in sigs.values() {
                mark_root_value(v, &id_by_key, &mut live, &mut worklist);
            }
        }

        // 4. Transitive reachability mark.
        while let Some(u) = worklist.pop() {
            for &v in &children[u] {
                if !live[v] {
                    live[v] = true;
                    worklist.push(v);
                }
            }
        }

        // 5. Sweep unreachable cyclic containers & closed upvalues while `nodes`
        //    keeps every Rc at strong_count >= 1 (avoiding deep recursive Drop).
        let mut arrays_cleared = 0usize;
        let mut maps_cleared = 0usize;
        let mut modules_cleared = 0usize;

        for i in 0..n {
            if live[i] {
                continue;
            }
            match &nodes[i] {
                GcNode::Array(rc) => {
                    if let Ok(mut vec) = rc.try_borrow_mut() {
                        if !vec.is_empty() {
                            vec.clear();
                            vec.shrink_to_fit();
                            arrays_cleared += 1;
                        }
                    }
                }
                GcNode::Map(rc) => {
                    if let Ok(mut map) = rc.try_borrow_mut() {
                        if !map.is_empty() {
                            map.clear();
                            map.shrink_to_fit();
                            maps_cleared += 1;
                        }
                    }
                }
                GcNode::Module(rc) => {
                    if let Ok(mut m) = rc.try_borrow_mut() {
                        let non_empty =
                            !m.globals.is_empty() || !m.exports.is_empty() || m.parent.is_some();
                        if non_empty {
                            m.globals.clear();
                            m.exports.clear();
                            m.parent = None;
                            modules_cleared += 1;
                        }
                    }
                }
                GcNode::Upvalue(rc) => {
                    if let Ok(mut uv) = rc.try_borrow_mut() {
                        if matches!(*uv, Upvalue::Closed(ref v) if !matches!(v, Value::Nil)) {
                            *uv = Upvalue::Closed(Value::Nil);
                        }
                    }
                }
                GcNode::Closure(_) => {}
            }
        }

        // 6. Drop snapshot references so broken cycles hit strong_count == 0,
        //    then compact all Weak registries and update the adaptive watermark.
        drop(children);
        drop(id_by_key);
        drop(nodes);

        let live_remaining = gc_prune_dead_weaks();
        GC_ALLOCS_SINCE_COLLECT.with(|c| c.set(0));
        GC_NEXT_THRESHOLD
            .with(|t| t.set(INITIAL_GC_THRESHOLD.max(live_remaining.saturating_mul(2))));

        (arrays_cleared, maps_cleared, modules_cleared)
    }
}
