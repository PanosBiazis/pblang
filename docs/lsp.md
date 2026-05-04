## Sapphire LSP (language server)

This workspace includes a minimal LSP server that provides:

- parse diagnostics (syntax errors)
- typecheck diagnostics (warnings) from `sapphire-typeck`

### Build

```bash
cargo build -p sapphire-lsp
```

### Run (stdio)

Most editors start LSP servers over stdio. The binary is:

- `target/debug/sapphire-lsp` (after build)

### VS Code / Cursor setup (example)

Use any LSP client extension that lets you define a custom language server command
for `*.sapp` files, and set the command to:

```bash
<repo>/target/debug/sapphire-lsp
```

The server uses `textDocumentSync: full`, so it expects the full document text on changes.

