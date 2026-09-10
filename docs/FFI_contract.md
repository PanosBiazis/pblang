# pblang FFI contract

This document is the canonical spec for pblang's foreign-function interface:
how scripts load C-ABI libraries, what types cross the boundary, what
signature shapes are supported, and what the embedder's responsibilities
are.

Read [`Capability_model.md`](Capability_model.md) first for the broader
sandbox story. This doc only covers what happens *after* the FFI capability
is granted.

## Quickstart

**Development/trusted-code only.** The 2026-09-06 source review confirmed
ABI-width and pointer/string marshalling defects in the retained bootstrap.
Do not grant FFI to untrusted scripts. See
[low-level findings and required fixes](LOW_LEVEL_REVIEW_2026-09-06.md).
The examples below describe the API, not production safety certification.

```pblang
// 1. Open a library. Returns an opaque numeric handle.
let lib = ffi_load("./libmylib.so");

// 2. Call a function by name + signature. Args are passed as a typed array.
//    Signature is "arg1,arg2,...->ret"; types: i64 i32 f64 f32 bool string ptr void
let result = ffi_call_typed(lib, "add", "i64,i64->i64", [40, 2]);
print result;   // 42

// 3. Close when done — releases the underlying dlsym/Library.
ffi_close(lib);
```

## Capability gating (two layers)

Both layers must allow the call or it errors with `ffi access denied` /
`ffi: ... not in allowlist`:

| Layer | Default | Set via |
|-------|---------|---------|
| `StdlibConfig.ffi` | `false` (deny) | `VM::with_stdlib(StdlibConfig { ffi: true, .. })` |
| `FfiPolicy` allowlist | empty deny-by-default | `vm.set_ffi_policy(Some(FfiPolicy { allow_all: false, allowed_libs, allowed_symbols }))` |

For reviewed code that needs FFI, set both: enable the `ffi` capability and pin
the allowlist so the script can only reach libraries and symbols the embedder
named. The current policy binds symbol names but not typed-call signatures, so
even an exact symbol allowlist is not a hostile-code sandbox; a mismatched
signature can cause undefined behavior. Errors **name the rejected entity** so embedders can debug
quickly:

```
ffi_load: library "./libnope.so" not in allowlist (set FfiPolicy.allowed_libs or allow_all=true)
ffi_call: symbol "free" not in allowlist (set FfiPolicy.allowed_symbols or allow_all=true)
```

Library paths are **canonicalised** before matching, so a script that writes
`./libsafe.so` and an allowlist that lists `/abs/path/libsafe.so` both
work (both forms are accepted).

## The three builtins

### `ffi_load(path: string) -> handle`

Open a shared library. Returns an opaque numeric token in `1..=2^53-1`,
tracked in the calling execution realm. Imports share their caller's realm;
independent root VMs and spawned VMs have separate realms. Passing the number
to another realm does not transfer its library authority. Tokens use OS
randomness with bounded allocation retries; table ownership is the boundary,
not token secrecy. Store the token without arithmetic or numeric coercion.

At most 128 libraries may be open per realm, including all nested imports and
`polyglot_compile_cached` handles. Capacity is checked before loading library
constructors or starting a cached polyglot compilation. Closing a library frees
one slot. Failed imports can retain already loaded libraries until the root
realm ends; those resources remain subject to the same cap.

Cross-platform path conventions:

| OS | Convention | Example |
|----|------------|---------|
| Linux | `lib<name>.so` | `./libmath.so`, `/usr/lib/libsodium.so.23` |
| macOS | `lib<name>.dylib` | `./libmath.dylib` |
| Windows | `<name>.dll` | `./math.dll` |

For portable code, do the platform fork in pblang:

```pblang
let os = import("stdlib:os.sapp");
let lib_name = "libmath";
let lib_path = if os.platform() == "windows" {
    "./" + lib_name + ".dll"
} else if os.platform() == "macos" {
    "./" + lib_name + ".dylib"
} else {
    "./" + lib_name + ".so"
};
let lib = ffi_load(lib_path);
```

### `ffi_call(handle, symbol: string, args: array<number>) -> number`

The **raw numeric path** calls the symbol through an `f64` argument/return
signature. A numeric PBLang value does not make an integer C ABI compatible.
This path does not enforce a host-owned signature and can cause undefined
behavior for a mismatched declaration or arity.

