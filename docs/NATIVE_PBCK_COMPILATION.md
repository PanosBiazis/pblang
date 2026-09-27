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
