# Deep linking, memory contracts and resource validation — 2026-09-15

## Implemented and unchanged behavior

The compatibility source inliner now uses an explicit dependency stack while
preserving resolver order, canonical identities, failures and emitted source.
The bounded linker is unchanged. No new project-size or compilation-time limit
was introduced. [The API notes](SOURCE_INLINER_TRAVERSAL.md) describe its scope.

The memory API and tutorial previously overstated automatic cleanup. `SafeRef`
has no lexical destructor. VM lifetime scopes release scoped borrows and
invalidate scoped owners, but retain owner/value entries until explicit free.
Documentation and examples now distinguish those operations. Regression tests
verify both borrow counts and allocation-entry counts, including explicit free
of expired handles. No VM allocator or FFI implementation was changed.

## Measurements

Baseline: `9177307`. Same Linux x86-64 host and retained executable for both
versions; executable SHA-256:
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.
The fixture is `bench/source_inliner_deep.sapp`, with count changed identically
in each version. One warmup per version/count, five measured samples per
version/count, alternating execution order. p95 uses nearest rank, so with five
samples it is the maximum. The host was shared with a bounded metadata scan
and normal desktop activity; these are local workload observations, not an
isolated machine or universal performance guarantee.

`link_ms` includes linking, validation and output/order hashing. Whole-process
measurements include interpreter startup. All corresponding output/order hashes
matched. Both old and new implementations completed 12,000 modules; no old
stack crash was reproduced. The linked 12,000-function program was not executed.

| Modules | Version | Link + identity median / p95 (ms) | Process median / p95 (s) | Peak RSS median / p95 (KiB) |
| ---: | --- | ---: | ---: | ---: |
| 40 | before | 17.80 / 20.66 | 0.91 / 0.93 | 33244 / 33436 |
| 40 | after | 17.88 / 19.23 | 0.93 / 0.96 | 33280 / 33444 |
| 2000 | before | 1105.55 / 1178.43 | 2.04 / 2.22 | 40336 / 40692 |
| 2000 | after | 1019.36 / 1184.80 | 1.96 / 2.56 | 39204 / 39508 |
| 12000 | before | 14040.68 / 14064.90 | 15.03 / 15.10 | 122576 / 123080 |
| 12000 | after | 6146.38 / 6788.87 | 7.20 / 7.81 | 115908 / 116404 |

At 12,000 modules the measured link-plus-identity median fell 56.2%, process
elapsed median fell 52.1%, and median peak RSS fell 5.4%. At 40 modules there is
no meaningful speedup and median RSS is 36 KiB higher. The 2,000-module process
p95 is worse despite the better median. No all-workload improvement is claimed.

A separate 12,000-module CPU pass used the same warmup/sample protocol and GNU
time process accounting. Total CPU is computed per sample as user + system,
then summarized. This avoids confusing time spent waiting with processor work.

| Metric | Before median / p95 | After median / p95 |
| --- | ---: | ---: |
| User CPU (s) | 14.66 / 16.15 | 6.71 / 7.77 |
| System CPU (s) | 0.44 / 0.49 | 0.42 / 0.44 |
| Total CPU (s) | 15.10 / 16.64 | 7.13 / 8.18 |
| Elapsed (s) | 15.11 / 16.64 | 7.14 / 8.19 |
| Peak RSS (KiB) | 122724 / 123276 | 116024 / 116260 |
| Involuntary context switches | 374 / 733 | 287 / 456 |
| Voluntary context switches | 4 / 4 | 4 / 4 |

Total CPU median fell 52.8%. The product change adds no threads or background
polling. These samples do not measure CPU cycles, power, thermals, GPU/NPU cost
or program-runtime performance. Warm measured samples reported zero filesystem
block operations; cached reads can still occur, so this is not a no-I/O claim.

## Artifact size and writes

One local generation run traced successful `write`, `writev` and `pwrite64`
calls for four WASM artifacts. Raw syscall output was discarded; only aggregate
counts were retained. All four hashes match the artifacts in the
[previous platform report](PATH_GRAPH_VALIDATION_2026-09-15.md).

| Artifact | Logical bytes | Locally allocated bytes | Observed bytes written | Artifact write calls |
| --- | ---: | ---: | ---: | ---: |
| sum | 86 | 4096 | 86 | 1 |
| recursion | 86 | 4096 | 86 | 1 |
| strings | 524 | 4096 | 524 | 1 |
| multifile | 64 | 4096 | 64 | 1 |

No other regular-path writes were observed in those traced calls. Final logical
size improvement is **0%** for these artifacts. This does not measure native
executable packaging, peak full-build temporary space, memory-mapped writes,
filesystem metadata or physical SSD write amplification. It also does not
measure application-generated files during long-running program execution.
Disk optimization remains a separate follow-up, not a consequence of faster
source linking.

## Correctness and platform checks

39 semantic suites ultimately passed: 16 linker/compiler/tooling suites,
17 emulator/device/input suites, and 6 memory suites. The freeze suite initially
failed because the isolated checkout lacked the expected retained executable
path; it passed after providing that path. This was a test-setup correction.
The memory tutorial also executed successfully.