If you need anything richer (strings, pointers, void returns), use
`ffi_call_typed` below.

### `ffi_call_typed(handle, symbol: string, sig: string, args: array) -> Value`

Caller-declared signature dispatch. Accepted type labels do not establish
exact C ABI marshalling: the current implementation collapses several distinct
types into the same trampoline shape, as described below.

#### Signature grammar

```
sig := arg_list "->" ret
arg_list := "" | type ("," type)*
type := "i64" | "i32" | "f64" | "f32" | "bool" | "string" | "ptr"
ret  := type | "void"
```

**Known defect:** `i32`/`f32` collapse to `i64`/`f64` trampoline shapes; the
current trampoline does not restore the correct C width. Numeric conversion
also needs finite, integral and range checks. Do not treat accepted signatures
as evidence that narrow-width foreign calls are safe.

`string` arguments are passed as `const char*` (null-terminated UTF-8);
`string` returns currently use an unbounded `CStr::from_ptr` scan before copying
(null pointers become `nil`). Invalid or unterminated pointers can cause memory
faults; bounded buffers, ownership and lifetime enforcement remain required.

`ptr`, `bool` and `i64` currently share an integer trampoline shape. This is not
a portable ABI equivalence. Pointer values also round through PBLang `Number`
and can lose exact bits above 2^53; opaque host-owned pointer handles are needed.

#### Currently dispatched shapes — not a safety guarantee

The dispatch table is explicit to keep the `unsafe extern "C"` surface
bounded. Adding a shape is local to `crates/pblang-vm/src/vm/ffi_dispatch.rs`.

| Args | Return types supported |
|------|------------------------|
| 0    | i, d, s, v             |
| i    | i, d, s, v             |
| d    | i, d                   |
| s    | i, s, v                |
| i,i  | i                      |
| d,d  | d                      |
| s,s  | i, s                   |
| i,s  | i                      |
| s,i  | i                      |
| i,i,i | i                     |
| d,d,d | d                     |
| s,s,s | i                     |
| i,i,i,i | i                   |

(`i` = i64/i32/bool/ptr, `d` = f64/f32, `s` = string/`*const c_char`,
`v` = void.)

If your function doesn't fit, the suggested workaround is to write a tiny
C wrapper that adapts to one of these shapes. Or open a PR adding the
new shape — each new arm is ~5 lines in `vm/ffi_dispatch.rs::dispatch_ffi`.

A signature outside the table returns:

```
ffi_call_typed: unsupported signature dispatch 'i,i,s,s->s'. Supported shapes are listed in the docstring.
```

### `ffi_close(handle) -> nil`

Drop the library handle. Closed, unknown and foreign-realm handles produce
`invalid handle`, including a repeated `ffi_close`. Failed symbol lookup leaves
the original handle valid. Subsequent `ffi_call(handle, ...)` after close errors
with `invalid handle`. **Strongly recommended:** wrap with `defer`:

```pblang
let lib = ffi_load("./libmylib.so");
defer ffi_close(lib);
// ... use lib ...
```

When the last VM owning the realm is dropped, its remaining libraries are
released. Dropping a temporary import VM does not unload the caller's libraries.
A native call retains a short-lived library reference, so closing its handle
does not unload code while that call is still executing. This is resource
lifetime management, not isolation of native memory faults or execution time.

## Safety contract — who is responsible for what

The pblang VM cannot type-check a foreign function. The contract is:

| Responsibility | Whose |
|---------------|-------|
| The library path is loadable on this platform | Embedder + script (set the allowlist; ship the file) |
| The signature string matches the C declaration exactly | Script author |
| Strings are valid UTF-8 (no embedded NUL inside the pblang `Value`) | Script author — the VM rejects NUL via `CString::new` and surfaces the error |
| Returned `const char*` is null-terminated and lives long enough for the immediate copy | Foreign function author |
| The library does not perform unsafe operations (mutate globals racily, longjmp out, etc.) | Foreign function author |
| `ffi_close` is called for every `ffi_load` (or the VM is dropped) | Script author (use `defer`) |

A signature/declaration mismatch is **undefined behaviour** — no different
from raw `libloading`. The reason pblang gates FFI behind a capability
flag AND an allowlist is precisely because once a script can load
arbitrary `.so`/`.dll` files, every line of host-side memory safety is
trust-extended to the foreign code.

