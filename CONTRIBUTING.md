# Contributing to pblang

Thank you for your interest in contributing to pblang! This document provides
guidelines and information to get you started.

## Getting Started

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Git

### Setup

```bash
git clone https://github.com/your-username/pblang.git
cd pblang
cargo build
cargo test
```

## Development Workflow

### Branch Strategy

- `main` — stable, always passes CI
- Feature branches — `feature/<description>` or `fix/<description>`

### Making Changes

1. Create a branch from `main`
2. Make your changes
3. Add or update tests for your changes
4. Run the full test suite: `cargo test --workspace`
5. Run clippy: `cargo clippy --workspace`
6. Run formatter: `cargo fmt --all`
7. Submit a pull request

### Commit Messages

Use clear, descriptive commit messages:

```
<area>: <short description>

<optional longer description>
```

Examples:
- `vm: fix stack overflow on deeply nested closures`
- `pm: add SHA256 verification for registry packages`
- `stdlib: add websocket write_frame helper`

## Project Structure

See [STRUCTURE.md](STRUCTURE.md) for a detailed map of the codebase.

Key areas:
- **Parser/AST** — `crates/pblang-core/src/parser.rs`
- **Compiler** — `crates/pblang-compiler/src/lib.rs`
- **VM** — `crates/pblang-vm/src/vm.rs`
- **Builtins** — registered in the VM crate
- **Stdlib** — `stdlib/*.sapp` (written in pblang itself)
- **CLI** — `src/main.rs`

## Testing

### Running Tests

```bash
# All tests
cargo test --workspace

# Semantics tests only (language behavior)
cargo test --test semantics

# Specific test
cargo test my_test_name

# With output
cargo test -- --nocapture
```

### Adding Tests

- **Language semantics**: Add a `.sapp` file in `tests/semantics/` and register
  it in `tests/semantics.rs`
- **Unit tests**: Add `#[test]` functions in the relevant crate
- **Integration tests**: Add a file in `tests/`

### Test Conventions

- Test names should be descriptive: `closure_captures_outer_local_via_fn_literal`
- Use `run_collect()` for testing print output
- Use `run_err()` for testing error conditions
- Flaky network tests should be marked `#[ignore]`

## Code Style

- Follow standard Rust formatting (`cargo fmt`)
- Use `cargo clippy` to catch common issues
- Prefer explicit error handling over `.unwrap()` in library code
- Add doc comments (`///`) to public APIs
- Keep functions focused and under ~50 lines when practical

## Areas for Contribution

Check [docs/Tasks.md](docs/Tasks.md) for the active backlog. Good first issues:

- Adding stdlib modules (pure `.sapp` files)
- Improving error messages
- Adding test cases for edge cases
- Documentation improvements
- VS Code extension enhancements

## Architecture Notes

- The VM uses **reference counting** with a cycle-breaking GC
- Threading uses **VM-per-thread** isolation (no shared heap)
- Security is **capability-based** (embedders control what scripts can do)
- The parser uses the **chumsky** combinator library
- Bytecode is a flat array of `OpCode` enums with absolute jump targets

## Questions?

- Check the `docs/` directory for design documents
- Open an issue for bugs or feature requests
- See `docs/CURRENT_STATUS.md` for what's working and what's planned
