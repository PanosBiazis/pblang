# PBLang continuation status — 2026-09-14

This status reconciles prior work with the present compiler change. Historical
reports are not new validation of the current checkout. Product changes remain
PBLang-only. This continuation performs local checks and pushes reviewed source to the existing
GitHub repository with CI skipped; it does not deploy or test PBLang on a server.

## Corrected platform picture

Earlier work recorded Kali guest execution, Windows 11 VM checks for PBRemote,
and Android emulator application checks. It is therefore inaccurate to say
that no guest operating-system tests have ever run. The retained Kali report
`kali-semantics-portable-8e5d612-v4.json` is present locally and declares
`ok=false`. Prior Windows/Android application reports were not found at their
previously recorded locations during this reconciliation. None of these is
current-platform acceptance evidence for the present compiler revision.

## Current local validation — 2026-09-15 ARM64 compiler allocation cost

The ARM64 emitter now counts unique slots in constant time without allocating
map-key lists. It also patches instruction bytes directly and expands long
branches backwards inside the original code array, after validating the entire
final layout. This removes the explicit second result array; it does not add a language
source-size or runtime limit or alter the strict ARM64 admission API.

The identical 4,100-local workload compiled about 15.8–16.9 times faster across
three alternating baseline/candidate pairs. The 24-nested-branch workload used
about 4.4%–4.5% less peak compiler RSS, with median compilation time about 4.1%–8.1%
lower. Small-program timings and large-slot RSS varied; there is no universal
speed or resource non-regression claim. All 18 measured processes are retained.

Current tests cover 49 selected suites (43 regular, including 17 emulator suites,
and six opt-in ARM64 suites), plus 12 independent local QEMU executions. Existing
ten goldens are unchanged. Additional byte-for-byte comparisons cover the 4,100-
local benchmark, 24 nested branches and 1,000 short conditions inside a long branch.
QEMU artifacts match their prior SHA-256 values and expected output/exit codes.
The new slot test covers name reuse, nested declarations and stack parameters.

All four touched PBLang files passed individual self-hosted checking, retained
lint and strict entry security scans with complete mandatory analysis, zero
blocking findings and stable source snapshots. Entry scanning is not a complete
semantic/dependency audit. Independent security review found no blocker.

Graphify completed a local AST-only refresh without clustering; installed support
excludes `.sapp`. A separate native lexical graph analyzed the compiler's 20
`.sapp` files with distinct stems. Neither result is full-project dependency or
platform acceptance evidence. The aggregate AAP graph was preserved.
See [local refresh procedure and limits](GRAPH_REFRESH.md),
[implementation and measured costs](ARM64_ENCODING.md),
[current correctness evidence](ARM64_LAYOUT_RESULTS.json) and
[all performance measurements](../bench/results/arm64_layout_2026-09-15.json).

The remaining work is still substantial: multi-file compiler scalability and
wider admission, runtime heap growth/allocation failure and stack probing,
runtime/FFI isolation, library/backend compatibility, audit/release authenticity,
PBIDE Run/Designer and current guest/hardware platform validation. This slice
addresses compilation cost and local graph visibility, not those guarantees.
All execution was local; no EnterpriseCore server was used.

## Previous local validation — 2026-09-15 ARM64 long branches

Conditional source branches now expand beyond signed imm19 into an inverse
condition plus B. Source control flow, calls and string addresses are relocated
together after final function layout. Small code retains its previous bytes.
No new language source-size or execution-time cap was added; the ARM64 strict
source API remains unchanged.

Validation passed 48 selected suites: 42 regular suites, including 17 emulator
suites and real Node/V8 execution, plus six opt-in ARM64 suites. Twelve long
programs passed both the PBLang reference and independent local QEMU, covering
both conditional outcomes, short circuiting, 24 nested branches, moved calls,
strings and forward/backward unconditional jumps. The exact positive imm19
endpoint remains compact and the next offset expands correctly through the CLI.
Ten whole-artifact goldens remain byte-identical to their respective baselines.

All four changed PBLang sources passed individual self-hosted checking and strict
entry scans with complete mandatory analysis, zero blocking findings and stable
snapshots. Entry scans do not claim complete semantic/dependency analysis. Retained
lint passed with conservative unused-variable warnings; independent security
review found no blocker. The ordinary-source benchmark retained all six alternating
processes: median compilation times were similar (-0.3% to +1.3%), while measured
peak compiler RSS was 0.9%–1.5% higher. Strict resource non-regression is not proven.
Expanded branches add one instruction; large layout temporarily holds two arrays.

See [implementation, costs and remaining limits](ARM64_ENCODING.md),
[QEMU evidence](ARM64_BRANCH_QEMU_RESULTS.json) and
[compiler measurements](../bench/results/arm64_branches_2026-09-15.json).
The tests were local; no EnterpriseCore server or remote deployment was used.
Graphify update was terminated after 45 seconds without completing.

Remaining work includes multi-file projects and wider source admission with
stack probing, automatic heap growth/allocation-failure handling, imm26-range
lowering, runtime/FFI isolation, audit/release authenticity, PBIDE Run/Designer,
compiler/library compatibility and current guest/hardware platform acceptance.

