# Fuzzing pblang

A `cargo-fuzz` harness lives under `fuzz/`. The fuzz crate is excluded
from the main workspace (`Cargo.toml [workspace] exclude = ["fuzz"]`),
so `cargo build --workspace` and `cargo test --workspace` ignore it —
the targets only build under `cargo +nightly fuzz`.

## Why fuzz?

The chumsky 1.0-alpha parser is the biggest single piece of recursive
grammar in the codebase (~2,300 lines in `crates/pblang-core/src/parser.rs`).
Hand-written test cases catch the bugs you remember; libFuzzer catches
the ones you didn't think of — deeply nested constructs, byte sequences
that mask UTF-8 boundary code paths, oversized integer literals,
malformed escape sequences inside string-interpolation blocks, etc.

The four fuzz targets cover every external-input decoder that ships in
the toolchain:

| Target            | Entry point                          | Used by                                |
|-------------------|--------------------------------------|----------------------------------------|
| `parse`           | `pblang_core::parse`                 | LSP / REPL / `pblang analyze`          |
| `parse_strict`    | `pblang_core::parse_strict`          | `pblang run` / `compile` / `lint`      |
| `manifest`        | `pblang_pm::parse_manifest`          | every PM command (`build`, `install`…) |
| `registry_index`  | `pblang_pm::parse_registry_index`    | `pblang install` (HTTP registry fetch) |

`parse` / `parse_strict` / `manifest` accept `&str`; libFuzzer's byte
stream is filtered to valid UTF-8 before being fed in. `registry_index`
takes `&[u8]` directly — that mirrors the HTTP path. In all four
targets, either we get an `Ok(...)` value or an `Err(...)` / parse-error
vec; a host crash / forced unwrap / arithmetic overflow is a fuzzing
finding.

## One-time setup

```bash
# Install cargo-fuzz (uses cargo, not rustup):
cargo install cargo-fuzz

# Make sure the nightly toolchain is installed (libFuzzer linkage
# requires nightly):
rustup install nightly
```

## Running

From the repo root:

```bash
# Default infinite fuzz loop on the recovery parser. Stop with Ctrl-C.
cargo +nightly fuzz run parse

# 60-second smoke run (good for CI):
cargo +nightly fuzz run parse -- -max_total_time=60

# Drive the strict parser instead:
cargo +nightly fuzz run parse_strict -- -max_total_time=60

# Manifest TOML decoder:
cargo +nightly fuzz run manifest -- -max_total_time=60

# Registry-index JSON decoder (already size-capped, but malformed inner
# JSON should still degrade to an Err, not a panic):
cargo +nightly fuzz run registry_index -- -max_total_time=60
```

Per-input artifacts (saved interesting / crashing inputs) land in:

```
fuzz/corpus/<target>/      # successfully-explored inputs
fuzz/artifacts/<target>/   # crashes, hangs, leaks
```

Both directories are gitignored. If a real crash is found, copy the
offending file out of `artifacts/` and add it to `tests/semantics/` (or
wherever's appropriate) as a regression test before fixing the bug.

## Reproducing a crash

```bash
cargo +nightly fuzz run parse fuzz/artifacts/parse/crash-<hash>
```

## Coverage report (optional)

```bash
cargo +nightly fuzz coverage parse
cargo +nightly cov -- show \
    target/x86_64-unknown-linux-gnu/coverage/x86_64-unknown-linux-gnu/release/parse \
    --instr-profile=fuzz/coverage/parse/coverage.profdata \
    --format=html > parse-coverage.html
```

## Adding a new target

1. Create `fuzz/fuzz_targets/<name>.rs` with the `#![no_main]` +
   `fuzz_target!` boilerplate (copy `parse.rs` as a template).
2. Add a `[[bin]]` block to `fuzz/Cargo.toml` pointing at the new file.
3. Drive the function you want to harden — typically a public entry
   point that takes untrusted bytes. Examples worth adding when the
   need arises:
   - `pblang_pm::parse_lockfile` — `pblang.lock` TOML parser (lower
     priority than `parse_manifest` because lockfiles are tool-generated)
   - VM bytecode dispatch — would need a fuzz-friendly `Chunk` builder
     first; not worth doing until/unless a bytecode corpus exists.

## CI

The harness is checked in but **not** wired into CI. Reason: a 60s
libFuzzer run costs ~1 CPU-minute per target per push, which doesn't
justify itself for a project with this commit volume. A periodic
(nightly / weekly) CI job that runs each target for ~30 minutes would
be the right intermediate; that's deferred.
