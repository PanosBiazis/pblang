# PBLang compilation for the retained native VM

`tools/pbc.sapp --target pbck` compiles a single source entry with the PBLang
parser, type checker and code generator, then translates its chunk with
`stdlib/selfhost_to_pbck.sapp`. The result starts with `PBCK`, not `PBCS`.
Run it with the native `pblang run`; `tools/pbrun.sapp` is the PBCS interpreter
and must not be used for this artifact.

Use the verified 0.6.0 project executable; an older executable earlier on PATH
may use an incompatible lock hashing scheme. In the source checkout:

```sh
./target/release/pblang run tools/pbc.sapp --allow-all -- examples/native_pipeline_method.sapp --target pbck --out pipeline.pbck
./target/release/pblang run pipeline.pbck --allow-all
```

The example uses trusted local imports and workers, hence the broad development
permission shown above. Grant only the capabilities needed by deployed programs.
A pure arithmetic/method program can run with `--sandbox`. The retained CLI
otherwise enables filesystem and several other capabilities by default; omission
of `--allow-all` alone is not a deny-all policy. `--sandbox` is a runtime capability
profile, not OS-level isolation. Compilation does
not grant capabilities to subsequent execution. The compiler does not run the
produced program automatically.

The library entry is `compiler/native_pbck.sapp::compile_source(source)`, returning
bytes. It uses the existing strict source admission (1 MiB UTF-8, 65,536 tokens),
rejects lexical/parser/type diagnostics and empty input, and translates only after
these checks. The CLI's existing whole-file read occurs before admission; these
are compiler work limits, not an OS memory sandbox. Code-generation/translation
errors propagate. Existing output is untouched on validation or compilation
failure. Output publication uses the existing file write boundary; it does not
promise atomic replacement, symlink protection, or recovery from a partial I/O
failure. Use a trusted output directory.

The CLI requires a separate `.pbck` output filename (default: source basename
with `.pbck`). `--safe`, `--overflow`, and `--local-source` are rejected for this
target rather than silently implying unsupported guarantees.

## Explicit entry cleanup

For a program that owns resources across calls, compile with an explicit entry
and cleanup function:

```sh
./target/release/pblang run tools/pbc.sapp --allow-all -- examples/native_cleanup.sapp --target pbck --entry work --cleanup release --out cleanup.pbck
./target/release/pblang run cleanup.pbck --allow-all
PBLANG_CLEANUP_DEMO_FAIL=1 ./target/release/pblang run cleanup.pbck --allow-all
```

The last invocation deliberately fails after allocating a managed VM resource.
It prints `resources released` and exits unsuccessfully with the original indexing
error. Remove the generated artifact when finished.

The library equivalent is `compile_source_with_cleanup(source, entry, cleanup)`.
Both names must identify distinct, explicit top-level functions with no arguments.
The compiler validates the whole source before publication. Source initialization
runs first; then the cleanup callback is captured before invoking the entry.
Reassigning its global name inside the entry does not replace that callback.

Cleanup runs once after a successful result or a catchable native runtime error,
including errors raised by imported code. If only cleanup fails, its error is
reported. If both fail, the entry's original **error message** wins. The native
boundary does not preserve a typed exception payload or the original stack trace.
The successful entry result is preserved by the bytecode boundary; the CLI's
appended entry expression discards it. No capability is granted by this option.

This is explicit resource ownership, not automatic cleanup of every program.
There is no cleanup guarantee for initialization failures before the boundary,
process termination, native FFI crashes, unavailable memory preventing cleanup,
or a cleanup callback that never returns. It does not install cancellation or
cycle collection, and does not change direct retained-source execution or other
backends. Ordinary PBCK compilation is unchanged. The low-level
`translate_guarded(chunk, index)` accepts only the canonical two-callback shape;
it is intended for trusted compiler-produced chunks, not untrusted bytecode.

`native_pbck_cleanup_test.sapp` covers successful false/nil/container results,
original versus secondary failures, callback replacement, denied capabilities,
invalid declarations and actual VM resource release after a host indexing error.

## Method-call correction

A colon call first checks the optional `__class` property and then looks up the
method on the class or receiver. The self-hosted VM returns nil for absent class
metadata, while native attribute lookup throws. The native translator now catches
only this reserved lookup and supplies nil without retaining an extra receiver
reference. Recovery code is emitted after the normal frame body, so successful
lookups do not jump over it on every call. Branch lowering also removes an
unnecessary jump on fallthrough while consuming the same stack values on both
paths. Native module exports remain supported; replacing lookup with a map-only
optional-access instruction would break them. It preserves class dispatch, single
receiver evaluation, argument order,
and propagation of method/argument errors. Nil class metadata falls back to the
receiver; false or other invalid class metadata is not treated as absent.

