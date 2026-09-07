# PBLang-first analysis continuation — 2026-09-06

## Scope and evidence

This is an analysis report, not a runtime patch or a completed whole-language
audit. Findings describe the working tree based on commit
`9d37ed0b204f3ecc195c55acf285c02a0d66eeff`. At analysis start there were 41
modified tracked paths and 152 untracked status entries; these pre-existing
changes were preserved. In particular, the frontend is locally modified.
Findings are not attributed to a particular author, model, or date without a
separate baseline comparison.

PBLang implementations are the primary subject. Rust is a retained host/reference
boundary, not assumed to be the only implementation of the language. Tests used
the local `target/release/pblang` 0.6.0 to execute PBLang modules; this does not
establish independent bootstrap or parity of every backend. No source was changed,
no full suite was run, and no devices, GUIs, services or foreign artifacts were
started. Only this report was added.

## Coverage added in this continuation

| Area | Review depth | Remaining boundary |
| --- | --- | --- |
| `stdlib/compiler/selfhost_frontend.sapp` | Complete module read; parser, compiler, lint, imports, file and bounded entrypoints traced | Parser/typechecker/optimizer internals not completely reviewed in this continuation |
| `stdlib/execution_pipeline.sapp` | Complete module read; consumer routing and size checks traced | Backend internals and actual emitted artifacts not all executed |
| `stdlib/compiler/module_cache.sapp` | Complete module read | Concurrent publication, alternate filesystem/platform behavior and publisher provenance not proved |
| `stdlib/migration_policy.sapp` | Complete module read; manifest-policy enforcement distinguished from mandatory release enforcement | Every CI/release invocation not established |
| `stdlib/host_pbck_vm.sapp`, `stdlib/host_pbck_pure.sapp` | Complete security-agent review with relevant contracts/tests; parent reproduced two small boundary cases | No hostile native execution, full heap accounting or wall-clock guarantee |
| Tests | Three targeted tests plus bounded synthetic probes below | No whole-suite, randomized-order, device or application compatibility claim |

Earlier reads of the self-host CLI and earlier native-backend findings are context,
not newly completed whole-subsystem reviews. Inventory and pattern matches are not
counted as complete implementation review.

## Findings requiring correction

### A1 — High: bounded frontend imports can execute without a VM budget

`frontend_run_bounded(source, nil)` builds the import cache with nil options.
`_resolve_module` then chooses `_VM.run_seeded`, while the main chunk uses
`_VM.run_seeded_bounded`. Supplying an options map bounds each module separately,
but does not establish one cumulative budget across imports and the main chunk.

The textual `_find_imports` scanner also matches import text in comments/strings
and resolves modules before the main chunk executes. Existing resolvable modules
can therefore run top-level code even when the text is not an executed import.
Preconditions are filesystem authority and an accessible module; this is not a
claim that the pure VM exposes arbitrary host access by default.

Evidence: complete source/control-flow review, independently corroborated by the
security reviewer; no live import-side-effect or infinite-loop fixture was run.
Relevant locations: `selfhost_frontend.sapp:306`, `:360`, `:431`, `:689`.

Needed: consistent nil/default routing, aggregate budgets and language-correct
import resolution without execution triggered solely by comments/dormant syntax.

### A2 — High: file bounds apply after whole-file materialization

`host_pbck_vm.run_file` calls `Decode.decode_file`, which calls
`decode_bytes(fs_read_bytes(path))`. Decoder limits cannot bound the earlier
whole-file read. Huge files can allocate before validation; special files can
block. The bounded frontend file/module paths similarly call `fs_read` before
checking UTF-8 source bytes.

Evidence: static call-chain review; no huge/special file was opened.
Relevant locations: `host_pbck_vm.sapp:644`, `host_pbck_decode.sapp:274`,
`selfhost_frontend.sapp:371`, `:717`.

Needed: bounded held-descriptor reads and explicit regular-file/identity policy
before materialization. Investigate analogous frame-VM file entrypoints separately.
The earlier retained-host reader fixes do not automatically fix these PBLang APIs.

### A3 — Medium: exception exits skip deferred cleanup

