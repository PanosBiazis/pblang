//! BuiltinFn registration table.
//!
//! Returns the `Vec<BuiltinFn>` of every native pblang builtin available
//! to scripts. The bodies of each builtin live in the sibling submodules
//! under `vm/` — this file is **pure registration boilerplate**: name,
//! arity, function pointer.
//!
//! Capability gating happens *inside* each builtin (`host.allow_*()`
//! check at the top), not at registration time. That's why every builtin
//! is always registered — the call-site checks decide who can actually
//! invoke it.
//!
//! Adding a builtin: implement the body in the appropriate `vm/<area>.rs`
//! submodule (or pick the closest one), expose it `pub(super)`, and add
//! a `BuiltinFn { name, arity, fun }` entry below in the right section.
//! Then thread the name through `VM::builtin_capability_class` (vm.rs)
//! so the sandbox classifier can route it.

use pblang_ir::BuiltinFn;

use super::{
    ai, arrays, atomics, bitwise, crypto, db, encoding, env, ffi, fs, http, json, maps, math,
    memory, net, os, parse, polyglot, predicates, quantum, re, runtime, spawn, strings, sync,
    tensor, time,
};

/// Construct the full builtin registration table.
///
/// `VM::builtins()` calls this, then extends with
/// `pblang_graphics::gfx_builtins()` for the optional SDL2 surface.
pub(super) fn build() -> Vec<BuiltinFn> {
    vec![
    BuiltinFn {
        name: "clock",
        arity: Some(0),
        fun: time::clock,
    },
    BuiltinFn {
        name: "now_ms",
        arity: Some(0),
        fun: time::now_ms,
    },
    BuiltinFn {
        name: "now_iso",
        arity: Some(0),
        fun: time::now_iso,
    },
    BuiltinFn {
        name: "type",
        arity: Some(1),
        fun: predicates::type_of,
    },
    BuiltinFn {
        name: "str",
        arity: Some(1),
        fun: runtime::str_,
    },
    BuiltinFn {
        name: "bigint",
        arity: Some(1),
        fun: math::bigint,
    },
    BuiltinFn {
        name: "rational",
        arity: None, // 1 (number|string) or 2 (num, den)
        fun: math::rational,
    },
    BuiltinFn {
        name: "decimal",
        arity: Some(1),
        fun: math::decimal,
    },
    BuiltinFn {
        name: "num_sqrt",
        arity: Some(1),
        fun: math::num_sqrt,
    },
    BuiltinFn {
        name: "num_pow",
        arity: Some(2),
        fun: math::num_pow,
    },
    // Bitwise (i64 semantics) — also reachable via the infix operators
    // `& | ^ << >>` and the prefix `~`, which the parser desugars to these.
    BuiltinFn {
        name: "bit_and",
        arity: Some(2),
        fun: bitwise::bit_and,
    },
    BuiltinFn {
        name: "bit_or",
        arity: Some(2),
        fun: bitwise::bit_or,
    },
    BuiltinFn {
        name: "bit_xor",
        arity: Some(2),
        fun: bitwise::bit_xor,
    },
    BuiltinFn {
        name: "bit_not",
        arity: Some(1),
        fun: bitwise::bit_not,
    },
    BuiltinFn {
        name: "bit_shl",
        arity: Some(2),
        fun: bitwise::bit_shl,
    },
    BuiltinFn {
        name: "bit_shr",
        arity: Some(2),
        fun: bitwise::bit_shr,
    },
    BuiltinFn {
        name: "num_ln",
        arity: Some(1),
        fun: math::num_ln,
    },
    BuiltinFn {
        name: "num_exp",
        arity: Some(1),
        fun: math::num_exp,
    },
    BuiltinFn {
        name: "num_atan2",
        arity: Some(2),
        fun: math::num_atan2,
    },
    BuiltinFn {
        name: "num_mod",
        arity: Some(2),
        fun: math::num_mod,
    },
    BuiltinFn {
        name: "num_floor",
        arity: Some(1),
        fun: math::num_floor,
    },
    BuiltinFn {
        name: "num_ceil",
        arity: Some(1),
        fun: math::num_ceil,
    },
    BuiltinFn {
        name: "parse_number",
        arity: Some(1),
        fun: parse::parse_number,
    },
    BuiltinFn {
        name: "parse_bool",
        arity: Some(1),
        fun: parse::parse_bool,
    },
    BuiltinFn {
        name: "read_line",
        arity: Some(0),
        fun: runtime::read_line,
    },
    BuiltinFn {
        name: "len",
        arity: Some(1),
        fun: runtime::len,
    },
    BuiltinFn {
        name: "error",
        arity: Some(1),
        fun: runtime::error_builtin,
    },
    BuiltinFn {
        name: "array_push",
        arity: Some(2),
        fun: arrays::array_push,
    },
    BuiltinFn {
        name: "array_pop",
        arity: Some(1),
        fun: arrays::array_pop,
    },
    // Ergonomic global aliases: much of the stdlib calls bare `push`/`pop`
    // (the array.sapp wrappers) without importing array.sapp. A user-defined
    // local `fn push` still shadows these, so this is purely additive.
    BuiltinFn {
        name: "push",
        arity: Some(2),
        fun: arrays::array_push,
    },
    BuiltinFn {
        name: "pop",
        arity: Some(1),
        fun: arrays::array_pop,
    },
    BuiltinFn {
        name: "map_keys",
        arity: Some(1),
        fun: maps::map_keys,
    },
    BuiltinFn {
        name: "map_has",
        arity: Some(2),
        fun: maps::map_has,
    },
    BuiltinFn {
        name: "has_member",
        arity: Some(2),
        fun: maps::has_member,
    },
    BuiltinFn {
        name: "map_remove",
        arity: Some(2),
        fun: maps::map_remove,
    },
    BuiltinFn {
        name: "fs_read",
        arity: Some(1),
        fun: fs::fs_read,
    },
    BuiltinFn {
        name: "fs_write",
        arity: Some(2),
        fun: fs::fs_write,
    },
    BuiltinFn {
        name: "fs_append",
        arity: Some(2),
        fun: fs::fs_append,
    },
    BuiltinFn {
        name: "fs_remove",
        arity: Some(1),
        fun: fs::fs_remove,
    },
    BuiltinFn {
        name: "fs_rename",
        arity: Some(2),
        fun: fs::fs_rename,
    },
    BuiltinFn {
        name: "fs_copy",
        arity: Some(2),
        fun: fs::fs_copy,
    },
    BuiltinFn {
        name: "fs_read_bytes",
        arity: Some(1),
        fun: fs::fs_read_bytes,
    },
    BuiltinFn {
        name: "fs_write_bytes",
        arity: Some(2),
        fun: fs::fs_write_bytes,
    },
    BuiltinFn {
        name: "env_get",
        arity: Some(1),
        fun: env::env_get,
    },
    BuiltinFn {
        name: "env_set",
        arity: Some(2),
        fun: env::env_set,
    },
    BuiltinFn {
        name: "sleep_ms",
        arity: Some(1),
        fun: time::sleep_ms,
    },
    BuiltinFn {
        name: "exec",
        arity: Some(1),
        fun: runtime::exec,
    },
    BuiltinFn {
        name: "proc_spawn_wait",
        arity: Some(2),
        fun: runtime::proc_spawn_wait,
    },
    BuiltinFn {
        name: "import",
        arity: Some(1),
        fun: runtime::import,
    },
    BuiltinFn {
        name: "gc_stats",
        arity: Some(0),
        fun: runtime::gc_stats,
    },
    BuiltinFn {
        name: "gc_collect",
        arity: Some(0),
        fun: runtime::gc_collect,
    },
    // --- Safety builtins (Rust-style guarantees for pblang code) ---
    BuiltinFn {
        name: "assert",
        arity: None,
        fun: runtime::assert_builtin,
    },
    BuiltinFn {
        name: "type_assert",
        arity: Some(2),
        fun: predicates::type_assert,
    },
    BuiltinFn {
        name: "freeze",
        arity: Some(1),
        fun: predicates::freeze,
    },
    BuiltinFn {
        name: "is_frozen",
        arity: Some(1),
        fun: predicates::is_frozen,
    },
    BuiltinFn {
        name: "checked_add",
        arity: Some(2),
        fun: math::checked_add,
    },
    BuiltinFn {
        name: "checked_mul",
        arity: Some(2),
        fun: math::checked_mul,
    },
    BuiltinFn {
        name: "checked_sub",
        arity: Some(2),
        fun: math::checked_sub,
    },
    BuiltinFn {
        name: "checked_div",
        arity: Some(2),
        fun: math::checked_div,
    },
    BuiltinFn {
        name: "is_nil",
        arity: Some(1),
        fun: predicates::is_nil,
    },
    BuiltinFn {
        name: "require_type",
        arity: Some(2),
        fun: predicates::require_type,
    },
    BuiltinFn {
        name: "sha256",
        arity: Some(1),
        fun: crypto::sha256,
    },
    BuiltinFn {
        name: "random_bytes",
        arity: Some(1),
        fun: math::random_bytes,
    },
    BuiltinFn {
        name: "random_float",
        arity: Some(0),
        fun: math::random_float,
    },
    BuiltinFn {
        name: "constant_time_eq",
        arity: Some(2),
        fun: crypto::constant_time_eq,
    },
    BuiltinFn {
        name: "base64_encode",
        arity: Some(1),
        fun: encoding::base64_encode,
    },
    BuiltinFn {
        name: "base64_decode",
        arity: Some(1),
        fun: encoding::base64_decode,
    },
    // FIX #22: binary-safe base64 decode
    BuiltinFn {
        name: "base64_decode_bytes",
        arity: Some(1),
        fun: encoding::base64_decode_bytes,
    },
    // Binary-safe base64 encode (byte array in)
    BuiltinFn {
        name: "base64_encode_bytes",
        arity: Some(1),
        fun: encoding::base64_encode_bytes,
    },
    BuiltinFn {
        name: "json_parse",
        arity: Some(1),
        fun: json::json_parse,
    },
    BuiltinFn {
        name: "json_stringify",
        arity: Some(1),
        fun: json::json_stringify,
    },
    BuiltinFn {
        name: "http_get",
        arity: Some(1),
        fun: http::http_get,
    },
    BuiltinFn {
        name: "http_post",
        arity: Some(2),
        fun: http::http_post,
    },
    BuiltinFn {
        name: "str_contains",
        arity: Some(2),
        fun: strings::str_contains,
    },
    BuiltinFn {
        name: "str_starts_with",
        arity: Some(2),
        fun: strings::str_starts_with,
    },
    BuiltinFn {
        name: "str_trim",
        arity: Some(1),
        fun: strings::str_trim,
    },
    BuiltinFn {
        name: "str_split",
        arity: Some(2),
        fun: strings::str_split,
    },
    BuiltinFn {
        name: "str_join",
        arity: Some(2),
        fun: strings::str_join,
    },
    BuiltinFn {
        name: "str_upper",
        arity: Some(1),
        fun: strings::str_upper,
    },
    BuiltinFn {
        name: "str_lower",
        arity: Some(1),
        fun: strings::str_lower,
    },
    BuiltinFn {
        name: "str_slice",
        arity: Some(3),
        fun: strings::str_slice,
    },
    // Polymorphic slice (string/array) backing the `seq[a..b]` syntax.
    BuiltinFn {
        name: "slice",
        arity: Some(3),
        fun: strings::slice,
    },
    BuiltinFn {
        name: "str_index_of",
        arity: Some(2),
        fun: strings::str_index_of,
    },
    BuiltinFn {
        name: "str_ends_with",
        arity: Some(2),
        fun: strings::str_ends_with,
    },
    BuiltinFn {
        name: "platform",
        arity: Some(0),
        fun: env::platform,
    },
    BuiltinFn {
        name: "cwd",
        arity: Some(0),
        fun: fs::cwd,
    },
    BuiltinFn {
        name: "fs_exists",
        arity: Some(1),
        fun: fs::fs_exists,
    },
    BuiltinFn {
        name: "fs_mkdirs",
        arity: Some(1),
        fun: fs::fs_mkdirs,
    },
    BuiltinFn {
        name: "fs_listdir",
        arity: Some(1),
        fun: fs::fs_listdir,
    },
    BuiltinFn {
        name: "path_join",
        arity: None,
        fun: fs::path_join,
    },
    BuiltinFn {
        name: "path_dirname",
        arity: Some(1),
        fun: fs::path_dirname,
    },
    BuiltinFn {
        name: "path_basename",
        arity: Some(1),
        fun: fs::path_basename,
    },
    BuiltinFn {
        name: "tcp_listen",
        arity: Some(1),
        fun: net::tcp_listen,
    },
    BuiltinFn {
        name: "tcp_accept",
        arity: Some(1),
        fun: net::tcp_accept,
    },
    BuiltinFn {
        name: "tcp_read",
        arity: Some(2),
        fun: net::tcp_read,
    },
    BuiltinFn {
        name: "tcp_read_until",
        arity: Some(3),
        fun: net::tcp_read_until,
    },
    BuiltinFn {
        name: "tcp_write",
        arity: Some(2),
        fun: net::tcp_write,
    },
    BuiltinFn {
        name: "tcp_close",
        arity: Some(1),
        fun: net::tcp_close,
    },
    BuiltinFn {
        name: "tcp_local_addr",
        arity: Some(1),
        fun: net::tcp_local_addr,
    },
    // TLS builtins (W4-gamma)
    BuiltinFn {
        name: "tcp_listen_tls",
        arity: Some(3),
        fun: net::tcp_listen_tls,
    },
    BuiltinFn {
        name: "tcp_connect_tls",
        arity: Some(2),
        fun: net::tcp_connect_tls,
    },
    BuiltinFn {
        name: "tcp_wrap_tls",
        arity: Some(3),
        fun: net::tcp_wrap_tls,
    },
    BuiltinFn {
        name: "http_parse_request",
        arity: Some(1),
        fun: http::http_parse_request,
    },
    BuiltinFn {
        name: "http_response",
        arity: Some(2),
        fun: http::http_response,
    },
    BuiltinFn {
        name: "http_response_ct",
        arity: Some(3),
        fun: http::http_response_ct,
    },
    BuiltinFn {
        name: "http_response_json",
        arity: Some(2),
        fun: http::http_response_json,
    },
    BuiltinFn {
        name: "onnx_infer",
        arity: None,
        fun: ai::onnx_infer,
    },
    BuiltinFn {
        name: "ai_chat",
        arity: Some(1),
        fun: ai::ai_chat,
    },
    BuiltinFn {
        name: "ai_embed",
        arity: Some(1),
        fun: ai::ai_embed,
    },
    BuiltinFn {
        name: "ai_token_count",
        arity: Some(1),
        fun: ai::ai_token_count,
    },
    BuiltinFn {
        name: "quantum_run",
        arity: None,
        fun: quantum::quantum_run,
    },
    BuiltinFn {
        name: "quantum_statevector",
        arity: None,
        fun: quantum::quantum_statevector,
    },
    BuiltinFn {
        name: "quantum_backends",
        arity: Some(0),
        fun: quantum::quantum_backends,
    },
    BuiltinFn {
        name: "polyglot_exec",
        arity: Some(2),
        fun: polyglot::polyglot_exec,
    },
    BuiltinFn {
        name: "polyglot_exec_with_input",
        arity: Some(3),
        fun: polyglot::polyglot_exec_with_input,
    },
    BuiltinFn {
        name: "polyglot_exec_compiled",
        arity: Some(2),
        fun: polyglot::polyglot_exec_compiled,
    },
    BuiltinFn {
        name: "polyglot_detect",
        arity: Some(0),
        fun: polyglot::polyglot_detect,
    },
    BuiltinFn {
        name: "polyglot_compile_cached",
        arity: Some(2),
        fun: polyglot::polyglot_compile_cached,
    },
    BuiltinFn {
        name: "polyglot_cache_clear",
        arity: Some(0),
        fun: polyglot::polyglot_cache_clear,
    },
    BuiltinFn {
        name: "polyglot_cache_info",
        arity: Some(0),
        fun: polyglot::polyglot_cache_info,
    },
    BuiltinFn {
        name: "ffi_load",
        arity: Some(1),
        fun: ffi::ffi_load,
    },
    BuiltinFn {
        name: "ffi_call",
        arity: Some(3),
        fun: ffi::ffi_call,
    },
    BuiltinFn {
        name: "ffi_close",
        arity: Some(1),
        fun: ffi::ffi_close,
    },
    BuiltinFn {
        name: "ffi_call_typed",
        arity: Some(4),
        fun: ffi::ffi_call_typed_builtin,
    },
    BuiltinFn {
        name: "sqlite_open",
        arity: Some(1),
        fun: db::sqlite_open,
    },
    BuiltinFn {
        name: "sqlite_close",
        arity: Some(1),
        fun: db::sqlite_close,
    },
    BuiltinFn {
        name: "sqlite_exec",
        arity: Some(3),
        fun: db::sqlite_exec,
    },
    BuiltinFn {
        name: "sqlite_query",
        arity: Some(3),
        fun: db::sqlite_query,
    },
    BuiltinFn {
        name: "sqlite_last_insert_rowid",
        arity: Some(1),
        fun: db::sqlite_last_insert_rowid,
    },
    BuiltinFn {
        name: "spawn",
        arity: Some(1),
        fun: spawn::spawn_builtin,
    },
    BuiltinFn {
        name: "join",
        arity: Some(1),
        fun: spawn::join_builtin,
    },
    // Channel builtins
    BuiltinFn {
        name: "channel",
        arity: Some(0),
        fun: spawn::channel_builtin,
    },
    BuiltinFn {
        name: "send",
        arity: Some(2),
        fun: spawn::channel_send_builtin,
    },
    BuiltinFn {
        name: "recv",
        arity: Some(1),
        fun: spawn::channel_recv_builtin,
    },
    BuiltinFn {
        name: "channel_close",
        arity: Some(1),
        fun: spawn::channel_close_builtin,
    },
    BuiltinFn {
        name: "channel_is_closed",
        arity: Some(1),
        fun: spawn::channel_is_closed_builtin,
    },
    BuiltinFn {
        name: "select",
        arity: Some(1),
        fun: spawn::channel_select_builtin,
    },
    // ---- Enhanced threading builtins ----
    BuiltinFn {
        name: "channel_buffered",
        arity: Some(1),
        fun: spawn::channel_buffered_builtin,
    },
    BuiltinFn {
        name: "recv_timeout",
        arity: Some(2),
        fun: spawn::channel_recv_timeout_builtin,
    },
    BuiltinFn {
        name: "select_with_timeout",
        arity: Some(2),
        fun: spawn::channel_select_timeout_builtin,
    },
    BuiltinFn {
        name: "channel_size",
        arity: Some(1),
        fun: spawn::channel_size_builtin,
    },
    BuiltinFn {
        name: "join_all",
        arity: Some(1),
        fun: spawn::join_all_builtin,
    },
    BuiltinFn {
        name: "spawn_n",
        arity: Some(2),
        fun: spawn::spawn_n_builtin,
    },
    BuiltinFn {
        name: "ticker",
        arity: Some(1),
        fun: spawn::ticker_builtin,
    },
    BuiltinFn {
        name: "timeout_channel",
        arity: Some(1),
        fun: spawn::timeout_channel_builtin,
    },
    // ---- Mutex (Go sync.Mutex / Rust Mutex) ----
    BuiltinFn {
        name: "mutex_new",
        arity: Some(0),
        fun: sync::mutex_new_builtin,
    },
    BuiltinFn {
        name: "mutex_lock",
        arity: Some(1),
        fun: sync::mutex_lock_builtin,
    },
    BuiltinFn {
        name: "mutex_try_lock",
        arity: Some(1),
        fun: sync::mutex_try_lock_builtin,
    },
    BuiltinFn {
        name: "mutex_unlock",
        arity: Some(1),
        fun: sync::mutex_unlock_builtin,
    },
    // ---- RwLock (Go sync.RWMutex / Rust RwLock) ----
    BuiltinFn {
        name: "rwlock_new",
        arity: Some(0),
        fun: sync::rwlock_new_builtin,
    },
    BuiltinFn {
        name: "rwlock_read",
        arity: Some(1),
        fun: sync::rwlock_read_builtin,
    },
    BuiltinFn {
        name: "rwlock_write",
        arity: Some(1),
        fun: sync::rwlock_write_builtin,
    },
    BuiltinFn {
        name: "rwlock_unlock",
        arity: Some(1),
        fun: sync::rwlock_unlock_builtin,
    },
    // ---- Barrier (Java CyclicBarrier / Go WaitGroup) ----
    BuiltinFn {
        name: "barrier_new",
        arity: Some(1),
        fun: sync::barrier_new_builtin,
    },
    BuiltinFn {
        name: "barrier_wait",
        arity: Some(1),
        fun: sync::barrier_wait_builtin,
    },
    // ---- Once (Go sync.Once / Kotlin by lazy) ----
    BuiltinFn {
        name: "once_new",
        arity: Some(0),
        fun: sync::once_new_builtin,
    },
    BuiltinFn {
        name: "once_is_done",
        arity: Some(1),
        fun: sync::once_is_done_builtin,
    },
    // ---- Runtime memory safety ----
    BuiltinFn {
        name: "allocate",
        arity: None,
        fun: memory::allocate_builtin,
    },
    BuiltinFn {
        name: "borrow_immutable",
        arity: Some(1),
        fun: memory::borrow_immutable_builtin,
    },
    BuiltinFn {
        name: "borrow_mutable",
        arity: Some(1),
        fun: memory::borrow_mutable_builtin,
    },
    BuiltinFn {
        name: "borrow_unique",
        arity: Some(1),
        fun: memory::borrow_unique_builtin,
    },
    BuiltinFn {
        name: "release_borrow",
        arity: None,
        fun: memory::release_borrow_builtin,
    },
    BuiltinFn {
        name: "load",
        arity: Some(1),
        fun: memory::mem_load_builtin,
    },
    BuiltinFn {
        name: "store",
        arity: Some(2),
        fun: memory::mem_store_builtin,
    },
    BuiltinFn {
        name: "deallocate",
        arity: Some(1),
        fun: memory::deallocate_builtin,
    },
    BuiltinFn {
        name: "push_lifetime",
        arity: Some(0),
        fun: memory::push_lifetime_builtin,
    },
    BuiltinFn {
        name: "pop_lifetime",
        arity: Some(0),
        fun: memory::pop_lifetime_builtin,
    },
    BuiltinFn {
        name: "memory_stats",
        arity: Some(0),
        fun: memory::memory_stats_builtin,
    },
    BuiltinFn {
        name: "memory_alloc",
        arity: None,
        fun: memory::memory_alloc_builtin,
    },
    BuiltinFn {
        name: "memory_borrow",
        arity: Some(2),
        fun: memory::memory_borrow_builtin,
    },
    BuiltinFn {
        name: "memory_release",
        arity: None,
        fun: memory::memory_release_builtin,
    },
    BuiltinFn {
        name: "memory_get",
        arity: Some(1),
        fun: memory::memory_get_builtin,
    },
    BuiltinFn {
        name: "memory_set",
        arity: Some(2),
        fun: memory::memory_set_builtin,
    },
    BuiltinFn {
        name: "memory_free",
        arity: Some(1),
        fun: memory::memory_free_builtin,
    },
    // ---- Atomic i64 (Java AtomicInteger / Go atomic.Int64 / Rust AtomicI64) ----
    BuiltinFn {
        name: "atomic_new",
        arity: None,
        fun: atomics::atomic_new_builtin,
    },
    BuiltinFn {
        name: "atomic_get",
        arity: Some(1),
        fun: atomics::atomic_get_builtin,
    },
    BuiltinFn {
        name: "atomic_set",
        arity: Some(2),
        fun: atomics::atomic_set_builtin,
    },
    BuiltinFn {
        name: "atomic_add",
        arity: Some(2),
        fun: atomics::atomic_add_builtin,
    },
    BuiltinFn {
        name: "atomic_sub",
        arity: Some(2),
        fun: atomics::atomic_sub_builtin,
    },
    BuiltinFn {
        name: "atomic_cas",
        arity: Some(3),
        fun: atomics::atomic_cas_builtin,
    },
    BuiltinFn {
        name: "atomic_swap",
        arity: Some(2),
        fun: atomics::atomic_swap_builtin,
    },
    // String helpers
    BuiltinFn {
        name: "str_slice",
        arity: Some(3),
        fun: strings::str_slice,
    },
    BuiltinFn {
        name: "str_index_of",
        arity: Some(2),
        fun: strings::str_index_of,
    },
    BuiltinFn {
        name: "str_replace_all",
        arity: Some(3),
        fun: strings::str_replace_all,
    },
    BuiltinFn {
        name: "hmac_sha256",
        arity: Some(2),
        fun: crypto::hmac_sha256,
    },
    BuiltinFn {
        name: "str_ends_with",
        arity: Some(2),
        fun: strings::str_ends_with,
    },
    BuiltinFn {
        name: "str_lower",
        arity: Some(1),
        fun: strings::str_lower,
    },
    BuiltinFn {
        name: "str_upper",
        arity: Some(1),
        fun: strings::str_upper,
    },
    BuiltinFn {
        name: "strfromcharcode",
        arity: Some(1),
        fun: strings::strfromcharcode,
    },
    BuiltinFn {
        name: "strcharcodeat",
        arity: Some(2),
        fun: strings::strcharcodeat,
    },
    // Binary-safe string<->byte-array conversion
    BuiltinFn {
        name: "str_to_bytes",
        arity: Some(1),
        fun: strings::str_to_bytes,
    },
    BuiltinFn {
        name: "bytes_to_str",
        arity: Some(1),
        fun: strings::bytes_to_str,
    },
    // Fundamental sequence/collection builtins
    BuiltinFn {
        name: "range",
        arity: None,
        fun: arrays::range,
    },
    BuiltinFn {
        name: "enumerate",
        arity: Some(1),
        fun: arrays::enumerate,
    },
    BuiltinFn {
        name: "sum",
        arity: Some(1),
        fun: math::sum,
    },
    BuiltinFn {
        name: "min_of",
        arity: Some(1),
        fun: math::min_of,
    },
    BuiltinFn {
        name: "max_of",
        arity: Some(1),
        fun: math::max_of,
    },
    BuiltinFn {
        name: "product",
        arity: Some(1),
        fun: math::product,
    },
    BuiltinFn {
        name: "clamp",
        arity: Some(3),
        fun: math::clamp,
    },
    // Type-checking predicates
    BuiltinFn {
        name: "is_number",
        arity: Some(1),
        fun: predicates::is_number,
    },
    BuiltinFn {
        name: "is_string",
        arity: Some(1),
        fun: predicates::is_string,
    },
    BuiltinFn {
        name: "is_array",
        arity: Some(1),
        fun: predicates::is_array,
    },
    BuiltinFn {
        name: "is_map",
        arity: Some(1),
        fun: predicates::is_map,
    },
    BuiltinFn {
        name: "is_bool",
        arity: Some(1),
        fun: predicates::is_bool,
    },
    BuiltinFn {
        name: "is_fn",
        arity: Some(1),
        fun: predicates::is_fn,
    },
    BuiltinFn {
        name: "is_nil",
        arity: Some(1),
        fun: predicates::is_nil,
    },
    // String helpers
    BuiltinFn {
        name: "str_repeat",
        arity: Some(2),
        fun: strings::str_repeat,
    },
    BuiltinFn {
        name: "str_pad_left",
        arity: Some(3),
        fun: strings::str_pad_left,
    },
    BuiltinFn {
        name: "str_pad_right",
        arity: Some(3),
        fun: strings::str_pad_right,
    },
    // New array helpers
    BuiltinFn {
        name: "array_unique",
        arity: Some(1),
        fun: arrays::array_unique,
    },
    BuiltinFn {
        name: "array_zip",
        arity: Some(2),
        fun: arrays::array_zip,
    },
    BuiltinFn {
        name: "array_chunk",
        arity: Some(2),
        fun: arrays::array_chunk,
    },
    BuiltinFn {
        name: "array_flatten",
        arity: Some(1),
        fun: arrays::array_flatten,
    },
    BuiltinFn {
        name: "array_take",
        arity: Some(2),
        fun: arrays::array_take,
    },
    BuiltinFn {
        name: "array_drop",
        arity: Some(2),
        fun: arrays::array_drop,
    },
    BuiltinFn {
        name: "array_sort",
        arity: Some(1),
        fun: arrays::array_sort,
    },
    // Regex builtins
    BuiltinFn {
        name: "regex_match",
        arity: Some(2),
        fun: re::regex_match_builtin,
    },
    BuiltinFn {
        name: "regex_find",
        arity: Some(2),
        fun: re::regex_find_builtin,
    },
    BuiltinFn {
        name: "regex_find_all",
        arity: Some(2),
        fun: re::regex_find_all_builtin,
    },
    BuiltinFn {
        name: "regex_replace",
        arity: Some(3),
        fun: re::regex_replace_builtin,
    },
    BuiltinFn {
        name: "regex_replace_all",
        arity: Some(3),
        fun: re::regex_replace_all_builtin,
    },
    BuiltinFn {
        name: "regex_split",
        arity: Some(2),
        fun: re::regex_split_builtin,
    },
    BuiltinFn {
        name: "regex_capture",
        arity: Some(2),
        fun: re::regex_capture_builtin,
    },
    // String formatting
    BuiltinFn {
        name: "str_format",
        arity: None,
        fun: strings::str_format,
    },
    BuiltinFn {
        name: "num_abs",
        arity: Some(1),
        fun: math::num_abs,
    },
    BuiltinFn {
        name: "num_max",
        arity: Some(2),
        fun: math::num_max,
    },
    BuiltinFn {
        name: "num_min",
        arity: Some(2),
        fun: math::num_min,
    },
    // OS builtins
    BuiltinFn {
        name: "os_args",
        arity: Some(0),
        fun: os::os_args_builtin,
    },
    BuiltinFn {
        name: "os_exit",
        arity: Some(1),
        fun: os::os_exit_builtin,
    },
    BuiltinFn {
        name: "os_signal_trap",
        arity: Some(2),
        fun: os::os_signal_trap_builtin,
    },
    BuiltinFn {
        name: "os_signal_pending",
        arity: Some(1),
        fun: os::os_signal_pending_builtin,
    },
    BuiltinFn {
        name: "os_signal_poll",
        arity: Some(0),
        fun: os::os_signal_poll_builtin,
    },
    BuiltinFn {
        name: "os_signal_handler_get",
        arity: Some(1),
        fun: os::os_signal_handler_get_builtin,
    },
    BuiltinFn {
        name: "tensor_add",
        arity: Some(2),
        fun: tensor::tensor_add_builtin,
    },
    BuiltinFn {
        name: "tensor_sub",
        arity: Some(2),
        fun: tensor::tensor_sub_builtin,
    },
    BuiltinFn {
        name: "tensor_mul",
        arity: Some(2),
        fun: tensor::tensor_mul_builtin,
    },
    BuiltinFn {
        name: "tensor_scale",
        arity: Some(2),
        fun: tensor::tensor_scale_builtin,
    },
    BuiltinFn {
        name: "tensor_matmul",
        arity: Some(2),
        fun: tensor::tensor_matmul_builtin,
    },
    BuiltinFn {
        name: "tensor_dot",
        arity: Some(2),
        fun: tensor::tensor_dot_builtin,
    },
    BuiltinFn {
        name: "tensor_relu",
        arity: Some(1),
        fun: tensor::tensor_relu_builtin,
    },
    BuiltinFn {
        name: "tensor_sigmoid",
        arity: Some(1),
        fun: tensor::tensor_sigmoid_builtin,
    },
    BuiltinFn {
        name: "tensor_tanh",
        arity: Some(1),
        fun: tensor::tensor_tanh_builtin,
    },
    BuiltinFn {
        name: "tensor_softmax",
        arity: Some(1),
        fun: tensor::tensor_softmax_builtin,
    },
    BuiltinFn {
        name: "tensor_sum",
        arity: Some(1),
        fun: tensor::tensor_sum_builtin,
    },
    BuiltinFn {
        name: "tensor_mean",
        arity: Some(1),
        fun: tensor::tensor_mean_builtin,
    },
    BuiltinFn {
        name: "tensor_argmax",
        arity: Some(1),
        fun: tensor::tensor_argmax_builtin,
    },
    BuiltinFn {
        name: "tensor_transpose2d",
        arity: Some(1),
        fun: tensor::tensor_transpose2d_builtin,
    },
    BuiltinFn {
        name: "tensor_gelu",
        arity: Some(1),
        fun: tensor::tensor_gelu_builtin,
    },
    BuiltinFn {
        name: "tensor_leaky_relu",
        arity: None, // 1 or 2
        fun: tensor::tensor_leaky_relu_builtin,
    },
    BuiltinFn {
        name: "tensor_log_softmax",
        arity: Some(1),
        fun: tensor::tensor_log_softmax_builtin,
    },
    BuiltinFn {
        name: "tensor_layer_norm",
        arity: None, // 1 or 2
        fun: tensor::tensor_layer_norm_builtin,
    },
    BuiltinFn {
        name: "tensor_batch_norm",
        arity: None, // 1 or 2
        fun: tensor::tensor_batch_norm_builtin,
    },
    BuiltinFn {
        name: "tensor_embedding_lookup",
        arity: Some(2),
        fun: tensor::tensor_embedding_lookup_builtin,
    },
    BuiltinFn {
        name: "tensor_cross_entropy",
        arity: Some(2),
        fun: tensor::tensor_cross_entropy_builtin,
    },
    BuiltinFn {
        name: "tensor_conv2d",
        arity: None, // 2-4
        fun: tensor::tensor_conv2d_builtin,
    },
    BuiltinFn {
        name: "tensor_maxpool2d",
        arity: None, // 2-3
        fun: tensor::tensor_maxpool2d_builtin,
    },
    BuiltinFn {
        name: "tensor_avgpool2d",
        arity: None, // 2-3
        fun: tensor::tensor_avgpool2d_builtin,
    },
    BuiltinFn {
        name: "quantum_simulate",
        arity: Some(1),
        fun: quantum::quantum_simulate_builtin,
    },
    BuiltinFn {
        name: "hex_encode",
        arity: Some(1),
        fun: encoding::hex_encode,
    },
    BuiltinFn {
        name: "hex_decode",
        arity: Some(1),
        fun: encoding::hex_decode,
    },
    BuiltinFn {
        name: "url_encode",
        arity: Some(1),
        fun: encoding::url_encode,
    },
    BuiltinFn {
        name: "url_decode",
        arity: Some(1),
        fun: encoding::url_decode,
    },
    BuiltinFn {
        name: "os_which",
        arity: Some(1),
        fun: os::os_which_builtin,
    },
    // str_from_char_code alias for strfromcharcode (used in stdlib)
    BuiltinFn {
        name: "str_from_char_code",
        arity: Some(1),
        fun: strings::strfromcharcode,
    },
    // WebSocket / HTTP / SSE builtins
    BuiltinFn {
        name: "websocket_accept_key",
        arity: Some(1),
        fun: http::websocket_accept_key,
    },
    BuiltinFn {
        name: "http_post_auth",
        arity: Some(3),
        fun: http::http_post_auth,
    },
    BuiltinFn {
        name: "http_get_auth",
        arity: Some(2),
        fun: http::http_get_auth,
    },
    BuiltinFn {
        name: "parse_sse_line",
        arity: Some(1),
        fun: parse::parse_sse_line,
    },
    BuiltinFn {
        name: "websocket_parse_frame",
        arity: Some(1),
        fun: http::ws_parse_frame,
    },
    BuiltinFn {
        name: "websocket_write_frame",
        arity: Some(2),
        fun: http::ws_write_frame,
    },
    ]
}
