# PBLang Documentation Hub (`2.7.0`)

Welcome to the official documentation hub for **PBLang `2.7.0`**. Every guide below is organized by topic so you can quickly find language semantics, Sovereign Zero-Rust runtime & universal `64/32/16/8-bit` self-hosting compiler backends, security & supply-chain guarantees, hardware/embedded governance, and performance benchmarks.

---

## 1. Language, Zero-Rust Host VM & Native Compilers

| Guide | Description |
| :--- | :--- |
| [Language Semantics](Language_Semantics.md) | Core syntax, lexical scoping, closures, error handling (`try`/`catch`), `defer`, modules, and type rules. |
| [Execution Pipeline](Execution_pipeline.md) | End-to-end pipeline: Tokenizer $\rightarrow$ Parser $\rightarrow$ AST $\rightarrow$ Optimizing Bytecode / IR $\rightarrow$ Sovereign Zero-Rust Host VM (`runtime_stage3/`) or Pure-PBLang Native Compilers (`stdlib/native/`). |
| [Executable Targets](Executable_targets.md) | Supported output targets: Zero-Rust `PBLZREXE` bundles, Pure-PBLang 3-stage self-hosting Linux `ELF64` (`--target x64`) & Dynamic `ELF64` (`--target x64-dyn`), Windows `PE32+` `.exe` (`--target pe64`), `ARM64 ELF64` (`--target arm64`), `WASM32` (`--target wasm`), `C99`, `.pbck`, and `ATmega328P AVR8` Intel HEX. |
| [Memory Model & Lifetimes](memory.md) | Safe-point adaptive mark-and-sweep GC (`runtime_stage3/`), Cheney 2-space copying GC + `MADV_DONTNEED` (`codegen_x64.sapp`), and deterministic `defer` cleanup. |
| [Threads & Concurrency](Threads.md) | Structured concurrency, worker pools, bounded channels, and hardware-governed scheduling. |
| [Serialization](Serialization.md) | Deterministic JSON, `.pbck` v1 bytecode serialization, Parquet metadata, and schema validation. |
| [PBLang Everywhere](PBLang_Everywhere.md) | Cross-platform deployment across Linux, Windows, WebAssembly, Android, and microcontrollers. |

---

## 2. Security, Memory Safety & Supply-Chain Defense

| Guide | Description |
| :--- | :--- |
| [Safety & Memory Guarantees](Safety.md) | Spatial (`W^X`, bounds checking), Temporal (`0` UAF/Double-Free, deterministic `defer`), Initialization, and Arithmetic safety (`stdlib/safety_guard.sapp`). |
| [Security Analysis](Security_Analysis.md) | Default-on static taint analysis, pattern auditing, and capability gating (`protect` / `strict` / `audit` modes). |
| [Capability Model](Capability_model.md) | Fine-grained capability sandbox (`--sandbox`, `fs`, `net`, `http`, `exec`, `ffi`, `env`) with trusted canonicalized `stdlib:...` imports. |
| [Lockfile & Supply-Chain Contract](Lockfile_contract.md) | Portable relative content-addressed `pblang.lock` (`pblang-package-tree-v2`), zero-install-hook hermetic manifests, Trojan Source (`CVE-2021-42574`) blocker, and `pblang update` verification. |
| [Bootstrap Safety](Bootstrap_safety.md) | 3-stage native self-hosting fixed-point verification (`stage2_elf == stage3_elf`) and TOCTOU source-replacement protection. |
| [Crash Resistance](Crash_resistance.md) | Fail-closed VM and runtime recovery invariants without host panics. |
| [Controlled Execution](Controlled_execution.md) | Resource quotas, instruction/tick budgets, memory ceilings, and `execve` child process supervision (`stdlib/linux_process.sapp`). |
| [TLS & Post-Quantum Security](TLS_notes.md) | OpenSSL TLS 1.2/1.3 & Windows `WinHTTP` HTTPS, PEM inspection (`stdlib/tls.sapp`), and NIST FIPS 203 `ML-KEM-768` (`stdlib/pqc_mlkem768.sapp`). |
| [Fuzzing](Fuzzing.md) | Compiler, parser, and bytecode VM fuzzing harnesses. |

