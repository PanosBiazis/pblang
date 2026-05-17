# pblang (legacy snapshot)

Legacy quick context for pblang. This file is kept for historical reference; use the top-level `README.md` for the canonical, up-to-date guide.

## Quick start

```bash
cargo build --release
./target/release/pblang run path/to/script.sapp
```

From a package directory (with `pblang.toml`):

```bash
pblang install    # resolve dependencies, generate pblang.lock
pblang run        # run package entry (main.sapp or [package].entry)
pblang test       # run tests/*.sap and tests/*.sapp
```

## Workspace layout

| Crate | Role |
|-------|------|
| **pblang-core** | Parser, lexer, AST |
| **pblang-ir** | Intermediate representation |
| **pblang-vm** | Bytecode VM (chunk, execution, builtins) |
| **pblang-compiler** | Compilation pipeline |
| **pblang-typeck** | Type checking |
| **pblang-ffi** | FFI layer |
| **pblang-pm** | Package manager (lockfile, registry, git deps) |
| **pblang-lsp** | Language server (tower-lsp) |
| **pblang-graphics** | Graphics (SDL2 / minifb), `.sapp` examples |

Root `src/main.rs` is the CLI entrypoint. Config: **pblang.toml** (workspace/package), **stdlib/** as path dependency.

## Commands (summary)

- `pblang run [file|dir]` — Execute script or package
- `pblang init` — Create `pblang.toml`
- `pblang test [dir]` — Run test files
- `pblang build` — Lockfile + typecheck package entry
- `pblang fmt` — Format source
- `pblang lint` — Parse + typecheck
- `pblang --doc [--html]` — Module docs (exports, polyglot blocks)
- `pblang --lock` / `install` / `pack` / `bundle` / `publish` — Package and registry workflows
- `pblang --dump-bytecode <file>` — Bytecode disassembly

## Tests

```bash
cargo test --all
cargo test --all --release
cargo test
```

Notable suites: `smoke`, `semantics`, `golden`, `sandbox`, `threads`, `modules`, `lockfile_repro`, `pack`, `install_cli`, `fmt`, `dump_bytecode`, `doc_cli`.

## Documentation

- [docs/Tooling.md](docs/Tooling.md) — CLI, stdlib APIs, registry, bytecode
- [docs/Language_Semantics.md](docs/Language_Semantics.md) — Syntax, semantics, errors
- [docs/Embedding.md](docs/Embedding.md) — Embedding API (VmHost, capabilities, limits)
- [docs/Report.md](docs/Report.md) — Progress report and history
- [docs/Help.md](docs/Help.md) — operator guidance
- [docs/Tasks.md](docs/Tasks.md) — active backlog

For feeding context into external tools (e.g. Perplexity), see [PERPLEXITY_CONTEXT.md](PERPLEXITY_CONTEXT.md).
