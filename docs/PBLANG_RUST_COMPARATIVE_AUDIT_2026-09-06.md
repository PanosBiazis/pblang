# PBLang 1.0.0 release-readiness assessment and comparative audit, 2026-09-06

## Decision and scope

**Not ready to certify 1.0 or hostile-code/multi-tenant isolation.** Both codebases
contain substantial working implementations and meaningful protections. Concrete
correctness, capability, lifecycle and package-verification defects remain.
This report consolidates the cross-domain assessment; it does **not** claim a
completed line-by-line audit of every file, all tests, dependencies or hardware.

Base commit: `30a450a` (full identity available in repository history). Analysis
used the live working tree, which already had 41 modified tracked paths and 152
untracked status entries. That work was preserved. The original continuation
changed only this report. Subsequent source fixes are recorded separately below.

Inventory is not coverage: `crates/` plus `src/` contain 90 Rust source files,
58,519 lines; `stdlib/` contains 639 `.sapp` files, including 19 compiler modules
totalling 13,114 lines; 972 semantic `*_test.sapp` files are discoverable. These
are current local filename/line counts, not assertions, executed tests or audited
percentages. Other examples/tools/tests and generated foreign code also matter.

## 1.0.0 readiness decision

Assessment criteria updated 2026-09-07; underlying audit evidence is dated above.

