//! Resource registries: realm-owned FFI handles and global net/DB handles.
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
use std::rc::Rc;

use rand::{rngs::OsRng, RngCore};
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

static DB_TABLE: OnceLock<Mutex<HashMap<usize, DbResource>>> = OnceLock::new();
pub(super) static DB_NEXT: AtomicUsize = AtomicUsize::new(1);

pub(super) fn net_table() -> &'static Mutex<HashMap<usize, NetResource>> {
    NET_TABLE.get_or_init(|| Mutex::new(HashMap::new()))
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

/// Imports share their caller's realm; independent and spawned VMs do not.
/// Borrow only for lookup/update, never across library loading or native calls.
pub(super) fn with_ffi_table<F, R>(host: &mut dyn VmHost, f: F) -> Result<R, RuntimeError>
where
    F: FnOnce(&mut HashMap<u64, FfiResource>) -> R,
{
    let resources = host
        .as_any_mut()
        .downcast_mut::<VM>()
        .map(|vm| Rc::clone(&vm.ffi_resources))
        .ok_or_else(|| host.err("ffi: internal host error".into()))?;
    let mut table = resources
        .try_borrow_mut()
        .map_err(|_| host.err("internal: ffi table unavailable".into()))?;
    Ok(f(&mut table))
}

pub(super) const MAX_FFI_LIBRARIES_PER_REALM: usize = 128;
const MAX_FFI_HANDLE: u64 = (1u64 << 53) - 1;

pub(super) fn value_to_ffi_handle(
    host: &mut dyn VmHost,
    value: Value,
    what: &str,
) -> Result<u64, RuntimeError> {
    match value {
        Value::Number(n)
            if n.is_finite() && n.fract() == 0.0 && n >= 1.0 && n <= MAX_FFI_HANDLE as f64 =>
        {
            Ok(n as u64)
        }
        _ => Err(host.err(format!("{what}: expected exact positive FFI handle number"))),
    }
}

/// Check before loading constructors or starting a polyglot compilation.
pub(super) fn require_ffi_capacity(host: &mut dyn VmHost) -> Result<(), RuntimeError> {
    if with_ffi_table(host, |table| table.len())? >= MAX_FFI_LIBRARIES_PER_REALM {
        return Err(host.err("ffi: library limit exceeded (128 per execution realm)".into()));
    }
    Ok(())
}

/// The owning table is the authority boundary. Random tokens add opacity only.
pub(super) fn insert_ffi_library(
    host: &mut dyn VmHost,
    library: libloading::Library,
) -> Result<u64, RuntimeError> {
    require_ffi_capacity(host)?;
    for _ in 0..32 {
        let mut bytes = [0u8; 8];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| host.err("ffi: handle entropy unavailable".into()))?;
        let id = u64::from_le_bytes(bytes) & MAX_FFI_HANDLE;
        if id != 0 && with_ffi_table(host, |table| !table.contains_key(&id))? {
            with_ffi_table(host, |table| {
                table.insert(id, FfiResource::Library(Rc::new(library)));
            })?;
            return Ok(id);
        }
    }
    Err(host.err("ffi: handle allocation failed".into()))
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
