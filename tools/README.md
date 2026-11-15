# PBLang Self-Hosted Compiler & Toolchain Drivers (`tools/`)

This directory contains the self-hosted compiler driver (`pbc.sapp`), the `@polyglot` universal newest-to-oldest device builder (`build_universal_devices.sapp`), security/audit tools, hosted VM lab evidence reporter (`vm_lab_hosted_report.sapp`), and portable lockfile drivers written 100% in pure PBLang (`5.0.0` — zero Python scripts in `tools/`).

---

## Key Drivers

| File | Description |
| :--- | :--- |
| [pbc.sapp](pbc.sapp) | Self-hosted PBLang compiler driver supporting `.pbck` v1 bytecode (`--target pbck`), hosted `C99` (`--target c`), 3-stage & Stage-4 SSA/SIMD self-hosting static Linux `x86-64 ELF64` (`--target x64`), dynamic OS-FFI `ELF64` (`--target x64-dyn`), Windows 11 `PE32+` `.exe` (`--target pe64`), `ARM64 ELF64` (`--target arm64`), `RISC-V 64` (`rv64`), `WASM32` (`--target wasm`), and `<= 32-bit` targets (`--target x86`, `pe32`, `rv32`, `dos16`, `cpm80`, `c64`, `avr8`). |
| [build_universal_devices.sapp](build_universal_devices.sapp) | `@polyglot` (`@c`, `@js`, `@python`) universal device builder producing standalone executables & deterministic packages across 64-bit, 32-bit, 16-bit, 8-bit, MCU, and WASI targets. |
| [vm_lab_hosted_report.sapp](vm_lab_hosted_report.sapp) | 100% pure-PBLang content-bound SHA-256 hosted VM lab evidence recorder, verifier, and multi-lane rollup generator (`record`, `verify`, `rollup`). |
| [pbsecure.sapp](pbsecure.sapp) | Interprocedural static taint, sink, and capability policy analyzer. |
| [pbaudit.sapp](pbaudit.sapp) | Atomic resumable per-module repository security & quality auditor. |
| [pblock.sapp](pblock.sapp) | Portable relative `pblang.lock` (`pblang-package-tree-v2`) validator and publisher. |
| [pblang_stage3_selfhost_driver.sapp](pblang_stage3_selfhost_driver.sapp) | Unified CLI driver for Stage-3 runtime checks, 6 Hardware Emulators (`emulators`), and 8-Target Bus Manager (`discover`). |
