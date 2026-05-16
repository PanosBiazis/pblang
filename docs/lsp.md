## pblang LSP (language server)

This workspace includes the `pblang-lsp` server (tower-lsp) used by `pblang lsp`.

Current capabilities:

- parse diagnostics (syntax errors)
- typecheck diagnostics (warnings) from `pblang-typeck`
- completion (`textDocument/completion`)
- hover (`textDocument/hover`)
- go-to-definition (`textDocument/definition`)
- workspace symbols (`workspace/symbol`)
- semantic tokens (`textDocument/semanticTokens/full`)

### Build

```bash
cargo build -p pblang-lsp
```

### Run (stdio)

Most editors start LSP servers over stdio. The binary is:

- `target/debug/pblang-lsp` (after build)

### VS Code / Cursor setup (example)

Use any LSP client extension that lets you define a custom language server command
for `*.sapp` files, and set the command to:

```bash
<repo>/target/debug/pblang-lsp
```

The server uses `textDocumentSync: full`, so it expects the full document text on changes.

For the full command/tooling matrix and examples, see `docs/Tooling.md`.

