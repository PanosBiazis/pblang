# pblang embedding lifecycle

How to drive a pblang VM from a host application (Rust, Python via PyO3,
C via the FFI shim, etc.). Covers VM creation, capability + limit
configuration, running scripts, error handling, module caching, the
spawn/thread model from the host side, resource cleanup, and the
`VmHost` trait override surface.

See also:
- [`Capability_model.md`](Capability_model.md) — what each capability flag denies.
- [`FFI_contract.md`](FFI_contract.md) — script-to-C-library bridging.

## VM creation — three modes

| Constructor | Builtins exposed | Use case |
|-------------|------------------|----------|
| `VM::new()` | **None.** A blank slate; even `print` is undefined. | Embedders that want to expose only their own bespoke builtins by overriding `VmHost`. |
| `VM::with_stdlib(StdlibConfig::default())` | Historical standalone/CLI-compatible profile: most host effects are enabled, while `exec` and `ffi` are denied. | Trusted local scripting only; not an untrusted-code profile. |
| `VM::with_stdlib(StdlibConfig { fs: true, http: true, .. })` | Pure-compute + the capabilities you turn on. | Trusted scripting / your own tooling. |

```rust
use pblang_vm::{StdlibConfig, VmLimits, FfiPolicy, VM};

// Trusted CLI tool
let mut vm = VM::with_stdlib(StdlibConfig {
    fs: true,
    http: true,
    sqlite: true,
    exec: true,
    spawn: true,
    ..Default::default()
});

// Untrusted plugin runner: start with an explicit deny profile.
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

`set_limits` and `set_ffi_policy` accept `Option<…>` so passing `None`
removes the prior setting.

## Running a script

```rust
use pblang_compiler::compile;
use pblang_core::parse;

