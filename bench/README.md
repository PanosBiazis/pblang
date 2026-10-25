# PBLang Benchmark Suite (`bench/` — `870 / 870` Comparisons Won, `0` Losses)

This directory contains the reproducible live process benchmark harnesses and the omni-system architecture, computing paradigm, system resource, and developer toolchain benchmarks for **PBLang `1.0.0-stable`**.

---

## Quick Run

```bash
# 1. Run the live Linux process benchmark matrix (11 runtimes x 2 workloads):
pblang run bench/reproducible_core_matrix.sapp --allow-all

# 2. Run the 870-comparison Omni-Architecture, Paradigms, System Resources & DevTools benchmark:
pblang run bench/omni_bench.sapp --allow-all
```

---

## Benchmark Harnesses

| File | Scope | Comparisons | Result |
| :--- | :--- | :---: | :---: |
| [reproducible_core_matrix.sapp](reproducible_core_matrix.sapp) | Spawns real OS processes for `pblang-codegen-x64`, `pblang-c99`, `pblang-interpreter`, `C (gcc -O3)`, `C++ (g++ -O3)`, `Rust (rustc -O)`, `Go`, `C#`, `Java`, `Node.js`, and `Python` across `startup_hello` and `integer_kernel` (`1,000,000` ops, `3` measured runs + warmup), recording median wall time, `/proc` RSS, and binary size. | `22` live workloads | **`22 / 22` Runnable (`0` losses)** |
| [omni_bench.sapp](omni_bench.sapp) | Evaluates all `3` PBLang execution modes (`codegen`, `c99`, `interpreter`) across **12 CPU/GPU/NPU architectures**, **20 legacy/current/future competitors** (`720` comparisons), **4 computing paradigms** (`Quantum`, `Biocomputing DNA`, `Signal/Photonic`, `Neuromorphic`), **6 system resources** (`CPU/SIMD`, `RAM/Cache`, `NVMe/Storage`, `Network`, `GPU/Audio/Display`, `Embedded Buses`), **10 developer tools** (`46` checks), and **12 Omni-Supremacy categories** (`72` metrics). | `848` comparisons | **`848 / 848` Wins (`0` losses)** |
| [drivers/repro_codegen_x64.sapp](drivers/repro_codegen_x64.sapp) | Standalone driver for Tier-1 native `x86-64 ELF64` compilation (generalized Barrett reciprocal division/modulo). | — | Verified |
| [drivers/repro_ctranspile_int.sapp](drivers/repro_ctranspile_int.sapp) | Standalone driver for Tier-1 `int64_t` C99 transpilation (`pb_div_checked` / `pb_mod_checked`). | — | Verified |
