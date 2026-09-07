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