Guest call propagation, failed unwrap and explicit throw can return directly
without `_finish_frame`/`_run_defers`. This conflicts with frozen defer semantics
requiring cleanup on throw. `_run_defers` additionally stops on a failing cleanup;
the frozen specification says cleanup errors must not hide the original failure.

A trusted in-memory probe registered a recording adapter as a defer, then threw:
`exception_caught=true, cleanup_count=0`. Its normal-return control produced
`normal_value=7, normal_cleanup_count=1`. No filesystem handles or other external
effects were involved. Relevant locations: `host_pbck_vm.sapp:352`, `:524`,
`:565`, `:573`; contract: `docs/Spec_freeze.md:66`.

Needed: one exit/unwind path preserving the original error and completing LIFO
cleanup. Existing full-VM tests cover normal defers, but lack throw-plus-defer;
their failing-cleanup expectation also needs reconciliation with the specification.

### A4 — Medium: stack/local admission is late; frame count is off by one

`_normalized_stack` allocates parameter cells before the first stack/local-limit
check. Global decoder/allocation limits still apply; this is not evidence of
unbounded heap corruption. It does violate smaller caller-selected limits.
Separately, depth checks use `depth > max_frames` with a zero-based root.

A two-frame, constant-return probe configured `max_frames=1` and returned
`observed_peak=2, value=7`. Parameter pre-allocation is statically established,
not measured by an allocation probe. Locations: `host_pbck_vm.sapp:286`,
`:367`, `:583`.

Needed: preflight parameter slots and admit frames using the actual live count.

### A5 — Medium: compile bypasses the unterminated-token diagnostic

`frontend_parse_safe` rejects `let x=1; /* unfinished`, but `frontend_compile`
directly tokenizes/parses without the same unterminated-token check. The probe
returned `parse_ok=false, compile_ok=true, compile_diagnostics=0`. The self-host
CLI's `compile_source` also emitted its artifact in memory for this input.
A closed-comment positive control compiled successfully.

Locations: `selfhost_frontend.sapp:100`, `:193`,
`compiler/selfhost_cli.sapp:11`. File/guard-preparation compilation uses similar
raw parsing, but those paths were not separately executed in this continuation.

Needed: shared validated frontend entrypoint and compilation-level negative tests,
not only parser tests. The existing unterminated-token test passes without covering
the compiler route that accepts the malformed input.

### A6 — Medium: build-source byte limit counts Unicode characters

`execution_pipeline.build_source` compares `len(source)` with `max_source_bytes`.
The synthetic source `let x="é";` has 10 characters and 11 UTF-8 bytes; an
interpreter build with a limit of 10 was accepted. Location:
`execution_pipeline.sapp:184`.

Do not extend this result to the final `run_source` boundary: it delegates to the
frontend's additional UTF-8 check. Artifact string/map sizing also needs a defined
serialized-byte contract; post-construction checks are not hard heap enforcement.

Needed: consistent byte units and exact multibyte boundary regressions.

### A7 — Medium: lint suggests a name invisible at the use site

For `fn f() {let value=1;return value;} print valeu;`, frontend lint suggested
`value`, which is local to `f`. The separate diagnostics surface returned no
suggestion. Locations: `selfhost_frontend.sapp:529`, `:543`.

Needed: one visibility-aware candidate policy across surfaces. This does not show
that the recently corrected diagnostics autofix makes the same mistake.

## Positive evidence and memory-safety interpretation

The PBLang code already implements meaningful safeguards: decoder byte/tag/count/
depth/value validation; opcode stepping; checked collection access; pure builtin
allowlists; adapter-error masking; and cumulative guest allocation charging.
The VM honestly reports `hard_heap_enforced=false` rather than claiming that
allocation units equal the host's complete heap usage.

Memory safety, resource ceilings, exception cleanup and sandbox authority are
different guarantees. A missing byte cap does not by itself prove a use-after-free;
a safe PBLang module does not automatically prove safety of emitted native code or
trusted foreign adapters. This audit therefore neither dismisses the PBLang safety
model nor certifies every execution route as safe.

## Validation and remaining analysis

This continuation passed exactly these targeted tests:

