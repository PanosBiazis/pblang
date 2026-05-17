# pblang – project context for Perplexity

**How to use:** Copy this file (and any code snippets you need) and paste them into [Perplexity](https://www.perplexity.ai) when asking questions. No API key required.

---

## What this project is

**pblang** is a programming language implementation written in Rust. It is a workspace with multiple crates: parser, IR, VM, compiler, type checker, package manager, LSP, FFI, and graphics.

## Workspace layout

- **pblang-core** – Parser, lexer, core language/ast
- **pblang-ir** – Intermediate representation
- **pblang-vm** – Bytecode VM (chunk, VM execution)
- **pblang-compiler** – Compilation pipeline
- **pblang-typeck** – Type checking
- **pblang-ffi** – FFI layer
- **pblang-pm** – Package manager
- **pblang-lsp** – Language server (tower-lsp)
- **pblang-graphics** – Graphics (SDL2 / minifb), `.sapp` examples

Other crates include backend/integration crates such as `pblang-ai`, `pblang-llvm`, `pblang-wasm`, `pblang-cranelift`, and `pblang-ctranspiler`.

Root `Cargo.toml` depends on workspace crates including `pblang-core`, `pblang-vm`, `pblang-compiler`, `pblang-typeck`, `pblang-pm`, `pblang-graphics`, and backend crates. Entrypoint is `src/main.rs`.

## Config

- **pblang.toml** – Workspace/package config (e.g. `stdlib = { path = "stdlib" }`).
- **stdlib/** – Standard library (path dependency in `pblang.toml`).

## Tests (root)

- `tests/parser_ast.rs`, `smoke.rs`, `vm_smoke.rs`, `red_features.rs`, `print_collect.rs`, `block_expr.rs`, `if_else.rs`, `semantics.rs`, `modules.rs`, `http_server.rs`, `sandbox.rs`, `golden.rs`, `lockfile_repro.rs`, `stdlib_parse.rs`, `fmt.rs`, `pack.rs`, `install_cli.rs`, `threads.rs`, `dump_bytecode.rs`, `doc_cli.rs`

Run: `cargo test`. See [README.md](README.md) for command summary and doc links.

## Key source files

- Parser/lexer: `crates/pblang-core/src/parser.rs`, `lexer.rs`
- VM: `crates/pblang-vm/src/vm.rs`, `chunk.rs`
- Compiler: `crates/pblang-compiler/src/lib.rs`
- Type checker: `crates/pblang-typeck/src/lib.rs`
- LSP: `crates/pblang-lsp/src/main.rs`

When asking about a specific area, paste the relevant file(s) below this context in the same Perplexity message.
