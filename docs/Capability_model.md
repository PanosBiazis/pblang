# pblang Capability Model

This document describes how pblang's sandboxing works for embedders. It is
the canonical reference for what each `StdlibConfig` and `FfiPolicy` flag
denies at runtime, how capabilities propagate into spawned VMs, and what
the secure defaults are.

## TL;DR

* **Secure by default.** A `VM` created with `VM::new()` has no builtins
  exposed at all. A `VM::with_stdlib(StdlibConfig::default())` enables the
  pure-compute builtins but denies every capability that touches the OS:
  `fs`, `net`, `http`, `sqlite`, `ffi`, `exec`, `spawn`.
* **Trusted scripting** — your own tooling — should opt in to capabilities
  it needs via `StdlibConfig { fs: true, http: true, .. }`.
* **Untrusted scripting** — code you didn't write — should run with
  `StdlibConfig::default()` (everything off) plus an explicit `VmLimits`
  (instructions, stack, memory, IO timeout).
* **FFI** is gated by two layers: the `ffi` capability flag, and an
  optional `FfiPolicy` allowlist of library paths and symbol names.

## The two layers

```
┌──────────────────────────────────────────────────────────────────┐
│ StdlibConfig — capability flags (yes / no for whole categories)  │
│   fs · net · http · sqlite · ffi · exec · spawn · …              │
└──────────────────────────────────────────────────────────────────┘
                              │ when ffi = true
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│ FfiPolicy — fine-grained allowlist for FFI                       │
│   allow_all · allowed_libs · allowed_symbols                     │
└──────────────────────────────────────────────────────────────────┘
```

`StdlibConfig` is the broad on/off switch. `FfiPolicy` is the fine-grained
filter that applies *after* the `ffi` flag is on.

## StdlibConfig flags

| Flag | Default | What turning it OFF denies |
|------|---------|----------------------------|
| `fs` | `false` | `fs_read`, `fs_write`, `fs_append`, `fs_exists`, `fs_mkdirs`, `fs_listdir`, `fs_remove`, `fs_rename`, `fs_copy`, `fs_read_bytes`, `fs_write_bytes` |
| `net` | `false` | `tcp_listen`, `tcp_accept`, `tcp_connect`, `tcp_read`, `tcp_write`, `tcp_close`, `tcp_listen_tls`, `tcp_connect_tls`, `tcp_wrap_tls` |
| `http` | `false` | `http_get`, `http_post`, `http_post_json`, `http_send_raw`, `http_status_reason`, plus all `ai_chat`/`ai_embed`/`onnx_infer` paths that hit the network |
| `sqlite` | `false` | `db_open`, `db_close`, `db_exec`, `db_query`, `db_last_insert_rowid` |
| `ffi` | `false` | `ffi_load`, `ffi_close`, `ffi_call`, `ffi_call_typed` |
| `exec` | `false` | `exec`, `proc_spawn`, `proc_spawn_wait`, `env_set` (disabled regardless — see below) |
| `spawn` | `false` | `spawn` builtin (per-VM thread creation), and therefore the higher-level `stdlib:thread.sapp` wrappers |

Capabilities are deny-by-default. A denied builtin returns a `RuntimeError`
with message `<capability> access denied` (e.g. `fs access denied`,
`proc_spawn_wait access denied`). The error includes the source span so the
script can `try { … } catch e { … }` and react.

### Non-capability builtins

These remain available even at `StdlibConfig::default()`:

* `print`, `read_line`, `len`, `str`, `type`, `clock`, `now_ms`, `now_iso`
* Math: `num_floor`, `num_ceil`, `num_sqrt`, `num_abs`, `num_pow`, `num_mod`, `parse_number`, `parse_bool`
* Strings: `str_split`, `str_trim`, `str_contains`, `str_starts_with`, `str_ends_with`, `str_index_of`, `str_slice`, `str_replace_all`, `str_upper`, `str_lower`, `str_repeat`, `str_pad_left`, `str_pad_right`, `str_format`, `strcharcodeat`, `strfromcharcode`
* Arrays / maps: `array_push`, `array_pop`, `array_sort`, `array_unique`, `array_zip`, `array_chunk`, `array_flatten`, `array_take`, `array_drop`, `map_keys`, `map_has`, `map_remove`
* Hashing / encoding: `sha256`, `hmac_sha256`, `hex_encode`, `hex_decode`, `url_encode`, `url_decode`, `random_bytes`
* JSON: `json_parse`, `json_stringify`
* Regex: `regex_match`, `regex_find`, `regex_find_all`, `regex_replace`, `regex_replace_all`, `regex_split`, `regex_capture`
* Quantum (pure-compute simulator): `quantum_simulate`, `quantum_grover`, `quantum_shor`
* Tensor / neural-net: `tensor_*`, `nn_*` — these allocate and compute in-process; they do not touch the network
* Concurrency primitives (without `spawn`): `channel`, `send`, `recv`, `select`

## FfiPolicy — fine-grained FFI control

When `StdlibConfig::ffi == true`, the VM by default allows loading **any**
library and calling **any** symbol. To narrow that, set an `FfiPolicy`:

