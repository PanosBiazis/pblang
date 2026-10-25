# PBLang Verification & Safety Test Suite (`tests/`)

This directory contains the automated verification tests for **PBLang `1.0.0-stable`**, covering language semantics, compiler backend parity, memory & general safety, supply-chain & toolchain attack defense, public release readiness, and hardware/UI integration.

---

## Key Verification Suites

```bash
# 1. Memory Safety, General Safety & Supply-Chain/Toolchain Security (36/36 checks):
pblang run tests/safety_test.sapp --allow-all

# 2. Omni-Computing, System Resources & DevTools Supremacy (50/50 checks):
pblang run tests/omni_bench_test.sapp --allow-all

# 3. Public Release Readiness, Zero-Leak Privacy Hygiene & 7-Backend Parity (36/36 checks):
pblang run tests/public_release_readiness_test.sapp --allow-all

# 4. Package Manifest & Supply-Chain Hook/Bidi Validation:
pblang run tests/semantics/manifest_validate_test.sapp --allow-all

# 5. Wave 1..12 Full Ecosystem & Domain Engines (128/128 checks):
pblang run tests/backlog_completion_wave_test.sapp --allow-all
```

---

## Directory Layout

| Path | Description |
| :--- | :--- |
| [safety_test.sapp](safety_test.sapp) | Verifies Spatial (`W^X`, `NX_COMPAT`, bounds checking), Temporal (`0` UAF/Double-Free, deterministic `defer`), Initialization/Type/NaN, Arithmetic/Barrett modulo, and Supply-Chain/Toolchain Attack Defense (`36/36 PASS`). |
| [omni_bench_test.sapp](omni_bench_test.sapp) | Verifies all 3 PBLang modes across 12 architectures, Quantum/Bio/Photonic/Neuromorphic kernels, 6 system resources, and 10 dev tools (`50/50 PASS`). |
| [public_release_readiness_test.sapp](public_release_readiness_test.sapp) | Verifies zero-Rust TLS 1.3 PEM inspection, bundle overlap prevention, zero-leak repository audit, and 7-backend differential compiler parity (`36/36 PASS`). |
| [backlog_completion_wave_test.sapp](backlog_completion_wave_test.sapp) | End-to-end verification across Waves 1..12 (`128/128 PASS`). |
| `semantics/` | Hundreds of focused `.sapp` tests covering parser, typechecker, bytecode VM, C99 transpiler, ARM64/x64/WASM codegen, crypto, SQL, HTTP, and stdlib modules. |
| `fixtures/` & `tooling/` | UI, reader, and embedded hardware export fixtures. |