let stmts = parse(source).map_err(|e| anyhow!("parse: {e:?}"))?;
let chunk = compile(stmts)?;             // CompileError implements Error
vm.run(chunk)?;                          // RuntimeError on failure
```

Two run flavours:

| Method | Use when |
|--------|----------|
| `vm.run(chunk)` | You want the script's `print` output to go straight to stdout. Returns `Result<(), RuntimeError>`. |
| `vm.run_collect(chunk)` | You want to **capture** every printed `Value` into a `Vec<Value>` (great for tests, REPLs, and golden-file comparisons). |

A `Chunk` can be re-run on the same VM (the script body becomes a 0-arg
"script" function that the VM call frame stack runs to completion). VM
state (globals, the module cache, registered handles, the GC arena) is
preserved across `run` calls. Two scripts that share globals can drive
each other; the embedder picks whether to keep or discard that state by
creating a fresh `VM` or reusing.

## Error model

Errors propagate as `pblang_vm::RuntimeError`:

```rust
pub struct RuntimeError {
    pub message: String,
    pub span: Option<Span>,
    pub stack: Vec<FrameTrace>,   // top-of-stack first
}
```

Embedders should render `message`, then `stack` for diagnostics. `Span`
carries `start`/`end` byte offsets into the *original source* of the
frame that errored, which is also reachable via the topmost
`FrameTrace`'s source name. Do not discard the `vm.run()` error in production
embeddings — every script error is recoverable.

A few capability errors have stable text the host can pattern-match:

| Substring | Meaning |
|-----------|---------|
| `<cap> access denied` | A capability builtin was denied (`fs access denied`, `proc_spawn_wait access denied`, etc.). Surface to the user as a permission prompt. |
| `not in allowlist` | An FFI library or symbol failed the `FfiPolicy` check. |
| `instruction limit exceeded` | Normal instruction cap reached; native cleanup receives one bounded 10,000-instruction allowance before termination. Existing primary errors are preserved. See [native cleanup limits](Native_cleanup_limits.md). |
| `import cycle detected` | Two modules import each other; the cycle is included in the message. |

## Module cache & import system

Imports (`import "path/to/mod.sapp"`, `import "stdlib:array.sapp"`,
`import("path.sapp")` as expression) resolve through
`pblang_pm::resolve_import`. Each resolved path is cached per-VM in
`module_cache` — a second import of the same path returns the same
`Module` object (so `let a = import("x"); let b = import("x"); print a == b;`
prints `true`).

Capability inheritance: an imported `.sapp` file runs inside a fresh
*child* VM constructed with the same `StdlibConfig`, `VmLimits`, and
`FfiPolicy` as the parent. There's no way for an imported module to
escalate beyond what the embedder granted.

To intercept imports (load from memory, content-addressed plugin
distribution, etc.), override `VmHost::import_module` — see
"VmHost override surface" below.

## Spawn / thread isolation from the host side

A pblang `spawn(fn() { ... })` constructs a *child* VM in a fresh OS
thread. The child:

- Inherits the parent's `StdlibConfig`, `VmLimits`, and `FfiPolicy` (by
  clone, not by reference).
- Has its own GC arena and its own globals — there is no shared mutable
  state between parent and child.
- Receives values via channels; only **transferable** values cross the
  boundary (numbers, booleans, nil, strings, deeply-cloneable arrays/maps,
  closed-upvalue closures). Handles (file descriptors, TCP streams,
  FFI libraries, SQLite connections) **cannot** be sent — they're
  rejected at `send_value` with `value not transferable`.

For the host:

- The host's main `VM` is **not `Send`** — keep it on the thread that
  owns it.
- Spawned child VMs run independently; if the parent's thread aborts, the
  children are unaffected and will be cleaned up when the OS reclaims
  their threads.
- If the embedder wants to inject a value into a running spawned VM
  out-of-band (e.g. a control-plane "shutdown" signal), the recommended
  pattern is a channel sent into the spawn closure before the script
  runs.

## Resource lifecycle

Every long-lived resource (TCP stream, SQLite connection, FFI library,
spawned-thread join handle, channel) is held by a numeric handle stored
in a VM-global table.

| Resource | Created by | Closed by |
|----------|-----------|-----------|
| TCP stream | `tcp_listen` / `tcp_connect` / `tcp_accept` | `tcp_close(stream)` or VM drop |
| SQLite connection | `db_open` | `db_close(conn)` or VM drop |
| FFI library | `ffi_load` | `ffi_close(lib)` or VM drop |
| Spawn handle | `spawn(fn)` | `join(handle)` |
| Channel | `channel()` | `channel_close(sender)` then drop |

The VM drops everything when it's dropped (`Drop` on the table types
closes underlying OS resources). Scripts are still encouraged to `defer
xxx_close(handle)` for deterministic cleanup; otherwise resources only
release when the VM is itself dropped.

## VmHost trait — override surface

The VM delegates host-facing behaviour to a trait so embedders can
substitute their own implementations. Default impls (in `pblang-ir`)
return sensible defaults; the concrete `VM` provides production impls
the embedder can override.

Most-overridden methods:

| Method | Default | When to override |
|--------|---------|-----------------|
| `allow_fs / allow_net / allow_http / allow_sqlite / allow_ffi / allow_exec / allow_spawn` | `false` (or read from `StdlibConfig` on the concrete `VM`) | Custom capability-resolution logic (e.g. per-user RBAC) instead of a flat `StdlibConfig`. |
| `ffi_allowed_library(path)` / `ffi_allowed_symbol(symbol)` | `false` (or read from `FfiPolicy` on the concrete `VM`) | Dynamic allowlist computation (regex match, signature verification of the .so file, etc.). |
| `allow_import()` | `false` (or read from `StdlibConfig` on the concrete `VM`) | Virtual/content-addressed module loading without granting arbitrary source-path imports. |
| `import_module(path)` | Reads disk via `pblang_pm::resolve_import` | Load modules from memory, from an embedded TAR/ZIP, from a remote CDN; or implement content-addressed module pinning. |
| `io_timeout_ms()` | Reads `VmLimits.io_timeout_ms` | Per-call dynamic timeouts (e.g. tighter during interactive REPL idle). |
| `err(msg)` | Builds a `RuntimeError` with current span + stack | Add embedder-specific telemetry (Sentry breadcrumbs, trace ids). |
| `gc_collect_cycles()` | Delegates to `VM::gc_collect_cycles_impl` | Pluggable GC scheduling (e.g. amortise GC pauses across `run` calls). |

## Hard rules (footguns to avoid)

1. **Never share a `VM` across threads.** Construct one per thread.
2. **Never `unwrap()` `vm.run()`** in code that processes untrusted input.
3. **Don't keep handles alive past VM drop.** A pblang Number that holds
   a stale handle ID will fail with `invalid handle` on use — fine for
   debugging, less fine in production where you wanted a clean error
   earlier.
4. **Set `VmLimits` before exposing untrusted code.** Without limits, a
   non-terminating script can occupy the host thread until the
   process is killed.
5. **Set an `FfiPolicy` if you grant `ffi: true`** to untrusted code.
   Without a policy, the allowlist defaults to "allow all" once the
   capability flag is on.

## Minimal complete embedding example

```rust
use anyhow::Result;
use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{FfiPolicy, StdlibConfig, VmLimits, VM};
use std::collections::HashSet;

fn run_untrusted(source: &str) -> Result<Vec<String>> {
    // Capability config: deny everything dangerous.
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

    // Resource limits.
    vm.set_limits(Some(VmLimits {
        max_instructions: Some(20_000_000),
        max_stack_depth: Some(256),
        max_memory_bytes: Some(32 * 1024 * 1024),
        io_timeout_ms: Some(1_000),
    }));

    // Pin a deny policy as a second FFI layer.
    vm.set_ffi_policy(Some(FfiPolicy {
        allow_all: false,
        allowed_libs: HashSet::new(),
        allowed_symbols: HashSet::new(),
    }));

    // Parse + compile + run.
    let stmts = parse(source).map_err(|e| anyhow::anyhow!("parse: {e:?}"))?;
    let chunk = compile(stmts).map_err(|e| anyhow::anyhow!("compile: {e}"))?;
    let out = vm.run_collect(chunk).map_err(|e| {
        anyhow::anyhow!(
            "{}\n{}",
            e.message,
            e.stack.iter().map(|f| format!("  at {}", f.fn_name)).collect::<Vec<_>>().join("\n")
        )
    })?;

    Ok(out.into_iter().map(|v| v.to_string()).collect())
}
```
