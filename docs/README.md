# PBLang Documentation Hub (`1.0.0-stable`)

Welcome to the official documentation hub for **PBLang**. Every guide below is organized by topic so you can quickly find language semantics, compiler backends, security & supply-chain guarantees, hardware/OS programming, and performance benchmarks.

---

## 1. Language & Core Runtime

| Guide | Description |
| :--- | :--- |
| [Language Semantics](Language_Semantics.md) | Core syntax, lexical scoping, closures, error handling (`try`/`catch`), `defer`, modules, and type rules. |
| [Execution Pipeline](Execution_pipeline.md) | End-to-end compilation pipeline: Tokenizer $\rightarrow$ Parser $\rightarrow$ AST $\rightarrow$ IR $\rightarrow$ E-Graph Optimizer $\rightarrow$ Native Codegen / C99 / Bytecode VM. |
| [Executable Targets](Executable_targets.md) | Supported output formats: Linux `ELF64`/`ELF32`, Windows `PE32+`/`PE32` (`.exe`/`.dll`), macOS `Mach-O 64`, FreeBSD `ELF64`, Android `ARM64` (`APK`/`DEX`), `WASM32`, and Bare-Metal (`MS-DOS`, `CP/M-80`, `AVR8`, `ESP32`). |
| [Memory Model & Lifetimes](memory.md) | Epoch-arena allocation, deterministic reference counting, Tricolor cycle GC, and zero-UAF/zero-double-free guarantees. |
| [Threads & Concurrency](Threads.md) | Structured concurrency, worker pools, bounded channels, and lock-free synchronization. |
| [Serialization](Serialization.md) | Deterministic JSON, binary formats, Parquet metadata, and schema validation. |
| [PBLang Everywhere](PBLang_Everywhere.md) | Cross-platform deployment across desktop, server, web, mobile, and microcontrollers. |

---

## 2. Security, Memory Safety & Supply-Chain Defense

| Guide | Description |
| :--- | :--- |
| [Safety & Memory Guarantees](Safety.md) | Spatial (`W^X`, bounds checking), Temporal (`0` UAF/Double-Free, deterministic `defer`), Initialization, and Arithmetic safety (`stdlib/safety_guard.sapp`). |
| [Security Analysis](Security_Analysis.md) | Default-on static taint analysis, pattern auditing, and capability gating (`protect` / `strict` / `audit` modes). |
| [Capability Model](Capability_model.md) | Fine-grained capability sandbox (`fs`, `net`, `exec`, `ffi`, `env`) and transitive dependency capability attenuation. |
| [Lockfile & Supply-Chain Contract](Lockfile_contract.md) | Content-addressed `pblang.lock` (`pblang-package-tree-v2`), zero-install-hook hermetic manifests, Trojan Source (`CVE-2021-42574`) blocker, typosquatting/dependency-confusion defense, and Diverse Double-Compiling (`DDC`). |
| [Bootstrap Safety](Bootstrap_safety.md) | Self-hosted compiler verification, fixed-point reproducibility, and TOCTOU source-replacement protection. |
| [Crash Resistance](Crash_resistance.md) | Fail-closed VM and runtime recovery invariants without host panics. |
| [Controlled Execution](Controlled_execution.md) | Resource quotas, instruction/tick budgets, memory ceilings, and child process supervision. |
| [TLS & Transport Security](TLS_notes.md) | Zero-Rust TLS 1.3 certificate validation, ECDSA/RSA PEM inspection, and carrier-independent transport pinning. |
| [Fuzzing](Fuzzing.md) | Compiler, parser, and bytecode VM fuzzing harnesses. |

---

## 3. Standard Library, Data, UI & Tooling

| Guide | Description |
| :--- | :--- |
| [Data Storage](Data_Storage.md) | Native SQLite, Redis helpers, bounded in-memory stores, and ACID persistence. |
| [CDN & Edge Caching](CDN_Caching.md) | Provider-neutral regional/edge caching and deterministic cache invalidation. |
| [Visual Designer & Modern UI](Visual_Designer.md) | Declarative desktop (`X11`), Web (`A+ CSP`), Android (`APK`), and TUI interfaces (`stdlib/modern_ui.sapp`). |
| [Tooling & CLI](Tooling.md) | `pblang build`, `test`, `check`, `fmt`, `lint`, `pack`, `lsp`, and self-hosted `tools/pbc.sapp`. |
| [Language Server Protocol (LSP)](lsp.md) | Diagnostics, completions, hover, go-to-definition, and semantic tokens. |
| [FFI Contract](FFI_contract.md) | Capability-guarded Foreign Function Interface and LIFO unwind safety. |
| [Embedding PBLang](Embedding.md) | Embedding the PBLang VM in host applications (`Embedding_lifecycle.md`). |
| [Protocols](Protocols.md) | HTTP/1.1, HTTP/2, WebSocket, and binary wire protocols. |

---

## 4. Hardware, Embedded, OS & AI / Quantum Computing

| Guide | Description |
| :--- | :--- |
| [Embedded Programming](Embedded_Programming.md) | Microcontroller targets (`Arduino Uno R4 WiFi`, `ESP32-D`, `RP2040`, `STM32F4`) and physical hardware bus discovery (`Embedded_GPIO.md`). |
| [OS Development](OS_Development.md) | Bare-metal kernel development, Stage-3 direct-syscall runtime, and `pbOS` primitives. |
| [GPU Stack](GPU_Stack.md) | Parallel compute dispatch, shader/tensor kernels, and damage-tracked rendering. |
| [pbAI Native Intelligence](pbAI.md) | Pure-PBLang neural networks, ONNX inference, BPE tokenization, RAG, and cognitive loops. |
| [Quantum, Bio & Photonic Computing](Quantum_providers.md) | Quantum circuit simulation (`Grover`, `ZX-calculus`), DNA strand displacement, MZI photonic meshes, and LIF spiking neuromorphic kernels. |

---

## 5. Benchmarks, Reports & Historical Archive

- [Benchmarking Guide](Benchmarking.md) & [Performance Contracts](Performance_Contracts.md)
- [Validation & Benchmark Reports (`docs/reports/`)](reports) — Detailed verification reports, benchmark session logs, and JSON evidence.
- [Historical Architecture Notes (`docs/archive/`)](archive) — Archived design plans, roadmaps, and migration records.
