# Sapphire language semantics (v0.1)

Stability snapshot for core semantics. Used as reference for conformance and compatibility.

## Truthiness

In conditionals and logical expressions, the following are **false**:

- `nil`
- `false`
- Number `0.0`
- Empty string `""`
- Empty array `[]`
- Empty map `map{}`

All other values are **true**.

## String literals

String literals are double-quoted: `"..."`. Inside a string, the following **escape sequences** are supported:

- `\\` — backslash
- `\"` — double quote
- `\n` — newline
- `\t` — tab
- `\r` — carriage return

Any other character (including unescaped `"`) ends or starts the string. The parser rejects invalid escapes.

## Indexing

- **Arrays**: index by number (0-based). Out-of-range or non-integer index is a runtime error.
- **Maps**: index by string key. Missing key returns `nil` (no error). Attribute access `obj.key` is equivalent to `obj["key"]`.

## Error model

- **Runtime error**: a `RuntimeError` value with message, optional span/source, and stack trace. Thrown by builtins and VM (e.g. type mismatch, undefined global, capability denied).
- **Exception handling**: `try { ... } catch e { ... }` catches runtime errors; `e` is the error value. `defer { ... }` runs on scope exit (LIFO).

## Function call arity

- User functions: arity is fixed at definition. Calling with wrong number of arguments is a runtime error ("arity mismatch").
- Builtins: many accept a fixed number of arguments; mismatch produces a runtime error with a message like "expects N arguments, got M".

## Capabilities (embedding / sandbox)

When the VM is created with `VM::with_stdlib(config)`, the host can disable categories of builtins:

- **allow_fs**: filesystem and path builtins (e.g. `fs_read`, `fs_write`, `path_join`, `cwd`).
- **allow_net**: TCP and HTTP server helpers (`tcp_listen`, `tcp_accept`, `http_parse_request`, etc.).
- **allow_http**: HTTP client (`http_get`, `http_post`).
- **allow_sqlite**: SQLite builtins.
- **allow_ffi**: `ffi_load`, `ffi_call`, `ffi_close`.
- **allow_exec**: `exec`, `env_get`, `env_set`, `sleep_ms`.

If a script calls a builtin whose capability is disabled, the VM returns a runtime error (e.g. "fs access denied"). `VM::new()` creates a VM with **no** builtins (embedding-only).

## Resource limits (embedding)

When **VmLimits** is set via `vm.set_limits(Some(limits))`:

- **max_instructions**: After this many VM instructions, execution stops with a runtime error ("instruction limit exceeded"). Used to prevent infinite loops.
- **max_stack_depth**: Maximum call stack depth (number of frames). Exceeding it yields "stack depth limit exceeded".
- **max_memory_bytes**: Optional cap on heap memory (best-effort enforcement when set).
- **io_timeout_ms**: Timeout in ms for IO: enforced for **http_get**, **http_post**, TCP streams (read/write timeout on **tcp_accept**), and for **fs_read**, **fs_write**, **fs_read_bytes**, **fs_write_bytes** (when set, fs I/O runs in a thread and returns an error on timeout).

These are optional; when not set, there is no limit (subject to host memory).

## FFI policy (embedding)

When **FfiPolicy** is set via `vm.set_ffi_policy(Some(policy))` and `allow_all` is false:

- **allowed_libs**: Only these library paths may be passed to `ffi_load`. Otherwise runtime error "ffi: library not in allowlist".
- **allowed_symbols**: Only these symbol names may be passed to `ffi_call`. Otherwise "ffi: symbol not in allowlist".

When `allow_all` is true or policy is not set, no allowlist check is applied (subject to `allow_ffi` capability).

## Embedding

For a full description of the embedding API (VM creation, sandboxing, limits, FFI policy, resource lifecycle), see [Embedding.md](Embedding.md).