**Current decision: NOT READY.** This is an evidence-based assessment of the
production scope in [`ROADMAP.md`](../ROADMAP.md#current-060-alpha-channel--working-toward-100),
including its mandatory final compatibility gate. It is not a release approval.
The readiness section was added after audit commit `f481c0a`; no implementation
fixes, new test results or version changes accompanied this documentation update.
The package remains `0.6.0`, channel `alpha`.

Status meanings:

- **FAIL**: a supported finding contradicts the acceptance criterion.
- **NOT ESTABLISHED**: required evidence is absent or insufficient. This is not
  proof the feature fails, and it is not a pass.
- **PASS**: the exact release candidate meets the criterion with reviewed evidence.

No gate below currently has enough evidence for PASS. Individual working features
and passing targeted tests remain positive evidence in the audit; their success
does not compensate for an unrelated failing release gate. Readiness is not an
average score or a percentage derived from test-file counts.

The matrix records the original gate evidence. The remediation ledger below
supersedes individual finding status; no whole release gate has been promoted.

### Acceptance and evidence matrix

| Gate | Required acceptance for 1.0.0 | Available evidence and current status | Evidence needed to close |
| --- | --- | --- | --- |
| R1 Release scope and stable contracts | Explicit supported OS/architecture/backend/API matrix; frozen syntax, bytecode and stable-library contracts; documented previews and migration rules | **NOT ESTABLISHED**: implementations/subsets are mapped below, but a reviewed candidate-wide stability/support contract is not established | Versioned compatibility contract, stable/preview inventory and reviewed deprecation/migration policy bound to the candidate |
| R2 Compiler/runtime correctness | Supported programs preserve values, types, effects, errors and control flow across applicable execution routes; unsupported constructs reject explicitly | **FAIL**: C1, C2, C4, C5 and prior A5–A7 demonstrate miscompilation, checker/diagnostic divergence and partial execution | Fix commits plus previously failing regression cases; exact differential results for baseline, candidate and each claimed backend; no unexplained release-blocking divergence |
| R3 Memory, lifecycle and isolation | Safety guarantees hold at each reachable authoritative boundary; cleanup on all exits; per-VM ownership; FFI/embedding contracts enforced | **FAIL**: C3, S1–S3, S5–S6 and prior A3; native safety limits in the preceding audit remain open | Reviewed fixes and positive/negative lifecycle, ownership, ABI and isolation tests; opaque authority or verified isolation where hostile callers are supported; no unresolved critical/high finding in shipped reachable scope |
| R4 Resource and input enforcement | Enforce declared source/decode/heap/stack/time/output/child limits before effects or excessive allocation; cancellation and cleanup work | **FAIL**: S4, P6 and prior A1, A2, A4, A6 contradict parts of that guarantee | Exact N/N+1 bounds, aggregate child/import accounting, supervised cancellation and cleanup evidence; measured hard limits at the runtime/process boundary actually shipped |
| R5 Trusted packaging and deployment | Publisher/operator trust, integrity, immutable artifact handoff, revocation/rollback and secure transport hold through install/update/launch | **FAIL**: P1–P5, P7; experimental publisher modules explicitly retain unverified authorization claims | Sidecar/path/transport regressions, trust/substitution/rehydration tests, same-object handoff and reviewed revocation/rollback evidence; install/upgrade/recovery tests on disposable fixtures |
| R6 Compatibility and test reliability | Candidate passes the reviewed historical/application/product corpus and stable-slice broader tests; test failures/skips/timeouts remain distinguishable | **NOT ESTABLISHED**: focused successes below are narrow; test-quality gaps remain and the selected Rust build timed out | Exact candidate/baseline/corpus identities, reviewed assertions and owned fixtures, expected-versus-observed results, isolated repeat/soak results, migration notes for intentional changes; no unresolved release-blocking regression |
| R7 Platform and device qualification | Every advertised target/surface runs the actual candidate or independently built adapter; host/model/emulator/device evidence is distinguished | **NOT ESTABLISHED**: CI definitions are not successful runs; iOS simulator boot is not PBLang app execution | Per-target build/install/run/interaction/shutdown/upgrade evidence, including physical checks required by the supported-target contract; explicit unavailable/unsupported entries |
| R8 Performance and efficiency | Meets predefined workload-specific startup/compile/run/memory/latency targets without changing semantics; deterministic fallbacks remain valid | **NOT ESTABLISHED**: no new reproducible benchmark campaign is in this audit | Reviewed targets before measurement; identical workloads and baselines, warmups, multiple samples, median/tail latency, peak memory and applicable energy/thermal data; correctness checks for accelerated and fallback paths |
| R9 Reproducible, reviewable release | Exact source, dependencies, toolchain and artifacts are bound to the release evidence; documentation/examples match; dependency security and release review are current | **NOT ESTABLISHED**: live dirty-tree analysis is not a qualified release artifact; fresh dependency/advisory and independent bootstrap evidence is absent | Clean identified candidate, relative lockfiles, reproducible build evidence, dated dependency review, accurate tutorials/limitations, artifact digests and named operator/reviewer approval |

The earlier A identifiers refer to
[`PBLANG_ANALYSIS_CONTINUATION_2026-09-06.md`](PBLANG_ANALYSIS_CONTINUATION_2026-09-06.md).
C/S/P identifiers refer to the findings below. A single finding may affect more
than one gate; it must not be counted as multiple independent defects.

### What a positive readiness conclusion must contain

Each gate's evidence record must identify the gate/finding, exact source and fix
revision, candidate artifact identity, baseline/corpus revision where applicable,
logical target/backend, test/build command, limits, expected and observed result,
exit status, pass/fail/skip/unavailable/timeout status, cleanup result, date and
reviewer. Keep reproducible synthetic fixtures and sanitized evidence references;
never commit credentials, personal paths or private application data.

An individual finding can be **fixed but unverified**. It becomes **closed** only
after the relevant regression and boundary tests succeed on the candidate and the
evidence is reviewed. A documentation edit, disabled test, missing tool, retry,
fallback to a different backend or version-number change is not closure.

The report may say **READY FOR 1.0.0** only when all required gates PASS for the
same identified candidate and its explicitly approved scope, with no unresolved
release blocker. Restricting scope requires an explicit maintainer decision,
updated promises, and evidence the excluded path cannot still be reached through
a shipped entrypoint. This assessment neither narrows scope automatically nor
approves publication. Readiness never means proof of zero possible vulnerabilities
or compatibility with every conceivable program and device.

Daily work stays focused, as requested. The broader compatibility/soak campaign
runs on isolated release hosts after a stable slice; it is not triggered by this
report update. Version, release tag and publication remain unchanged.

## Architecture: where the implementation actually lives

The source observations below preserve the original audit evidence. See the
dated remediation ledger at the end for current closure/partial-fix status.

| Route | Owning code | Meaning of its result |
| --- | --- | --- |
| Installed/local host CLI | `Cargo.toml` selects `src/main.rs` | The local executable still uses the retained Rust host |
| PBLang workspace entry | `pblang.toml` selects `src/main.sapp` | Status/help/benchmark commands, not a replacement for every host CLI command |
| Self-host source compiler | `stdlib/compiler/{tokenizer,parser,typeck,codegen,optimizer,selfhost_frontend}.sapp` | Real PBLang frontend and bytecode implementation |
| Self-host artifact CLI | `stdlib/compiler/selfhost_cli.sapp` | PBCS serialization/round-trip, not the same format/VM as host PBCK |
| PBLang host-PBCK execution | `stdlib/host_pbck_vm.sapp`, `host_pbck_pure.sapp` | PBLang execution of host-format instructions with explicit limits/allowlists |
| Native PBLang backends | `stdlib/native/codegen_x64.sapp`, `codegen_arm64_ast.sapp`, `codegen_wasm.sapp` | Direct emission; supported subsets and runtime guarantees differ |
| Retained compilation routes | Rust compiler/VM, C transpiler, LLVM, WASM and Cranelift crates | Separate implementations; availability does not establish semantic parity |
| PBLang ownership policy | `stdlib/memory_safety.sapp`, `safety.sapp` | Real PBLang composition plus underlying explicit-handle VM enforcement |
| Packages/trust | Rust `pblang-pm`, CLI; PBLang manifest/publisher modules | Experimental PBLang evidence is not automatically an installer authorization |

The migration policy deliberately retains old paths until replacement/parity and
retirement are separately verified (`docs/PBLANG_ONLY_POLICY.md`). It evaluates a
supplied change manifest; mandatory trustworthy CI enforcement was not established.
No Rust source was added or modified during this analysis.

## Review ledger

| Domain | Evidence obtained | Not established |
| --- | --- | --- |
| PBLang frontend | Prior complete frontend/pipeline/cache/migration reads; new complete optimizer and typechecker reads | Complete parser, tokenizer and codegen internals across all syntax |
| Rust typechecker | Complete `crates/pblang-typeck/src/lib.rs`, tests included; selected host CLI probes | Soundness of gradual typing or complete Rust parser/compiler review |
| PBLang memory policies | Complete 548-line memory-safety and 219-line safety modules and relevant tests via security reviewer; small lifecycle probe | Isolation from hostile mutation of policy objects |
| VM, FFI, embedding | Complete Rust FFI wrapper/header, FFI dispatch/handle modules and contracts; targeted large-VM/spawn/DB/net paths | Every builtin, all concurrency schedules and native ABI on every platform |
| Package/release | Security review of PM implementation, relevant CLI handlers, manifest/publisher/admission and attestation paths; test inventory | Exploit execution, remote deployment and immutable operator trust |
| LLVM | Complete 830-line Rust module including tests; relevant CLI caller | Newly built/emitted/native execution in this continuation |
| Cranelift | Eligibility/print/trampoline, selected tests and CLI route excerpts | Complete JIT/AOT code generation or machine-code verification |
| Rust C/WASM and PBLang ARM64/WASM | Entrypoint/subset/caller inventory and selected excerpts | Complete emitter, bundled C runtime, allocator and engine parity |
| Standard library/AI/quantum/kernel | Ownership and module inventory plus earlier scoped findings | Complete math, data, network, AI, quantum or kernel correctness |
| Graphics/IDE/LSP | Product ownership and selected LSP diagnostic route; earlier UI reports are context | Fresh live UX/accessibility/lifecycle audit of all products |
| Tests/CI/platforms | Selected tests and supervisors, CI routing, iOS job read | Full suite, repeated/random-order stability, recent CI outcomes or physical-device success |
| Performance | Existing reproducibility policy located | New comparative benchmarks, energy/thermal or latency claims |
| Dependencies | Manifests/CI audit configuration inspected | Fresh advisory database, transitive supply-chain or malware audit |

Relevant older reports: `PBLANG_ANALYSIS_CONTINUATION_2026-09-06.md` (A1–A7),
`LOW_LEVEL_REVIEW_2026-09-06.md`, `TEST_QUALITY_REVIEW_2026-09-06.md`.
Old passing results are not presented as freshly rerun tests here.

## New correctness and parity findings

### C1 — High correctness: aggressive optimizer changes ordinary control flow

`stdlib/compiler/optimizer.sapp:421` strength reduction deletes instructions and
compacts the array without remapping jumps, unlike `_compact_and_remap`. It also
does not guard entry into the middle of a rewritten pattern. Numeric code is
affected independently of the documented operator-overloading caveat.

Reproduced with:

```pblang
let x=1; let y=0; if false { x=x+0; } y=7;
```

Unoptimized and `optimize_safe` execution yielded `x=1,y=7`; `optimize` yielded
`x=1,y=0`. Both were evaluated with the bounded PBLang VM. Bootstrap consumers
call the aggressive API at `stdlib/compiler/bootstrap.sapp:274,452`; this does
not imply that normal `frontend_run` uses it.

Needed: preserve CFG targets/basic-block boundaries and use proven type/effect
conditions for strength reduction. A boolean/stringified-result differential test
is insufficient; compare value type, value, side effects and failure category.

### C2 — Medium: typecheckers disagree, in both directions

- PBLang `_infer_expr` infers a number for `nil - 1` without a diagnostic
  (`typeck.sapp:840`). Its frontend accepted `let x: number = nil - 1;`.
  The retained Rust `--check` reported arithmetic on nil.
- Rust call checking zips arguments/parameters without checking count
  (`pblang-typeck/src/lib.rs:1062`): `fn f(x:number){return x;} f();` printed
  `typecheck ok`; PBLang reported an arity mismatch.
- Rust ordinary index inference ignores the index expression
  (`lib.rs:1014`): `let a=[1]; let x=a[missing_index];` printed `typecheck ok`;
  PBLang reported the missing identifier.
- PBLang optional-to-optional conversion handles the generic destination-option
  branch before the two-option branch (`typeck.sapp:127`). Directly probing
  `number? -> decimal?` returned false; the Rust branch order accepts numeric
  optional widening. This last Rust comparison is source-derived, not a new
  compiled Rust unit-test result.

These are checker/parity defects, not proof that the VM executes invalid native
memory access. Both checkers deliberately support gradual `any` typing; neither
should be described as a universal static proof of safety.

### C3 — Medium: public mutable-borrow/release pair is inconsistent

`memory_safety.borrow_mut` discards the native borrow ID and returns the memory
handle; `release(handle)` always releases immutable type 0
(`memory_safety.sapp:69–84`). A small probe acquired a mutable borrow, observed
the documented release fail, then successfully released type 1 and dropped the
owned value. `move_val`/try-borrow forms need lifecycle review too.

Needed: ID-carrying tokens with an explicit release API, retaining compatibility
through a documented transition rather than silently changing handle meaning.

### C4 — Medium/high correctness: LLVM reports success for partial semantics

The emitter warns and drops unsupported statements, or substitutes numeric zero
for unsupported expressions (`pblang-llvm/src/lib.rs:292,500`). `compile_to_native`
prints warnings and still invokes clang (`:551`). Thus a function containing an
unsupported print can compile while omitting observable behavior. CLI fallback
only follows a returned error, not these warnings (`src/main.rs:5148`).

Also, `compile_for` uses the condition header as the continue destination
(`lib.rs:382`), while the increment is emitted only after normal body fallthrough.
`continue` therefore bypasses the increment; a finite source loop can fail to
terminate. This was established from source, not by running an infinite binary.
Locals additionally use source-name slots without lexical restoration, requiring
shadowing/duplicate-SSA tests before broader compatibility claims.

Needed: explicit subset rejection, distinct continue/increment blocks and scoped
unique locals; verify generated IR and behavior, not just successful linking.

### C5 — Medium correctness: JIT eligibility admits discarded output

Cranelift accepts `OpPrint` (`pblang-cranelift/src/lib.rs:385`) but lowers it to
pop/discard (`:1028`). The associated test checks compilation/return value, not
captured output (`:1777`). This is a library/JIT-route parity defect; it is not
evidence that the normal interpreter or every CLI execution loses output.
The CLI's named Cranelift branch also emits a C stub with PBCK and links a VM
runtime (`src/main.rs:5175`); that route is not itself proof of Cranelift machine
code generation for the program.

## Security findings in retained Rust boundaries

The following are security-review source findings. No hostile payload, real FFI
call, cross-tenant resource access or allocation-exhaustion test was executed.

| ID / priority | Finding and preconditions | Owning source |
| --- | --- | --- |
| S1 High | `ticker`/`timeout_channel` create threads without the spawn gate; `spawn_n` allocates from an insufficiently validated count before gated spawning. Reachable builtins in a restricted VM can consume threads/memory. | `vm/spawn.rs:711–811`; registration `vm.rs:786` |
| S2 High | FFI/DB/network handle tables are process-global sequential IDs without VM ownership. A second VM with the category enabled can use/close another VM's guessed handle; FFI calls do not reapply the second VM's library allowlist. | `vm/handles.rs:23–69`; `vm/ffi.rs:41–349`; DB/net callsites |
| S3 High | FFI permission does not bind a host-owned exact ABI tuple; widths collapse and returned C strings are unbounded. Wrong declarations can cause native undefined behavior. Requires an FFI grant, not default-denied sandbox access. | `vm/ffi.rs:90–341`; `ffi_dispatch.rs:18–70,219–231` |
| S4 High availability | `max_memory_bytes` lacks main-VM allocation enforcement. Instructions do not meter all builtin/child work; C eval copies a caller C string before a source bound. | `vm.rs:457–477,3144`; `pblang-ffi/src/lib.rs:146,365` |
| S5 Medium, conditional | Canonical-path FFI allowlist check and `Library::new` are separate operations. An attacker-writable namespace can change between them. | `vm.rs:3088`; `vm/ffi.rs:54–66` |
| S6 Medium embedding contract | Concurrent C calls/free on one VM pointer can form aliased mutable Rust references. No internal serialization/thread-ownership contract is enforced. Host misuse/contract hazard, not a script-only remote exploit. | `pblang-ffi/src/lib.rs:205–225,340–382`; exported header |

Paths abbreviated `vm/...` above are beneath `crates/pblang-vm/src/`.
Positive safeguards include default-denied FFI, canonical library/symbol policies,
explicit-handle borrow conflict/move/lifetime checks and bytecode instruction
limits. They do not close these separate bypasses or establish an OS sandbox.

## Package, publisher and release findings

| ID / priority | Finding | Evidence / condition |
| --- | --- | --- |
| P1 High | Pack/verify HMAC sidecar identity differs; verify can silently do structure-only inspection. Archive paths are also insufficiently validated by verify. | Pack writes `app.tar.gz.sha256` (`src/main.rs:2960`), verifier looks for `app.gz.sha256` (`pblang-pm/src/lib.rs:2796`). Unsafe names can count as manifests (`:2747,2782`). Verification itself is not extraction; risk includes misleading the operator before unpack/run. |
| P2 High | Hosted publish/refresh can send bearer/refresh credentials over plain HTTP. | POST paths `lib.rs:2490,2551` bypass fetch's HTTP policy. Requires an HTTP registry configuration or influence over that destination; no credentials were read or transmitted in this audit. |
| P3 Medium | Trust-store ownership/type/mode check precedes pathname reopen; parent is not validated; size cap follows full read. | `lib.rs:733`; exploitation requires writable parent/namespace and registry influence. |
| P4 Medium/high conditional | Warning-only manifest validation permits raw name/version to reach filename and hand-built JSON sinks. | CLI `src/main.rs:883`; local/hosted publication `lib.rs:2418,2497`. Out-of-root path impact is Windows-conditional; authenticated JSON field injection is separate. |
| P5 Medium | Dependency hashing and later import reopen are not one immutable operation. | `lib.rs:1371,1575,1622`; concurrent local mutation required. |
| P6 Medium availability | Git clone and recursive package hashing lack practical aggregate limits; hashing retains complete contents. | `lib.rs:1418,2071`; untrusted or oversized dependency/package input required. |
| P7 Medium | Cache rehydration does not recompute the fetched archive's recorded SHA-256. | `lib.rs:1510`; later tree hashing still protects represented file paths/content, so this is not wholesale absence of integrity. Modes/archive identity remain different properties. |

Here `lib.rs` is `crates/pblang-pm/src/lib.rs`. Verified positive safeguards:
Ed25519 index verification, mandatory release SHA-256, fetch redirect denial,
bounded safe registry extraction, symlink/special-file rejection, private staging,
deterministic tree hashes and full Git commit IDs.

Experimental PBLang publisher modules honestly keep operator origin, publisher
authenticity, rollback/same-object handoff and execution authorization false.
Their current tests/docs callers do not establish integration into the installer.
Do not turn an experimental `ok` result into authorization or replace these false
claims with true merely to make the audit green.

## PBLang safety: preserve the implemented model, repair its boundaries

The PBLang modules implement substantive policy: lifetime cleanup via
`scope_result`, Box/Rc/Weak liveness/uniqueness, checked slices, subtraction-first
arena quotas, checkpoint/rewind/drop, checked indexing/arithmetic and channel
helpers. Native explicit-handle checks support borrowing, mutation, moves,
deallocation and lifetime expiry. These are real features, not documentation only.

However, exported Box/guard/Rc/slice/arena objects are ordinary mutable maps.
Helpers trust caller-editable handles, pin/alive fields, counters and quotas.
They cannot be a non-forgeable authority boundary between hostile callers sharing
a VM. This is policy/provenance weakness, **not demonstrated native memory
corruption**; the underlying VM still rejects invalid handles and borrow conflicts.
The documentation limits the checker to explicit handles, not all ordinary values
(`docs/Language_Semantics.md:157–175`).

Needed: module-private authoritative policy state plus VM-owned opaque capabilities
for hostile boundaries, without discarding the existing PBLang abstractions.
Smaller follow-ups include integer validation in `bounds_check`, the array-freeze
promise of `frozen_copy`, and the non-atomic channel check/send helper. These were
source-reviewed, not added to dynamically reproduced findings.

## Test quality and current validation

Passed in this continuation:

- `bash scripts/t --trusted selfhost_optimizer_fold selfhost_typeck_parity`
  (two selected semantic tests, no full suite).
- Bounded in-memory optimizer comparison reproducing C1.
- PBLang and retained-host stdin checker probes for nil arithmetic, missing call
  arguments and an undefined index expression; direct optional-type probe (C2).
- Small mutable-borrow lifecycle probe with explicit successful cleanup (C3).

The local executable was `target/release/pblang` 0.6.0, not the unrelated PATH
installation. These execute the current PBLang modules, but are not a fresh rebuild
of the entire Rust CLI. One selected Rust unit-test attempt with offline/locked
dependencies, one build worker and a 45-second deadline timed out while compiling
`pblang-core`. **No Rust test pass is claimed for that attempt.** It was not
retried into an apparently green result. No ABI/LLVM/Cranelift machine-code probe,
full suite, device test or benchmark was run.

Additional test-quality evidence:

- Optimizer differential helper maps all exceptions to `"THREW"` and compares
  `str(value)`, potentially conflating failure with a legitimate string or
  values of different types. The existing test passed despite C1.
- Rust `test_result_unwrap` discards diagnostics and never tests an unwrap;
  `test_every_vm_builtin_name_is_seeded` checks the same local list used for
  seeding, not the authoritative VM registration table.
- LLVM tests include real clang validation, but missing clang returns normally
  rather than producing a structured unavailable result. Some fixture names are
  predictable and cleanup follows assertions, so they were not run blindly.
- Direct spawn denial tests do not cover the timer and pre-allocation paths in S1.
- Regression gaps remain for exception defers, sidecar naming, unsafe verify
  archives, HTTP publish, exact resource bounds and cross-VM handle ownership.

Separate tooling observation: piping the verbose host help to an early-closing
consumer produced a child broken-pipe panic. That is not a Codex/application crash
or a result of executing hostile PBLang code; it is a CLI robustness follow-up.

## Platform/release evidence and exit criteria

CI configuration has Linux/Windows/macOS jobs and a hosted VM matrix. Configuration
is not a successful run. The iOS job builds/tests the macOS host binary and boots
and deletes a simulator; it does not install/run that PBLang binary or an app in
the simulator. It therefore cannot establish iOS execution compatibility. Android
workflow existence likewise does not establish a current successful device run.

To close this audit rather than relabel partial coverage as complete:

1. Resolve/retest C1–C5 and prior A1–A7 with exact cross-route behavior checks.
2. Close S1–S6 and P1–P7 at their authoritative boundaries, retaining deny defaults.
3. Review every remaining owning module and caller, including parser/compiler/IR,
   all emitters, bundled C runtime, builtins, graphics/LSP and standard-library
   domains; record complete versus excerpt-only coverage in a file ledger.
4. Audit test logic/effects/fixture ownership; require exact failure and cleanup
   assertions, structured skips, and reproducible isolated stability runs.
5. Run the broader compatibility corpus only after a stable reviewed slice,
   separating PBLang implementations, retained-host routes and foreign adapters.
6. Obtain fresh dependency-advisory, independent bootstrap, performance and
   physical-platform evidence. Declare target/subset support explicitly.

Source review alone cannot provide these absent executions or prove zero future
vulnerabilities. No version bump, universal support declaration or safety-model
replacement is authorized or justified by this report.

## Remediation ledger — 2026-09-07, first implementation slice

The maintainer explicitly authorized repairing the retained Rust implementation
for these findings, then returning it to maintenance only for concrete issues.
New language/product development remains PBLang-first/PBLang-only. This is not
authorization for unrelated Rust features, a release or a version bump.

Source baseline: `268a274`, plus preserved existing working-tree changes. The
fix identity is the commit containing this ledger. The live tree is not a clean
release candidate; unrelated files and the pre-existing lock change are excluded
from this scoped commit. The local lock was refreshed after standard-library
changes, then made repository-relative with `tools/pblock.sapp`.

| Finding | Current disposition | Fix and evidence |
| --- | --- | --- |
| C1 | Fixed and focused regressions passed | `compiler/optimizer.sapp` no longer rewrites unknown `x op 0`. The public pass only evaluates numeric literal pairs within one basic block and remaps all supported jump targets after compaction. Tests cover the original wrong-branch result, loops, overloaded add/subtract/multiply effects, output types/values, input preservation, incoming edges and jump-to-end. |
| C2 | Partial fix; remains open | Optional-to-optional payload comparison now precedes destination unwrapping. Positive/negative numeric, array, map and result tests pass. Nil arithmetic and retained Rust call/index checking discrepancies remain. |
| C3 | Legacy lifecycle defect fixed; focused regressions passed | Legacy handle-returning APIs are unchanged. `release(handle)` releases one active borrow of any supported kind; exact ID APIs were added. Tests cover shared/mutable/unique cleanup, conflicts, dropped/unborrowed handles, active-count balance and idempotent exact release without consuming a newer borrow. Raw numeric IDs remain guessable; this does not establish hostile-caller isolation. |
| P2 | Source/library fix verified; deployment qualification pending | Retained Rust publisher rejects non-HTTPS/ambiguous URLs before package access and derives both endpoints on the same origin/base path. TLS is explicit, redirects are disabled, only `2xx` succeeds, requests have a 30-second timeout and a refresh retries once. Four admission/response/redirect tests plus two positive refresh/retry tests pass. No live registry or updated host executable was deployed. |
| P4 | Partial sink repair only; remains open | Hosted publication now serializes metadata with `serde_json` rather than string interpolation. Filename admission and the other manifest-validation concerns were not repaired in this slice. |

Security review approved the narrow C3 and P2 changes after inspecting their
actual diff. That review is not a certification of the whole VM or installer.
P1, P3, P5–P7, C4–C5, S1–S6 and prior A1–A7 remain open; the broader readiness,
compatibility, performance, dependency and physical-platform evidence is still
required. **Overall decision remains NOT READY for 1.0.0.**

### Executed evidence and limits

- The new C1, optional C2 and C3 tests each failed before their source fix, then
  passed after it. The new P2 public HTTP rejection test likewise failed before
  the Rust fix (package access occurred before URL admission), then passed.
- PBLang runner: `target/release/pblang` version `0.6.0`, SHA-256
  `74ca877f685c16a27804ab9e9310d0b94ef2151d32fd9dce7ae5018aef7a00e4`.
  It loads the changed PBLang modules, but is not a rebuilt host containing the
  new Rust package-manager code. That code was compiled/tested as its own crate.
- `bash scripts/t --trusted selfhost_optimizer_strength selfhost_optimizer_fold
  selfhost_typeck_optional selfhost_typeck_parity memory_safety_legacy_release
  memory_safety_stdlib memory_safety_builtins`: **7/7 passed**, exit 0, with
  20 seconds/test, 2 GiB address-space cap and capped temporary output.
- `cargo test --offline --locked -j 1 -p pblang-pm --lib authenticated_ --
  --test-threads=1`: **4/4 passed**. Separate exact-name selections for
  `publish_retries_on_401_with_refresh_token` and
  `refresh_access_token_json_escapes_refresh_token`: **1/1 each passed**.
  Each invocation had a 45-second external deadline and 3 GiB address-space cap.
  Network effects were local synthetic fixtures only, with dummy tokens; no
  process-global token environment mutation and no real publication occurred.
- Local `pblang build .`, portable lock publication, and a second build after
  refresh: exit 0. The initial stale-hash manifest-validator warning was not a
  successful validation; the subsequent build used the refreshed lock.
- PBLang lint on all seven touched PBLang source/test files: exit 0 with existing
  owner-module warnings plus the linter's `a != a` NaN-test false positive and
  an explicit-boolean style warning. The differential helper no longer converts
  unexpected exceptions into a `"THREW"` success sentinel or stringifies values
  for equality. Numeric NaN equivalence is explicit: runtime `% 0` yields NaN,
  not the exception implied by the old test comment.
- `rustfmt --check --edition 2021 crates/pblang-pm/src/lib.rs`: exit 0.
  `cargo clippy --offline --locked -j 1 -p pblang-pm --lib --no-deps`: exit 0,
  three pre-existing dead-code warnings. No full workspace suite was run.
- `graphify update .` reached its external 20-second deadline (exit 124), with
  no completion output. The graph update is not claimed as completed.

Reproduction uses the reviewed live dependency tree, not unrelated generated
product artifacts. A clean candidate rebuild and broad compatibility campaign
remain separate requirements; these targeted successes do not replace them.

## Remediation ledger — 2026-09-07, PBLang guest cleanup and admission

Baseline `0e61bce`; fix identity is the commit containing this ledger section.
This slice changes only PBLang implementation/tests and documentation, not Rust
source. Existing unrelated changes remain excluded from the commit. The detailed
contract and intentional budget compatibility correction are documented in
[`HOST_PBCK_RUNTIME_LIMITS.md`](HOST_PBCK_RUNTIME_LIMITS.md).

- **A3 guest-exception paths fixed, focused verification passed.** Explicit
  throws, failed unwraps and escaping child exceptions now unwind frame defers
  in LIFO order. Nested cleanup throws do not replace the original result or
  stop older cleanups. Same-frame catches do not unwind early. Direct engine
  and resource-budget faults are deliberately not swallowed. Direct faults in
  a frame body still bypass this unwind path: universal runtime-fault cleanup
  remains open, so this is not full closure of every defer guarantee.
- **Related tail-handler correctness fixed.** Tail reuse now requires both an
  empty defer list and an empty handler list. A child exception cannot discard
  the caller's active catch. Existing unguarded tail reuse remains verified.
- **A4 identified admission defects fixed.** Parameter cells, including default
  and empty-varargs cells, are checked against stack and local ceilings before
  materialization. Admission counts simultaneous live frames, including root.
  `max_frames=N` intentionally no longer permits the accidental N+1 frame.
  Cleanup guest calls count as live frames too. This closes the specific
  parameter/frame defects, not every resource-enforcement gap in the report.

Both new regression suites failed against the applicable pre-fix behavior and
passed after correction. During test development, a named `catch error` helper
was found to read the builtin named `error`, not the caught exception, matching
the earlier test-quality report. The final helper uses boolean rejection with
identical-artifact positive boundary controls and adapter-effect counts. It
does not claim exact error-message validation. Pre-allocation ordering is
source-reviewed; no native allocation telemetry was collected.

Final security review approved this scoped diff without blockers. On the same
retained `0.6.0` host recorded above, the targeted command was:

```sh
bash scripts/t --trusted host_pbck_defer_unwind host_pbck_frame_admission \
  host_pbck_vm_full host_pbck_vm host_pbck_frame_vm
```

**5/5 passed**, with 20 seconds per test, 2 GiB address-space ceiling, bounded
temporary output, and synthetic in-memory fixtures only. This covers guest
exception identity/order, ordinary and exceptional cleanup, same-frame catches,
step/allocation N/N-1 cases, frame 1/2 boundaries, stack/local admission, empty
bodies/defaults/varargs, tail reuse and absence of denied callback effects.
No full suite, independent-backend, live-device or production qualification is
claimed. The overall **NOT READY for 1.0.0** decision remains unchanged.

Final `pblang build .` and portable lock publication succeeded. Lint on the
four touched PBLang files exited 0 with three pre-existing owner-module warnings
(exported opcode constant reported unused, boolean style, unconditional loop
style). `git diff --check` passed. `graphify update .` reached the external
20-second deadline, exit 124, without completion output; graph freshness is not
claimed. No persistent application, VM, device, or server was started.

## Remediation ledger — 2026-09-07, package verification P1

Baseline `ce151cc`; fix identity is the commit containing this section. This is
an explicitly authorized repair of existing Rust package-manager behavior,
not a change to the PBLang-first development policy. Unrelated working-tree
changes remain excluded. See [package verification contract](PACKAGE_ARCHIVE_VERIFICATION.md).

**P1 identified source defects repaired, focused library verification passed.**
Pack, verify and local-registry HMAC now use the identical lossless sidecar name.
Present sidecars require a nonblank key, bounded no-follow regular-file reads
and exact hex syntax; HMAC failures no longer echo signatures. Encoded file URLs
use the same decoding/admission as fetch. Detailed verification and CLI output
distinguish unsigned structural checks from HMAC verification; the legacy
unit-result API remains available. Neither result proves publisher authenticity.

Verify and unpack now share relative-path and duplicate-path admission. Only an
exact root regular manifest and a regular module satisfy verification. Harmless
dot-prefixed paths remain supported. Nonempty directory payloads are rejected;
extraction no longer ignores an unsuccessful `unpack_in` result. Effective PAX
sizes are counted, and a shared decoded-byte budget is applied before tar parsing
so GNU/PAX preprocessing cannot allocate from an unlimited decompressed stream.
One metadata allocation can still approach that budget; this is not a hard
process-memory or wall-clock guarantee.

The 17 new targeted package-manager tests passed on the local Linux host. The
central CLI passed `cargo check --offline --locked -j 1 -p pblang --bin pblang`.
These are source/library results, not evidence that the retained release binary
has been rebuilt or that CLI end-to-end, Windows/macOS or full-suite testing
has passed. Tests use synthetic data and owned disposable directories without
global environment mutation. The overall **NOT READY for 1.0.0** remains.

Outstanding packaging requirements include mandatory publisher trust, immutable
handoff, revocation/rollback, atomic signed publication, full cross-platform
filename policy, and parser-wide resource enforcement. An absent sidecar remains
explicitly unsigned, not fail-closed authentication; high-assurance callers must
reject `StructureOnly`. The source repair does not close these broader gates.

Final scoped security review found no remaining P1 blocker. Package-manager
Clippy (library and tests), package-manager rustfmt checks and `git diff --check`
passed. Existing dead-code and filesystem hard-link-cache warnings remain;
whole-CLI rustfmt also reports pre-existing formatting outside the changed hunks,
which was left untouched. Added-line privacy checks found no machine paths or
credential material. The final Graphify update reached its 20-second deadline
(exit 124), so graph freshness is not claimed. No test or analyzer processes or
owned archive-test temporary directories remained. No full suite was run.

## Remediation ledger — 2026-09-07, PBLang pipeline UTF-8 accounting

Baseline `c3b0ace`; fix identity is the commit containing this section. This
slice modifies PBLang implementation/tests and canonical documentation only;
no Rust source or retained executable was rebuilt. Unrelated working-tree
changes remain excluded.

**A6 direct-source admission repaired.** Both pipeline source entrypoints now
enforce UTF-8 byte ceilings before backend dispatch, with a character-count
early rejection before byte-array allocation. String artifacts and rendered
map/scalar artifacts also measure UTF-8 bytes rather than characters. Existing
binary vectors keep their byte-element count. Diagnostic chunk rendering is
explicitly not serialized PBCK or heap usage. See [execution pipeline contract](Execution_pipeline.md).

The new regression failed against the unchanged source, then passed after the
repair. The bounded targeted batch passed **3/3**: `execution_pipeline_byte_limits`,
`execution_pipeline`, and `selfhost_vm_bounded`, using the existing local 0.6.0
host, 20 seconds per test and a 2-GiB address-space limit. Tests use synthetic
in-memory source only: no imports in guest fixtures, filesystem writes, native
execution or devices. No full suite or independent-backend execution is claimed.

The separate module-snapshot linker still counts characters for several
byte-named limits (new supported open item A6-L). Per-backend allocations,
serialized-bytecode sizing, aggregate import budgets and physical-platform
qualification remain outside this repair. **NOT READY for 1.0.0** is unchanged.

Scoped security review found no blocker in this diff. Lint exited 0 with one
pre-existing boolean-style warning, the test's deliberate exact-boolean check,
and a false unused-variable warning for the variable used to generate NaN.
The multibyte diagnostic-artifact case is exercised; the C99 string-artifact
control is ASCII, leaving its multibyte branch source-reviewed rather than
independently exercised. Project build and portable-lock publication passed;
the pre-existing dirty lockfile remains excluded from this commit. Added-line
privacy checks and `git diff --check` passed. Graphify reached its 20-second
deadline (exit 124), so graph freshness is not claimed. No test/analyzer workers
or temporary test-output directories remained.

## Remediation ledger — 2026-09-07, bounded linker UTF-8 admission

Baseline `11e8d1a`; fix identity is the commit containing this ledger. This is a
PBLang-only change with no Rust changes or retained-executable rebuild. Existing
unrelated changes remain excluded.

**A6-L bounded source/linker accounting repaired.** All bounded-linker source,
path and output ceilings use UTF-8 bytes, including raw and canonical virtual
paths. Cached admission sizes feed per-module and reachable totals; all supplied
modules, even unused ones, count toward project admission. Output includes one
LF per dependency. Snapshot selection and the file wrapper apply matching units
after loading and before import scanning. Legacy resolver behavior is unchanged.

The regression failed against unchanged source and passed with the correction.
Six distinct focused suites passed: `source_linker_byte_limits`,
`source_linker_bounded`, `source_inliner_prefixed`, `source_inliner_package`,
`execution_pipeline_byte_limits`, and `execution_pipeline`. Each ran with a
20-second deadline, 2-GiB address-space cap and synthetic in-memory fixtures.
The existing comment-import test was not executed because it writes predictable
temporary paths and starts a PATH-resolved subprocess; that harness issue remains.

The complete owning module, relevant consumers/contracts and selected tests were
reviewed. Scoped security review found no blocker. File-backed snapshot selection
changes are source-reviewed, not exercised against live files in this slice.
Full-array UTF-8 measurement, transformed-body allocation before output admission,
and pre-admission snapshot loading remain resource boundaries. Byte counts are
not heap usage or VM-wide hard enforcement. **NOT READY for 1.0.0** is unchanged.

Compatibility: Unicode byte limits are intentionally stricter; raw aliases that
exceed the configured cap are rejected even when they normalize to short paths.
`immutable_broker_bundle` includes output length in its closure identity, so
persisted Unicode closure evidence must be regenerated without weakening the
verifier. With identical source/manifest/dependency inputs, ASCII byte-count
contributions to identity are unchanged; dependency/lock updates independently
affect identity. The source-graph hash algorithm is unchanged. See
[source linker contract](Source_linker.md).

Final project build and portable-lock publication passed. The pre-existing
dirty lockfile is excluded from the commit. Lint exited 0 with two existing
owner-module warnings and a false unused-variable warning on the new test's
checked rejection result. Added-line privacy checks and `git diff --check`
passed. Graphify reached its 20-second deadline (exit 124); graph freshness is
not claimed. No test/analyzer workers or temporary test-output directories
remained. No full suite, device or native-artifact execution was performed.

## Remediation ledger — 2026-09-07, A5 strict lexical compilation

Baseline `4fb0f12`; fix identity is this ledger's commit. PBLang-only frontend
change, no Rust changes or host executable rebuild. The baseline regression
accepted an unfinished comment; the correction rejects lexical/parser errors
across direct/file/guard/batch compilation, CLI serialization and the execution
pipeline. Recovery ASTs no longer produce instructions or file semantic/type
analysis on those errors.

Raw tokenizer flags are checked before guard rewriting and imported dependency
scanning. Imported processed syntax is checked before cache lookup. Project
indexing ignores errored files. The committed lint lexical gate is independent
of preserved, excluded user editor-lint changes (35 additions/4 deletions).
Type-only diagnostic bytecode remains inspectable, with `ok=false`.

Eight targeted suites passed: `selfhost_compile_lexical`,
`selfhost_project_lexical`, `selfhost_tokenizer_unterminated`,
`selfhost_parse_safe`, `execution_pipeline`, `execution_pipeline_byte_limits`,
`selfhost_vm_bounded`, `guard_preprocessor_native_match`. The existing harness
uses 20 seconds/test, a 2-GiB address-space ceiling and capped output. Static
repository-owned project fixtures bound compiler fan-out to two workers.

Scope limits: source loading/preprocessing allocation, naive import discovery,
cache authenticity, raw parser consumers outside this frontend and hard VM
resource enforcement are not repaired here. Numeric legacy guard arms retain
their pre-existing nil-versus-number type diagnostic. General editor lint
parser recovery remains separate uncommitted work. **NOT READY for 1.0.0**.

The escaped-quote/semicolon legacy-arm control also confirms a remaining
rewriter defect: raw-complete text can become an unterminated processed string.
Compiler and lint now reject it; source compatibility needs a separate repair.
Four lexical lint checks also passed against an exact staged frontend snapshot,
without the user's pending lint recovery changes. Its temporary copy was removed.
Final build and portable-lock publication passed; the pre-existing dirty lock
is excluded. Touched-file lint exited 0 with style/unused warnings, not a clean
warning count. Graphify reached the 20-second deadline (exit 124); no graph
freshness claim. No full suite or native/device artifact execution.

## Remediation ledger — 2026-09-07, legacy guard string preservation

Baseline `f4dc392`; fix identity is this ledger's commit. The escaped-string
rewriter defect recorded above is repaired in PBLang only. The new regression
failed before the change on a quoted semicolon and passed after correction.
Shared exclusive-end scanning preserves escape parity and exact source slices
in scrutinees, guards, arms/defaults and keyword/brace searches. Identifier
replacement skips strings and treats an empty old name as a no-op. Malformed
spans fail without advancing beyond a fabricated terminator.

The tests compare bounded-VM values, not merely successful parsing. Only fixed
synthetic chunks are explicitly executed despite existing type-only legacy
diagnostics. A5 raw-input rejection remains covered; its previously corrupted
escaped-string control now expects correct parsing. No frontend source or Rust
source was changed. Unrelated user changes remain excluded.

Open legacy limitations: nil-versus-value type checking, coarse source-wide
activation (which can rewrite unrelated native matches), incomplete comments
and nesting, generated-name collisions and ASCII-only keyword/identifier
boundaries. No global compatibility/performance claim. **NOT READY for 1.0.0**.

Validation: nine focused suites passed (`guard_preprocessor_strings`,
`guard_preprocessor_native_match`, `selfhost_compile_lexical`,
`selfhost_project_lexical`, `selfhost_tokenizer_unterminated`,
`selfhost_parse_safe`, `selfhost_vm_bounded`, `execution_pipeline`,
`execution_pipeline_byte_limits`). Each uses the targeted 20-second, 2-GiB,
capped-output harness; value probes additionally use 2,000 VM steps/eight
frames. Final project build and portable-lock publication passed. Lint exited
0 with existing style/unused warnings. The dirty lockfile is excluded.
Graphify hit its 20-second deadline (124); freshness is not claimed. No broad
suite, device execution, temporary fixture writes or retained-host rebuild.

## Remediation ledger — 2026-09-07, per-match legacy admission

Baseline `0738f0a`; fix identity is this ledger's commit. The activation
regression reproduced native-match corruption caused by an unrelated quoted
separator. The PBLang adapter now validates complete supported legacy arms
within each candidate; `has_guards` is not transformation authority. Every arm
must have a non-keyword ASCII identifier, `if`, a real top-level tilde/hash
separator, a value and a semicolon; only one optional final `else:` is admitted.
Code is emitted after all arms validate. Native/mixed/partial/unsupported forms
return no transformation rather than partial or always-true arms.

Strings are preserved, grouping is balanced with a 64-level limit, and discovery
skips complete comments or terminates at unfinished comments. Whole-keyword
boundaries stop suffix lookalikes; bind substitution preserves neighboring
Unicode/combining-mark identifiers. The canonical tokenizer supplies keyword
authority and is already included in module-cache dependency measurement.

The intentionally narrower legacy grammar is a correctness compatibility change:
comments inside expressions, brace-based nesting, colon catchalls, partial arms
and missing semicolons are left unlowered and may fail checked compilation.
General expression semantics, generated-name hygiene, result/bind typing, nested
legacy support and hard resource enforcement remain open. No frontend or Rust
source edits, device testing or universal compatibility claim. **NOT READY for
1.0.0** remains unchanged.

Validation: ten focused suites passed (`guard_preprocessor_activation`,
`guard_preprocessor_strings`, `guard_preprocessor_native_match`,
`selfhost_compile_lexical`, `selfhost_project_lexical`,
`selfhost_tokenizer_unterminated`, `selfhost_parse_safe`, `selfhost_vm_bounded`,
`execution_pipeline`, `execution_pipeline_byte_limits`). The new regression
checks unchanged native text, correct branch values in both sibling orders,
unsupported/malformed forms, Unicode boundaries and exact 64/65 nesting.
Tests used the 20-second/test, 2-GiB, capped-output harness; runtime probes add
2,000-step/eight-frame limits. Final build and portable-lock publication passed;
the pre-existing dirty lock is excluded. Lint exited 0 with two warnings
(an unused-variable report on the checked local `code` and existing `while true`
style). Graphify timed out cleanly at 20 seconds (124), so graph freshness is
not claimed. No full suite, device scan or retained executable rebuild.

## Remediation ledger — 2026-09-07, legacy result values and scoped bindings

Baseline `6d8b484`; fix identity is this ledger's commit. A numeric-result
regression first failed checked compilation because the old emitter assigned
a number to a synthetic nil-initialized result variable. The PBLang adapter
now returns selected values directly from its existing outer IIFE. Each arm
gets a lexical statement-if scope containing its binding, original guard and
value. Bare braces cannot serve this purpose: the parser lowers those to a
separate value-block IIFE, which would intercept returns. No parser, frontend,
typechecker, VM, optimizer or Rust source changes were made in this slice.

The subject is evaluated once; guards run in order; later guards and unselected
values do not run. Bindings work in values as well as guards without rewriting
member names or strings. Repeated bindings are isolated; the default sees the
outer scope; a missing default returns nil. A deterministic helper name avoids
all canonical identifier tokens in the admitted match span. Tokenization
failure/unterminated tokens decline lowering; candidate search is bounded by
token count plus one, not a hard global memory/time guarantee.

This entry supersedes earlier open nil-result assignment, missing body-binding
and lexical helper-collision limitations **only for the admitted legacy subset**.
It does not establish reflective/eval hygiene, precise union/result inference
(function literals retain the existing `any` return type), general expression
or nested grammar support, native/Rust parity or universal compatibility.
The admission grammar and raw/processed compilation gates remain unchanged.
Broader resource, runtime security and physical-platform evidence gaps remain;
**NOT READY for 1.0.0** is unchanged.

Validation: twelve focused suites passed (`guard_preprocessor_results`,
`guard_preprocessor_bootstrap`, `guard_preprocessor_activation`,
`guard_preprocessor_strings`, `guard_preprocessor_native_match`,
`selfhost_compile_lexical`, `selfhost_project_lexical`,
`selfhost_tokenizer_unterminated`, `selfhost_parse_safe`, `selfhost_vm_bounded`,
`execution_pipeline`, `execution_pipeline_byte_limits`). Results cover numeric,
string, boolean and array values; defaults/nil; bindings/shadowing; member and
literal preservation; two helper-candidate collisions; deterministic sibling
matches; side-effect order; exceptions; optimized execution and genuine name
errors. Bootstrap's five guard checks now use checked bounded execution for
value cases, not old helper substrings. Existing string/native/lexical controls
now require successful compilation instead of tolerating legacy type errors.
The harness bounds each target to 20 seconds, 2 GiB and capped output; new VM
probes additionally use 4,000 steps/12 frames (bootstrap: 2,000/eight).
Lint exited zero with ten warnings, including existing unused/style reports
and deliberate exact-boolean checks; no lint-clean claim. Security postreview
approved this scoped change. No full suite, hardware scan or runtime rebuild.
Final project build and portable-lock publication passed. Added-line privacy
checks and `git diff --check` passed. The pre-existing frontend diff is byte-for-
byte unchanged and unrelated work, including the dirty lockfile, is excluded.
Graphify reached its 20-second deadline (124) without retained processes;
graph freshness is not claimed.

## Remediation ledger — 2026-09-07, C2 concrete nil arithmetic

Baseline `b16aaf9`; fix identity is this ledger's commit. The new targeted
regression first failed on the unchanged checker. `typeck.sapp` now emits
`nil-arithmetic` for binary `+`, `-`, `*`, `/`, `%` when a known nil is paired
with a numeric type or nil, and for unary negation of nil. Both operands are
visited; a failing expression returns `any` after recording its diagnostic,
avoiding a fabricated numeric result and redundant outer type errors.

The C2 `nil - 1` finding is closed for the PBLang checked route. C2 overall
remains partial: retained Rust call-arity/index discrepancies are not changed.
Gradual `any`, generics, optional types, string-addition inference and custom
forward/reflected operators retain existing behavior. They are not newly
certified. In particular, globally rejecting all nil operands would break
valid map overloads; an executable reflected-subtraction test protects this.
Exponentiation already uses the `num_pow` call checker, not this binary path.

Compatibility correction: previously accepted concrete nil arithmetic now
fails checked compilation, including inside `try`. The existing frontend run
gate refuses execution before earlier statements can produce output. The
inspectable-bytecode contract for type errors is unchanged. No Rust, parser,
VM, optimizer or frontend source was changed; pre-existing dirty work remains
excluded. This is a correctness repair, not a memory-safety, sandbox, complete
type-soundness or 1.0.0 readiness certification. **NOT READY** remains unchanged.

Validation: twelve targeted suites passed (`selfhost_typeck_nil_arithmetic`,
`selfhost_typeck`, `selfhost_typeck_parity`, `selfhost_typeck_optional`,
`selfhost_operator_overload`, `selfhost_diagnostic_suggestions`, `selfhost_lint`,
`guard_preprocessor_results`, `guard_preprocessor_bootstrap`,
`selfhost_compile_lexical`, `selfhost_project_lexical`, `selfhost_vm_bounded`).
The new test covers left/right/both nil, numeric precision types, nested
diagnostic counts, unary negation, source-span/category preservation, no source
echo, malformed AST, unchanged gradual cases, valid execution, reflected
overloads and rejection before output. Each target uses the bounded trusted
20-second/2-GiB/capped-output harness; new execution probes additionally use
1,000 VM steps/eight frames. Lint exited zero with existing unused/style
warnings; no lint-clean or full-suite claim.
The expanded new regression passed again after adding span/privacy and nested
controls. Final build and portable-lock publication passed; the dirty lockfile
is excluded. Added-line privacy checks and `git diff --check` passed. The
pre-existing frontend diff is unchanged. Graphify reached its 20-second limit
(124); no graph freshness is claimed and no test/Graphify process remained.

## Remediation ledger — 2026-09-07, native match guards and enum coverage

Baseline `b8d9cfc`; fix identity is this ledger's commit. Further review of the
PBLang match path found two related correctness defects: enum exhaustiveness
credited conditional arms as guaranteed coverage, and native code generation
omitted guards entirely for literal patterns. The enum regression failed
before the change on a false guarded catchall. The literal-emitter finding was
established from its complete control flow; its initial test attempt was
invalidated by a stale dependency lock, not counted as behavioral evidence.

The checker now credits only absent guards or exact boolean-true literal ASTs.
Conditional literal/bind/wildcard arms cannot hide a missing enum variant.
Complementary conditions and calls are deliberately not proven or executed.
Malformed arm metadata does not grant coverage; ordinary AST validation remains
responsible for its diagnostics. This check retains its existing scope: a known
enum instance's `__variant` member, not every possible match expression.

The emitter now generates the guard for every pattern kind. Pattern mismatch
and guard failure have separate jumps to the following arm; literal mismatch
skips the guard, guard failure skips the body, and successful arms jump to the
shared end. No opcode, parser, VM, optimizer, frontend or Rust changes were
made. The complete owning codegen module and relevant parser/checker/caller
paths were reviewed. Existing dirty frontend changes remain excluded.

Compatibility correction: formerly accepted conditional-only enum coverage can
now require an unguarded fallback. Literal guards now affect execution as the
source requests. Tests explicitly parenthesize call bodies around the existing
colon/method-call parser ambiguity; this is a recorded limitation, not a claim
that arbitrary native syntax is repaired. Native helper-name/binding-scope
issues and general match/type soundness are outside this slice. No universal
compatibility or 1.0.0 readiness claim; **NOT READY** remains unchanged.

Validation: fourteen targeted suites passed (`selfhost_enum_guard_coverage`,
`selfhost_literal_match_guard`, `selfhost_type_system_parity`,
`selfhost_typeck_parity`, `selfhost_typeck_optional`,
`selfhost_typeck_nil_arithmetic`, `guard_preprocessor_results`,
`guard_preprocessor_activation`, `guard_preprocessor_native_match`,
`selfhost_optimizer_strength`, `selfhost_closure`, `selfhost_loop_kw`,
`selfhost_compile_lexical`, `selfhost_vm_bounded`). New tests cover literal,
bind and wildcard coverage, true/false/dynamic guards, duplicate conditions,
fallbacks, malformed metadata, source-error preservation, ordered effects,
exceptions and no-output rejection. Runtime probes use 2,000 or 3,000 VM steps
and eight frames; all targets use the trusted 20-second/2-GiB/capped-output
harness. No whole-suite or physical-platform test was run.

Final build and portable-lock publication passed. Lint exited zero with
existing unused/style reports plus an intentional exact-boolean comparison.
Added-line privacy and `git diff --check` checks passed; pre-existing frontend
diff remained unchanged, and the dirty lockfile is excluded. Graphify timed out
at 20 seconds (124), so graph freshness is not claimed. No test/Graphify worker
or temporary test-output directory remained.

## Remediation ledger — 2026-09-07, native match lexical scope

Baseline `fc35a31`; fix identity is this ledger's commit. The new regression
first failed because `let n=99; match 2 { case n if false: n; else: n; }`
selected the failed arm's `n` rather than the outer value. Codegen kept every
arm's binding in a shared match scope, unlike the typechecker. Its named
`__match_scrut` temporary also shadowed legal source references in guards,
values and closures.

Codegen now opens/closes one lexical scope per arm. Name resolution entries
are removed before compiling the next arm/default; monotonic physical slots
are not reclaimed, so existing captured closure values remain valid. The
subject reserves an internal slot without any source-name entry, eliminating
that temporary's lexical capture without name scanning or renaming. Subject
evaluation still precedes storage reservation. Previous pattern/guard failure
jumps, first-match order and opcode/bytecode formats remain unchanged.

This supersedes the previous native match helper-name and inter-arm binding
limitations for this lowering path. It does not fix the colon-method-call
parser ambiguity, other synthesized loop/try temporaries, arbitrary malformed
AST handling, reflection or every backend. The PBLang owning module was fully
reviewed in the preceding slice and its live scope/slot/closure consumers were
rechecked here. No Rust, VM, typechecker or frontend source changes. Existing
frontend and unrelated dirty work remain excluded. **NOT READY for 1.0.0**
remains the release assessment, not overturned by this targeted correction.

Validation: fourteen targeted suites passed (`selfhost_match_scope`,
`selfhost_literal_match_guard`, `selfhost_enum_guard_coverage`,
`selfhost_type_system_parity`, `selfhost_typeck_parity`,
`selfhost_typeck_nil_arithmetic`, `guard_preprocessor_results`,
`guard_preprocessor_activation`, `guard_preprocessor_native_match`,
`selfhost_optimizer_strength`, `selfhost_closure`, `selfhost_loop_kw`,
`selfhost_compile_lexical`, `selfhost_vm_bounded`). The new test compares plain
and safely optimized values for defaults, later literal/wildcard/bind arms,
outer locals, repeated and Unicode binders, helper-like names, nested/sibling
matches and selected/fallback/loop closures. Undefined outer names remain
checked errors. New runtime probes use 4,000 steps/12 frames; the trusted
harness caps each target at 20 seconds/2 GiB with bounded output.

The expanded new test initially used a parenthesized nested subject, which the
parser classifies as a call to the contextual identifier `match`. It now uses
the supported unparenthesized nested form and passed on rerun; thirteen other
targets passed in the original batch. That syntax limitation is recorded,
not claimed fixed. This is not a full-suite or physical-device verification.

Final build and portable-lock publication passed; the pre-existing dirty lock
is excluded. Lint exited zero with one unused-variable warning in the new test.
Privacy checks on added lines and `git diff --check` passed. The pre-existing
frontend diff is unchanged. Graphify hit its 20-second timeout (124), so graph
freshness is not claimed; no test/Graphify process or temporary output remained.

## Remediation ledger — 2026-09-07, array for-in lexical scope

Baseline `5470c0c`; fix identity is this ledger's commit. The new
`selfhost_forin_scope` regression first failed on restoration of an enclosing
name after iteration. `codegen_for_in` now opens/closes a lexical scope and
allocates collection/index slots without source-name entries. The iterable
runs once before the binder is introduced, including `for x in x` and legal
helper-like names. Slots remain monotonic, preserving by-value captures.
Break/continue targets and instruction/bytecode formats are unchanged.

Fourteen targeted suites passed: `selfhost_forin_scope`, `selfhost_match_scope`,
`selfhost_literal_match_guard`, `selfhost_enum_guard_coverage`,
`selfhost_type_system_parity`, `selfhost_typeck_parity`,
`selfhost_typeck_nil_arithmetic`, `guard_preprocessor_results`,
`guard_preprocessor_activation`, `guard_preprocessor_native_match`,
`selfhost_optimizer_strength`, `selfhost_closure`, `selfhost_loop_kw`,
`selfhost_vm_bounded`. The new test compares plain and safely optimized
results for empty/nested/sibling loops, shadowed locals, iterable-before-binder
lookup, synthetic-name collisions, Unicode, closures, single evaluation,
break/continue and early return. An undefined outer binder still rejects.
New probes use 5,000 steps/12 frames and the trusted harness's per-target
20-second/2-GiB/capped-output limits. No full-suite or hardware run.

Only PBLang codegen, its new regression and documentation changed in this
slice. Count/range-loop scopes and bound evaluation, other synthetic
temporaries and existing parser ambiguities are not repaired. No Rust, VM or
frontend edits; unrelated dirty work remains outside the change.

Final build and portable-lock publication passed. Lint exited zero; the new
test has an unused-variable warning for `missing` despite the following
member access. Added-line privacy checks and `git diff --check` passed. The
pre-existing frontend diff is unchanged and the dirty lock remains excluded.
Graphify reached its 20-second deadline (124), so freshness is not claimed.
No test/Graphify worker or temporary test-output directory remained.

## Reconciled remaining work toward 1.0.0 — 2026-09-07

This snapshot reconciles the dated remediation ledgers above with the
original findings and A1–A7 continuation; it is not a fresh exhaustive audit
of all files or platforms. Later scoped fixes supersede older descriptions
only where explicitly stated. **NOT READY for 1.0.0** remains the decision;
no R1–R9 gate is promoted to PASS by this slice, and no completion percentage
is justified by the current coverage.

Already repaired with focused evidence: C1 optimizer behavior, C3's legacy
borrow-release lifecycle, A4's identified frame/parameter admission defects,
A5 lexical admission, A6/A6-L UTF-8 accounting, and the dated legacy/native
match, array for-in, count/range and try-context corrections. C2's optional-assignment and concrete nil
arithmetic repairs are complete within their described scope, not all of C2.
A3 guest-exception cleanup and P4 JSON serialization are partial repairs.
P1/P2 source/library fixes are not an updated, qualified deployed host.

| Area / finding | Remaining implementation or evidence | Status |
| --- | --- | --- |
| C2, C4, C5; compiler follow-ups | Retained checker call arity/index-expression checks; LLVM unsupported-subset rejection, continue/increment and scoped locals; JIT observable print parity. Resolve parser ambiguities, block-tail/semicolon divergence, general try control transfer, remaining temporaries and numeric edge cases. Count/range lowering and try slot/capture context are repaired below. Programming-model composition remains partially audited in the companion coverage report. | Open; no general compiler/backend parity claim |
| A7 diagnostics | Visibility-aware name suggestions across analysis/lint surfaces; exclude function-local candidates outside their scope. Existing unrelated frontend edits are not certified by this slice. | Open |
| A1, A2 imports and file entrypoints | Syntax-aware import discovery, aggregate import/child budgets including cache paths, and descriptor-held bounded reads before whole-file allocation with file-kind/identity policy. | Open |
| A3 cleanup; S1, S2 ownership/concurrency | Unwind on direct engine/resource faults; gate timer/spawn paths before allocation; VM-owned handle authority and cross-VM lifecycle tests. Existing guest-exception and legacy borrow fixes must be retained. | Partial overall; these listed paths remain open |
| S3–S6 and safety boundaries | Exact host-enforced typed FFI and bounded returns; hard aggregate heap/work/wall limits; same-object library loading; serialized or owner-enforced C embedding. Preserve PBLang safety abstractions while supplying private authoritative state/opaque capabilities where hostile callers are supported. Check integer bounds, array freezing and channel atomicity follow-ups; establish the claimed hostile-code isolation boundary. | Open; memory safety is not equivalent to resource enforcement or an OS sandbox |
| P1, P2 deployment | Rebuild the retained executable with the verified library repairs and qualify CLI install/publish/verify behavior on disposable fixtures and declared targets. | Source/library fixed; deployment evidence pending |
| P3–P7; publisher/operator trust | Secure bounded trust-store loading, remaining manifest/name/version admission, immutable hash-to-import handoff, bounded clone/hash traversal, archive-identity verification on rehydration. Complete pinned operator trust, revocation freshness/rollback and authenticated publication/handoff. P4's JSON sink repair does not close filename admission. | Open or partial as noted; publisher authenticity not established |
| R1, R2, R6 scope and compatibility | Freeze stable/preview syntax, bytecode, API and target contracts; complete the remaining owning-module/caller ledger. Review assertion quality, distinguish errors/skips/timeouts, isolate fixtures and cleanup, add missing negative/resource tests. Run the historical/product corpus and broader repeat/soak tests once a stable identified candidate exists. | Evidence and implementation work remain; focused passes are not a whole-corpus result |
| R7 platforms/devices | Build/install/run/interact/shutdown/upgrade the actual candidate or independent adapter for each advertised target. Obtain required physical-platform evidence; an iOS simulator boot or host test is not iOS application execution. Mark unavailable targets explicitly. | Not established; no universal device support claim |
| R8, R9 release qualification | Reproducible baseline/candidate measurements (warmups, samples, median/tails, peak memory and applicable energy), clean candidate/artifact identity, independent bootstrap and build reproducibility, fresh dependency review, relative lockfiles, matching documentation and release approval. | Not established |

Remaining PBLang correctness work includes other generated temporaries and
parser ambiguities; security-boundary repairs require defensive review first.
Unavailable hardware and release-wide evidence cannot be replaced by
documentation or model/emulator results. Graph freshness is a separate tooling
limitation, not proof for or against language semantics.

## Remediation ledger — 2026-09-07, count/range scope and bound evaluation

Baseline `3f81b2c`; fix identity is this ledger's commit. The new count and
range regressions both failed on outer-binding restoration before the change.
Complete codegen review and parser/typechecker/semantic/reference tracing also
showed bound evaluation on each condition check and reuse of the user binder
as the internal counter. The typechecker already resolves bounds before the
binder; the retained count emitter evaluates its bound once and uses a separate
index. The retained range parser lowers to a once-created range collection.

Only the two PBLang numeric-loop emitters are changed. Each opens/closes a
lexical scope, reserves unnamed counter/bound slots, evaluates bounds before
introducing the binder, and copies the counter into the binder each iteration.
Range evaluation is start then end; numeric +1 stepping remains scalar.
Counter increments and patched break/continue destinations are unchanged.
Physical slots remain monotonic and closure capture remains by value.
No Rust, VM, parser, typechecker, optimizer or frontend edits were made.

This supersedes the previous count/range scope and repeated-bound limitations
only for these lowering paths. Programs relying on those defects must express
dynamic conditions/counters with `while`, as documented in
`Bootstrap_safety.md`. General malformed-AST, fractional/non-finite/extreme
number, overloaded-comparison, shadowed-range-builtin and backend parity are
not established. This is not a whole-language resource or safety guarantee.

Fourteen targeted suites passed: `selfhost_count_scope`,
`selfhost_range_scope`, `selfhost_forin_scope`, `selfhost_match_scope`,
`selfhost_literal_match_guard`, `selfhost_enum_guard_coverage`,
`selfhost_typeck_parity`, `selfhost_type_system_parity`,
`selfhost_optimizer_strength`, `selfhost_closure`, `selfhost_loop_kw`,
`selfhost_vm_bounded`, `loop_range_continue`, `selfhost_compile_lexical`.
The two new tests compare plain/optimized typed values and ordered effects for
local/global restoration, empty/negative/nested/sibling loops, outer-bound
lookup, bound mutation, binder assignment, helper-like names, Unicode,
closures, continue/break, early return and bound exceptions. Undefined names
and malformed loop source reject. Probes use 5,000 steps/12 frames, with the
trusted harness's 20-second/2-GiB/capped-output per-test limits.

No full suite or physical-platform run. The R1–R9 release gate status remains
unchanged: NOT READY for 1.0.0. Existing unrelated edits remain excluded.

Final build and portable-lock publication passed; lint on all three touched
PBLang files exited zero without warnings. Added-line privacy checks and
`git diff --check` passed; the pre-existing frontend diff is unchanged.
The dirty lock remains excluded. Graphify stopped at its 20-second timeout
(124), so graph freshness is not claimed. No test/Graphify process or owned
temporary test-output directory remained.

## Remediation ledger — 2026-09-07, try local slots and captures

Baseline `b9a1b68`; fix identity is this ledger's commit. Both new regressions
failed behaviorally before the source change: a try-body local overwrote an
enclosing parameter, and a closure's outer variable used only inside try was
not captured. Subchunks copied local names but started their allocator at zero
and discarded the lexical parent/upvalue context, despite VM execution sharing
the enclosing locals and upvalues.

`codegen_try` now propagates the monotonic slot allocator through body,
handler and subsequent outer code. The branches keep independent local-name
tables, share enclosing upvalue descriptors and inherit the enclosing lexical
parent (not a fictitious extra function frame). Captures discovered in either
branch become part of the enclosing closure constant. Nested tries inherit
these rules. No Rust, VM, parser, typechecker, optimizer or serializer change.
The owning codegen module had been read completely in preceding slices; the
live try/scope/capture/closure consumers, parser/checker and serializer were
rechecked here. No opaque-handle, FFI or other security authority was changed.

Eighteen distinct targeted suites passed across two bounded batches:
`selfhost_try_scope`, `selfhost_try_capture`, `selfhost_count_scope`,
`selfhost_range_scope`, `selfhost_forin_scope`, `selfhost_match_scope`,
`selfhost_literal_match_guard`, `selfhost_enum_guard_coverage`,
`selfhost_optimizer_strength`, `selfhost_typeck_parity`, `selfhost_closure`,
`selfhost_vm_bounded`, `selfhost_loop_kw`, `selfhost_compile_lexical`,
`multi_paradigm_composition`, `oop_complete`, `functional_combinators`,
`generics`. New compiler probes compare exact values/effects on plain,
optimized and in-memory PBCS round-trip paths, bounded at 6,000 steps/16 frames.
The host harness caps each test at 20 seconds/2 GiB and bounded output.
No full suite, device run or file-backed serialization fixture was used.

Test development exposed two incorrect fixture assumptions: an unparenthesized
anonymous function at statement start parses as a named declaration, and the
checker intentionally tolerates unresolved names inside a caught try body.
The fixtures now use the supported parenthesized value form and test rejection
outside that caught body. Neither limitation/rule was silently changed.
The composition test also exposed retained/self-hosted block-tail semicolon
divergence; the retained-host positive case now uses an unterminated value tail.
The companion paradigm report records this open difference, not a repaired one.

These fixes do not certify arbitrary malformed ASTs, named catch binding,
cross-subchunk control transfer, every backend or universal runtime-fault
cleanup. The broader resource, FFI, packaging, compatibility and platform
blockers remain open; release status remains NOT READY for 1.0.0.

Final build and portable-lock publication passed. Lint on the four touched
PBLang files exited zero, with two unused-variable warnings for the composition
test's outcome maps despite their following member reads. Added-line privacy
and diff checks passed; the pre-existing frontend diff remains unchanged and
the dirty lock is excluded. Graphify reached its 20-second timeout (124);
no graph freshness is claimed. No test/Graphify process or owned temporary
output directory remained.

## Remediation ledger — 2026-09-07, typed memoization identity

Baseline `81768c6`; fix identity is this ledger's commit. The new regression
failed before the change because numeric 1 and string "1" returned the same
cached result. Both complete owning modules (`functional.sapp`, `pipe.sapp`)
and discovered callers/tests were reviewed. Their duplicate string-only
implementations now delegate to `functional_memo.sapp`.

The shared helper admits immutable scalar keys with type tags and compares
the stored scalar before every cache hit. Mutable/callable inputs are not
serialized or compared; they recompute. Zero/non-finite numbers are bypassed.
An entry is published only after callback success, with at most 1024 entries
and 4096-character string keys. At capacity new keys recompute while old hits
remain available. Callback/result memory and execution are not bounded by
these retention limits; the report's hard-resource blockers remain open.
No Rust, compiler, VM, serializer or existing frontend edits.

Ten targeted suites passed: `memoize_identity`, `selfhost_memoize_identity`,
`pipe_stdlib`, `pipe_enrich`, `functional_combinators`,
`multi_paradigm_composition`, `oop_complete`, `generics`,
`selfhost_try_scope`, `selfhost_try_capture`. The new host test checks both
public APIs, type distinctions, exact results/counts, nil caching, exceptions,
mutable/callable and zero bypass, and entry/key N/N+1 boundaries.
The self-hosted test compiles the reviewed helper source as one unit, checking
plain, optimized and in-memory serialized results at 10,000 steps/16 frames.
This is not independent module-loader, backend or physical-platform evidence.
All tests use the 20-second/2-GiB/capped-output trusted harness.

The public API is unchanged, but bypass/capacity cases deliberately execute
more often. The pure-callback contract and shared-result limitation are in
`FUNCTIONAL_MEMOIZATION.md`. This closes the memoization identity follow-up
only: unique/grouping, general arity and paradigm composition remain partial.
The overall 1.0.0 assessment remains NOT READY; no full suite was run.

Final build and portable-lock publication passed. Lint on all five touched
PBLang files exited zero, with equal-operand warnings on intentional
non-finite guards plus existing/fixture unused-variable reports. Added-line
privacy and diff checks passed. The unrelated frontend diff is unchanged;
the dirty lock remains excluded. Graphify stopped at 20 seconds (124), so
graph freshness is not claimed. No test/Graphify process or owned temporary
test-output directory remained.

## Remediation ledger — 2026-09-07, curried argument preservation

Baseline `92354d8`; fix identity is this ledger's commit. The full functional
module, discovered curry callers and relevant tests were reviewed. The previous
dispatcher called with four values for any argument vector of length >=4.
The new regression failed before the change because `curry(f, 5)` accepted
an unsupported positional arity instead of rejecting it before collection.

`curry` now validates integer 0–4 arity, preserving nil/default 2 and immediate
zero invocation. The internal dispatcher rejects excess arguments rather than
dropping them. Negative/fractional/non-number arities also reject before effects.
The additive `curry_args(f, n)` accepts integral 0–256 and invokes an array-taking
callback with all values in order. Every step copies only the outer prefix
array; element references remain shared. Exceptions do not consume a reusable
prefix. This is a vector convention, not arbitrary positional apply/spread.

Ten targeted suites passed: `functional_curry_contract`,
`functional_curry_args`, `selfhost_curry_args`, `functional_combinators`,
`multi_paradigm_composition`, `memoize_identity`, `pipe_stdlib`,
`pipe_enrich`, `oop_complete`, `generics`. New cases cover malformed
admission without effects, 0–4/default behavior, 256/257 boundaries, order,
prefix reuse/mutation, exception retry and object/generic/functional composition.
The self-hosted probe imports the actual functional library through
`frontend_run_bounded` with 15,000 steps, 24 frames, eight modules and
32-KiB per-source admission. These are the existing frontend limits, not proof
of aggregate import/heap enforcement. Each host test is capped at 20 seconds,
2 GiB address space and bounded output. No broad suite or device execution.

Only PBLang library/tests/docs changed; no Rust, compiler, VM or existing
frontend changes. `FUNCTIONAL_CURRY.md` records migration and the quadratic
prefix-copy cost. General positional calls, callback signature inference,
retained checker C2, class/block semantics and remaining resource/FFI/package/
platform evidence remain open. This does not promote any release gate to PASS;
the 1.0.0 assessment remains NOT READY.

Final build and repository-relative lock publication passed. Lint on the four
touched PBLang files exited zero, with existing/fixture unused-variable warnings.
Added-line/new-file privacy checks and `git diff --check` passed. The pre-existing
frontend diff is unchanged; unrelated work and the dirty lock are excluded from
this commit. Graphify reached its 20-second deadline (124), so the graph update
is incomplete. No test/Graphify worker or temporary test-output directory
remained after the checks.

## Remediation ledger — 2026-09-08, extrema effects and bounded comparisons

Baseline `d4a2c11`; fix identity is this ledger's commit. Full functional/pipe
modules, discovered extrema callers and relevant tests were reviewed. The new
singleton regression reproduced two key calls for one element. `max_by` and
`min_by` now compare from the second element, with one evaluation per element,
first-tie retention and immediate error propagation. Numeric ordering and
borrowed references remain explicit in `FUNCTIONAL_EXTREMA.md`; callbacks must
not structurally mutate input. No new general comparator or purity guarantee.

Ten targeted suites passed: `functional_extrema`, `selfhost_extrema`,
`functional_combinators`, `multi_paradigm_composition`,
`functional_curry_contract`, `functional_curry_args`, `selfhost_curry_args`,
`memoize_identity`, `pipe_stdlib`, `benchmark_enrich`. Tests cover exact effects,
empty/singleton inputs, stateful selection, ties, exceptions, nonnumeric
comparison rejection and bound-method/generic composition. The self-hosted
probe imports the actual library at 15,000 steps, 24 frames, eight modules and
32-KiB source limits. This is not aggregate-resource or all-backend evidence.

The PBLang-only `bench/session_compare.sapp` reuses the existing competitor
harness with one bounded row per external process. All twelve local rows passed
exact checksums; raw measurements and limitations are in
`BENCHMARK_SESSION_2026-09-08.md` and `bench/results/session-2026-09-08.jsonl`.
Invalid language/workload selections both reject. No device, VM, GUI or broad
test suite was launched; no cross-platform or all-application benchmark claim.

Final build and portable-lock publication passed; lint on four touched PBLang
files exited zero with existing/fixture unused-variable warnings. Graphify
reached its 20-second deadline (124), so graph freshness is not claimed.
The initial post-edit test attempts rejected the stale dependency hash; after
normal build/lock refresh the tests ran. An initial string-order assertion was
corrected to reflect the runtime's existing numeric-only comparisons, not hidden
as a supported feature. The implementation did not change comparison semantics.

Remaining work: general call/arity support, class/default/type and block-tail
semantics, unique/grouping identity, once failure/reentrancy consistency, full
model composition, compiler/LLVM/JIT findings, import/file resource accounting,
runtime cleanup/isolation and hard heap/preemption, typed FFI/sandbox,
publisher/package trust and delivery, reproducible builds, broad compatibility
and physical platform/device evidence. The overall 1.0.0 decision is NOT READY.

Added-line/new-file privacy and diff checks passed. The pre-existing frontend
diff is unchanged, and unrelated dirty work/lock changes remain excluded from
this commit. No test, benchmark or Graphify worker or temporary test/benchmark
directory remained. `BENCHMARK_COVERAGE_PROGRAM.md` records the expanded
all-category/language/backend and multi-fragment polyglot target separately from
today's measured subset; it does not claim that target is implemented.

## Remediation ledger — 2026-09-08, explicit class super owner

Baseline `bcf223e`; fix identity is this ledger's commit. The full owning class
module, discovered super callers and relevant tests were reviewed. Legacy
super selects the parent of the dynamic class, so an inherited override can
resolve itself again. The additive final owner argument selects the declaring
class's parent instead, without mutating the instance or global dispatch state.
Omitted/nil-owner behavior is preserved; this does not automatically repair old
inheritable overrides. Class definitions remain mutable maps, and explicit
ownership is not verified as an ancestor/nominal-identity guarantee.

The pre-change new-API regression failed with arity mismatch. An initial actual
frontend import test rejected the self-referential class initializer; explicitly
predeclaring the owner binding resolved the fixture, not that compiler
restriction. `CLASS_SUPER_OWNER.md` documents this portable idiom and migration.
No Rust, compiler or VM source change. The pre-existing frontend diff is unchanged.

Ten targeted suites passed: `class_super_owner`, `selfhost_super_owner`,
`class_stdlib`, `oop_complete`, `multi_paradigm_composition`,
`functional_extrema`, `functional_curry_args`, `generics`, `operator_overload`,
`benchmark_enrich`. Coverage includes four-level dispatch/order, raw and bound
methods, zero/one/two argument forwarding, mixins, parent redispatch, failures
and preserved dynamic identity. The actual class-library frontend/VM import
uses 20,000 steps, 32 frames, eight modules and 32-KiB source limits; this is not
all-backend or aggregate runtime-resource qualification.

Nine sequential local integer-kernel benchmark rows passed exact checksums:
three PBLang execution paths plus C, C++, Go, Rust, Python and Node. Existing
foreign fixtures and algorithms were unchanged. Build/run/RSS/artifact metrics,
raw samples, noise, flags and cache limitations are recorded in
`BENCHMARK_SESSION_2026-09-08_SUPER_OWNER.md`. This is neither an OOP performance
benchmark nor evidence of embedded polyglot or physical-platform support.

Remaining: automatic lexical super and old-call migration, class/default/type
semantics, general arity, block semantics, unique/grouping and once contracts,
full paradigm composition; retained compiler/LLVM/JIT, import/file limits,
runtime cleanup/isolation and hard heap/preemption, typed FFI/sandbox,
package/publisher authenticity and delivery, reproducibility, broad compatibility
and actual platform/device verification. The 1.0.0 assessment stays NOT READY.

Final build/portable-lock publication and lint on four touched PBLang files
passed; lint retains existing style/shadowing and fixture unused-variable
warnings. Both invalid benchmark selections reject. Added-line/new-file privacy
and diff checks passed. No test/benchmark/Graphify worker or owned temporary
directory remained. Graphify stopped at its 20-second deadline (124); graph
freshness is not claimed. Unrelated dirty source and lock changes stay excluded.

## Remediation ledger — 2026-09-08, PBLang VM small calls

Baseline `ee97db2`; this commit changes only the PBLang-written VM call path,
not the retained Rust interpreter or opcode format. The owning VM module and
callers/tests/contracts were reviewed before edits. Security delegation identified
malformed raw OP_CALL count handling and ambiguous builtin/closure descriptors.
Count/stack admission now precedes indexing, popping and argument allocation;
the existing sticky bounded limit remains. Builtin tags take precedence, including
the formerly inconsistent two-argument import case.

Pure closures with arities 0–3 use fixed-shape arrays, preserving argument order,
arity validation, lexical globals, cache propagation and shared runtime/depth.
Nine focused suites passed on the final source. Exact measured bounded step
counts match baseline across all six workloads; serialized/optimized paths and
exact budget boundaries are tested. No full suite ran. `VM_SMALL_CALLS.md` lists
coverage and remaining descriptor-boundary gaps. Final security review found
no blocker in this diff; mutable descriptors are not an unforgeable capability.

The final lint has warnings but no new type errors. An initial nil-initialized
argument array caused type diagnostics; an if-expression alternative also failed
lint and was discarded. The retained array-initialized branch passed. Build and
portable-lock publication passed; build initially noted a stale dependency hash
before lock refresh. This is not reproducible-bootstrap qualification.

All four self-host VM measurement batches and a separate nine-language/backend
integer-kernel batch are retained in `BENCHMARK_SESSION_2026-09-08_VM_CALLS.md`.
Timings are mixed and controls drift: no general speedup is demonstrated. The
retained-host PBLang interpreter still trails Python/Node on the measured kernel.
This does not test all languages/categories/platforms or embedded polyglot.

Remaining: general arity/class/default/block semantics and paradigm composition;
compiler/LLVM/JIT; import/file limits; descriptor validation, runtime cleanup and
isolation, hard heap/preemption, typed FFI/raw bypass closure and hostile-code
sandbox; package trust/artifact delivery/revocation/rollback; reproducibility,
broad product compatibility and physical-platform verification. PBLang 1.0.0
remains NOT READY. Benchmark leadership remains a target, not an established fact.

Final validation also passed portable_lock_current, manifest_validate and
benchmark_enrich (twelve targeted suites total), a fresh build without the stale
hash note, relative-lock publication, lint with recorded warnings, and diff/privacy
checks. The pre-existing frontend diff is unchanged. Graphify timed out cleanly
at 20 seconds (124), so graph freshness is not claimed. No owned test/benchmark/
Graphify worker or benchmark temporary directory remained. Unrelated dirty work
and local lock changes are excluded from the commit.

## Remediation ledger — 2026-09-08, sequence successor evaluation

Baseline `123991a`. Complete `functional.sapp`, its shared memoization dependency,
the related pipe module and discovered iterate callers/tests/contracts were
reviewed. `iterate` evaluated an unused successor after the final emitted value,
causing an extra callback, state mutation or exception. The singleton regression
failed before implementation. The loop now checks whether another output value
is needed before invoking the callback. No Rust, compiler, VM or public signature
change. This is a correctness fix, not evidence of general interpreter speedup.

`FUNCTIONAL_ITERATE.md` documents exact callback count/order, terminal values,
borrowed-reference behavior and the side-effect compatibility correction. The
new direct and actual-library bounded frontend/VM tests exercise success, nil,
required exceptions, invalid callbacks, mutation, composition and larger counts.
Legacy invalid/unbounded count behavior is not redesigned or newly qualified.
The once failure/reentrancy compatibility decision remains separate.

Remaining: unique/grouping and once contracts; general arity/class/default/block
semantics and full paradigm composition; retained compiler/LLVM/JIT; import/file
limits; runtime cleanup/isolation and hard heap/preemption; typed FFI/raw bypass
closure and hostile-code sandbox; package trust/artifact delivery/revocation/
rollback; reproducibility, broad product compatibility and physical-platform
verification. PBLang 1.0.0 remains NOT READY.

Twelve targeted suites passed: functional_iterate, selfhost_iterate,
functional_combinators, functional_extrema, selfhost_extrema,
functional_curry_args, pipe_stdlib, multi_paradigm_composition,
selfhost_vm_small_calls, portable_lock_current, manifest_validate and
benchmark_enrich. No full suite ran. Final build, relative-lock publication,
lint (warnings retained), added-line/new-file privacy and diff checks passed.
The pre-existing frontend diff remains unchanged; unrelated dirty files and
local lock changes are excluded from this commit.

`BENCHMARK_SESSION_2026-09-08_ITERATE.md` records nine sequential local numeric
competitor rows with checksums, raw samples, build/time/RSS/artifact metrics and
measurement limits. The initial batch's retrieved transcript was truncated; the
complete batch was repeated, not cherry-picked. This is neither a performance
measurement of iterate nor proof of all-language/platform/polyglot superiority.
Graphify stopped cleanly at its 20-second deadline (124), without an updated
graph claim. No owned test/benchmark/Graphify worker or benchmark temporary
directory remained.

## Remediation ledger — 2026-09-08, typed scalar collection keys

Baseline `addbb8a`. Full owning functional/array modules, shared memoization,
related map contracts and discovered callers/tests were reviewed. Graphify query
did not surface the relevant PBLang helpers, so source navigation was used.
A new mixed-scalar regression failed before implementation: functional.unique
collapsed number/string and bool/string inputs through str(value).

The PBLang-only collection_keys module now provides type-tagged buckets with
verified finite-f64/string/bool/nil equality, canonical zero bucketing, stable
first representatives and bounded membership/key/comparison work. Both stdlib
unique wrappers and array.distinct_by use it. New functional.unique_by supports
object scalar projection; group_by_entries returns ordered typed-key records.
Legacy group_by/frequencies string maps are unchanged. `COLLECTION_KEYS.md`
documents deliberate narrowing: scalar projection keys, at most 4096 items,
4096 text-key characters, 65536 aggregate bucket characters and 65536 comparisons.
This is not a whole-VM memory or callback execution bound. Same-length input
mutations/ABA and rollback of callback side effects are not provided.

Defensive-security review approved the scalar-only path and array wrapper
alignment. Unsupported keys are rejected before stringify or recursive equality.
Raw array_unique still exposes unbounded/recursive composite comparison on the
retained host, with different composite identity semantics in C99; no Rust source
was changed. Bigint/rational/decimal keys and C99 embedded-NUL strings remain
unqualified. A finite-f64 rendering collision was not verified; equality checks
are defense-in-depth rather than a claim that the current formatter is lossy.

Twelve targeted suites passed: collection_keys, collection_keys_bounds,
selfhost_collection_keys, array_map_functional, functional_combinators,
functional_iterate, selfhost_iterate, memoize_identity, multi_paradigm_composition,
portable_lock_current, manifest_validate and benchmark_enrich. Tests cover mixed
types, zeros, nearby f64s, Unicode/NUL on the retained host, typed grouping, stable
representatives, callback order/failures/mutation, malformed and unsupported keys,
exact-number/nonfinite rejection and item/text/aggregate boundaries. A cyclic
key is rejected without traversal; the raw builtin was not exercised on cycles.
Comparison-budget exhaustion remains statically guarded, not fault-injected.

Remaining: once failure/reentrancy, broader collection/exact-number semantics,
general arity/class/default/block semantics and complete paradigm composition;
compiler/LLVM/JIT, import/file limits, runtime cleanup/isolation and hard heap/
preemption, typed FFI/raw bypass closure and hostile-code sandbox, package trust/
artifact delivery/revocation/rollback, reproducibility, broad compatibility and
physical-platform qualification. PBLang 1.0.0 remains NOT READY.

The separate nine-row numeric competitor batch is recorded, with all samples,
checksums, build/time/RSS/artifact metrics and explicit limits, in
`BENCHMARK_SESSION_2026-09-08_COLLECTION_KEYS.md`. It does not measure collection
performance or prove cross-backend key parity. Final build and relative-lock
publication passed. Lint completed with warnings, including misleading identical-
operand warnings on the deliberate NaN/infinity guards (nonfinite rejection is
tested); those guards were not removed. No full suite ran.

Canonical Tooling/PrjAnalysis/Language_Semantics entries now distinguish bounded
stdlib wrappers from the legacy raw builtin and remove the unconditional O(n)
claim. The pre-existing Tooling and frontend diffs remain unchanged; only this
turn's Tooling hunk is staged. Added-line/new-file privacy and diff checks pass.
Unrelated dirty work and local lock changes stay excluded from the commit.

Graphify's final update stopped at its 20-second deadline (124), so graph
freshness is not claimed. No owned test/benchmark/Graphify worker or benchmark
temporary directory remained after validation.

## Remediation ledger — 2026-09-08, once attempt lifecycle

Baseline `d50f136`. Full functional/pipe modules and shared memoization, relevant
callers, tests and exception contracts were reviewed. A valid regression failed
before implementation: after an ordinary first exception, functional.once returned
nil on the next call despite never completing successfully. An initial test fixture
used an unsupported assignment target (call().field); a local binding repaired
the fixture before reproducing the real lifecycle failure.

The PBLang functional_once helper tracks started/completed state without catching
the first error. Functional.once delegates; new pipe.once_attempt shares the
explicit at-most-one-attempt behavior. Legacy pipe.once retains retry-on-failure
and reentry semantics. Completed nil/false values and first-input behavior are
preserved; later calls on an incomplete live closure throw a constant diagnostic.
Caught nested reentry can still be followed by successful outer completion.
Security review found no blocker in this sequential, per-instance scope.

Thirteen targeted suites passed: once_attempt, selfhost_once_attempt,
selfhost_once_roundtrip, pipe_stdlib, functional_combinators, memoize_identity,
collection_keys, selfhost_collection_keys, functional_iterate,
multi_paradigm_composition, portable_lock_current, manifest_validate and
benchmark_enrich. Coverage includes nil/errors, references, invalid callbacks,
bounded caught/uncaught reentry and legacy retries. Actual module import tests
and standalone helper plain/optimized/serialized execution passed; plain and
restored instruction counts match. Two uncaught fixtures exited with expected
failure: original first sentinel preserved, later constant diagnostic without
echoing the original private marker. No full suite ran.

`FUNCTIONAL_ONCE.md` documents compatibility, the extra transitive module,
callback side effects without rollback, cached shared references, lack of atomicity/
thread-global/cross-VM persistence, and no quota/engine/process recovery or callback
sandbox guarantee. The roundtrip fixture reads its trusted source file before the
size assertion; it is not evidence of pre-allocation bounded file reading.

Remaining: intentional legacy pipe.once migration and concurrency semantics;
broader collection/exact-number/raw-builtin/C99 boundaries; general arity/class/
default/block semantics and full paradigm composition; compiler/LLVM/JIT, import/
file limits, runtime cleanup/isolation and hard heap/preemption, typed FFI/raw
bypass closure, hostile-code sandbox, package trust/artifact delivery/revocation/
rollback, reproducibility, broad compatibility and physical-platform verification.
PBLang 1.0.0 remains NOT READY. No Rust/compiler/VM implementation was changed.

Final build, repository-relative lock publication, lint (warnings retained),
added-line/new-file privacy and diff checks passed. The pre-existing frontend
diff is unchanged. `BENCHMARK_SESSION_2026-09-08_ONCE.md` retains all nine local
numeric competitor rows, raw samples and build/time/RSS/artifact metrics. This
unchanged workload does not measure once or establish general interpreter wins.
Graphify stopped cleanly at its 20-second deadline (124), without a freshness
claim. No owned test/benchmark/Graphify workers or benchmark temporary directory
remained. Unrelated dirty work and local lock changes are excluded from the commit.

## Remediation ledger — 2026-09-08, opt-in stable sorting

Baseline `b7f760f`. Reviewed complete functional module, relevant callers/tests,
benchmark helper and prior contracts. Added PBLang-only functional_sort and
functional.stable_sort_by: bottom-up stable merge sorting with at most 4,096
elements, two fixed shallow arrays, no recursion and finite numeric comparator
results. Legacy sort_by/sort_asc/sort_desc are unchanged, including callback trace.
The initial new-API test failed because the export did not yet exist; this is an
additive feature, not a claim of a pre-existing sort correctness regression.

Fourteen targeted suites passed: functional_stable_sort,
functional_stable_sort_bounds, selfhost_stable_sort, stdlib_undefined_names_fix,
functional_combinators, functional_extrema, functional_iterate, once_attempt,
memoize_identity, collection_keys, multi_paradigm_composition,
portable_lock_current, manifest_validate and benchmark_enrich. Tests cover empty/
singleton, odd lengths, duplicates/stability, all 243 five-element ternary arrays,
borrowed payloads, source mutation through callbacks, failure propagation,
4,096/4,097 boundaries and comparison counts, malformed/nonfinite/exact-number
comparator results including subtraction overflow, and legacy comparison trace.
Actual library import through the PBLang frontend/VM and helper plain/optimized/
serialized execution pass, including equal plain/restored instruction counts.
An uncaught negative fixture exits 1 with the constant comparator diagnostic,
without echoing its private payload marker. No full suite ran.

Independent defensive review found no blocker in this trusted-callback scope.
`FUNCTIONAL_SORT.md` explains the extra transitive import, lazy comparator
validation for fewer than two items, consistent total preorder, shared mutable
payloads, persistent callback effects, and stricter per-host collection limits.
Algorithmic work/slot bounds are not callback containment, total-memory bounds,
snapshot isolation, rollback, C99/device parity or a hostile-code sandbox.

`BENCHMARK_SESSION_2026-09-08_STABLE_SORT.md` retains six pre-change legacy cells,
six post-change legacy controls and six new-API cells with identical fixtures,
warmups, five raw samples, first timings, median/p95 and comparison counts.
At 1,024 items, stable-sort medians were 4.856–6.832 ms versus the original
130.140–260.180 ms; this is retained-host local sort evidence only. The separate
nine-row numeric competitor batch passed exact checksums and records time/build/
RSS/artifact metrics. It does not invoke the sorter or establish interpreter wins.

Remaining: deliberate migration of legacy sort/pipe.once callers, comparator
purity/concurrency semantics and broader collection/exact-number/raw-builtin/C99
boundaries; general arity/class/default/block semantics and full paradigm
composition; compiler/LLVM/JIT, import/file limits, runtime cleanup/isolation and
hard heap/preemption, typed FFI/raw bypass closure, hostile-code sandbox, package
trust/artifact delivery/revocation/rollback, reproducibility, broad product
compatibility and physical-platform verification. PBLang 1.0.0 remains NOT READY.
No Rust/compiler/VM implementation changed.

Final build and repository-relative lock publication passed. Lint completed with
warnings, including misleading identical-operand warnings on intentional NaN/
infinity guards; those guards remain and the nonfinite cases pass. Initial tests
hit the stale dependency hash after the source addition; refreshing the local
lock resolved it before validation. Added-line/new-file privacy and diff checks
pass. The final Graphify update stopped at its 20-second deadline (124), so graph
freshness is not claimed. No owned test/benchmark/Graphify worker or benchmark
temporary directory remained. Only the fourteen slice files are staged;
pre-existing dirty work and the local lock remain excluded.

## Remediation ledger — 2026-09-08, functional window size validation

Baseline `9e2e930`. Reviewed the complete functional module, callers and targeted
contracts/tests. A regression reproduced fractional chunk size being accepted
instead of rejected. Added a private PBLang guard for array input and ordinary
finite integer size, before iteration even when input is empty. Chunk rejects
nonpositive size; windows preserves the finite nonpositive-integer empty sentinel.
Valid positive integer algorithms/results remain unchanged. Documentation names
the intentional invalid-input compatibility tightening, including accidental
string input and chunk zero/negative sizes. No imports or Rust/compiler/runtime
implementation changed.

Fourteen targeted suites passed: functional_windowing, selfhost_windowing,
functional_combinators, functional_iterate, selfhost_iterate,
functional_stable_sort, selfhost_stable_sort, array_map_functional, once_attempt,
memoize_identity, multi_paradigm_composition, portable_lock_current,
manifest_validate and benchmark_enrich. New coverage includes 90 length/size
combinations with every slot checked, partial chunks/full windows, invalid
empty/nonempty inputs, finite huge sizes, exact-number variants, NaN/infinities,
shallow structural independence, borrowed payloads and sum pipelines. Actual
PBLang frontend/VM imports also reject NaN and both infinities. The uncaught
invalid-size fixture exits 1 with a constant error and no private marker echo.
No full suite ran.

Independent defensive review approved the narrow guard implementation. This is
input-shape validation, not a total resource limit: windows still eagerly
allocates `(n-size+1)*size` payload slots and can amplify hostile input. The
`FUNCTIONAL_WINDOWING.md` contract explicitly requires caller budgeting, with no
hard heap, preemption, snapshot or sandbox claim.

Related findings from that static review remain unresolved, not fixed by this
functional-only change (unsafe malformed native cases were not executed):

- High-priority C99 boundary: raw `array_chunk` converts nonfinite/out-of-range
  double sizes to `size_t` in `crates/pblang-ctranspiler/src/pblang_rt.h:1041-1053`.
  C conversion can have undefined behavior and a resulting zero increment could
  prevent progress. This is static risk, not a reproduced exploit.
- `array.windows` checks positivity only; fractional/nonfinite size/step handling
  remains inconsistent (`stdlib/array.sapp:263-273`).
- `compute.chunk` clamps only values below one; fractional/nonfinite sizes can
  misalign work used by run_chunked (`stdlib/compute.sapp:501-529`).
- Retained raw VM array_chunk truncates floats/accepts infinity; its boundary is
  separate (`crates/pblang-vm/src/vm/arrays.rs:220-237`).

Remaining release work also includes generator/count contracts, legacy API
migration, broader collection/exact-number semantics, general arity/class/default/
block semantics and paradigm composition, compiler/LLVM/JIT, import/file budgets,
runtime cleanup/isolation and hard heap/preemption, typed FFI/raw bypass closure,
hostile-code sandbox, package trust/artifact delivery/revocation/rollback,
reproducible builds, full product compatibility and physical-platform checks.
PBLang 1.0.0 remains NOT READY.

The nine unchanged numeric competitor rows passed exact checksums and retain
first values, raw samples, median/p95 and CPU/RSS/build/artifact metrics in
`BENCHMARK_SESSION_2026-09-08_WINDOWING.md`. They do not invoke chunk/windows and
do not establish a guard speedup or universal interpreter win. The prior sort
contract's wording now correctly says first run, not flushed-cache cold run.

Final build and repository-relative lock publication passed. The initial stale
dependency hash after the source change was refreshed before testing. Lint
completed with warnings, including inaccurate identical-operand advice on the
deliberate nonfinite guards; passing NaN/infinity tests justify retaining them.
Added-line/new-file privacy and staged/unstaged diff checks passed. The final
Graphify update reached its 20-second deadline (124), without a freshness claim.
No owned test/benchmark/Graphify workers or benchmark temporary directories
remained. Only the eleven slice files are staged; unrelated dirty work and the
local lock remain excluded.

## Remediation ledger — 2026-09-08, array chunk and stepped windows

Baseline `bd697fb`. The prior array.windows fractional/nonfinite count finding
is now corrected at the PBLang library boundary. Array.chunk also validates before
work and no longer delegates to raw array_chunk. New array_windowing.sapp clamps
chunk ends and avoids unused overflowing window advances; only derived bounded
indices reach the existing native bulk slice primitive. Public array.slice is
unchanged; no Rust source edits. This adds one transitive module import.

Sixteen focused suites passed: array window contracts, actual PBLang frontend/VM,
plain/optimized/PBCS roundtrip, array/map/functional regressions, functional and
selfhost windowing, collection keys/bounds/selfhost, stable sort, once-attempt,
multi-paradigm composition, portable lock, manifest and benchmark enrichment.
The shape oracle covers 90 chunk and 900 stepped-window combinations. Invalid
types/counts, empty input, huge finite counts and borrowed payloads are covered.
No full suite ran. See `ARRAY_WINDOWING.md`.

Two independent limits are explicitly tested, not hidden: the PBLang frontend
treats explicit nil as an omitted default step whereas the retained host rejects
it; PBCS serialization rejects numeric literals outside its existing supported
constant interval. Neither frontend nor serializer was changed.

Before/intermediate/after raw measurements and nine numeric comparison rows are
in `BENCHMARK_SESSION_2026-09-08_ARRAY_WINDOWING.md`. Bulk copying improves the
measured windows cases; many small chunks remain slower than the original raw
native builtin. No universal performance or memory claim follows. The attempted
retained C99 library fixture failed at unresolved import compilation, so no C99
library execution parity is established.

The user-requested executable refresh was attempted through PBLang pbbuild with
direct-x64, no cache and no fallback, targeting the PBLang compiler CLI in an
isolated temporary directory. Dependency analysis blocked publication before
backend execution. It reported module_cache path-control taint, optimizer dynamic
evaluation, and incomplete linux_dirfs parsing. These analyzer findings are not
automatically confirmed vulnerabilities. No candidate was published and the
active executable was not replaced. `tools/pbbootstrap_refresh.sapp` refreshes
PBCK snapshots and a return-42 native probe, not a full native CLI replacement;
that existing snapshot set was not overwritten by this attempt.

Independent defensive diagnosis distinguished the reported findings: optimizer
dynamic-eval is comment text, not execution. At module_cache:257-261, read bytes
control whether already-formed staging paths are cleaned up; the high finding is
conservative control-dependency classification, not confirmed path traversal.
The non-Linux cache fallback nevertheless lacks confined held-directory/no-follow
guarantees; a cache-directory writer can corrupt staging and induce cleanup/DoS.
The Linux secure-file branch returns before that fallback. Do not blanket-disable
the analyzer rule: path dataflow versus control-only cleanup needs a regression,
and unsupported platforms need confined same-object handling or fail-closed cache
behavior. The linux_dirfs parser coverage gap remains a real build-gate blocker.

Remaining: resolve and verify the build-analysis blockers, complete standalone
CLI/runtime parity and reproducible self-bootstrap; fix default/nil and wider
arity/class/block semantics; qualify raw C99/VM array_chunk and raw slice plus
compute.chunk; finish count/generator contracts and general paradigm composition.
Eager quadratic windows, hard heap/preemption, import/file budgets, runtime
cleanup/isolation, typed FFI/raw bypass, sandbox, package trust/artifact handoff/
revocation/rollback, full product compatibility and physical-platform evidence
remain open. PBLang 1.0.0 remains NOT READY.

Final project build, PBLang-native relative lock publication and touched-file lint
completed. Lint retains existing style warnings and inaccurate identical-operand
advice on deliberately tested nonfinite guards. The final portable host fixture
passes; the invalid-step CLI fixture exits 1 without its payload marker. Privacy
scans and diff checks pass. Graphify stopped at its 20-second deadline (124),
so graph freshness is not claimed. The isolated native-candidate directory was
empty and removed; the active executable's hash, size and modification timestamp
remain unchanged. No test/benchmark/build/Graphify process is retained.

## Remediation ledger — 2026-09-08, selfhost lexical build blockers

Baseline `9274342`. Corrected three reproduced PBLang-owned defects: contextual
match postfix access, physical line/column accounting after multiline strings,
and the default dynamic-eval rule's raw comment/string substring false positives.
Sixteen focused suites pass, including checked malformed-source rejection,
plain/optimized/PBCS execution, whole-linux_dirfs parsing, source positions,
security suppression/custom-policy/raw-builtin regressions and dependency tests.
Independent defensive review found no concrete security regression. No Rust edits.

See `SELFHOST_LEXICAL_BUILD_PROGRESS_2026-09-08.md` for exact implementation,
compatibility and lexical-analysis boundaries. The prior linux_dirfs parser gap
and optimizer-comment eval finding are addressed without changing those callers.
The full native compiler build retry did not complete: 40-second timeout (124),
no final report, no artifact, active executable unchanged. This is not a refreshed
dependency-wide pass or proof that backend execution was reached. Module-cache
taint classification/confinement and standalone CLI/runtime parity remain open.

The reproducible parser control retained a slightly higher median after the
change; no speedup claim. All nine separate numeric rows passed checksums, but
PBLang's retained interpreter remains slower than Python/Node on that kernel.
See `BENCHMARK_SESSION_2026-09-08_SELFHOST_LEXICAL.md`. No full suite ran.

Still required for 1.0.0: complete self-bootstrap and build analysis, contextual/
parenthesized match and general default/arity/class/block semantics, broader
paradigm/compiler/LLVM/JIT qualification, import/file budgets, owner-scoped
runtime cleanup/heap/preemption, typed FFI/raw bypass closure, hostile-code
isolation, package trust/artifact delivery/revocation/rollback, product regression
and physical-platform evidence. PBLang 1.0.0 remains NOT READY.

## Remediation ledger — 2026-09-08, taint admission and completeness

Baseline `ec42869`. Reproduced and fixed a security admission failure: a medium
finding could fill the retained index before a high finding, leaving truncated
evidence with passed=true. Unique finding overflow now blocks; duplicate retained
identities do not falsely mark truncation. AST/call/finding exhaustion makes
interprocedural_complete=false. Retained finding_count semantics are unchanged.

Shared numeric admission rejects nonfinite, fractional, non-number and out-of-
range budgets; dependency analysis validates resolved numeric policy before reads.
The baseline NaN host probe threw an unstructured comparison error, not a proved
unbounded execution. Direct taint now rejects unterminated tokens before parsing.
Per-module and whole-program failures cannot become protect-mode approval.
Intentional compatibility tightening and nil normalization are documented in
`TAINT_COMPLETENESS.md`.

Sixteen targeted suites passed, including all three dependency modes and security
CLI publication/config regressions. Independent defensive review approved the
narrow change. No Rust edit, full suite, source-content scan or host executable
replacement. The nine separate numeric rows pass their exact checksum; the small
before/after analyzer control establishes no speedup. See
`BENCHMARK_SESSION_2026-09-08_TAINT_ADMISSION.md`.

Remaining: pre-tokenization bounds for direct analysis, nested-flow and summary
soundness/performance, cache confinement and control/data reporting, full native
self-bootstrap, general grammar/default/arity/class/block and paradigm coverage,
runtime heap/preemption/ownership, typed FFI/sandbox, package trust/artifact/
revocation/rollback and full product/physical-platform qualification.
PBLang 1.0.0 remains NOT READY.

## Remediation ledger — 2026-09-08, taint source admission

Baseline 56d41d3. Direct taint analysis now enforces a UTF-8 source byte cap before
the tokenizer builds its character array. Default 1 MiB, configurable finite
integer 1..128 MiB. Scalar-count rejection and a four-byte-per-scalar fast path
avoid an unbounded measurement conversion; exact conversion is reserved for the
boundary region. The size failure is constant and omits source/path. This fixes
the missing direct source admission, not a proved hard heap or time guarantee.

Dependency module and aggregate linked budgets retain their existing defaults.
Explicit nested/top-level source caps remain effective through linking, including
legacy nil normalization. Fifteen targeted suites passed: new UTF-8 and two-module
source-bound coverage plus taint, dependency, lexical, manifest/lock and CLI
regressions. Defensive review found no blocking defect. No full suite ran.

The identical small analyzer control rose from median 5.758 to 6.174 ms per ten
analyses; no speedup claim. See BENCHMARK_SESSION_2026-09-08_TAINT_SOURCE_BOUNDS.md
for retained raw samples, separate competitor rows and qualification limits.

Still required: tokenizer token/per-token growth and parser nesting limits,
nested-flow/summary soundness, cache confinement, full native self-bootstrap,
general compiler/paradigm compatibility, VM heap/preemption/ownership, typed
FFI/raw bypass and sandbox, package trust/artifact/revocation/rollback, product
regression and physical platforms. No executable refresh or Rust edits in this
slice. PBLang 1.0.0 remains NOT READY.