- `execution_pipeline`
- `selfhost_tokenizer_unterminated`
- `selfhost_parse_safe`

Synthetic stdin probes additionally confirmed A3, the frame part of A4, A5, A6
and A7, with positive controls where described. They used a 15-second external
timeout, a 2-GiB virtual-memory ceiling and disabled core dumps. Those are host
test safeguards, not evidence of VM-wide hard enforcement. Results were inspected;
interpreter process success alone was not treated as a semantic assertion.

The preceding commit-date audit separately reported 19 targeted tests and nine
exact negative cases. They were not rerun here and are not added to this turn's
test count. Fixed shared-path CLI fixtures and the parallel compile test were not
run indiscriminately; inspect fixture ownership and worker bounds first.

Next analysis priorities: exception/unwind parity across runtimes; import and
shared-budget design; complete type/optimizer review; remaining native/WASM/C
backend safety and equivalence; concurrency/cancellation; packages and publisher
trust; standard-library/network/data contracts; comprehensive test-quality review;
reproducible performance; and application/platform/device compatibility. Physical
Windows/macOS/mobile evidence remains unavailable in this continuation.

Release 1.0 readiness, zero vulnerabilities and universal compatibility are not
established. Findings above remain open: recording them is not fixing them.

## Remediation update — 2026-09-07, A6 direct-source byte admission

At baseline `c3b0ace`, the A6 direct-source defect remained reproducible. The new
`execution_pipeline_byte_limits` regression failed on the unchanged pipeline:
the interpreter build accepted the Unicode source with a byte ceiling one below
its actual UTF-8 size. It passed after the PBLang-only correction.

Both `build_source` and `run_source` now share source validation: reject non-text,
reject an excessive character count before encoding, then enforce UTF-8 byte
length before backend dispatch. Text artifacts and diagnostic map/scalar
representations also use UTF-8 byte lengths, while binary byte vectors retain
their element-count metric. Exact N/N-1 tests cover ASCII, two-/three-/four-byte
characters, combining marks, all three source-build targets and bounded preview.
Artifact boundary tests and malformed numeric-limit tests are included.

This repairs **A6 direct source admission** and the adjacent artifact-unit
mismatch; it does not establish serialized-bytecode or hard-heap enforcement.
Artifact checks happen after generation, and UTF-8 measurement allocates an
array. Map/scalar metrics describe diagnostic rendering, not serialized PBCK.

**Related open finding, A6-L:** the in-memory module linker in
`stdlib/source_inliner.sapp` still uses character counts for byte-named virtual
path, module-source, aggregate-source and linked-output ceilings. Relevant
reviewed paths include `_canonical_module_path`, `_canonical_modules`,
`_collect_bounded` and `link_modules`. Unicode project snapshots can exceed
their named byte budgets; do not claim end-to-end UTF-8 project bounds until
these owning-module policies and tests are corrected. This is source-reviewed,
not a newly executed project-snapshot exploit. A1/A2/A5/A7 remain separate work;
the A3/A4 remediation scope is recorded in the comparative audit ledger.

## Remediation update — 2026-09-07, A6-L bounded module linking

Baseline `11e8d1a`. A6-L's bounded-linker byte-accounting defect is now repaired
in PBLang. Raw/canonical virtual paths, main and module sources, supplied and
reachable totals, and transformed output all use UTF-8 units. Module sizes are
cached at admission; shared dependencies are counted once. Output accounting
includes LF separators and admits empty text without confusing zero with failure.
The file-backed wrapper and reachable-snapshot selection now check the same units
before import scanning, but their initial snapshot loading is not newly bounded.

The new in-memory test failed against the unchanged module on Unicode root-byte
metadata and passed after correction. Exact/minus-one source, module, total,
output, raw-path and canonical-expansion boundaries are exercised, along with
unused modules, a diamond dependency graph and the hosted-C99 pipeline consumer.
No file-backed snapshot execution or universal safety claim follows from these
tests. Legacy resolver implementations are unchanged. Unicode closure evidence
whose identity incorporates `output_bytes` must be regenerated; see the canonical
source-linker contract. Other open findings and 1.0.0 release gates remain.
