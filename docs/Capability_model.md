# pblang Capability Model

This document describes how pblang's sandboxing works for embedders. It is
the canonical reference for what each `StdlibConfig` and `FfiPolicy` flag
denies at runtime, how capabilities propagate into spawned VMs, and what
the secure defaults are.

## TL;DR

* **Blank by default only with `VM::new()`.** It exposes no builtins at all.
* **CLI-compatible, not an untrusted-code profile.**
  `VM::with_stdlib(StdlibConfig::default())` keeps the historical standalone
  defaults: `fs`, `net`, `http`, `sqlite`, `spawn`, `gfx`, `env`, `import`,
  and `process_exit` are on; `exec` and `ffi` are off. Embedders must not use
  that compatibility profile for hostile input.
* **Untrusted scripting** must construct an explicit deny profile, grant only
  the required pure-compute categories, and set `VmLimits`. The CLI's locked
  `--sandbox` profile denies every host-effect category, including imports,
  spawning, environment reads, graphics, and process exit.
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
| `fs` | `true` | `fs_read`, `fs_write`, `fs_append`, `fs_exists`, `fs_mkdirs`, `fs_listdir`, `fs_remove`, `fs_rename`, `fs_copy`, `fs_read_bytes`, `fs_write_bytes` |
| `net` | `true` | `tcp_listen`, `tcp_accept`, `tcp_connect`, `tcp_read`, `tcp_write`, `tcp_close`, `tcp_listen_tls`, `tcp_connect_tls`, `tcp_wrap_tls` |
| `http` | `true` | `http_get`, `http_post`, `http_post_json`, `http_send_raw`, `http_status_reason`, plus all `ai_chat`/`ai_embed`/`onnx_infer` paths that hit the network |
| `sqlite` | `true` | `db_open`, `db_close`, `db_exec`, `db_query`, `db_last_insert_rowid` |
| `ffi` | `false` | `ffi_load`, `ffi_close`, `ffi_call`, `ffi_call_typed` |
| `exec` | `false` | `exec`, `proc_spawn`, `proc_spawn_wait`, `env_set` (disabled regardless — see below) |
| `spawn` | `true` | `spawn` builtin (per-VM thread creation), and therefore the higher-level `stdlib:thread.sapp` wrappers |
| `gfx` | `true` | Graphics builtins, which are removed at registration time when denied. |
| `env` | `true` | `env_get`, preventing reads of process secrets when denied. |
| `import` | `true` | Source-module loading, before any path is resolved. |
| `process_exit` | `true` | `os_exit`, preventing a script from terminating the host process when denied. |

Capability decisions are enforced at the VM boundary, but the historical
`StdlibConfig::default()` is a trusted-CLI compatibility profile rather than a
deny profile. A denied builtin returns a `RuntimeError`
with message `<capability> access denied` (e.g. `fs access denied`,
`proc_spawn_wait access denied`). The error includes the source span so the
script can `try { … } catch e { … }` and react.

### Non-capability builtins

These remain available when all host-effect capabilities are explicitly off:

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

`StdlibConfig::ffi == true` enables the FFI surface but does not grant a library
or symbol by itself. The current runtime installs an empty deny-by-default
`FfiPolicy`; an embedder must explicitly allow exact libraries and symbols, or
set `allow_all` only for fully reviewed trusted source:

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

Linux descriptor-backed stdlib modules expose library readiness separately
from operational verification. `ffi_library_authority` means the reviewed
library could be loaded and closed; `ffi_symbol_contract_verified` remains
false until the runtime can bind every allowed symbol to an approved typed
signature. `operations_attemptable` permits trusted maintenance code to try a
postcondition-checked operation, but is not a sandbox or security attestation.

`secure_file.append_text_bounded_locked(...)` is the private application-state
primitive for cooperating Linux writers. It requires a current-user-owned
mode-0700 native-filesystem parent: an absent final leaf is created exclusively,
while an existing leaf is verified without changing its mode. It rejects
symlinks and multi-link targets,
bounds the complete UTF-8 value, and publishes through fsynced atomic
replacement. Its stable sibling `flock` coordinates local cooperating writers
only; it is not a distributed-filesystem lock and does not constrain code that
ignores the lock contract. First publication currently uses a sealed generated
no-replace syscall helper and therefore requires Linux x86-64/amd64 plus the
reviewed exec capability. A `published:true, retry_safe:false` error must never
be retried automatically because the complete record may already be visible.

## VmLimits — resource limits

`StdlibConfig` controls *what* a script can touch. `VmLimits` controls
*how much* it can do:

| Field | Effect |
|-------|--------|
| `max_instructions: Option<u64>` | After N bytecode ops, normal execution terminates. Registered cleanup shares at most 10,000 additional bytecode ops; nested cleanup never renews this allowance. See [native cleanup limits](Native_cleanup_limits.md). |
| `max_stack_depth: Option<usize>` | Anti-stack-overflow. Recursion past N call frames errors out. |
| `max_memory_bytes: Option<usize>` | Reserved configuration; intrinsic VM allocations are not hard-accounted or bounded by this field. |
| `io_timeout_ms: Option<u64>` | Per-call timeout for `http_*`, `tcp_*`, and the `read_*`/`write_*` variants. |

`VmLimits` are independent of `StdlibConfig` — you can grant `http: true`
but cap each request at 500 ms via `io_timeout_ms`.

## Spawn / capability inheritance

When a script calls `spawn(fn () { … })`, a fresh child VM is created. It
inherits:

* The same `StdlibConfig` as the parent (verbatim clone).
* The same `VmLimits` (each child enforces independently).
* The same `FfiPolicy`.

