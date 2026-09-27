# Continuation status — 2026-09-27

This reconciles the runtime audit with nine recent product/library delivery
summaries and the PBLang main tree at `4a4e528`. It is a scoped status ledger,
not a line-by-line audit, full conversation export, performance certificate or
claim that every historical request is implemented. Related-product results
below are reported by those deliveries; they were not all rerun in this review.
The release remains alpha.

## Delivered work that must not be counted as missing

| Area | Available implementation / source evidence | Remaining qualification |
| --- | --- | --- |
| Source checking | Checked PBLang frontend and public `pblang check`; semantic diagnostics produce failure | Separate retained compiler routes and broad backend parity |
| Nested colon calls | `compiler/native_pbck.sapp`, `tools/pbc.sapp --target pbck`, real method/pipeline regressions | Direct retained-source compiler still has the old implicit-capture defect; imported source is not automatically migrated |
| Exception cleanup | Native explicit entry/cleanup boundary preserves primary error message; self-host scalar cleanup covers controlled failures and escaping guest index operations | General host/FFI errors, crashes and identity-aware composite ownership |
| Async ownership | Owned race results, explicit settlement, validated worker counts, isolated workers and pipeline facade | Cancellation/preemption, uncatchable host failures and latency/resource regressions |
| Student persistence | Shared pbAI checkpoint codec, detached restored student and explicit private file operations (`ab03c39`) | Linux-only file boundary, training continuation, crash durability, encryption and representative model evaluation |
| Collection ordering | Stable cached-key sort and index permutation (`f4532b3`) | Simple keys can cost more; the new functions remain explicit choices |
| Study/lesson workflows | Worksheet generation (`60996de`), deterministic deck mixing (`b473851`); PBQuiz/PBLesson/PBDeckMix/PBStudy source deliveries | Full browser/mobile editing, signed installers and real target-device testing |
| Allocation scenarios | Two-total comparison with identical weights (`1161bea`), exposed by PBPortions | Physical platforms and editable browser/mobile workflow |
| Graphics | Shared card presets and per-card rendering options (`20fcce5`) | Full graphics-category coverage and actual Windows/macOS/iOS execution |
| Other product repairs | PBWorkdays invalidates stale export; pbOS parses exact ELF64 header integers | Remaining product catalogue; ELF execution is not implemented by a header parser |
| Discovery-derived examples | Bounded physical-unit/print-preview examples (`d05ff3d`) | Source discovery and proposal implementation remain partial; no renewed scan is implied here |

The referenced PBLang commits are ancestors of the reviewed main revision.
See [the portfolio roadmap](PRODUCT_PORTFOLIO_ROADMAP.md),
[student checkpoint contract](pbAI_Student_Checkpoints.md),
[functional sorting](FUNCTIONAL_SORT.md) and
[native PBCK compilation](NATIVE_PBCK_COMPILATION.md).

## Ordered unfinished work and completion criteria

| Priority | Work | Required evidence before calling it complete |
| --- | --- | --- |
| 1 | Retained-source nested method/capture defect | Original loop/try/catch reproducer and imported-module variants must pass on the actual selected entry route. A PBLang-to-PBCK workaround is not a repair of the Rust source compiler. New product implementation remains PBLang-only. |
| 1 | General exception/resource cleanup | Preserve the primary failure while releasing every owned handle exactly once; test nested catches, cleanup failure, cancellation, borrowed contexts and external effects. Do not replay arbitrary operations or claim recovery from native crashes. |
| 1 | Composite identity/cycles | Specify shared VM/FFI ownership, preserve alias semantics, collect unreachable cycles without freeing reachable values, and test resurrection/reentrancy/concurrent use before default enablement. |
| 2 | GC pause bounds and async overhead | Repeated identical workloads including startup, steady state, contention, waiting and failure paths; median/tail latency and peak memory. Previous regressions remain open until reproduced against their original baselines. |
| 2 | Platform and file/process boundaries | Actual Windows lock publication/race tests, stable process lifecycle, ABI validation and fresh target-specific evidence. Generated code or a Linux pass does not qualify another platform. |
| 2 | Product installation/integration | Identify the module actually loaded by each installed product; source tests plus installed end-to-end checks. The pinned desktop PB-MCP does not contain this self-host VM, so replacing unrelated bundle contents would not establish benefit. |
| 2 | CI and release gates | Account-level GitHub Actions startup failure must be resolved and required jobs rerun. Local tests cannot substitute for a claimed hosted matrix pass. No billing or protection settings are changed by this work. |
| 3 | Native web/product completeness | Persistent account workflows, listener restart/TLS, safe publication and deployment, preserving data and privacy; cryptographic replacements require independent correctness/security evidence. |
| 3 | Form/application conversion and ND UI | Explicit supported controls/events/business logic, fail-closed unsupported input, source-preserving overlays and end-to-end interactive target tests. Layout import alone is not whole-application conversion. |
| 3 | Polyglot replacement and devices | Replace each verified capability with an equivalent PBLang path before disabling its preserved foreign implementation. GPU/NPU/device dispatch requires measured end-to-end benefit including transfers and precision. |
| 3 | Tooling/packaging | Debugger/LSP correctness, signed reproducible installers/updates, platform matrix and consistent documented contracts across CLI/GUI/MCP. |

A current work item can close without closing every row. Keep its exact test,
revision, entry route and installation scope with the result. Preserve the old
measurements; do not redefine workloads to turn a regression into a speedup.

## Runtime index-cleanup refinement

The follow-up VM change keeps the successful primitive access and its error
handler together, eliminating an additional helper frame per managed access.
The same ownership and outer-handler tests remain; private state presence is
checked dynamically because allocation may occur after entering a frame.
The cold handler repeats only an already-failed, synchronous primitive on
already-evaluated operands, under the documented retained-host assumptions.
No guest expression, function, callback or FFI operation is replayed.

`selfhost_index_cleanup_test.sapp` also covers changing receivers/keys during
repeated accesses and allocations made after frame entry. Performance must be
reported per measured workload; this change is not a general GC or async fix.

## Static-linking follow-up

The `compiler/native_bundle.compile_bundle` entry now compiles supported static
modules through PBLang as well as the main source. The imported nested-method
reproducer passes under the native VM sandbox without dependency files. Linker
admission rejects computed/aliased/nested imports, cycles and bounded-graph
violations instead of returning an artifact with hidden retained-source imports.
Whitespace, comments and decoded literal paths work. Existing filesystem/dist
bundle consumers share this fix. See the explicit initialization, shared-global,
size and resolver limits in `NATIVE_PBCK_COMPILATION.md`.

This closes the hidden-import admission defect and supplies an explicit route
for the imported reproducer. Direct retained-source execution, generic host/FFI
cleanup, cycles/pauses, async regressions and the installed desktop PB-MCP remain
separate unfinished work. The nine previously reviewed product deliveries were
checked for newer task updates; no newer delivery was found in this follow-up.

## Static-bundle CLI integration

The public PBLang compiler tool now accepts `--target pbck --bundle`, backed by
`compiler/native_bundle.compile_file`. The file-based example and live CLI test
cover imported nested method calls, transitive dependencies, source deletion and
runtime capability denial. Unsupported target/cleanup combinations fail before
input reads and compilation failures preserve an existing output artifact.
The previous nine-task delivery inventory was rechecked for newer updates;
no newer delivery was found. This completes this CLI integration only. Direct
retained-source compilation, universal host/FFI cleanup, cycles, GC/async latency,
platform qualification and installed desktop PB-MCP integration remain open.
