# Changelog

All notable changes to **pblang** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.7] — 2026

### Added
- `pblang_ir::serialize_chunk_pbck()` — single canonical PBCK bytecode serializer shared by CLI and Cranelift.
- Semantics tests: `thread_pool_module_test.sapp`, `build_stdlib_test.sapp`.
- Fixed `stdlib/build.sapp`: valid `export` syntax and `host_platform()` via `stdlib:os.sapp` (was invalid `export fn` and undefined `os()`).
- `[workspace.dependencies]` in root `Cargo.toml` for shared crate versions.

### Changed
- Workspace crate versions aligned to **0.5.7** (was 0.5.0 in manifests while docs referenced 0.5.6+).
- Removed ~180 lines of duplicate PBCK serialization from `src/main.rs` and `pblang-cranelift`.

### Added (accumulated)
- `tests/semantics/memory_stdlib_test.sapp` — covers `stdlib/memory.sapp`
  convenience API (`memory.box`, `read`, `write`, `borrow`, `release`,
  `free`, `stats`).
- `CHANGELOG.md` for tracking changes across releases.
- Self-hosting compiler modules: `stdlib/compiler/{ir,optimizer,bootstrap}.sapp`.
- Memory safety stdlib wrapper: `stdlib/memory.sapp` exposing the runtime
  `memory_*` builtins behind a Rust-flavoured API.
- `stdlib/thread_pool.sapp` — enhanced thread-pool helpers.
- `stdlib/registry.sapp` — high-level package-registry operations.

### Changed
- `crates/pblang-vm/src/concurrency.rs` `VmThreadSupervisor` now polls the
  join handle until completion **or** the deadline passes, so `timed_out`
  statistics reflect real timeouts instead of blocking forever on `join()`.
  Added `Default` impl and removed unused struct fields.
- `src/main.rs` hoists the Cranelift AOT helpers
  (`bincode_serialize_chunk`, `generate_c_stub_with_bytecode`) to a single
  module-level definition. The previous four duplicated copies have been
  removed.
- `stdlib/memory.sapp` documentation now reflects the actual runtime
  behaviour: prefer the top-level `memory.box/read/write/...` helpers
  because method-style dispatch does not auto-bind `self`.
- `STRUCTURE.md` rewritten to cover all 16 workspace crates and the full
  stdlib layout, including self-hosting compiler modules.
- `README.md` feature list now lists runtime memory safety, concurrency
  primitives, async/await, multi-backend codegen, AI/quantum integration,
  TLS, native binaries, and self-hosting.
- `docs/CURRENT_STATUS.md` and `docs/Tasks.md` test counts updated.

### Fixed
- Removed `unreachable_patterns` warnings in
  `src/main.rs` and `crates/pblang-cranelift/src/lib.rs` (the `Obj` enum
  has a single variant, so a fall-through arm is dead).
- Removed `unused_variables` warnings (`_arity`, `_use_i64`, `_obj_path`).
- Marked `pblang_cranelift::serialize_chunk_to_vec` with `#[allow(dead_code)]`
  + doc comment until a public caller is wired up.

## [0.5.6] — 2026

### Added
- **AI integration** (`pblang-ai` crate, `ai_chat`, `ai_embed`,
  `ai_token_count`, provider auto-registration, streaming, RAG helpers).
- **Self-hosting type checker** completes the 4-stage pipeline.
- **Bit-level optimizer**: string constant folding, peephole, jump
  threading, native bitwise opcodes.
- **New stdlib modules**: `bitops`, `compression`, `serialization`,
  `machine_learning`, `blockchain`, `networking_advanced`,
  `data_structures`, `image_processing`, `algorithms_advanced`.
- **LSP**: goto-definition, hover with 40+ builtin signatures, completions.

## [0.5.2] — 2026

### Added
- Concurrency primitives: `SafeMutex`, `SafeRwLock`, `SafeChannel`,
  `ThreadPool`, `Barrier`, `Once`, `Timer`, `WaitGroup`, `Semaphore`,
  `CircuitBreaker`, `RateLimiter`, `FanOut`/`FanIn`.

## [0.5.1] — 2026 — Security Hardening

### Fixed
- 🔴 **Critical**: TLS `DangerousAllowAllVerifier` replaced with
  webpki-roots + rustls-native-certs.
- 🟠 **High**: Path traversal in `static_file_response()` (`_path_safe`
  strips `../`, URL-decodes, returns 403).
- 🟠 **High**: `fs.sapp` now blocks absolute paths and `../` by default;
  opt-in `*_abs()` variants.
- 🟡 **Medium**: Command injection in `os.which()` — replaced with safe
  PATH search builtin.
- 🟡 **Medium**: URL query parameters now URL-decoded.
- 🟡 **Medium**: Full RFC 7231 HTTP status code table.
- 🟡 **Medium**: `NonNull` (`T!`) type preserved in typechecker.

## [0.5.0]

Initial public release.

### Highlights
- Stack-based bytecode VM with RC + cycle-breaking GC.
- Capability-based sandboxing (`VmLimits`, `FfiPolicy`, `StdlibConfig`).
- Package manager with semver, lockfiles, SHA256 verification.
- 50+ stdlib modules.
- VM-per-thread concurrency with typed channels and `select`.
- Polyglot blocks (`@python`, `@sql`, `@rust`).
- FFI to C libraries with allowlist security.
- LSP server, REPL, full CLI toolchain.