The child retains the parent capability policy. Limits are per child, so this
is not an aggregate thread, memory or execution-time budget.

Transferable values include numbers, booleans, nil, strings, deeply cloneable
arrays/maps and supported closed-upvalue closures. FFI handle numbers can be
copied as data, but their libraries belong to the originating execution realm
and are not available in the child's fresh realm. Imports within a program
share its FFI realm and its 128-library cap. Network and SQLite tables still
have process-global ownership; do not treat their numeric values as isolated
capabilities. Concurrency tables are explicitly shared with spawned children.

The C API `pblang_create_vm_sandboxed` always disables VM thread spawning and
host process exit. This closes its previous unconditional `spawn` grant; the
ordinary `pblang_create_vm` compatibility constructor remains unchanged.
Neither constructor provides hard native memory or wall-time isolation.

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
let mut vm = VM::with_stdlib(StdlibConfig {
    core: true,
    json: true,
    crypto: true,
    fs: false,
    net: false,
    http: false,
    sqlite: false,
    ffi: false,
    exec: false,
    gfx: false,
    import: false,
    spawn: false,
    env: false,
    process_exit: false,
});
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

## Path-sandbox host contract

The `fs` capability gates **which builtins** are callable
(`fs_read`, `fs_write`, `fs_listdir`, …) — it **does not** gate **which
paths** they may touch. When `fs: true`, the VM trusts whatever path the
script supplies: there is no internal `..`-stripping, no chroot, no
allowlist of permitted directories. `path_join` / `path_dirname` /
`path_basename` are pure string operations on the path text. `fs_remove`
will happily delete `/etc/passwd` if the host process has the OS
permission to do so.

> **The host is responsible for enforcing a path sandbox** when
> `fs: true` is granted with broad filesystem permissions. The VM
> guarantees only that no `fs_*` builtin runs while `fs: false`.

This split is deliberate. It keeps the VM small and lets each embedder
pick the right OS-level mechanism for its threat model. Recommended
patterns, ranked by isolation strength:

1. **OS sandbox (strongest).** Run the embedder inside `bubblewrap`,
   `firejail`, `landlock`, `seccomp`, `chroot`, an unprivileged user
   namespace, a Docker container with read-only mounts, or a Linux jail.
   These intercept syscalls, so even an `fs_open("/etc/passwd")` from
   within pblang can't escape.

2. **Process drop-privileges + chroot.** Before calling `vm.run()`,
   `chroot()` the host process into a scratch dir and `setuid()` to a
   low-privilege user. The pblang VM inherits that view of the
   filesystem; nothing outside the chroot is reachable.

3. **Custom `VmHost` that wraps the fs builtins.** Provide a host that
   intercepts `fs_*` calls at the trait boundary (or pre-validates paths
   before delegating). See `Embedding.md` for the trait shape. This is
   the right answer when you want allowlist semantics like "only `./data/**`
   is writable" without an OS sandbox.

4. **`fs: false` + explicit shim.** Keep `fs` denied; expose only the
   narrow file operations the script actually needs through pblang
   functions in your embedding (passed in as `Closure` values or
   `VmHost::import_module`-resolved modules).

The same logic applies, with different threat surfaces, to:

* **`exec: true`** — `proc_spawn_wait(cmd, args)` will spawn any binary
  the host process can execute. Either constrain `PATH`, use an OS
  sandbox, or wrap the builtin via a custom `VmHost`.
* **`http: true`** — `http_get`/`http_post` honour the system DNS resolver
  and have no internal allowlist. To restrict outbound destinations, use
  an outbound proxy or firewall.
* **`net: true`** — `tcp_connect` is unrestricted. Same story.
* **`ffi: true`** — `ffi_load("./lib.so")` will execute the library's
  constructors **in the host process**, with the host process's full
  privileges. `FfiPolicy::allow_all = false` plus an allowlist of vetted
  libraries is the minimum bar; OS sandboxing is the belt-and-suspenders
  layer.

The TOCTOU window between path resolve and open is hardened at the VM
level via `O_NOFOLLOW` on Unix (`read_text_file_with_race_check`), so a
symlink swap mid-resolve is rejected with `ELOOP`. That covers the
*resolve→open* race; it does **not** cover the larger question of
"should the script be able to name this path in the first place" — that
remains a host-contract responsibility.

## Audit checklist for embedders

Before exposing a pblang VM to untrusted input, verify:

* [ ] `StdlibConfig` is the minimum that lets the script do its job.
* [ ] `VmLimits` are set, with all four fields populated.
* [ ] If `ffi: true`, an `FfiPolicy` with `allow_all: false` is configured.
* [ ] If `fs: true`, an **OS-level path sandbox** is in place (chroot,
      bwrap, landlock, container, or a path-validating `VmHost`). See
      "Path-sandbox host contract" above.
* [ ] If `exec: true`, you've audited *which* binaries the script can spawn.
      (The deny convention is `<builtin_name> access denied`; check the
      message in `proc_spawn_wait_sandbox_denied` tests for the exact text.)
* [ ] If `http: true` or `net: true`, you've decided whether outbound
      connections need an egress proxy or firewall.
* [ ] Errors from `vm.run()` are caught — never `unwrap()` in an embedding
      that processes untrusted input.

## See also

* `Threads.md` — the spawn/join channel model and transferable values.
* `Embedding.md` — host integration patterns (`VmHost` trait).
* `Safety.md` — language-level safety semantics (truthiness, error model).
* `Future_plans.md` v0.4 section — the source of this hardening pass.
* `tests/sandbox.rs` — every capability has at least one denied-by-default
  test; that's the executable spec.
