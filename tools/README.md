# PBLang Self-Hosted Compiler & Toolchain Drivers (`tools/`)

This directory contains the self-hosted compiler driver (`pbc.sapp`) and Stage-3 verification tools written in 100% Pure PBLang.

---

## Key Drivers

| File | Description |
| :--- | :--- |
| [pbc.sapp](pbc.sapp) | Self-hosted PBLang compiler driver supporting bytecode (`native_pbck`), `C99` transpilation (`--emit-c`), `x86-64 ELF64` (`--emit-x64`), `ARM64 ELF64` (`--emit-arm64`), and `WASM32` (`--emit-wasm`). |
| [pblang_stage3_selfhost_driver.sapp](pblang_stage3_selfhost_driver.sapp) | Unified CLI driver for the Stage-3 Zero-Rust ELF64 emitter, Deep Runtime Kernel, 6 Hardware Emulators (`emulators`), and 8-Target Physical Bus Manager (`discover`). |
| [build_selfhost_compiler_bundle.sapp](build_selfhost_compiler_bundle.sapp) | Builds the deterministic self-hosted compiler bytecode bundle (`pblang.pbck`). |
