# PBLang 1.0 stable: release criteria and planning estimate

Assessment date: 2026-09-22. This is a planning assessment based on inspected
source and focused local evidence, not a completed release audit. The manifest
still declares version `0.6.0` and channel `alpha`; neither field changes here.
A version number is not a percentage: 0.6 does not mean 60% complete.

## What stable must mean

A 1.0 release needs a written language/API compatibility contract, an explicit
support matrix, reproducible artifacts and repeated passing release gates for
those artifacts. It need not finish every future application or hardware feature.
It must not advertise unfinished routes as stable. Targets outside the verified
matrix can remain available with an explicit experimental status. This stages
testing without abandoning the goal of broad platform support.

The user's full goal additionally includes PBLang implementations replacing
foreign blocks, all application families and old/new devices. That is a larger
program than a first stable language release. A pure-PBLang runtime cannot be
claimed merely because a compiler compiles itself or an application is written
in PBLang. Retained-runtime and OS/driver boundaries must be disclosed.

## Blocking gates

| Area | Current evidence / gap | Gate for a stable claim |
| --- | --- | --- |
| Language and compiler parity | Shared binding cells now cover owner, sibling and transitive captures in tested routes; expression-block fixes cover only eligible blocks. General backend parity remains incomplete. | Specify capture, scope, mutation, exceptions and numeric behavior; differential fixtures pass on every supported compiler/backend; no known result-changing defect in the supported subset. |
| Memory and resource lifetime | Kernel precise GC is separate from the retained VM; explicit valid roots are required. General cycles, exceptional resource cleanup and FFI ownership are not universally integrated. | Define ownership at each boundary; verify escapes, cycles, exception preservation, cancellation, concurrency and long-running memory behavior. Unsupported guarantees fail explicitly. |
| Isolation and foreign execution | Bounded facade checks do not prove hard preemption, heap isolation or absence of raw bootstrap bypasses. Existing foreign boundaries remain. | Verify actual enforcement at every exposed entry point; malicious-input, race and cleanup tests plus independent review. If PBLang-only is a release promise, migrate and disable remaining blocks with verified equivalent behavior first. |
| Packages, audit reports and releases | Manifest validation and individual artifact checks do not establish the complete publisher/report trust chain. | Verify publisher identity, exact artifact binding, rollback policy, report authenticity, reproducible builds, upgrades and recovery for the actual distribution. |
| Platforms and products | Local Linux and QEMU evidence is partial. ISA/emulator success is not guest-OS, driver, physical-device or installed-product parity. | Declare exact supported OS/architecture versions and test the release artifact there. Include installation, permissions, paths, Unicode, networking, UI where advertised, shutdown and recovery. |
| Libraries and developer tools | Many functional modules exist; complete compatibility, packaging and documentation checks for the chosen public API have not been demonstrated here. | Freeze the supported API, publish migration notes/examples, run full release suites, and verify CLI/IDE/library behavior against the same release revision. |
| Performance and operations | Workload-specific improvements coexist with regressions and long collection pauses. No universal streaming/device/resource gain is established. | Reproducible before/after baselines for CPU, peak/live memory, pause distributions, disk and relevant devices; agreed regression budgets, soak tests and no unresolved severe defects. |

Useful evidence pointers:

- [Closure and expression-block boundary](SELFHOST_INLINE_BLOCKS.md)
- [Kernel ownership, pause and rooted-collector measurements](KERNEL_GC_ROOTLESS_SWEEP.md)
- [Heap regression coverage and measured experiments](KERNEL_HEAP_MERGE.md)
- [Release identity and publisher limitations](RELEASE_CHANNELS.md)
- [VM and emulator evidence classes](VM_TEST_LAB.md)
- [Shared bindings and the remaining cycle boundary](SELFHOST_SHARED_CAPTURES.md)
- [Current closure/call-cost measurements](SELFHOST_CALL_CLASSIFICATION.md)
- `stdlib/compiler/vm.sapp`: raw OP_CLOSURE copies references to compiler-lowered cells; raw legacy upvalues retain their old semantics.
- `stdlib/runtime_gc.sapp`: explicit retained-host collection limitations.

Older aggregate status tables and the historical "End of Month 6" milestone are
not current completion evidence. In particular, broad "parity reached" or
"production" labels cannot override a reproducible remaining compatibility defect.
The already-modified aggregate status file is preserved in this slice.

## How long?

No reliable completion date or percentage can be computed from the present
repository. There is no agreed frozen 1.0 scope, measured defect burn-down rate,
complete release matrix or known staffing/availability in this assessment.
The following is a **low-confidence planning range**, not a delivery promise and
not a conversion of test counts into calendar time:

- **Focused first 1.0: roughly 6–12 months** of sustained development with one or
  two dedicated engineers, access to the declared target machines and time for
  independent QA/security review. This assumes a frozen core/API and a limited,
  explicitly verified platform/backend matrix. New broad features must stop
  displacing release-blocking work. These are assumptions, not current staffing.
