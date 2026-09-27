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
