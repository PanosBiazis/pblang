# Rust to pblang Migration Plan (Incremental)

This project should migrate from Rust to pblang gradually, with hard safety gates.

## Principles

- Keep Rust for critical runtime safety/performance until parity is proven.
- Move high-level tooling and orchestration to pblang first.
- Every migration step must add/keep automated tests.
- No silent regressions in determinism, diagnostics, or resource safety.

## Migration Tracks

1. Frontend tooling in pblang (tokenize/parse/type diagnostics helpers).
2. Build/package orchestration in pblang.
3. CLI command logic in pblang (with Rust as host runtime).
4. Selected compiler pipeline logic in pblang.
5. Long-term: deeper self-hosting components.

## Exit Gates Per Step

- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- Targeted semantics/integration tests pass.
- New behavior has deterministic output for same input.
- Failure mode is structured (no panic-only pathways).

## Started Now

- [x] Introduced pblang-owned frontend driver:
  - `stdlib/compiler/selfhost_frontend.sapp`
  - Exposes `frontend_analyze`, `frontend_parse_file`, `frontend_smoke`.
- [x] Added migration regression test:
  - `tests/semantics/selfhost_frontend_test.sapp`
  - `tests/semantics.rs::selfhost_frontend_pass`

## Next Recommended Step

- Add a pblang-owned package-manifest validator and use it from CLI paths,
  while preserving current Rust implementation as fallback until parity.
