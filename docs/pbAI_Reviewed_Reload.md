# Compare before replacing a loaded student

A structurally valid checkpoint can still regress on the application's examples.
The bundled loaded student handle now provides `compare(candidate, examples)`:
read-only comparison against its actual loaded fixed-point weights. It returns
aggregate metrics and a bounded admission result. It does not replace, close,
train or persist either model. No new provider framework or binding is needed:
direct loading, the existing specialist binding, legacy facade loading and file
loading return the same bundled handle contract in both products.

```pblang
let current=storage.load_file(current_path);
let candidate=storage.load_file(candidate_path);
let review=current.compare(candidate.checkpoint(),[map{x:[6,0],y:5}]);
candidate.close();
print review.passed;
current.close();
```

That minimal fragment illustrates the method. For cleanup after a catchable
error and an actual session replacement, use the full runnable example below.
Custom providers retain responsibility for their own returned-handle methods;
existing binding metadata does not authenticate or verify a provider's compare.

## Bounded comparison and decision

The candidate must pass the unchanged strict student-checkpoint validator.
Examples use the existing 1..32 unique-context contract: each has exactly x/y,
x contains 1..4 integer token IDs 0..7, and y is an integer label 0..7. Malformed,
cyclic, extra-field and duplicate-context inputs are rejected. Positive and
negative zero denote the same context. There is no recursive arbitrary-payload
walk or invocation of supplied functions. The loaded handle's terminal guard
runs before candidate/example validation.

The operation reconstructs only the candidate head from its fixed-point weights
and reuses the active handle's frozen model and baseline student. The existing
paired evaluator supplies both losses, correct counts and per-example regressions
on the same features. This is code reuse, not a measured performance improvement.

The `pbai.neural.student-comparison.v1` report contains:

| Field | Meaning |
| --- | --- |
| `baseline`, `candidate` | Aggregate examples, mean loss, correct count, accuracy |
| `baseline_correct_regressions` | Baseline-correct examples made incorrect |
| `non_regression_passed` | Valid finite metrics, non-increasing mean loss, non-decreasing correct count, zero correct-example regressions |
| `non_vacuous` | Baseline has at least one correct example |
| `passed` | Both preceding conditions hold |
| `baseline_digest`, `candidate_digest` | Content identities of the compared checkpoints, not authenticated provenance |

Identical checkpoints can pass; an application may treat the same digest as a
no-op. Equal accuracy with worse loss fails. A both-wrong tie fails because it is
vacuous. Reports are detached; editing them cannot affect later comparisons.
Failed comparisons also leave the active checkpoint and candidate artifact
unchanged. Saved compare methods share closed state with predict/evaluate/export;
after close they throw `student checkpoint is closed` before validation. Reports
already returned remain caller-owned and usable.

All results concern caller-declared examples only. Labels are not authenticated;
prior training overlap, independent testing and globally limited repeated attempts
are not established. The comparison does not claim quality beyond these examples,
generalization, trustworthy training history or authorization to activate a model.
It returns no raw examples or weights. Aggregate metrics and digests are not an
anonymization guarantee. No request or result is logged or persisted by compare.

## Runnable session recovery example

Use `examples/pbai_student_checkpoint_reload_reviewed.sapp` in PBLang or
`demo/pbai_student_checkpoint_reload_reviewed.sapp` in standalone pbAI. Run from
the selected product with its materialized manifest/lock and trusted runtime.
The file backend currently requires Linux plus its existing filesystem/FFI/process
grants. No network is needed.

```sh
# Linux mktemp creates a fresh current-user-owned directory with mode 0700.
export PBAI_CHECKPOINT_TEST_DIR="$(mktemp -d)"
PBAI_RELOAD_PHASE=prepare pblang run examples/pbai_student_checkpoint_reload_reviewed.sapp --allow-all --deny-net --deny-http
PBAI_RELOAD_PHASE=recover pblang run examples/pbai_student_checkpoint_reload_reviewed.sapp --allow-all --deny-net --deny-http
PBAI_RELOAD_PHASE=review-error pblang run examples/pbai_student_checkpoint_reload_reviewed.sapp --allow-all --deny-net --deny-http
```