## Fixed-method tuple contracts

`stdlib/ffi_tuple_contract.sapp` provides a capability-free declaration format
for future fixed-method brokers. It binds one method ID to a library identity
and a `sha256-raw-library-bytes-v1` digest (SHA-256 over the exact complete raw
library file bytes, with no text conversion, framing, metadata, or path),
symbol, exact scalar-only signature, C calling mode,
platform, architecture, and input/output bounds. The registry rejects unknown
fields, duplicate IDs or tuples, non-ASCII/confusable IDs, pointer types,
callbacks, and variadic signatures, and produces an order-independent digest.

This is a metadata and readiness contract, not runtime enforcement. The current
VM still authorizes libraries and symbols independently and accepts the typed
signature from the caller. Therefore the module always reports
`runtime_signature_tuple_enforced=false`,
`raw_ffi_builtin_bypass_blocked=false`, and
`production_execution_ready=false`. A worker must not be enabled from this
registry until the runtime enforces the complete tuple and loads the verified
library identity without reopening a mutable pathname.

## Error model

Detected validation errors from the FFI surface are `RuntimeError` values with a span pointing
at the call site. ABI undefined behavior and memory faults are not reliably
catchable PBLang errors. The table below covers reported validation failures:

| Error message | Cause |
|---------------|-------|
| `ffi access denied` | `StdlibConfig.ffi == false` |
| `ffi_load: library "X" not in allowlist (set ...)` | `FfiPolicy` set, `X` not in `allowed_libs` |
| `ffi_load(): <libloading error>` | Path not found, ELF mismatch, missing dependency |
| `ffi_call: symbol "X" not in allowlist (set ...)` | `FfiPolicy` set, `X` not in `allowed_symbols` |
| `ffi_call(): args must be numbers for now` | `ffi_call` (numeric variant) received a non-Number arg — switch to `ffi_call_typed` |
| `ffi_call_typed: unsupported signature dispatch 'X'` | Shape not in the table — write a C wrapper or add the shape |
| `signature must be of the form 'a,b->c'` | Malformed `sig` string (missing `->`, etc.) |
| `unknown arg type 'X'` / `unknown ret type 'X'` | Typo in signature; allowed: i64 i32 f64 f32 bool string ptr (ret also: void) |
| `invalid handle` | Handle was closed, never existed, or belongs to another execution realm |
| `expected exact positive FFI handle number` | Non-number, fractional, non-finite, zero, negative or greater than `2^53-1` |
| `ffi: library limit exceeded (128 per execution realm)` | Root program and imports reached their shared open-library cap |

## Cross-platform notes

- **`use-vcpkg` for SDL2** on Windows is the only optional native-dep
  pattern in the project — see `crates/pblang-graphics/Cargo.toml`. FFI
  itself ships with the default build (libloading is in the default
  feature set).
- On macOS, the loader honours `@rpath` / `@loader_path` in the library's
  install names. Set `DYLD_LIBRARY_PATH` or pass an absolute path if
  you're loading a library outside the system paths.
- On Linux, the loader honours `LD_LIBRARY_PATH` and `RPATH`. If a script
  needs to dlopen a library inside the package directory at runtime,
  consider shipping with `-Wl,-rpath,$ORIGIN` so the path is portable.

## Implementation files

| File | What's there |
|------|--------------|
| `crates/pblang-vm/src/vm/ffi_dispatch.rs` | `parse_ffi_sig`, `dispatch_ffi` (the trampoline table), `c_str_to_value` |
| `crates/pblang-vm/src/vm.rs` | The `ffi_load` / `ffi_call` / `ffi_call_typed` builtin closures (capability + allowlist + resource lookup) |
| `crates/pblang-vm/src/vm.rs::FfiPolicy` | The allowlist policy struct |
| `tests/sandbox.rs` | `ffi_sandbox_denied`, `ffi_allowlist_restricts_libraries`, `ffi_allowlist_restricts_symbols` |

## See also

- [`Capability_model.md`](Capability_model.md) — full capability story.
- [`Embedding_lifecycle.md`](Embedding_lifecycle.md) — `VmHost` overrides
  for embedders that need to customise FFI loading from in-memory data
  (e.g. for codesigning or content-addressed plugin distribution).
