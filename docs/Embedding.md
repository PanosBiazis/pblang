# Embedding API (v0.1)

How to embed the Sapphire VM in a Rust application. For language semantics and limits, see [Language_Semantics.md](Language_Semantics.md).

## Creating a VM

- **`VM::new()`** — Minimal VM with **no** builtins. Use when you register your own functions or load a custom stdlib.
- **`VM::with_stdlib(config)`** — VM with standard builtins (fs, net, http, json, sqlite, ffi, exec, etc.) according to `StdlibConfig`. Use for scripting. Default config enables all capabilities except `allow_ffi` and `allow_exec` (both false by default).

## Running code

- **`vm.run(chunk)`** — Execute a compiled chunk. Returns `Result<(), RuntimeError>`.
- To run source: parse with `pblang_core::parse`, compile with `pblang_compiler::compile`, then `vm.run(chunk)`.
- Chunks can have `source_name` and `source` set for better error messages and stack traces.

## Sandboxing (capabilities)

Set **`StdlibConfig`** when creating the VM to disable categories of builtins:

- `allow_fs` — filesystem and path
- `allow_net` — TCP, HTTP server helpers
- `allow_http` — HTTP client (get/post)
- `allow_sqlite` — SQLite
- `allow_ffi` — ffi_load, ffi_call, ffi_close (default **false**)
- `allow_exec` — exec, env_get, env_set, sleep_ms (default **false**)

If a script calls a disabled builtin, the VM returns `RuntimeError` (e.g. "fs access denied").

## Resource limits

- **`vm.set_limits(Some(limits))`** — Set `VmLimits`:
  - `max_instructions` — stop after N VM steps (anti infinite-loop)
  - `max_stack_depth` — max call frames
  - `max_memory_bytes` — optional heap cap (best-effort)
  - `io_timeout_ms` — timeout in ms for IO: applied to **http_get**, **http_post** (request timeout), **TCP streams** (read/write timeout on **tcp_accept**), and to **fs_read**, **fs_write**, **fs_read_bytes**, **fs_write_bytes** (I/O runs in a thread; on timeout an error is returned).
- Pass `None` to clear limits. Limits are inherited by child VMs when importing modules.

## FFI policy

- **`vm.set_ffi_policy(Some(policy))`** — When `allow_ffi` is true, you can restrict which libraries and symbols are callable:
  - `allow_all: true` — no allowlist
  - `allowed_libs` — set of library paths allowed for `ffi_load`
  - `allowed_symbols` — set of symbol names allowed for `ffi_call`
- When `allow_all` is false, only listed libs/symbols are allowed.

## Resource lifecycle (handles)

Builtins that open resources (TCP sockets, SQLite DBs, FFI handles) return a handle (e.g. a number or opaque value). The script is responsible for closing them (e.g. `tcp_close`, `sqlite_close`, `ffi_close`). **`defer`** in Sapphire runs cleanup on scope exit (LIFO), so recommend scripts use `defer { ... close(handle); }` for predictable cleanup. The host does not automatically close handles when the VM stops; any cleanup beyond script-driven close must be implemented by the host if needed.

## Module loading

- **`import("path")`** in script calls **`VmHost::import_module(&mut self, path)`**. The default implementation (used by the CLI) resolves paths relative to the manifest dir and caches loaded modules. An embedder can override `import_module` to load from an in-memory map, a virtual filesystem, or a custom registry.

## Example

See **`examples/embed.rs`** in the repository: a minimal host that creates a VM with only fs and json enabled, then runs a short script.

## Related

- [Language_Semantics.md](Language_Semantics.md) — Capabilities, limits, FFI policy.
- [Stability_policy.md](Stability_policy.md) — Versioning and breaking changes.
