# Biapryx

A scripting language implemented in Rust: parser, IR, bytecode VM, type checker, package manager, LSP, FFI, and graphics. Suitable for embedded scripting and standalone use, with a growing stdlib and tooling.

## Quick start

```bash
cargo build --release
./target/release/biapryx run path/to/script.biapryx
```

From a package directory (with `biapryx.toml`):

```bash
biapryx install    # resolve dependencies, generate biapryx.lock
biapryx run        # run package entry (main.biapryx or [package].entry)
biapryx test       # run tests/*.biapryx and tests/*.biapryx
```

## Workspace layout

| Crate | Role |
|-------|------|
| **biapryx-core** | Parser, lexer, AST |
| **biapryx-ir** | Intermediate representation |
| **biapryx-vm** | Bytecode VM (chunk, execution, builtins) |
| **biapryx-compiler** | Compilation pipeline |
| **biapryx-typeck** | Type checking |
| **biapryx-ffi** | FFI layer |
| **biapryx-pm** | Package manager (lockfile, registry, git deps) |
| **biapryx-lsp** | Language server (tower-lsp) |
| **biapryx-graphics** | Graphics (SDL2 / minifb), `.biapryx` examples |

Root `src/main.rs` is the CLI entrypoint. Config: **biapryx.toml** (workspace/package), **stdlib/** as path dependency.

## Commands (summary)

- `biapryx run [file|dir]` — Execute script or package
- `biapryx init` — Create biapryx.toml
- `biapryx test [dir]` — Run test files
- `biapryx build` — Lockfile + typecheck package entry
- `biapryx fmt` — Format source (indent, operator/comma spacing, optional line length)
- `biapryx lint` — Parse + typecheck
- `biapryx --doc [--html]` — Module docs (exports, polyglot blocks)
- `biapryx --lock` / `install` / `pack` / `bundle` / `publish` — Package and registry workflows
- `biapryx --dump-bytecode <file>` — Bytecode disassembly

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
- [docs/Help.md](docs/Help.md) — Active backlog and task cards
- [docs/Future_plans.md](docs/Future_plans.md) — Roadmap and vision

For feeding context into external tools (e.g. Perplexity), see [PERPLEXITY_CONTEXT.md](PERPLEXITY_CONTEXT.md).