This makes the historical loop/try example produce `7 / 7 / 7` after PBLang
compilation, and lets the provided nested `thread.pipeline` example run its real
workers. Direct source execution through the retained compiler still has the
old implicit-closure capture bug. This is a working compiler route, not a patch
to that Rust compiler.

## Limits and checks

- This is a native-VM artifact, **not a standalone native executable**.
- Runtime imports remain imports, using the retained resolver and the artifact's
  project context. Keep the artifact in a project with its manifest and stdlib;
  copying it alone to a temporary directory does not make it self-contained.
- Imported source still uses the retained loader/compiler. This option does not
  migrate every dependency or installed product automatically.
- Existing self-hosted subset limitations remain, including defer support and
  backend differences for ordinary missing attributes. Runtime/FFI cleanup,
  cycle collection and all-platform parity are separate unfinished work.
- There are no new grants, background services, telemetry, or persisted source
  logs. Bytecode includes program constants and is not source-code encryption.

`native_pbck_method_test.sapp` executes plain, optimized and PBCS-roundtripped
translations on the actual native VM, including nested methods, classes, native
modules with and without exported class metadata, invalid metadata, single
evaluation, argument order, thrown errors and real pipeline workers. `native_pbck_cli_live_test.sapp` verifies option/diagnostic failures,
output preservation, execution and capability denial. These live tests explicitly
skip non-Linux systems; they are not proof of Windows/macOS compatibility.

## Automatic guest-index failure cleanup

The PBLang interpreter (`compiler/vm.sapp`) now closes its private scalar lifetime
state when an uncaught guest index read/write fails and that context has
`close_on_throw` enabled. Internally owned runs already enable this policy.
Explicitly borrowed contexts retain their existing default of caller-owned cleanup.
No `--entry`/`--cleanup` option is required for this automatic path.

Covered operations are guest index reads, index assignments, attribute assignments
lowered to index writes, and map-key construction. This does not change successful
index semantics, numeric truncation, Unicode indexing, frozen-map rejection, growth
limits, or guest catch behavior. Caught failures leave the execution's resources
available to the handler. Only an escaping failure closes the complete context.

On the retained host these primitives invoke no guest callbacks and their reported
errors occur before writes. The cold failure path repeats only the failed primitive
on its already-evaluated operands after cleanup; it never reruns an operand expression
or the guest program. This preserves the host's error message, including errors that
PBLang catch cannot bind. Cleanup invokes no guest callbacks and its state must remain
private; embedding code must not alias an execution context into guest objects.
Native backend changes adding observable index hooks would require revalidating this
assumption. Tests compare the original messages against the prior implementation.

This is not a universal host-error handler, general cycle collection, or cleanup of
native FFI crashes. It does not cover every malformed raw chunk, builtin error,
process exit, OOM or hardware failure. See `examples/index_cleanup.sapp` and
`tests/semantics/selfhost_index_cleanup_test.sapp` for the tested ownership rules.

The index success/handler path is emitted inline in the interpreter dispatch to
avoid an additional helper frame on each managed access. This preserves the
same ownership policy and failed-primitive assumptions. Changing receiver/key
loops and allocation after frame entry are regression-tested. For the wider
unfinished work, see [continuation status](CONTINUATION_STATUS_2026-09-27.md).

## Explicit static dependency bundles

`compiler/native_bundle.sapp::compile_bundle(source, resolve)` compiles the entry
and its transitive static imports in PBLang and returns native PBCK bytes.
`resolve(canonical_path)` must return trusted source text or nil. This API does
not read dependencies, execute the artifact or grant capabilities. The existing
`selfhost_bundle.bundle`, filesystem writers and `dist` bytecode builds use the
same corrected linker. Ordinary `compile_source` and `pbc --target pbck` retain
runtime imports; they do not opt into this profile automatically.

```sh
./target/release/pblang run examples/native_static_bundle.sapp --allow-all -- bundle.pbck
./target/release/pblang run bundle.pbck --sandbox
```

The example compiles a virtual module containing the nested loop/try `p:run`
case. The artifact prints `7` twice with no source dependency on disk. This
extends the verified PBLang compiler route to supported imported modules; it
**does not repair the retained Rust source compiler** or migrate every product.

Import discovery examines compiler instructions, including decoded literal
paths, rather than searching for one source spelling. Whitespace/comments between
`import`, parentheses and the literal are supported. `stdlib:` and `stdlib/`
canonicalize to one dependency. Computed paths, first-class import aliases,
imports inside functions/try chunks, cycles, invalid paths and unresolved modules
fail closed. Invalid entry source is rejected before invoking the resolver.
The `__bundle_` global prefix and rebinding `import` are reserved. Rejection
reasons exclude source diagnostics and dependency paths. The native bridge
raises a fixed error rather than silently reverting to the retained resolver.

