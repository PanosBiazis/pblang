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

The current slice adds explicit WebAssembly local source admission via
`compile_wasm_local` and `pbc --target wasm --local-source`, with optional
developer byte/token budgets. Existing strict APIs and bounded facades stay
strict. CLI policy errors reject before reading input, and failed compilation
preserves an existing output. Unknown/duplicate arguments now reject instead of
being silently ignored; legacy trailing source arguments remain supported.

The shared emitter sizes initial linear memory after final static data is known,
retaining the old 16-page minimum and checking memory32 representation. Type
inference now reaches its finite monotone fixed point instead of silently
stopping after 200 rounds. A 210-function regression failed before that correction
and passed afterward in Node/V8. This fixes a correctness restriction; it is not
a performance benchmark. Static-memory sizing does not add runtime heap growth.

Current validation passed 43 selected suites, including 17 emulator suites,
real Node/V8 execution, actual local `pbc` artifact execution and the preceding
interpreter/C99 regressions. Five changed source files passed self-hosted
checking with zero errors/warnings and strict entry scans with complete mandatory
analysis, zero blocking findings and stable snapshots. Retained lint passed with
conservative/style warnings; independent security review found no new blocker.
The fixed-point solver can still take quadratic work for long chains; no
performance improvement is claimed. The high memory32 representation endpoints
were reviewed but not exercised with multi-gigabyte allocations. Isolated and
main retained-bootstrap builds and portable locks passed. Graphify was stopped
after its 45-second update window; graph freshness remains unverified. No server,
guest operating system or remote runner was used for this slice.

The previous local-source slice separated interpreter/C99 admission as follows.

The preceding continuation separates local developer source admission from strict
input budgets. `pbi`, explicit interpreter library entry points and hosted C99
local translation/build/run support source above the former direct 1 MiB /
65,536-token ceiling without preset byte/token caps. Optional budgets are exact,
validated before effects, and token caps stop production early. Lexical and parser
errors still reject before execution/emission. Existing strict APIs are unchanged.
The CLI still reads the whole file before admission; this is not a streaming
memory guarantee. Existing parser nesting guards and backend/runtime limitations
remain. See [local compilation](LOCAL_COMPILATION.md).

Validation: 57 distinct selected suites passed, including 17 emulator suites,
real pbi execution and a real C99 executable from a source above both former
ceilings. Optional budgets, UTF-8/EOF boundaries, malformed large suffixes,
old-output preservation, runner option forwarding and numeric-mode restoration
were exercised. The synthetic input establishes admission behavior, not a
performance benchmark or unrestricted language/backend compatibility.
The seven changed PBLang files passed self-hosted checking with zero errors or
warnings and strict entry-source scans with complete mandatory analysis, zero
blocking findings and stable snapshots. Retained lint passed with conservative
and style warnings. Independent security review found no material blocker. Isolated and main
retained-bootstrap builds and portable locks passed. The knowledge-graph update
was stopped after 45 seconds; no current-graph claim is made. No server, guest
operating system or remote runner was used.

The preceding continuation restores literal defaults in named interpreter functions.
Omitted arguments use their number/string/bool/nil defaults on all call paths;
explicit nil remains nil. Existing required/extra-argument behavior is preserved.
Defaults are decoded once, never evaluated as expressions. A live negative test
rejects a prepared call substituted as a default with a fixed diagnostic and no
print effect. The shared positive fixture passes both the retained host and pbi.
No new source-size, token-count or wall-clock limit was introduced.

Forty selected suites passed, including 17 emulator suites, existing interpreter
and admission regressions, default diagnostics and local driver parity. Five
touched source files passed the self-hosted checker with zero errors/warnings
and strict security scans with complete mandatory analysis, zero blocking
findings and stable snapshots. Isolated and main retained-bootstrap builds and
portable lock publication passed. Retained lint completed with conservative/style
warnings. Independent security review found no new blocker. Mutable closure
arrays remain an existing isolation gap, documented in INTERPRETER_SCOPES.md.

