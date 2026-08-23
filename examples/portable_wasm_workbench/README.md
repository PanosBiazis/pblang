# Portable WASM Workbench

A native PBLang migration and expansion of a small C `add(a, b)` WebAssembly
exercise. The example generates a real `.wasm` module directly from PBLang,
checks its bounded binary envelope and exports, and cross-checks the arithmetic
with the independent bounded WASM instruction model.

## Improvements

- no C source, Emscripten, generated JavaScript glue or Python web server;
- signed 32-bit operand and overflow validation before code generation;
- bounded source, artifact, section and export work;
- WebAssembly magic, version, section-envelope and export inspection;
- content digest and explicit `main` function-export verification;
- deterministic arithmetic cross-check through `emulator_wasm`;
- one responsive Modern UI tree for Web, native desktop and Android records;
- no network, filesystem or foreign-compiler access during normal execution;
- malformed-module, duplicate-section, truncation and budget regression tests.

Envelope validation is not a complete WebAssembly semantic validator. The
repository's `codegen_wasm_test.sapp` additionally executes generated modules
in an independent production WebAssembly engine when Node is available. This
example does not claim that a headless UI record is a packaged browser, desktop
or Android application.

## Run

```sh
pblang run examples/portable_wasm_workbench/main.sapp
```

## Verify

```sh
pblang run examples/portable_wasm_workbench/test.sapp --allow-all
pblang run tests/semantics/wasm_module_test.sapp
pblang lint examples/portable_wasm_workbench/wasm_workbench.sapp
```

The retained bootstrap currently resolves a published local-dependency lock
path relative to the process working directory during nested imports, rather
than consistently relative to the lockfile. The generated package lock is
therefore intentionally not retained here: an absolute lock would expose a
machine path, while the portable manifest-relative lock fails the current
runtime. Repository-level execution remains verified; fixing that bootstrap
resolver inconsistency is a separate package-manager slice.
