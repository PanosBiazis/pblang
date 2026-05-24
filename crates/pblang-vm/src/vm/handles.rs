//! VM-global resource registries for net, FFI, and DB handles.
//!
//! These tables back the numeric handles pblang scripts use to refer to
//! TCP listeners/streams, dynamically loaded libraries, and SQLite
//! connections. Centralising the table accessors here (instead of three
//! near-identical `Mutex<HashMap<…>>` blobs in vm.rs) gives one place to
//! handle mutex-poison recovery and one place to add e.g. telemetry or
//! resource caps in the future.
//
// Visibility: every item is `pub(super)` so the parent `vm` module can
// reach it, but nothing escapes the crate.

use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, OnceLock};

use pblang_ir::{RuntimeError, Value, VmHost};

use crate::concurrency::VmHandleRegistry;

use super::{DbResource, FfiResource, NetResource, VM};

// -------------------------------------------------------------------------
// Static registries — one Mutex<HashMap> per resource kind. Initialised on
// first use via OnceLock; each NEXT counter hands out fresh u64 ids.
// -------------------------------------------------------------------------

static NET_TABLE: OnceLock<Mutex<HashMap<usize, NetResource>>> = OnceLock::new();
pub(super) static NET_NEXT: AtomicUsize = AtomicUsize::new(1);

static FFI_TABLE: OnceLock<Mutex<HashMap<usize, FfiResource>>> = OnceLock::new();
pub(super) static FFI_NEXT: AtomicUsize = AtomicUsize::new(1);

static DB_TABLE: OnceLock<Mutex<HashMap<usize, DbResource>>> = OnceLock::new();
pub(super) static DB_NEXT: AtomicUsize = AtomicUsize::new(1);

pub(super) fn net_table() -> &'static Mutex<HashMap<usize, NetResource>> {
    NET_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn ffi_table() -> &'static Mutex<HashMap<usize, FfiResource>> {
    FFI_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn db_table() -> &'static Mutex<HashMap<usize, DbResource>> {
    DB_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Get the per-VM handle registry (for channel / spawn / sync primitives).
/// Errors if the host isn't actually a [`VM`] (e.g. a custom embedder
/// substituted its own `VmHost` impl).
pub(super) fn vm_handles(host: &mut dyn VmHost) -> Result<Arc<VmHandleRegistry>, RuntimeError> {
    host.as_any_mut()
        .downcast_mut::<VM>()
        .map(|vm| Arc::clone(&vm.handle_registry))
        .ok_or_else(|| host.err("concurrency: internal host error".into()))
}

/// Coerce a pblang `Value` into a positive integer handle id, with a
/// friendly error that names the call site (`what`).
pub(super) fn value_to_handle(
    host: &mut dyn VmHost,
    v: Value,
    what: &str,
) -> Result<usize, RuntimeError> {
    match v {
        Value::Number(n) if n.is_finite() && n.fract() == 0.0 && n >= 1.0 => Ok(n as usize),
        _ => Err(host.err(format!("{what}: expected handle number"))),
    }
}

// -------------------------------------------------------------------------
// Safe table accessors. Mutex poison is reported as a RuntimeError instead
// of unwrapped, so a panicking thread holding any table lock doesn't take
// the VM down on the next access (defensive — matches the same pattern
// applied in concurrency.rs).
// -------------------------------------------------------------------------

pub(super) fn with_ffi_table<F, R>(host: &mut dyn VmHost, f: F) -> Result<R, RuntimeError>
where
    F: FnOnce(&mut HashMap<usize, FfiResource>) -> R,
{
    let mut t = ffi_table()
        .lock()
        .map_err(|_| host.err("internal: ffi table unavailable".into()))?;
    Ok(f(&mut t))
}

pub(super) fn with_db_table<F, R>(host: &mut dyn VmHost, f: F) -> Result<R, RuntimeError>
where
    F: FnOnce(&mut HashMap<usize, DbResource>) -> R,
{
    let mut t = db_table()
        .lock()
        .map_err(|_| host.err("internal: db table unavailable".into()))?;
    Ok(f(&mut t))
}

pub(super) fn with_net_table<F, R>(host: &mut dyn VmHost, f: F) -> Result<R, RuntimeError>
where
    F: FnOnce(&mut HashMap<usize, NetResource>) -> R,
{
    let mut t = net_table()
        .lock()
        .map_err(|_| host.err("internal: net table unavailable".into()))?;
    Ok(f(&mut t))
}