Large-program compatibility remains a continuation priority beyond the new local
interpreter/hosted-C99 route. Smart builds, direct x64/ARM64/WASM routes, module
linking and security/dependency scans retain separate limits. The checked C99
module profile's 16 MiB / 1,048,576-token ceiling is not universal backend
support. See [backend source admission](BACKEND_SOURCE_ADMISSION.md).

The preceding tree-interpreter continuation fixes stale lexical-depth reuse after
late shadowing declarations and across closures sharing an AST. Repeated binding
names use the existing dynamic lookup for reads and writes; uniquely declared
names retain caching. Prepared-AST version 3 rebuilds earlier metadata. This
correctness change passed 36 selected suites, including 17 emulator suites and
actual local `pbi` driver execution. Five touched PBLang files passed strict
entry-source security scans with complete mandatory analysis, zero blocking
findings and stable snapshots. Independent security review found no new blocker.
The self-hosted checker reported zero errors and one expected unused-variable
warning for the deliberately shadowing fixture; retained lint completed with
style/conservative warnings. The retained-bootstrap build and portable lock
passed in both isolated and main working copies. No new performance claim is made.

A retained-bootstrap comparison of the same fixture produced different captured
binding behavior. The fix restores consistency with the tree interpreter's own
generic lookup, not cross-runtime parity. See [interpreter scopes](INTERPRETER_SCOPES.md)
for the exact output difference and trusted-AST limits. Existing source admission
checks remain unchanged. The earlier native slice below is historical validation,
not a rerun of all its native suites against this interpreter revision.

The preceding native continuation bounds all eight bytes of type-predicate tag
reads and rejects invalid/non-finite heap configuration before state changes.
`compile_safe`, cache/session results and the CLI now state the limited scope
and explicitly deny memory-safety certification and runtime isolation. Legacy
`safe` remains only for compatibility. General native `type()` is still blocked
by scalar/pointer representation collisions; no guessed lowering was added.
This slice passed 57 selected suites including 17 emulator suites, real native
boundary/CLI execution, cold/warm cache and smart-build publication, and Node/V8
WASM execution. The original end-minus-one native tag probe exited with SIGSEGV;
all tested mapping-edge predicates now complete normally at three heap sizes.
Thirteen touched PBLang files passed self-hosted checking with zero errors or
warnings and mandatory runtime-security checks. Twelve source scans had no
blocking findings; the smart-build test retains its two deliberate negative
fixture findings, unchanged from the baseline. Retained lint completed with
style/conservative warnings. The optional
CLI path variables now declare their dynamic type explicitly, correcting
pre-existing retained-lint failures without changing path selection.

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

The preceding shadow-lookup slice completed isolated Graphify regeneration (8,591
nodes and 11,062 edges), but the main repository update exceeded its 45-second
bound and was stopped. The current literal-default slice also attempted a main
update and stopped it at 45 seconds. The main graph is not claimed current. Earlier C99 and
native attempts also had incomplete main updates. No guest OS, server deployment,
push or remote test runner was used for these changes.

Current checked-source work tests generated artifacts locally and distinguishes
reference emulators from guest operating systems. A complete current guest
matrix, physical-device support and general production readiness remain open.

## Workstreams and next acceptance evidence

| Workstream | Established progress | What remains |
| --- | --- | --- |
| Compiler | Explicit local interpreter/C99/WASM source budgets, data-sized WASM memory and convergent string-type inference; named-function literal defaults and corrected interpreter late-shadow lookup; checked numeric tokens; WASM/ARM64/x64 admission including caches; C99/interpreter admission and independently checked linked files | Extend developer budgets across smart builds/backends and separate compilation; repair ARM64 branch/frame/slot encoding range hazards before widening its source admission; bootstrap closure-binding parity, native-selfhost interpreter builtin lowering, broader language/ABI parity, current cross-backend execution matrix |
| Runtime/FFI | Existing resource contracts, bounded native type-tag reads, heap configuration validation and explicit uncertified scope | Hard memory/time limits, typed native dispatch and hostile-code isolation with actual enforcement evidence |
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
