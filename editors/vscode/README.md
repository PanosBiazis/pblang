# pblang VS Code Extension

Language support for [pblang](https://github.com/pblang/pblang) in Visual Studio Code.

## Features

- Syntax highlighting for `.sapp` and `.sap` files
- Language Server Protocol (LSP) integration:
  - Code completion (builtins + local symbols)
  - Hover documentation
  - Go-to-definition for local functions
  - Syntax error diagnostics
- Commands:
  - `pblang: Run current file` — runs the current file with `pblang run`
  - `pblang: Check current file` — syntax-checks with `pblang --check`

## Requirements

- `pblang` binary on your PATH (or configure `pblang.serverPath` in settings)
- VS Code 1.75.0 or later

## Installation

### From Source

1. Clone the pblang repository
2. Navigate to `editors/vscode/`
3. Run `npm install`
4. Run `npm run compile`
5. Open VS Code and press F5 to run the extension in a development host

### From VSIX (when published)

```pblang/editors/vscode/README.md#L0-0
code --install-extension pblang-0.1.0.vsix
```

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `pblang.serverPath` | `"pblang"` | Path to the pblang binary (must support `lsp` subcommand) |

## Usage

1. Open a `.sapp` or `.sap` file
2. The LSP server starts automatically
3. Use `Ctrl+Space` for completions, `F12` for go-to-definition

## Language Server

The extension launches `pblang lsp` as the language server (stdio transport).
See [docs/lsp.md](../../docs/lsp.md) for LSP server details.
