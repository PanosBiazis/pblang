//! Cycle-collecting garbage collector for the pblang VM.
//!
//! pblang values are mostly reference-counted; cycles (`let a = []; a[0] = a;`)
//! would leak under naive RC. We keep weak registries of every container
//! object the VM allocates (`alloc_array` / `alloc_map` / `import_module`
//! register their `Rc` here), then `gc_collect_cycles` runs a tracing pass
//! from VM roots and **clears** the contents of unreachable containers so
//! the RC graph can release them.
//!
//! Roots are: the VM stack, every active call frame's closure, every closed
//! upvalue, frame-held defers/return values, pending unwind closures, the current
//! module, the builtins module, and the module cache.
//!
//! Each cleared container's `Rc` may still be held by some user value; that's
//! fine — clearing its body breaks the cycle, and the `Rc` itself is dropped
//! when the last strong reference goes away.
//
// Visibility note: free functions are `pub(super)` so they're reachable from
// the parent `vm` module (used by `alloc_array`, `alloc_map`, and the
// `import_module` path). The VM methods live in a sibling `impl VM` block;
// inherent methods don't need a visibility modifier to be visible within
// the crate, but `pub(super)` makes intent explicit.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

use pblang_ir::{Closure, Module, Upvalue, Value};

use super::VM;

type GcArrayRegistry = HashMap<usize, Weak<RefCell<Vec<Value>>>>;
type GcMapRegistry = HashMap<usize, Weak<RefCell<HashMap<String, Value>>>>;
type GcModuleRegistry = HashMap<usize, Weak<RefCell<Module>>>;

thread_local! {
    static GC_ARRAYS: RefCell<GcArrayRegistry> = RefCell::new(HashMap::new());
    static GC_MAPS: RefCell<GcMapRegistry> = RefCell::new(HashMap::new());
    static GC_MODULES: RefCell<GcModuleRegistry> = RefCell::new(HashMap::new());
}

// NOTE on the id: the mark phase keys live objects by `Rc::as_ptr(..)` (a
// pointer to the `RefCell<T>` allocation). Registration MUST use the same
// pointer. Using the method form `x.as_ptr()` resolves to `RefCell::as_ptr`,
// which returns a pointer to the *inner* value — offset from the RefCell by
// the borrow flag — so it would never match the mark ids, and every live
// container would be wrongly swept. Always use `Rc::as_ptr` here.
pub(super) fn gc_register_array(a: &Rc<RefCell<Vec<Value>>>) {
    let id = Rc::as_ptr(a) as usize;
    GC_ARRAYS.with(|t| {
        t.borrow_mut().insert(id, Rc::downgrade(a));
    });
}

pub(super) fn gc_register_map(m: &Rc<RefCell<HashMap<String, Value>>>) {
    let id = Rc::as_ptr(m) as usize;
    GC_MAPS.with(|t| {
        t.borrow_mut().insert(id, Rc::downgrade(m));
    });
}

pub(super) fn gc_register_module(m: &Rc<RefCell<Module>>) {
    let id = Rc::as_ptr(m) as usize;
    GC_MODULES.with(|t| {
        t.borrow_mut().insert(id, Rc::downgrade(m));
    });
}

/// Live-set bitmap used by the mark phase.
#[derive(Default)]
pub(super) struct GcMarks {
    pub arrays: HashSet<usize>,
    pub maps: HashSet<usize>,
    pub modules: HashSet<usize>,
    pub closures: HashSet<usize>,
}

impl VM {
    /// Run one full mark-and-sweep over container registries.
    /// Returns `(arrays_cleared, maps_cleared, modules_cleared)`.
    pub(super) fn gc_collect_cycles_impl(&mut self) -> (usize, usize, usize) {
        let mut marks = GcMarks::default();

        // --- roots: VM stack values ---
        for v in self.stack.iter() {
            self.gc_mark_value(v, &mut marks);
        }

        // --- roots: active call frames / closures ---
        for f in self.frames.iter() {
            self.gc_mark_closure(&f.closure, &mut marks);
            for deferred in &f.defers {
                self.gc_mark_value(deferred, &mut marks);
            }
            if let Some(result) = &f.module_result {
                self.gc_mark_value(result, &mut marks);
            }
        }

        // Removed call frames no longer root their closed captures. Their
        // pending cleanup closures remain live until invoked or abandoned.
        for unwind in &self.unwinds {
            for deferred in &unwind.pending {
                self.gc_mark_value(deferred, &mut marks);
            }
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

    pub(super) fn gc_mark_closure(&self, c: &Rc<Closure>, marks: &mut GcMarks) {
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

    pub(super) fn gc_mark_module(&self, m: &Rc<RefCell<Module>>, marks: &mut GcMarks) {
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

    pub(super) fn gc_mark_value(&self, v: &Value, marks: &mut GcMarks) {
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

    pub(super) fn gc_sweep_arrays(&self, live: &HashSet<usize>) -> usize {
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

    pub(super) fn gc_sweep_maps(&self, live: &HashSet<usize>) -> usize {
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

    pub(super) fn gc_sweep_modules(&self, live: &HashSet<usize>) -> usize {
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
}