Use the demo path for standalone. The directory retains the synthetic files for
inspection after the runs; no checkpoint is automatically deleted. Preparation exclusively creates three synthetic
checkpoints, refusing to overwrite existing names: student.json (label 5, 64
steps), worse.json (label 6, 64 steps), better.json (label 5, 128 steps). The
example uses one declared application example, x=[6,0], y=5. These tiny rank-1
8-token specialists are a reproducible fixture, not a representative AI benchmark.

Recovery tries missing, worse, better and better again. The expected decisions
are load_failed, rejected, replaced, unchanged, with active token 5 throughout.
An unsuccessful acquisition or review keeps the current handle available. Each
loaded candidate is reviewed inside with_file and closed before adoption is
considered. Only a passing detached checkpoint is returned from the review.
A new independent handle is reconstructed from that exact artifact, not by
rereading the file path. Only after this load succeeds does the example select it
and close the retired handle. The same digest avoids unnecessary replacement.
If reconstructing the replacement throws, the example retains the current handle
and reports replacement_load_failed. The reconstruction acceptance below tests
this branch with an exception injected at the constructor entry in private copies.

An outer resource scope owns the final active handle and closes it at the end.
The cleanup factory directly captures a private state map whose current close
function is updated on replacement. This uses the previously tested capture
pattern; it is not a general runtime closure fix. The example verifies that saved
prediction methods for retired/final handles are terminal. review-error supplies
an invalid comparison set and demonstrates candidate cleanup plus preservation
of the current model. The installed test also corrupts a private candidate copy
and verifies recovery without rewriting it.

The comparison gate is an explicit application policy. The example's replacement
is in this synchronous process only. Original checkpoint files are unchanged;
restarting the example loads the original student.json again. There is no global
router/registry update, persistence of a selected model, automatic training,
concurrent-request coordination or durable transaction. The example uses bundled
handles; it does not establish recovery guarantees for dishonest or failing custom
closers. Hard termination, OOM, nonreturning callbacks and deadlines remain outside
the existing scope contract. Earlier detached outputs cannot be revoked.

## Verification

- `tests/semantics/pbai_student_compare_test.sapp`: direct, legacy and explicitly
  bound loads; actual paired metrics against independent loaded evaluation;
  better/worse/equal/vacuous comparisons, equal-accuracy loss regression, max 32
  examples, invalid artifacts/examples, signed-zero duplicate contexts, detached
  results, unchanged model/artifact, and terminal saved compare method.
- `tests/semantics/pbai_student_reload_reviewed_installed_test.sapp`: set trusted
  PBLANG_BIN and PBAI_RELOAD_PRODUCT=pblang|pbai for one bounded product run.
  Optional PBAI_SOURCE_HIDE_ROOT hides another source tree. Both product copies
  run with original sources hidden, a foreign manifest/cwd, eight decoy modules
  and network denied. It characterizes the old format-only reload adopting the
  regressing token 6, then verifies the reviewed sequence, invalid-review recovery,
  corrupt-file recovery, exact closed-handle error without private input, unchanged
  checkpoint bytes and fixture cleanup.

- `tests/semantics/pbai_student_reload_reconstruction_test.sapp`: Linux acceptance
  using the same trusted runtime/product variables and optional hidden source root.
  Run from the selected checkout with adjacent `pblang` and `pbai` checkouts,
  `/usr/bin/bwrap`, `/usr/bin/timeout`, `/usr/bin/env` and `/bin/cp`, `/bin/chmod`,
  `/bin/rm` available. The retained runtime must permit the required file/process
  operations; outer and inner invocations deny network/HTTP.
  It copies both products, hides original sources, uses a foreign manifest/cwd with
  nine decoy modules (including json_decode), and denies network. An unmodified
  example run calibrates normal behavior before any instrumentation. The companion
  `tests/fixtures/pbai_reload_reconstruction_hooks.sapp` is inlined only into the
  copied shared neural owner at exact constructor/export anchors; it is never a
  production dependency or runtime fault option. Both copied package locks are
  rebuilt after instrumentation. Each mode runs in a fresh process:

  | Mode | Four recovery decisions | Real handles | Injected failures |
  | --- | --- | --- | --- |
  | none | load_failed, rejected, replaced, unchanged | 5 | 0 |
  | once | load_failed, rejected, replacement_load_failed, replaced | 5 | 1 |
  | repeat | load_failed, rejected, replacement_load_failed, replacement_load_failed | 4 | 2 |

  The no-fault example output must be byte-identical to calibration. Faults occur
  only for reconstruction of the already-reviewed better checkpoint, after its
  review candidate has closed. The probe verifies the old prediction alias and
  digest survive each failure, a subsequent successful retry retires the old
  handle, and every successfully constructed native handle is closed before the
  diagnostic repeated-close probe. All three checkpoint files remain byte-identical.
  The private fixture is removed after success or a caught test failure.

