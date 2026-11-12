# PBLang Benchmark & Modeling Suite (`bench/`)

This directory contains the live, correctness-gated OS process benchmark harness (`reproducible_core_matrix.sapp`) and the synthetic architectural/toolchain catalog runner (`omni_bench.sapp`) for **PBLang `2.1.0`**.

---

## Quick Run

```bash
# 1. Run the live OS process benchmark matrix across installed toolchains:
pblang run bench/reproducible_core_matrix.sapp --allow-all

# 2. Run the synthetic multi-architecture & toolchain modeling catalog (`measured_live: false`):
pblang run bench/omni_bench.sapp --allow-all
```

---

## Harnesses & Scope

| File | Mode | Scope |
| :--- | :---: | :--- |
| [reproducible_core_matrix.sapp](reproducible_core_matrix.sapp) | **Live OS Process** | Spawns real, timed subprocesses via GNU `time` + `timeout` for `pblang-interpreter` (bytecode VM), `pblang-c99` (`.sapp -> .c` transpile + `gcc -O3`, uncached), `pblang-codegen-x64`, `pblang-tier1-x64` (native `x86-64` ELF64 subset compiler), `C`, `C++`, `Rust`, `Go`, `Java`, `C#`, `Python`, `Node.js`, and `Zig` across `startup` and `integer_kernel` (`1,000,000` ops), recording cold/warm wall time, CPU time, peak RSS, and binary size. |
| [omni_bench.sapp](omni_bench.sapp) | **Synthetic Catalog (`measured_live: false`)** | Evaluates deterministic analytical/catalog design models (`mode: "synthetic_catalog_model"`) across architectures, specialized computing paradigms, system resource profiles, and developer tool categories. Not a live cross-language process benchmark. |
| [drivers/repro_codegen_x64.sapp](drivers/repro_codegen_x64.sapp) | **Compiler Driver** | Standalone driver for Tier-1 native `x86-64 ELF64` subset compilation (`stdlib/native/codegen_x64_tier1.sapp`). |
| [drivers/repro_ctranspile_int.sapp](drivers/repro_ctranspile_int.sapp) | **Compiler Driver** | Standalone driver for integer-profile C99 transpilation (`pb_div_checked` / `pb_mod_checked`). |