```rust
let mut allowed_libs = HashSet::new();
allowed_libs.insert("./libsafe.so".to_string());
allowed_libs.insert("/usr/lib/libsodium.so".to_string());

let mut allowed_symbols = HashSet::new();
allowed_symbols.insert("crypto_sign_keypair".to_string());
allowed_symbols.insert("crypto_sign".to_string());

vm.set_ffi_policy(Some(FfiPolicy {
    allow_all: false,
    allowed_libs,
    allowed_symbols,
}));
```

Behaviour:

* `allow_all = true` — every FFI call passes (back-compat / trusted use).
  Default for `FfiPolicy::default()` is `allow_all = false`.
* `allow_all = false` — only paths in `allowed_libs` may be loaded; only
  names in `allowed_symbols` may be called.
* Library paths are **canonicalised** before matching, so a script that
  writes `./libsafe.so` and an allowlist that lists `/abs/path/libsafe.so`
  both work.
* Denied loads / calls produce errors that name the rejected entity:
  `ffi_load: library "./libnope.so" not in allowlist (set FfiPolicy.allowed_libs or allow_all=true)`.

## VmLimits — resource limits

`StdlibConfig` controls *what* a script can touch. `VmLimits` controls
*how much* it can do:

| Field | Effect |
|-------|--------|
| `max_instructions: Option<u64>` | Anti-infinite-loop. The VM aborts with `instruction limit exceeded` once N bytecode ops have run. |
| `max_stack_depth: Option<usize>` | Anti-stack-overflow. Recursion past N call frames errors out. |
| `max_memory_bytes: Option<usize>` | Best-effort heap cap. Enforced where the allocator can ask. |
| `io_timeout_ms: Option<u64>` | Per-call timeout for `http_*`, `tcp_*`, and the `read_*`/`write_*` variants. |

`VmLimits` are independent of `StdlibConfig` — you can grant `http: true`
but cap each request at 500 ms via `io_timeout_ms`.

## Spawn / capability inheritance

When a script calls `spawn(fn () { … })`, a fresh child VM is created. It
inherits:

* The same `StdlibConfig` as the parent (verbatim clone).
* The same `VmLimits` (each child enforces independently).
* The same `FfiPolicy`.

This means a sandboxed parent **cannot** be tricked into running unrestricted
code via spawn — there is no privilege-escalation channel.

The values passed across the channel are restricted to *transferable*
types: numbers, booleans, nil, strings, and deeply-cloneable arrays / maps
(including closed-upvalue closures). Handles (file descriptors, TCP
streams, FFI library handles, SQLite connections) **cannot** cross the
boundary — they're rejected at `send_value` with `value not transferable`.

This is the same VM-per-thread isolation Erlang and Go use; it eliminates
data races by construction.

## Recipes

### Trusted CLI tool (your own code)

```rust
let mut vm = VM::with_stdlib(StdlibConfig {
    fs: true,
    http: true,
    sqlite: true,
    exec: true,
    spawn: true,
    ..Default::default()
});
// ffi and net usually left off unless explicitly needed
```

### Untrusted plugin (third-party script)

```rust
let mut vm = VM::with_stdlib(StdlibConfig::default()); // all off
vm.set_limits(Some(VmLimits {
    max_instructions: Some(50_000_000),
    max_stack_depth: Some(256),
    max_memory_bytes: Some(64 * 1024 * 1024),
    io_timeout_ms: Some(2_000),
}));
```

### Allow only one specific C library

```rust
let mut vm = VM::with_stdlib(StdlibConfig {
    ffi: true,
    ..Default::default()
});
let mut libs = HashSet::new();
libs.insert("./libmyplugin.so".to_string());
let mut syms = HashSet::new();
syms.insert("plugin_init".to_string());
syms.insert("plugin_process".to_string());
vm.set_ffi_policy(Some(FfiPolicy {
    allow_all: false,
    allowed_libs: libs,
    allowed_symbols: syms,
}));
```

## Why `env_set` is gone even with `exec: true`

`std::env::set_var` is unsound in multi-threaded programs — Rust 2024 marks
it `unsafe` for this reason. pblang's v0.5.1 security audit removed
`env_set` entirely; reading env vars via `env_get` is always safe and
remains capability-free. If you need per-VM env scratchpad semantics,
use a `map{}` global or pass overrides through the `host.invoke_*` path
instead.

## Audit checklist for embedders

Before exposing a pblang VM to untrusted input, verify:

* [ ] `StdlibConfig` is the minimum that lets the script do its job.
* [ ] `VmLimits` are set, with all four fields populated.
* [ ] If `ffi: true`, an `FfiPolicy` with `allow_all: false` is configured.
* [ ] If `exec: true`, you've audited *which* binaries the script can spawn.
      (The deny convention is `<builtin_name> access denied`; check the
      message in `proc_spawn_wait_sandbox_denied` tests for the exact text.)
* [ ] Errors from `vm.run()` are caught — never `unwrap()` in an embedding
      that processes untrusted input.

## See also

* `Threads.md` — the spawn/join channel model and transferable values.
* `Embedding.md` — host integration patterns (`VmHost` trait).
* `Safety.md` — language-level safety semantics (truthiness, error model).
* `Future_plans.md` v0.4 section — the source of this hardening pass.
* `tests/sandbox.rs` — every capability has at least one denied-by-default
  test; that's the executable spec.