## Previous local validation — 2026-09-15 ARM64 large literals/calls

Array/map literal allocation and element stores, outgoing call areas, staged
argument loads and cleanup now expand to valid instructions. Call173, array512
and map256 compile instead of failing at the former compact encoding boundary.
Strict source admission, ordinary compact instruction bytes and existing runtime
heap behavior are unchanged. No new source-size/time cap was added.

Validation passed 47 distinct selected suites: 42 regular suites (including 17
emulator suites and real Node/V8 execution) plus five opt-in extended ARM64
suites. Eight generated programs also passed in independent local QEMU, covering
arrays up to 4,095 elements, maps up to 2,048 entries and calls with 4,105 arguments.
Nested argument order and caller preservation matched the PBLang reference.
Ten complete normal/boundary artifacts remain byte-identical to their baselines.

All seven touched PBLang sources passed checking with zero errors/warnings and
strict entry scans with complete mandatory analysis, zero blocking findings and
stable snapshots. Retained lint passed with conservative unused-variable warnings;
independent security review found no new blocker. Previous compiler RSS/timing
measurements are historical evidence for the previous frame-encoding slice, not
new measurements of this extension. No all-device performance guarantee is made.

QEMU was authenticated through Ubuntu package metadata and extracted temporarily;
there was no system installation. These are local user-mode emulation tests, not
native hardware or guest-OS acceptance. No server or EnterpriseCore infrastructure was
used. See [implementation and reproducible fixtures](ARM64_ENCODING.md) and the
[QEMU result record](ARM64_WIDE_QEMU_RESULTS.json).

Isolated and main retained-bootstrap builds and repository-relative lock
publication passed; the final main build was clean after dependency refresh.
Main boundary/fingerprint/live rejection tests passed too. All 212 pre-existing
dirty/untracked files were preserved, with only the necessary working lock hash
refreshed. Graphify exceeded its 45-second update window and was terminated;
graph freshness remains unverified.
The user authorized Git push; automatic remote workflows are skipped for this
push to keep this validation local.

## Previous ARM64 frame-encoding validation

The ARM64 AST backend now expands frame adjustments and distant frame slots
instead of corrupting instruction fields. Incoming stack arguments share the
wide addressing path. Branch/compact immediate overflows reject with fixed
errors, preserving an existing CLI output. Strict source-admission policy is
unchanged; this adds no source-size or execution-time restriction to the language.
Large literals, outgoing argument areas and long branches still need expanded
lowering; runtime heap growth and stack probing remain pending.

The byte emitter removes one temporary array per instruction. Twenty benchmark
processes retained all measurements, with identical workloads and seven identical
ELF fingerprints. Peak compiler RSS was consistently about 38% lower. Elapsed
and CPU times were variable, including slower candidate runs; CPU speedup and
non-regression are not established. No GPU/NPU/peripheral performance claim is
made. See [ARM64 implementation, commands and measurements](ARM64_ENCODING.md).

Validation passed 43 distinct selected suites: 41 regular suites (including the
17 emulator suites and real Node/V8 execution) plus two opt-in extended ARM64
probes. Large frames and incoming arguments ran in the PBLang reference, not
on real ARM hardware or QEMU. Positive imm19 boundary and next-offset rejection,
large literal/argument rejection and real local CLI old-output preservation
passed. Baseline regressions reproduced the corrupt 4,096-byte frame encoding
and acceptance of an unrepresentable positive imm19 offset before the fixes.

All eight touched source files passed checking with zero errors and two unused
benchmark-loop-variable warnings. Strict entry scans had complete mandatory
analysis, zero blocking findings and stable source snapshots. Retained lint
completed with conservative/style warnings. Independent security review found
no blocker in the final retained files. A proposed standalone test runner was
removed after review exposed false-pass and supervision gaps; new ordinary
regressions use the existing semantics suite and heavy probes remain opt-in.
A standalone supervised gate still requires an available audited process backend.

Isolated and main retained-bootstrap builds and repository-relative lock
publication passed. The final main build was clean after the dependency hash
refresh. All 212 pre-existing dirty/untracked files were preserved, with only
the necessary working dependency hash refreshed. Main-checkout frame, fingerprint,
execution and live rejection checks also passed. Graphify was terminated after
its 45-second update window; graph freshness remains unverified. No server,
remote runner or guest OS was used. This slice is committed locally only.

## Previous WebAssembly local validation

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
| Compiler | Explicit local interpreter/C99/WASM source budgets, data-sized WASM memory and convergent string-type inference; named-function literal defaults and corrected interpreter late-shadow lookup; checked numeric tokens; WASM/ARM64/x64 admission including caches; C99/interpreter admission and independently checked linked files | Extend developer budgets across smart builds/backends and separate compilation; expand remaining ARM64 long branches and validate stack/runtime behavior before widening its source admission; bootstrap closure-binding parity, native-selfhost interpreter builtin lowering, broader language/ABI parity, current cross-backend execution matrix |
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
