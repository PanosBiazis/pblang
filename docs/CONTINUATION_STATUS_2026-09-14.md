# PBLang continuation status — 2026-09-14

This status reconciles prior work with the present compiler change. Historical
reports are not new validation of the current checkout. Product changes remain
PBLang-only. This continuation performs local checks and a local commit only;
it does not deploy or upload PBLang to a server.

## Corrected platform picture

Earlier work recorded Kali guest execution, Windows 11 VM checks for PBRemote,
and Android emulator application checks. It is therefore inaccurate to say
that no guest operating-system tests have ever run. The retained Kali report
`kali-semantics-portable-8e5d612-v4.json` is present locally and declares
`ok=false`. Prior Windows/Android application reports were not found at their
previously recorded locations during this reconciliation. None of these is
current-platform acceptance evidence for the present compiler revision.

## Current local validation

The x64/cache admission change passed 50 selected suites, including 17 reference
emulator suites, native x64 execution and real WASM execution in Node/V8. The
seven touched PBLang files passed self-hosted checking with zero errors/warnings
and mandatory security scans with no findings. Retained lint completed with
style/conservative unused-variable warnings. The retained-bootstrap project
build and repository-relative dependency lock passed. The cache test was rerun
after its cleanup was made compatible with the self-hosted checker.

Graphify extraction ran locally but the update did not complete within 45
seconds; no graph-freshness claim is made. No guest OS, server deployment or
remote test runner was used for this change.

Current checked-source work tests generated artifacts locally and distinguishes
reference emulators from guest operating systems. A complete current guest
matrix, physical-device support and general production readiness remain open.

## Workstreams and next acceptance evidence

| Workstream | Established progress | What remains |
| --- | --- | --- |
| Compiler | Checked numeric tokens; bounded WASM/ARM64 admission; x64 admission including cached builds in this continuation | C99/interpreter consumers, broader language/ABI parity, current cross-backend execution matrix |
| Runtime/FFI | Existing resource contracts and selected boundary checks | Hard memory/time limits, typed native dispatch and hostile-code isolation with actual enforcement evidence |
| Audits | Policy preflight, checkpoint/counter validation and retained-finding bounds | Whole-report work/decoding budgets and trusted report provenance |
| Releases | Selected signature/trust/artifact-copy checks | Complete publisher selection, revocation, rollback policy and immutable handoff |
| PBIDE | Drafts, Check, preview, download and bounded UTF-8 import | Live highlighting/caret, Run/Designer integration, bidirectional forms and real UI acceptance |
| VM/device lab | Historical guest and emulator work | Reproduce against current artifacts, resolve failed guest baselines, verify claimed OS/device capabilities |
| Related products | Prior PBRemote/MCP and pbAI integration work | Keep module boundaries and approvals explicit; do not confuse plans, manifests or catalog entries with working execution |
| Maintenance | Isolated validation and preservation of unrelated work | Review remaining local changes independently and complete knowledge-graph regeneration |

For the exact compiler/caching guarantee and its limits, see
[backend source admission](BACKEND_SOURCE_ADMISSION.md). For accumulated platform
workflow documentation, see [VM test lab](VM_TEST_LAB.md). Neither document is a
claim that all listed platforms or features pass today.
