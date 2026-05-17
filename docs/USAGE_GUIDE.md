# pblang Usage Guide

This guide explains how to use pblang in practice: writing scripts, creating projects, using modules, testing, and packaging apps.

## 1) Install and run

Prerequisites:

- Rust toolchain (stable)
- A clone of this repository

Build:

```bash
cargo build --release
```

Run a script:

```bash
cargo run -- run hello.sapp
```

Or use the built binary:

```bash
./target/release/pblang run hello.sapp
```

## 2) Your first script

Create `hello.sapp`:

```sapp
print "Hello from pblang!";

let name = "world";
print "Hello, " + name + "!";
```

Run:

```bash
pblang run hello.sapp
```

## 3) Core language basics

Variables:

```sapp
let x = 42;
let msg = "ok";
let arr = [1, 2, 3];
let cfg = map{host: "127.0.0.1", port: 8080};
```

Functions:

```sapp
fn add(a, b) {
    return a + b;
}

print add(2, 3);
```

Control flow:

```sapp
if x > 10 {
    print "big";
} else {
    print "small";
}

for i 5 {
    print i;
}

for v in arr {
    print v;
}
```

Match:

```sapp
let status = 404;
let label = match status {
    case 200: "ok",
    case n if n >= 400 # "error",
    case _: "other"
};
print label;
```

Error handling:

```sapp
try {
    let data = fs_read("config.json");
    print data;
} catch e {
    print "failed: " + str(e);
}
```

## 4) Modules and imports

Import stdlib:

```sapp
let json = import("stdlib:json.sapp");
let http = import("stdlib:http.sapp");
```

Import local file:

```sapp
let util = import("./util.sapp");
print util.answer();
```

Export from module:

```sapp
fn answer() { return 42; }
export answer;
```

## 5) Create and run a project

Initialize manifest:

```bash
pblang init
```

This creates `pblang.toml`. Typical layout:

```text
my-app/
  pblang.toml
  src/main.sapp
```

Run from project root:

```bash
pblang run .
```

## 6) Useful CLI commands

- `pblang run <file|dir>`: execute script/project
- `pblang test [dir]`: run tests
- `pblang fmt [files...]`: format files
- `pblang lint [files...]`: parse/typecheck diagnostics
- `pblang --check <file>`: check single file without running
- `pblang --doc <file> [--html]`: generate docs for a module
- `pblang bundle [--self-extract]`: create distributable bundle
- `pblang pack [path]`: create package archive (`.tar.gz`)

## 7) Testing

Put test scripts in `tests/` and run:

```bash
pblang test tests
```

For Rust integration tests in this repo:

```bash
cargo test
```

## 8) Formatting and linting workflow

Recommended loop while developing:

```bash
pblang fmt src/main.sapp
pblang lint src/main.sapp
pblang run src/main.sapp
```

For broader checks:

```bash
cargo test --test semantics
```

## 9) Capabilities and sandboxing (embedding)

When embedding pblang in Rust, use `StdlibConfig` to control access to system features (fs, net, http, sqlite, ffi, exec, etc.). Disable anything your script does not need.

See:

- `docs/Embedding.md`
- `docs/Language_Semantics.md`
- `docs/Safety.md`

## 10) Packaging and distribution

Bundle app directory:

```bash
pblang bundle .
```

Create tarball package:

```bash
pblang pack .
```

Self-extract mode (current workflow support):

```bash
pblang bundle --self-extract .
```

## 11) Common troubleshooting

- Parse error in guarded `match`: prefer `#` or `then` in guarded arms.
- Capability denied at runtime: enable required capability in embedding config.
- Registry dependency issues: verify `PBLANG_REGISTRY_INDEX` and lockfile state.
- Native/backend differences: check `ROADMAP.md` and `docs/CURRENT_STATUS.md` for current backend maturity.

## 12) Next docs to read

- `README.md` for project overview
- `docs/PROJECT_MANUAL.md` for operational project workflows
- `docs/IDE_MANUAL.md` for IDE workflows and shortcuts
- `docs/Tooling.md` for complete CLI/tooling details
- `docs/Language_Semantics.md` for language rules
- `docs/CURRENT_STATUS.md` for latest implementation status