- **The broader PBLang-only, multi-product, multi-backend/device goal: 12–24+
  months as an initial planning allowance**, possibly substantially longer.
  The breadth currently requested is not sufficiently bounded for a dependable
  upper date. Universal support for every OS/device/application is not a
  finite verifiable 1.0 acceptance criterion.
- Intermittent development, unavailable target hardware, major runtime rewrites
  or additional scope extend those ranges. AI-assisted coding does not remove
  integration, release validation or observation time.

Suggested sequence for the focused release:

| Phase | Illustrative duration | Exit evidence |
| --- | --- | --- |
| Freeze contract and release matrix | 2–4 weeks | Written supported subset, exact target versions, tracked blocker list and complete baseline run. |
| Correct semantics and resource lifecycle | 8–16 weeks | Differential/negative/concurrency tests and stable long-running workloads; ownership boundaries documented. |
| Finish enforced boundaries and distribution | 8–12 weeks | End-to-end permission, package, artifact, report, upgrade and recovery checks. |
| Validate platform matrix and performance | 6–10 weeks | Same revision tested in declared environments; regression budgets met; libraries/products checked. |
| Beta/RC stabilization | 4–8 weeks | Repeated clean release builds and soak runs, no unresolved release-blocking defects, documentation and migration complete. |

The serial total is 28–50 weeks, approximately the 6–12 month planning range;
phases can overlap, but runtime changes can also invalidate earlier results.
Re-estimate after the first 2–4 week inventory/baseline phase using observed
throughput and the actual blocked targets. Do not change the manifest to stable
until the gates pass. This document schedules no background work and deploys
nothing; EnterpriseOrg servers are excluded from all testing.

## Prioritized 1.0.0-stable & Public GitHub Release Backlog (2026-10-18)

| Priority | Backlog ID | Area | Deliverable (100% PBLang, `0` Lint Warnings) | Status |
| --- | --- | --- | --- | --- |
| **P1 — Critical (S1)** | `PB-02-022` | Rust-Independence in Native TLS & PEM Engine | `stdlib/tls.sapp`: `validate_cert_pair`, `inspect_certificate`, `pblang_tls13_status` bypassing `rustls` `CryptoProvider` (`ring` vs `aws-lc-rs`) conflicts + `stdlib/auto_tls_gateway.sapp` ACME HTTP-01 & 30-day renewal. | **Delivered (`tests/public_release_readiness_test.sapp`)** |
| **P1 — Critical (S1)** | `PB-02-019` | Relative-Root Bundle Source Integrity & Overlap Guard | `stdlib/selfhost_bundle.sapp` & `stdlib/compiler/native_bundle.sapp`: `validate_bundle_destination`, `bundle_file_from_root`, `write_bundle_safe`, `publish_bundle_safe` preserving source bytes and rejecting self-copy/overlap/`.sapp` targets before write. | **Delivered (`tests/public_release_readiness_test.sapp`)** |
| **P1 — Critical (S1)** | `PB-02-023` | Public GitHub Hygiene, Privacy & `1.0.0` Gate Auditor | `stdlib/public_release_audit.sapp`: `scan_text_for_private_leaks`, `audit_repository_tree`, `audit_readme_rust_independence`, `evaluate_1_0_stability_gates` with O(1) `len(args)` guards and `ordered_input: true`. | **Delivered (`tests/public_release_readiness_test.sapp`)** |
| **P2 — High (S1)** | `PB-02-024` | Public Documentation & Zero-Rust Quickstart | `README.md`: Standalone `pblang` execution and 100% PBLang native compilation (`tools/pbc.sapp` to `x64`, `arm64`, `wasm`, `pbck`) without requiring `cargo`/`rustc` as a user prerequisite; by-default A+ web security (`stdlib/http_server.sapp`, `stdlib/modern_ui_web.sapp`). | **Delivered (`README.md`)** |
| **P2 — High (S2)** | `PB-02-025` | Differential Multi-Backend Compiler Parity Suite | `stdlib/compiler/backend_parity.sapp`: Automated parity verification across `selfhost_vm`, `native_pbck`, `c99_transpiler`, `x86_64_elf`, `arm64_elf`, and `wasm32`. | **Delivered (`tests/public_release_readiness_test.sapp`)** |
| **P3 — Open Gate** | `PB-02-001` / `PB-02-018` | Full Retained Bootstrap Removal & Cross-OS Physical Matrix | Eliminate remaining retained host bootstrap boundaries (`ROOT-CORE-01`) and complete physical non-modeled Windows/macOS/iOS/Android hardware release qualification before flipping `pblang.toml` channel from `alpha` to `1.0.0-stable`. | **Open (Tracked in `CURRENT_BACKLOG.md`)** |

