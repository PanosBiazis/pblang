# pblang FFI contract

This document is the canonical spec for pblang's foreign-function interface:
how scripts load C-ABI libraries, what types cross the boundary, what
signature shapes are supported, and what the embedder's responsibilities
are.

Read [`Capability_model.md`](Capability_model.md) first for the broader
sandbox story. This doc only covers what happens *after* the FFI capability
is granted.

## Quickstart

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
| `FfiPolicy` allowlist | `None` (allow all when `ffi: true`) | `vm.set_ffi_policy(Some(FfiPolicy { allow_all: false, allowed_libs, allowed_symbols }))` |

For untrusted code, set both: enable the `ffi` capability and pin the
allowlist so the script can only reach libraries and symbols the embedder
named. Errors **name the rejected entity** so embedders can debug
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

Open a shared library. Returns a numeric handle that the VM tracks in a
global table. The handle is opaque — store it in a variable; arithmetic
on it is undefined and a future error.

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

The **numeric-only fast path**. All arguments must be numbers; the return
value is always a `Number`. Useful for math libraries where every arg
fits in an `i64` or `f64`.

If you need anything richer (strings, pointers, void returns), use
`ffi_call_typed` below.

### `ffi_call_typed(handle, symbol: string, sig: string, args: array) -> Value`

Full signature dispatch. The `sig` string says what the C function looks
like; the VM picks the matching `unsafe extern "C"` trampoline.

#### Signature grammar

```
sig := arg_list "->" ret
arg_list := "" | type ("," type)*
type := "i64" | "i32" | "f64" | "f32" | "bool" | "string" | "ptr"
ret  := type | "void"
```

`i32`/`f32` are passed/returned as `i64`/`f64` at the pblang Value level
(everything is a Number) but the trampoline casts to the right C width.

`string` arguments are passed as `const char*` (null-terminated UTF-8);
`string` returns are copied into a Rust `String` immediately and returned
as a pblang string (null pointers become `nil`).

`ptr` and `i64` share the same trampoline shape (both 8-byte integer
words). The grammar accepts both for documentation purposes.

#### Supported shapes

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

Drop the library handle. Subsequent `ffi_call(handle, ...)` errors with
`invalid handle`. **Strongly recommended:** wrap with `defer`:

```pblang
let lib = ffi_load("./libmylib.so");
defer ffi_close(lib);
// ... use lib ...
```

If the VM is dropped without closing a handle, the `Library` is freed by
its own `Drop` impl — no leak, just less deterministic ordering than a
`defer`.

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

## Error model

Every error from the FFI surface is a `RuntimeError` with a span pointing
at the call site, so a script can `try { ... } catch e { ... }`:

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
| `invalid handle` | Handle was closed, never existed, or got coerced from a non-handle Number |

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