The injection throws **before** the selected call delegates to the original
constructor. Successful calls return actual unmodified native handles. This tests
catchable reconstruction failure and ownership in the unchanged application
example; it does not simulate partial allocation failure inside the constructor,
OOM, abrupt process termination, or recovery on other operating systems. Test
output contains synthetic fixture metrics/digests and bounded trace fields only.
See [reconstruction evidence](evidence/pbai_reconstruction_failure-2026-10-05.json).

## Failures after partial construction

`tests/semantics/pbai_student_reload_partial_test.sapp` extends the preceding
entry-point test. Use its same Linux utilities, adjacent product checkouts and
trusted runtime/product variables, plus `PBAI_PARTIAL_STAGE=model|initial|student`.
Run each stage separately so each product/stage remains a bounded acceptance run.
The real runnable reviewed-reload example is unchanged. With `PBLANG_BIN` exported
to the absolute path of the trusted executable, run from the selected checkout:

```sh
PBAI_RELOAD_PRODUCT=pblang PBAI_PARTIAL_STAGE=model "$PBLANG_BIN" run tests/semantics/pbai_student_reload_partial_test.sapp --allow-all --deny-net --deny-http
```

Repeat with `initial` and `student`. For the standalone checkout select
`PBAI_RELOAD_PRODUCT=pbai`; use the same test path and stages.

The test copies the production constructor and inserts an observer immediately
**after** one of three real operations has returned:

| Stage | Actual operation already completed | Later operations skipped on failure |
| --- | --- | --- |
| model | `_model()` creates the tiny decoder | initial adapter, restored student, returned handle |
| initial | `AD.lm_head_adapter` creates the rank-1 seed | restored student, returned handle |
| student | `AD.from_state` creates the saved fixed-point student | returned handle |

Each run calibrates against the uninstrumented example, then tests none/once/repeat
fault modes. The observer checks the real model/adapter type, matrix dimensions
and restored step count, and records only the ordered stage names and bounded
booleans. It retains no reference to a partially created model or adapter. A
failure is injected only into reconstruction of the previously approved candidate,
after its review handle has closed. Full stage traces prove that subsequent
constructor stages did not run. Normal runs retain byte-identical example output.

Both products must retain the old active digest and prediction alias on each
failure, permit a later successful retry, close all successfully returned handles
before the diagnostic close probe, and leave the three checkpoint files unchanged.
The test-only observer lives in `tests/fixtures/pbai_reload_partial_hooks.sapp`;
its exact-anchor source instrumentation is confined to temporary package copies.
It introduces no production fault setting, alternate provider, JSON or VM edit.

**Cleanup boundary:** this construction path creates managed PBLang maps and
arrays, not independently closable files, device contexts or native allocation
handles. A failed invocation has no returned student handle to close. This test has no per-object allocator-release observation for those
managed values. The retained `stdlib:runtime_gc.sapp` contract declares
`owner_isolation:false`, `collection_allowed:false` and
`allocation_counters_are_live_usage:false`; cumulative allocation counters and
forced collection cannot substitute for this missing evidence. Therefore the
trace explicitly reports `partial_memory_release_verified: false`. The tests
verify recovery after real partial construction and closure of the successful
handles; they do not prove physical release, GC timing, RSS reduction, zeroization,
OOM recovery, faults *inside* a model/matrix operation, or hard-termination cleanup.
A process exiting or temporary fixture removal is not used as evidence of memory
release. Existing entry-point evidence and its limits remain unchanged.
See [partial-construction evidence](evidence/pbai_partial_construction-2026-10-05.json).

The format-only reload example remains available with its original contract.
JSON/VM owners, checkpoint formats, codecs, storage, scopes and provider binding
surfaces are unchanged. No speedup, AGI/ASI, universal-device support or independent
model-quality certification is claimed. See [recorded evidence](evidence/pbai_reviewed_reload-2026-10-05.json).
