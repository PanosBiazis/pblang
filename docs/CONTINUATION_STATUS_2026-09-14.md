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

The latest C99/interpreter/checked-linker change passed 75 selected suites,
including 17 reference emulator suites, native C99/x64 execution, sanitizer
checks of linked module globals and real WASM execution in Node/V8. The ten
touched PBLang files passed self-hosted checking with zero errors/warnings and
mandatory security scans. Independent security review found no material blocker.
Retained lint completed with style/conservative unused-variable warnings. The
retained-bootstrap project build and repository-relative dependency lock passed.

The linked-global AddressSanitizer test initially could not reserve shadow
memory under the external 2 GiB virtual-address test cap. The unchanged suite
passed with that virtual cap removed. All other test resource limits remained.
The older x64/cache slice separately passed 50 suites before this continuation.

The native self-hosted interpreter harness now includes and rewrites the new
checked-source dependency. A single current direct-x64 compilation probe still
rejects with `codegen: call to undefined fn type`: required builtin lowering
remains incomplete. The interpreter's retained-host semantic suites pass, but
this is not a successful native-selfhost interpreter validation.

Graphify AST regeneration completed in the isolated checkout. Updating the
main working repository did not complete within 45 seconds, so its graph is
not claimed current. No guest OS, server deployment, push or remote test runner
was used for this change.

Current checked-source work tests generated artifacts locally and distinguishes
reference emulators from guest operating systems. A complete current guest
matrix, physical-device support and general production readiness remain open.

## Workstreams and next acceptance evidence

| Workstream | Established progress | What remains |
| --- | --- | --- |
| Compiler | Checked numeric tokens; WASM/ARM64/x64 admission including caches; C99/interpreter admission and independently checked linked files | Native-selfhost interpreter builtin lowering, broader language/ABI parity, current cross-backend execution matrix |
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