---

## 3. Standard Library, Data, Studio UI & Tooling

| Guide | Description |
| :--- | :--- |
| [Data Storage](Data_Storage.md) | Native SQLite (`libsqlite3` / `winsqlite3.dll`), Redis helpers, bounded in-memory stores, and ACID persistence. |
| [CDN & Edge Caching](CDN_Caching.md) | Provider-neutral regional/edge caching and deterministic cache invalidation. |
| [Visual Designer, Sovereign Studio & Modern UI](Visual_Designer.md) | WCAG AAA Obsidian/Alabaster Sovereign Studio (`pblang studio`, `stdlib/sovereign_studio_ui.sapp`) and declarative desktop (`X11`/`Win32 GDI`), Web (`A+ CSP`), Android, and TUI interfaces (`stdlib/modern_ui.sapp`). |
| [Tooling & CLI](Tooling.md) | `pblang run`, `check`, `test`, `doctor`, `update`, `studio`, `--ide`, `build --bundle-exe`, `fmt`, `lint`, `lsp`, and self-hosted `tools/pbc.sapp`. |
| [Language Server Protocol (LSP)](lsp.md) | Diagnostics, completions, hover, go-to-definition, and semantic tokens. |
| [FFI Contract](FFI_contract.md) | Capability-guarded Foreign Function Interface (`dlopen` / `LoadLibraryA` & `x64-dyn`) and LIFO unwind safety. |
| [Embedding PBLang](Embedding.md) | Embedding the Zero-Rust C99 VM or optional legacy Rust workspace (`Embedding_lifecycle.md`). |
| [Protocols](Protocols.md) | HTTP/1.1, HTTPS, WebSocket, and binary wire protocols. |

---

## 4. Hardware Governor, Embedded, Quantum & Scientific Computing

| Guide | Description |
| :--- | :--- |
| [Embedded Programming](Embedded_Programming.md) | Microcontroller descriptors (`Arduino Uno R4 WiFi`, `ESP32-D`, `ATmega328P AVR8`), deterministic emulators, and hardware bus discovery (`Embedded_GPIO.md`). |
| [OS & Freestanding Development](OS_Development.md) | Direct Linux syscall native binaries (`codegen_x64.sapp`), Zero-Rust host runtime (`runtime_stage3/`), and freestanding primitives. |
| [GPU & Hardware Governor Stack](GPU_Stack.md) | Multi-accelerator Hardware Governor (`stdlib/hardware_governor.sapp`: `CPU`, `RAM`, `GPU`, `NPU`, `TPU`), `hwinfo.sapp`, and CPU/opt-in tensor dispatch. |
| [Standalone AI Ecosystem (`pbAI` & `noophyon`)](https://github.com/PanosBiazis/pbai) | De-embedded standalone [`pbai`](https://github.com/PanosBiazis/pbai) and [`noophyon`](https://github.com/PanosBiazis/noophyon) repositories, plus built-in CPU ONNX (`stdlib/onnx.sapp`) and `stdlib/algorithms_24.sapp`. |
| [Quantum & Scientific Providers](Quantum_providers.md) | Exact `1..20`-qubit statevector simulator (`Grover`, `VQE`, `OpenQASM 3.0`, `Cirq`), IBM Quantum & Google Quantum Engine REST QPU clients (`stdlib/quantum_computing.sapp`), and DNA alignment/assembly (`stdlib/bioinformatics_dna.sapp`). |

---

## 5. Benchmarks, Reports & Historical Archive

- [Benchmarking Guide](Benchmarking.md) & [Performance Contracts](Performance_Contracts.md)
- [Validation & Benchmark Reports (`docs/reports/`)](reports) — Detailed verification reports, benchmark session logs, and JSON evidence.
- [Historical Architecture Notes (`docs/archive/`)](archive) — Archived design plans, roadmaps, and migration records.
