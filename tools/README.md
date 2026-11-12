# PBLang Self-Hosted Compiler & Toolchain Drivers (`tools/`)

This directory contains the self-hosted compiler driver (`pbc.sapp`), security/audit tools, and portable lockfile drivers written in 100% Pure PBLang (`2.0.0`).

---

## Key Drivers

| File | Description |
| :--- | :--- |
| [pbc.sapp](pbc.sapp) | Self-hosted PBLang compiler driver supporting `.pbck` v1 bytecode (`--target pbck`), hosted `C99` (`--target c`), 3-stage self-hosting static Linux `x86-64 ELF64` (`--target x64`), dynamic OS-FFI `ELF64` (`--target x64-dyn`), Windows 11 `PE32+` `.exe` (`--target pe64`), `ARM64 ELF64` (`--target arm64`), and `WASM32` (`--target wasm`). |
| [pbsecure.sapp](pbsecure.sapp) | Interprocedural static taint, sink, and capability policy analyzer. |
| [pbaudit.sapp](pbaudit.sapp) | Atomic resumable per-module repository security & quality auditor. |
| [pblock.sapp](pblock.sapp) | Portable relative `pblang.lock` (`pblang-package-tree-v2`) validator and publisher. |
| [pblang_stage3_selfhost_driver.sapp](pblang_stage3_selfhost_driver.sapp) | Unified CLI driver for Stage-3 runtime checks, 6 Hardware Emulators (`emulators`), and 8-Target Bus Manager (`discover`). |