The new traversal suite checks exact baseline source for shared/prefixed diamond
graphs, callback and dependency ordering, cycles, missing and escaping imports,
resolver exceptions, and 3,000-module chains. Both flattened small programs
execute with the expected result. The memory regression checks successful,
failing, nested and 100 repeated failing lifetimes, manual guard behavior,
expired-owner retention, explicit reclamation and final allocation counts.

Retained build and portable dependency lock refresh passed. Six touched PBLang
files passed self-hosted checks and lint with zero self-hosted diagnostics.
The retained bootstrap linter still reports existing unused-variable warnings.
Six strict entry-only security scans completed mandatory checks with zero
blocking findings and stable snapshots; these are not dependency-wide security
proofs. Independent review identified the allocation-reclamation overclaim,
which was corrected and covered by the new count assertions.

The same four WASM artifacts ran on Linux Node 18.19.1 and the approved Windows
11 Pro workstation's existing Node 26.8.2, producing 49995000, 144, 300 and 42.
Both engines rejected half-truncated copies. The multi-file case was regenerated
through the new compatibility traversal. No remote software or test program
files were installed, and no EnterpriseOrg server was used. This validates those
artifacts on a real Windows OS, not a native Windows PBLang runtime or full
platform coverage.

Additional local AArch64 QEMU execution passed all five fixed cases: add (12),
multiply (42), bounded loop (11), zero branch (19), and signed compare (1).
The existing cached executable was measured and held through the tool's
verified-descriptor path. Every case passed its lifecycle/cleanup comparison.
This is userspace emulation, not a booted guest OS or a physical ARM device;
the report retains `independent_execution_verified=false`.

Local VM inventory found six registered domains, all shut off. The read-only
VM doctor rejected the existing manifest: two guest entries contained OS,
evidence, suite and baseline fields unsupported by the current schema. No VM
was started and no configuration was rewritten. Resolving the lab/controller
contract is required before reporting current guest-OS test coverage.

### Cross-platform testing requirement

PBLang's development target includes old and modern operating systems and
devices, from constrained hardware to powerful general-purpose machines. This
is a design and test requirement, not a statement of universal current support.
Future validation should preserve four independent axes: OS/version, CPU/ABI,
device/peripheral capability, and available resources. A passing ISA model does
not establish host-OS integration, and a new workstation result does not replace
legacy or low-memory testing.

| Target family | Current evidence in this slice | Still unverified here |
| --- | --- | --- |
| Linux x86-64 | retained runtime, C99 cases, compiler/memory suites | other distributions/versions and all libraries |
| Windows x86-64 | four generated WASM cases in workstation Node | native runtime and legacy Windows versions |
| ARM64 | PBLang model and five external QEMU cases | guest OS, physical boards and peripherals |
| Other legacy/embedded ISAs | selected 6502, Z80, AVR, MSP430, MIPS, PowerPC, SPARC, RISC-V model subsets | full ISA, real firmware/RTOS and devices |
| Android/device models | selected semantic emulator suites | current APK/physical-device integration |
| BSD, macOS, iOS and other/legacy OS families | no current execution evidence | native/guest/device validation |
| GPU, NPU and specialized peripherals | no current execution evidence | measured backend correctness and resource cost |

Test infrastructure must remain local to the authorized laptop/VMs/emulators
or the separately authorized corporate workstation. Broader platform intent
does not authorize any EnterpriseOrg or Hetzner server testing or deployment.

## Discovery coverage and follow-up ideas

The visible catalog was refreshed and three additional relevant conversation
excerpts about modular electronics, embedded programming and interactive
simulations were read. This extends the previous bounded intake; it does not
establish full cloud-history coverage. Earlier assistant proposals are ideas,
not evidence of implemented features or current hardware capabilities.

An automated UTF-8 source-content keyword pass covered 761 files (10,990,298
bytes) in selected PBLang, PBRemote, PBOffice and pbAI source directories.
It was not a complete semantic audit of those files. Relevant owning linker
and memory modules, callers, contracts, tests and examples were examined in
detail. A separate project-volume pass examined up to 600 eligible source/form
files. That 1,200-second pass enumerated 579,784 file entries and 64,762
directories, read 600 eligible files (6,584,473 bytes), and skipped 471 files
that failed UTF-8 decoding. The metadata walk hit its deadline and the content
pass hit its file budget; both remain incomplete. These repeated-scan counts
must not be added to the previous volume inventory as unique files, nor are
the two content passes guaranteed disjoint. Private source text and personal
paths were not copied into this report, fixtures or committed artifacts.

Useful leads are explicit lifetime/resource APIs; measured streaming and
write-if-changed output; shared document/history and declarative form models;
reusable hardware capability descriptors; and interactive circuit/simulation
libraries. These are development candidates requiring ownership, semantics and
platform tests. They do not authorize importing proprietary code or training
models on discovered content.

## Remaining work

Graphify's AST-only update completed in 44.43 seconds with clustering disabled.
The installed extractor excludes `.sapp`, so it does not provide full PBLang
graph coverage. Its isolated local snapshot preserves the existing graphs.

Automatic owner-storage reclamation is still incomplete. Further work includes
runtime/FFI enforcement, multi-file compilation across the other backends,
incremental builds and temporary-space measurements, library improvements,
authenticated audits/releases, PBIDE Run/Designer, and wider guest/device tests.
Full disk-content and cloud-history intake remain unfinished. This slice is
not a production-readiness or universal resource-efficiency claim.