Resource admission: 64 distinct modules, dependency depth 32, at most 16 MiB
UTF-8/1,048,576 tokens in the main translation unit, 1 MiB/65,536 tokens per
module and 20 MiB aggregate source. The larger main limit preserves the existing
`dist.bundle_pbck_blob` linked-source profile. Paths are limited to 4,096 Unicode
scalars and cannot contain NUL. These are compiler-work limits, not a process
memory/time sandbox: resolver work, whole-file reads, already allocated strings,
native allocations and caller concurrency require their own boundaries.

**Static linking is a separate execution profile.** Modules initialize in
`dependencies first` order before main, not at each runtime import expression.
Globals occupy a shared namespace and collisions are rejected; exported maps
are snapshots, not live module bindings or an isolation boundary for mutually
untrusted source. Existing export/closure/backend limitations remain. Do not use
this profile for applications that depend on lazy/conditional initialization or
runtime module mutation. The filesystem convenience resolver is for trusted
project paths; the in-memory resolver is preferable for exact source snapshots.
No CLI default, filesystem authority, telemetry or desktop-control approval was
changed. Native live tests currently qualify Linux only.

### Compile a trusted project from the CLI

```sh
./target/release/pblang run tools/pbc.sapp --allow-all -- examples/static_project/main.sapp --target pbck --bundle --out static-project.pbck
./target/release/pblang run static-project.pbck --sandbox
```

The artifact prints `7`. `--bundle` is opt-in and uses
`compiler/native_bundle.compile_file(path)`. Imports are resolved against the
compiler's **working directory**, not relative to the importing file. Run from
the project root. This profile accepts trusted local project paths and reads
whole files before the documented linker admission limits. It is not an import
allowlist, a filesystem sandbox, or a guarantee of one consistent filesystem
snapshot during concurrent edits. Use `compile_bundle(source, resolve)` with an
immutable in-memory source set when that guarantee is required.

The CLI rejects duplicate `--bundle`, other targets and unpaired
`--entry`/`--cleanup` before reading the entry. Paired callbacks are supported
with static bundles as described below. Ordinary
PBCK compilation is unchanged and does not load the optional linker. Every
compilation/link check completes before writing output. The existing trusted
output-directory and non-atomic-write limitations still apply.

`native_bundle_cli_live_test.sapp` compiles transitive imports, deletes all source
files, and executes the artifact under the native VM sandbox. It also checks
option failures before input reads, output preservation, private diagnostic
canaries and denied filesystem access. This is Linux evidence, not a portable
platform certification or a repair of direct retained-source compilation.


### Cleanup across a static dependency bundle

```sh
./target/release/pblang run tools/pbc.sapp --allow-all -- examples/static_project/cleanup.sapp --target pbck --bundle --entry work --cleanup release --out cleanup-bundle.pbck
./target/release/pblang run cleanup-bundle.pbck --allow-all
PBLANG_BUNDLE_DEMO_FAIL=1 ./target/release/pblang run cleanup-bundle.pbck --allow-all
```

The successful example prints `7` and `resources released`. The failure example
prints `resources released`, exits unsuccessfully and retains the imported
indexing error. Its owned buffer is cleared by an explicit callback. The example
is not a general collector or an FFI lifetime implementation.

Library entries are `native_bundle.compile_bundle_with_cleanup(source, resolve,
entry, cleanup)` and `compile_file_with_cleanup(path, entry, cleanup)`. The lower
linker exposes `bundle_with_cleanup` and `bundle_file_with_cleanup`; these return
ordinary linked IR. Only the native bridge's guarded translation installs the
cleanup behavior. Passing that IR to ordinary translation or the self-hosted VM
does **not** install a finally boundary. `native_pbck.checked_cleanup_chunk` is a
compiler integration helper returning validated IR, not an execution API.

Callbacks must be distinct zero-argument top-level functions declared in the
entry source. A dependency cannot supply missing callback declarations. Main
validation occurs before dependency resolver callbacks. Main is compiled once,
then its instructions are relocated after the dependencies and the final guarded
instruction is checked again before translation. The original error **message**
is retained if entry and cleanup both fail; typed exception/stack preservation is
not promised. Cleanup is captured before the entry call and runs once without
replaying the failed operation. Runtime capabilities remain separate.

Guarded main source, **including the generated callback expression**, has the
same 1 MiB UTF-8 / 65,536-token budget as ordinary explicit-cleanup compilation.
This is intentionally narrower than the unguarded static main limit. Existing
module-count, depth, per-module and aggregate limits remain. The filesystem
convenience call still reads whole files before compiler admission.

Module and entry initialization precede the boundary. Initializer errors,
process termination, native FFI crashes, cleanup that cannot complete, and
resource exhaustion preventing cleanup remain outside the guarantee. Shared
globals and export snapshots remain properties of this explicit static profile.
`native_bundle_cleanup_test.sapp` and `native_bundle_cli_live_test.sapp` cover
transitive imports, caught and escaping failures, callback reassignment, actual
file cleanup, source deletion, denied capabilities and invalid callback contracts.
