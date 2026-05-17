# pblang Project Manual

This manual is the practical "how to use pblang end-to-end" reference for project owners, contributors, and users.

## 1. What pblang is

pblang is a scripting/runtime language and tooling platform with:

- language runtime + bytecode VM
- standard library modules (web, threads, crypto, data, quantum helpers, etc.)
- package/lockfile workflow
- LSP and IDE backend support
- bundling and distribution tooling

Use this manual for workflows. Use `docs/Language_Semantics.md` for exact language behavior and `docs/CURRENT_STATUS.md` for maturity status.

## 2. Build and run

Prerequisites:

- Rust stable toolchain
- repository checkout

Build:

```bash
cargo build
```

Run a script:

```bash
cargo run -- run path/to/file.sapp
```

Run a project:

```bash
cargo run -- run .
```

## 3. Core CLI commands

- `pblang run <file|dir>`: run script or project entry
- `pblang --check <file>`: parse/type check only
- `pblang fmt [files...]`: format source
- `pblang lint [files...]`: diagnostics-oriented checks
- `pblang lsp`: start language server on stdio
- `pblang serve [--port N]`: start local IDE backend
- `pblang --doc <file> [--html] [out_dir]`: docs generation
- `pblang --lock [path]`: generate lockfile
- `pblang install [path | <name>@<version>]`: resolve/install dependencies
- `pblang search <query>`: search registry
- `pblang publish`: publish package
- `pblang pack [path] [out_file]`: tarball package
- `pblang bundle [path] [out_dir]`: app bundle

For complete options and flags see `docs/Tooling.md`.

## 4. Project layout and manifest

Typical layout:

```text
my-app/
  pblang.toml
  src/main.sapp
```

Initialize:

```bash
pblang init
```

`pblang.toml` controls package metadata and dependencies. Lockfile (`pblang.lock`) pins resolved dependency state.

## 5. Dependency workflow (registry + lockfile)

Recommended flow:

1. Set registry index if using hosted/file registry:
   - `PBLANG_REGISTRY_INDEX=<url-or-file://...>`
2. Run install/lock:
   - `pblang install .` or `pblang --lock .`
3. Commit both `pblang.toml` and `pblang.lock`.

For checksum/rehydration contracts, read `docs/Lockfile_contract.md`.

## 6. Testing workflow

Language/runtime integration tests:

```bash
cargo test --test semantics
```

Focused areas:

```bash
cargo test --test lsp
cargo test --test new_cmd
cargo test --test pack
```

Fast local loop for edited `.sapp` modules:

```bash
pblang fmt path/to/file.sapp
pblang --check path/to/file.sapp
pblang run path/to/file.sapp
```

## 7. Packaging and distribution

Pack tarball:

```bash
pblang pack .
```

Bundle runnable layout:

```bash
pblang bundle .
```

Self-extracting bundle flow:

```bash
pblang bundle --self-extract .
```

Native/backend notes: `docs/Native_backend_notes.md`.

## 8. IDE and developer tooling

- IDE backend: `pblang serve --port 8090`
- Browser IDE: open `editors/ide/index.html`
- LSP integration in editors: `pblang lsp`

Read:

- `docs/IDE_MANUAL.md`
- `docs/lsp.md`
- `docs/Tooling.md`

## 9. Security and safety model

pblang supports capability-based restrictions in embedding/runtime configurations. Important references:

- `docs/Safety.md`
- `docs/Embedding.md`
- `docs/Crash_resistance.md`

For untrusted code, disable unnecessary capabilities (fs/net/http/exec/ffi) and enforce limits.

## 10. Operational troubleshooting

Common issues:

- **Backend unreachable in IDE**
  - start with `cargo run -- serve --port 8090`
- **Registry install errors**
  - verify `PBLANG_REGISTRY_INDEX`, token env vars, and lockfile state
- **Type/parse failures**
  - run `pblang --check file.sapp` and inspect diagnostics
- **Behavior mismatch with expectations**
  - check `docs/Language_Semantics.md` and `docs/CURRENT_STATUS.md`

## 11. Contributor workflow (recommended)

1. Pick one scoped area (runtime, stdlib module, IDE, docs).
2. Implement minimal coherent feature.
3. Add or update tests.
4. Update docs (`docs/Tasks.md`, `docs/Tooling.md`, relevant manual).
5. Run targeted tests before broad suites.

This keeps changes reviewable and stable.
