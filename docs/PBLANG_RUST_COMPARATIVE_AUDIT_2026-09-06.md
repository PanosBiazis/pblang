# PBLang and retained Rust: comparative audit, 2026-09-06

## Decision and scope

**Not ready to certify 1.0 or hostile-code/multi-tenant isolation.** Both codebases
contain substantial working implementations and meaningful protections. Concrete
correctness, capability, lifecycle and package-verification defects remain.
This report consolidates the cross-domain assessment; it does **not** claim a
completed line-by-line audit of every file, all tests, dependencies or hardware.

Base commit: `30a450a` (full identity available in repository history). Analysis
used the live working tree, which already had 41 modified tracked paths and 152
untracked status entries. That work was preserved. This continuation changes only
this report; it does not fix or retire either implementation.

Inventory is not coverage: `crates/` plus `src/` contain 90 Rust source files,
58,519 lines; `stdlib/` contains 639 `.sapp` files, including 19 compiler modules
totalling 13,114 lines; 972 semantic `*_test.sapp` files are discoverable. These
are current local filename/line counts, not assertions, executed tests or audited
percentages. Other examples/tools/tests and generated foreign code also matter.

## Architecture: where the implementation actually lives

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
