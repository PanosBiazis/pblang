# PBLang Verification & Safety Test Suite (`tests/`)

This directory contains the automated verification tests for **PBLang `2.4.0`**, covering the Sovereign Zero-Rust Host VM & `.sapp` compiler, Complete `MSC2020 (00–97)` & Risch–Liouville Universal Mathematics (`542` assertions across 4 suites), universal `64/32/16/8-bit` native machine-code compilers, Hardware Governor & Sovereign Studio UI, smart polymorphic `pblang update`, 24 algorithms / Quantum / DNA / FIPS 203 `ML-KEM-768`, memory safety, and public release hygiene.

---

## Key Verification Suites

```bash
# 1. Sovereign Zero-Rust Host VM, C99 .sapp Compiler, SQLite, HTTP/TLS & Bundle (34/34 checks):
pblang run tests/zero_rust_vm_test.sapp --allow-all

# 2. Complete MSC2020 (00-97), Risch-Liouville, Indeterminacies, Adaptive Governor & World Canon (542 checks):
pblang test tests/mathematics_omni_suite_test.sapp tests/math_pure_foundations_test.sapp tests/math_complex_riemann_stochastic_test.sapp tests/math_interdisciplinary_world_canon_test.sapp

# 3. Pure-PBLang 3-Stage Self-Hosting Native Compilers — ELF64, ELF64-dyn & Windows PE32+:
pblang run tests/path_b_native_sovereign_test.sapp --allow-all

# 4. Universal <= 32-bit Native Codegen (32-bit x86/PE32/RV32, 16-bit 8086 DOS, 8-bit Z80/6502/AVR8) & @polyglot:
pblang run tests/sub32_and_universal_devices_test.sapp --allow-all

# 5. Multi-Accelerator Hardware Governor & Sovereign Studio UI (76/76 checks):
pblang run tests/hardware_governor_and_studio_ui_test.sapp --allow-all

# 6. Smart Polymorphic `pblang update` & Portable Relative Lockfiles (75/75 checks):
pblang run tests/smart_updater_test.sapp --allow-all

# 7. 24 Algorithms, Quantum Simulator + QPU, DNA Bioinformatics & ML-KEM-768 (53/53 checks):
pblang run tests/algorithms_quantum_pqc_bio_test.sapp --allow-all

# 8. Memory Safety, General Safety & Supply-Chain/Toolchain Security (36/36 checks):
pblang run tests/safety_test.sapp --allow-all

# 9. Public Release Readiness, Zero-Leak Privacy Hygiene & Multi-Backend Parity (36/36 checks):
pblang run tests/public_release_readiness_test.sapp --allow-all
```

---

## Directory Layout

| Path | Description |
| :--- | :--- |
| [zero_rust_vm_test.sapp](zero_rust_vm_test.sapp) | Verifies the `~415 KiB` Zero-Rust Host VM (`runtime_stage3/`), `.sapp -> .pbck` compilation, closures, GC, SQLite, HTTP, `PBLZREXE` bundling, and `--sandbox` (`34/34 PASS`). |
| [mathematics_omni_suite_test.sapp](mathematics_omni_suite_test.sapp) | Verifies the unified `stdlib/mathematics.sapp` (`v2.4.0`) facade, Risch–Liouville integration, parametric & limit indeterminacies, `AdaptiveMathGovernor`, and `@math`/`@wolfram` polyglot (`61/61 PASS`). |
| [math_pure_foundations_test.sapp](math_pure_foundations_test.sapp) | Verifies Set/Lattice theory, First-Order Resolution ATP, LTL Model Checking, Category Theory, Galois Theory up to $S_5$, Buchberger Gröbner Bases, Matrix Lie Algebras, and $\mathbb{Z}$-Simplicial/Persistent Homology (`112/112 PASS`). |
| [math_complex_riemann_stochastic_test.sapp](math_complex_riemann_stochastic_test.sapp) | Verifies Complex Residue Calculus, Analytic $\zeta(s)$, Symbolic Riemannian/Einstein Tensors, Euler-Lagrange, Lyapunov Chaos, Itô/Milstein SDEs, Nash/Shapley, Reed-Solomon, Sparse Krylov, Galerkin FEM & Integral Equations (`141/141 PASS`). |
| [math_interdisciplinary_world_canon_test.sapp](math_interdisciplinary_world_canon_test.sapp) | Verifies all `63` `MSC2020` divisions, 4-way epistemic filter, Elliptic Curves, Wavelets, Radon CT, DFA/CYK, Kalman/LQR, Physics/Bio/Actuarial/Social Choice, and Scientifically Proven World Mathematical Canons (`228/228 PASS`). |
| [path_b_native_sovereign_test.sapp](path_b_native_sovereign_test.sapp) | Verifies 3-stage native `ELF64` self-hosting fixed-point (`stage2_elf == stage3_elf == 55,272 B`), `x64-dyn` dynamic FFI, and Windows `PE32+` `.exe` generation. |
| [hardware_governor_and_studio_ui_test.sapp](hardware_governor_and_studio_ui_test.sapp) | Verifies CPU/GPU/NPU/RAM hardware probing, zero-idle frame pacing, EL/EN explainability, and WCAG AAA Sovereign Studio HTML5/native UI (`76/76 PASS`). |
| [smart_updater_test.sapp](smart_updater_test.sapp) | Verifies `pblang update` across `self`, `pblang.toml` workspaces, `.sapp` scripts, archives, `.bak` rollback, and privacy/hygiene guards (`75/75 PASS`). |
| [algorithms_quantum_pqc_bio_test.sapp](algorithms_quantum_pqc_bio_test.sapp) | Verifies all 24 algorithms, statevector Grover/VQE + QPU payloads, DNA alignment/De Bruijn assembly, and NIST FIPS 202/203 `ML-KEM-768` (`53/53 PASS`). |
| [safety_test.sapp](safety_test.sapp) | Verifies Spatial (`W^X`, `NX_COMPAT`, bounds checking), Temporal (`0` UAF/Double-Free, deterministic `defer`), Arithmetic, and Supply-Chain Defense (`36/36 PASS`). |
| [public_release_readiness_test.sapp](public_release_readiness_test.sapp) | Verifies TLS 1.3 PEM inspection, bundle overlap prevention, zero-leak repository audit, and multi-backend differential compiler parity (`36/36 PASS`). |
| [omni_bench_test.sapp](omni_bench_test.sapp) | Verifies the synthetic architectural/toolchain catalog models (`measured_live: false`, `50/50 PASS`). |
| `semantics/` | Focused `.sapp` tests covering parser, typechecker, bytecode VM, C99 transpiler, ARM64/x64/PE32+/WASM codegen, crypto, SQL, HTTP, and stdlib modules. |
| `fixtures/` & `tooling/` | UI, reader, and embedded hardware export fixtures. |
