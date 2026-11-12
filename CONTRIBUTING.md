# Contributing to PBLang (`2.3.0`)

Thank you for your interest in contributing to **PBLang**! This guide covers building the Sovereign Zero-Rust host runtime, working on the self-hosted compiler & standard library, running verification suites, and maintaining public repository hygiene.

## Getting Started

### Prerequisites

- **Primary (`PBLang 2.3.0` Sovereign Zero-Rust Runtime & Universal `64/32/16/8-bit` Toolchain)**:
  - Linux / macOS: C99 compiler (`gcc` or `clang`) + `git`
  - Windows (`x86-64` / `x86` / `ARM64`): `MinGW-w64` (`x86_64-w64-mingw32-gcc`, `i686-w64-mingw32-gcc`) + `git`
- **Optional (Legacy Reference Workspace `crates/*` only)**:
  - Rust stable toolchain (`rustup`) — *not required to build, run, or test `pblang 2.3.0`*.

### Build & Quick Verification

```bash
git clone https://github.com/PanosBiazis/pblang.git
cd pblang

# Build the ~375 KiB Sovereign Zero-Rust Linux binary:
gcc -O3 -std=gnu99 -Wall -Wno-unused-function \
  runtime_stage3/pblang_zero_rust_vm.c -ldl -lpthread -lm -s -o pblang

./pblang --version
./pblang run tests/zero_rust_vm_test.sapp --allow-all
./pblang run tests/path_b_native_sovereign_test.sapp --allow-all
```

For Windows `PE32+` cross-compilation:

```bash
x86_64-w64-mingw32-gcc -O3 -std=gnu99 -Wall -Wno-unused-function \
  runtime_stage3/pblang_zero_rust_vm.c -lws2_32 -lwinhttp -lgdi32 -luser32 -lbcrypt -s -o pblang.exe
```

## Development Workflow

### Branch Strategy & Making Changes

1. Create a focused branch from `main` (`feat/<description>` or `fix/<description>`).
2. Implement bounded, tested changes with zero regressions.
3. If you modify files under `stdlib/`, refresh `pblang.lock` (`./pblang update .`) and verify that `path = "stdlib"` remains strictly relative.
4. Run the verification suites before opening a pull request.

### Commit Messages

Use clear Conventional Commit style messages:

```text
<type>(<scope>): <short summary>
```

Examples:
- `feat(vm): add O(1) FNV-1a open-addressing map index`
- `fix(codegen-x64): preserve register callee-saves across nested try/catch`
- `docs(readme): align binary footprint metrics with v2.0.0 release`

## Project Structure

- **Sovereign Zero-Rust Host VM & `.sapp -> .pbck` Compiler (`C99`)** — `runtime_stage3/pblang_zero_rust_vm.c` and `runtime_stage3/zr_*.h`
- **Pure-PBLang Self-Hosted Compiler Pipeline** — `stdlib/compiler/` (`tokenizer.sapp`, `parser.sapp`, `optimizer.sapp`, `native_pbck.sapp`, `ctranspile.sapp`)
- **Pure-PBLang Native Machine-Code Emitters** — `stdlib/native/` (`codegen_x64.sapp`, `elf_dyn_x64.sapp`, `codegen_pe64.sapp`, `codegen_arm64_ast.sapp`, `codegen_wasm.sapp`)
- **Standard Library (`100% .sapp`)** — `stdlib/*.sapp` (Hardware Governor, Sovereign Studio UI, 24 Algorithms, Quantum Simulator + QPU clients, FIPS 203 `ML-KEM-768`, Bioinformatics, Smart Updater, Modern UI, Security/Safety)
- **Toolchain CLI Drivers** — `tools/` (`pbc.sapp`, `pbsecure.sapp`, `pbaudit.sapp`, `pblock.sapp`)
- **IDE & Editor Extensions** — `editors/ide/` and `editors/vscode/`
- **Verification & Benchmarks** — `tests/` and `bench/`
- **Optional Legacy Reference Crates** — `src/` and `crates/`

## Testing & Public Hygiene Rules

```bash
# Core Zero-Rust VM, compiler, SQLite, HTTP/TLS, and bundle checks:
./pblang run tests/zero_rust_vm_test.sapp --allow-all

# 3-stage native ELF64 fixed-point, x64-dyn FFI, and Windows PE32+ checks:
./pblang run tests/path_b_native_sovereign_test.sapp --allow-all

# Hardware Governor & Sovereign Studio UI checks:
./pblang run tests/hardware_governor_and_studio_ui_test.sapp --allow-all

# Smart updater, portable lockfiles, safety, and public release hygiene:
./pblang run tests/smart_updater_test.sapp --allow-all
./pblang run tests/safety_test.sapp --allow-all
./pblang run tests/public_release_readiness_test.sapp --allow-all
```

### Mandatory Security, Privacy & Honesty Standards

- **Zero Local Path or Secret Leaks**: Never commit hardcoded workstation paths (`/home/...`, `/media/...`, `C:\Users\...`), usernames, hostnames, RFC1918 LAN IPs, or tokens/keys anywhere in code, tests, docs, or `pblang.lock`.
- **Engineering Honesty**: Clearly distinguish live OS measurements (`measured_live: true`) from synthetic models or in-VM simulations (`measured_live: false`, `mode: "in_vm_simulation"`). Never hardcode fake benchmark or hardware numbers.
- **Capability Discipline**: Respect `--sandbox` and explicit `--allow-*` / `--deny-*` boundaries.

## Documentation & Release Notes

- Browse [`docs/README.md`](docs/README.md) for architecture, safety, and target guides.
- Document user-visible changes in [`CHANGELOG.md`](CHANGELOG.md).
