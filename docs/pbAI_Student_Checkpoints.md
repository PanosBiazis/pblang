# Explicit student checkpoints

The fixed rank-1 student returned by `neural_specialist_distill` or
`neural_specialist_distill_supervised` now exposes `checkpoint()`. Calling it
returns a detached artifact; it does not write a file or change the live student.
The shared implementation is PBLang `stdlib/pbai_neural_adapter.sapp`.

## Portable payload and loading

Both product facades expose:

- `neural_specialist_student_encode(artifact)` returns JSON.
- `neural_specialist_student_decode(text)` validates JSON and returns an artifact.
- `neural_specialist_student_load(artifact)` returns a private diagnostic handle.
- `neural_specialist_student_save_new(path, artifact)` explicitly creates a new file.
- `neural_specialist_student_load_file(path)` reads and validates a saved student.

An artifact has exactly seven fields: `format`, `profile`, `base_digest`, `a`, `b`,
`steps`, `digest`. Its distinct `pbai.neural.student-checkpoint.v1` format binds
rank 1, alpha 2 and the fixed tiny base configuration. It cannot be substituted
for the rank-2 teacher artifact. The 8x1 and 1x8 matrices use integer fixed-point
values at scale 1,000,000, bounded to +/-10,000,000,000; step count is 0..4096.
The digest binds format/profile/base/matrices/counter; the base digest must match
the locally reconstructed frozen model. Digests establish content consistency,
not a trusted publisher, authorized training, or authenticity. A party able to
edit weights can also recompute their digest.

Encoding validates the complete fixed shape before serialization. Decoding uses
`serialization.parse_safe` with 4096 UTF-8 bytes, depth 4, 64 items, 128 lines,
4096 bytes per line and decoded duplicate-key rejection. These checks run before
the builtin JSON parser. Malformed, extra-field, nonfinite, cyclic, wrong-profile,
wrong-base and digest-mismatched payloads are rejected. Whitespace and ordinary
JSON formatting are accepted within those bounds. The payload contains no raw
training/validation examples, source paths, teacher weights or training labels.
Weights and hashes can nevertheless encode information; the artifact is not an
anonymized or encrypted representation.

Loading produces `report`, `predict`, `evaluate`, `checkpoint`, and `close`.
The weights are detached from the caller's input; editing reports, artifacts or
prediction arrays cannot change subsequent inference. There is no promotion,
default-router replacement, automatic learning or in-place training method.
[Explicit continuation](pbAI_Student_Continuation.md) accepts a checkpoint and
returns a separate candidate, preserving the loaded parent.
Evaluation compares the restored student against its initial frozen-base student.
Because split history is not exported, `exact_student_input_disjointness_checked`
is false. No teacher agreement or independent-holdout claim is reconstructed.

## Quantization is explicit

The original live student retains its floating-point weights. Export rounds each
factor to six decimal places. Restored logits and loss may therefore differ;
near a decision boundary even a predicted token could change. The checkpoint
identity differs from the live float-weight digest. Do not interpret one as the
other, or claim a lossless floating-point checkpoint. Export/decode/load/export
is idempotent for checkpoints produced by this implementation.

Run `bench/pbai_student_checkpoint.sapp` to compare the original and restored
student for the same three predeclared objectives and 16 synthetic held-out
examples. It reports actual encoded size, changed predictions, maximum absolute
logit difference, before/after loss/accuracy, and seven decode/load and evaluation
timing samples after one warmup. Timings exclude training and filesystem access;
shared-desktop load is uncontrolled and no speed advantage is claimed.

## Explicit file persistence and platform boundary

`stdlib/pbai_student_checkpoint_io.sapp` delegates to `secure_file.sapp`.
The caller must provide an existing current-user-owned directory with mode 0700.
Save creates a new mode-0600 file and refuses an existing destination. Neither
operation follows symlink path components, creates parent directories, logs the
path, or falls back to ordinary unguarded filesystem operations. Reads are bounded
to 4096 bytes before parsing. Validation of an artifact precedes file creation.

Private-parent ownership guarantees are currently implemented only on Linux;
the file adapter rejects other platforms. The pure payload codec and inference
have no platform branch, but this release was tested only on Linux x86-64 with
the retained PBLang 0.6.0 runtime. The existing Linux backend needs filesystem,
FFI and process-execution authority for its local descriptor/ownership checks;
network access is not needed. A denied required capability causes failure.
Do not infer whole-backend certification from successful operations: its general
FFI-symbol capability probe still reports an unverified contract on this runtime.

Exclusive creation protects against replacing an existing name. It does not
provide atomic content visibility or verified crash durability; a concurrent
reader may reject a partial write. No fsync durability or encryption is claimed.
Mode 0700 is not a boundary against privileged or malicious same-user processes.
Failures are reported generically without returning the caller's path; a failed
save must not be treated as a usable checkpoint.

## Example and verification

See `examples/pbai_student_checkpoint.sapp` in PBLang and
`demo/pbai_student_checkpoint.sapp` in standalone pbAI for the pure round trip.
For explicit private storage, use the separate
[checkpoint binding](pbAI_Checkpoint_Binding.md), including when running from a
foreign working directory. Legacy wrappers remain compatible from the trusted
product directory:

```sapp
// Place these imports at application top level.
let AI = import("stdlib/pbai.sapp");
let IO = import("stdlib:pbai_student_checkpoint_io.sapp");
let storage = AI.neural_specialist_checkpoint_bind(IO);
let checkpoint = student.checkpoint();
let receipt = storage.save_new(path, checkpoint);
let restored = storage.load_file(path);
print restored.predict([6,0]).token;
restored.close();
```

`pbai_student_checkpoint_test.sapp` checks both distillation variants, round-trip
identity, caller-mutation isolation, tampering, shape/size/depth/duplicate-key
bounds, and honest evaluation scope. The explicitly invoked fixture
`tests/fixtures/pbai_student_checkpoint_file.sapp` uses `PBAI_CHECKPOINT_TEST_DIR`
and `PBAI_CHECKPOINT_TEST_PHASE=write|read|denied`; it must run only against an isolated
fixture directory. Separate processes verify actual persistence, refusal to
replace existing files, symlink/private-directory checks, oversized/malformed
reads and nonblocking rejection of a FIFO. Host checks verify file mode and
preservation of rejected destinations.

See [measurements](benchmarks/pbai_student_checkpoint_release-2026-09-27.json)
and [verification](benchmarks/pbai_student_checkpoint_release-checks-2026-09-27.json).
Representative language evaluation, authenticated provenance, production
replacement gates and other-platform private storage remain
separate work. This is not evidence of AGI, ASI or universal device support.

Recorded Linux results: artifacts occupy 455, 459 and 464 bytes for weights 0,
0.5 and 1. None of the 16 predictions changed in any variant; accuracy stayed
8/16. Maximum absolute logit drift was respectively 0.000002658, 0.000005105 and
0.000002847. Both products produced identical checkpoint digests and quality
metrics. This establishes fixture round-trip behavior, not accuracy improvement.

The adapter also validates backend result types, exact byte counts and exclusive
creation flags before reporting success. Malformed responses cannot replace a
previous loaded handle; callers assign a replacement only after successful load.
See [backend failure handling](pbAI_Checkpoint_Binding.md#backend-failures-and-response-validation)
for the preserved error contract, synthetic fault tests and `reload` example.

## Explicit loaded-handle close

The bundled `student_load(artifact)` handle also provides `close()`. This reaches
both public facades, explicit specialist binding, and the real checkpoint file
adapter without adding another facade or binding entry point.

- First close returns `true`; later calls return `false`, including through saved
  method aliases. Closing an unused, predicted, evaluated or exported handle is
  allowed.
- `predict`, `evaluate` and `checkpoint` then throw `student checkpoint is closed`
  before inspecting ordinary or malformed inputs. Saved method references share
  that terminal state. There is no reopen method.
- The closed state drops its references to the model, initial/student adapters,
  checkpoint identities and counters. Closures are built in a state-only helper,
  so they do not retain the loading function's caller artifact or model locals.
- Existing `report` metadata and earlier prediction/evaluation/checkpoint exports
  remain caller owned. Editing public fields cannot reopen the private state.
  A previously exported checkpoint can explicitly create a new independent handle.
- Close performs no file I/O, provider call, training or activation. It neither
  deletes persisted checkpoints nor closes independent handles.

This is logical ownership release for single-threaded local use. It does not
promise immediate garbage collection, reduced resident memory, secure zeroization,
revocation of prior exports, or concurrent close/inference synchronization. The
mutable diagnostic `report` is retained for compatibility and is not live status
or authority to reopen the handle. Arbitrary custom providers remain responsible
for their own returned-handle contracts.

The file example's `reload` phase closes the retired handle only after a valid
replacement is returned and assigned. Missing/corrupt replacements keep the
previous model open. Its `close` phase demonstrates terminal reuse rejection and
reloading an earlier export. With the existing explicit private directory set,
use `PBAI_CHECKPOINT_TEST_PHASE=close` after the `write` phase.

`tests/semantics/pbai_student_load_close_test.sapp` covers direct, legacy facade and
explicit bound facade loading across four prior-use states, including saved method
aliases, malformed/cyclic inputs, independent handles and retained exports.
`tests/semantics/pbai_checkpoint_receipts_installed_test.sapp` has a `close` phase
that tests the real file backend with original sources hidden, a foreign caller
and decoy modules, exact cross-product output parity, unchanged persisted bytes,
and the primary terminal error for predict/evaluate/checkpoint. The existing
`roundtrip` phase checks retirement only after successful replacement. The fault
receipt fixture confirms close makes no further backend calls.

A focused regression also rejects a foreign base digest even when the checkpoint
content digest is recomputed consistently, preserving the existing fixed-base
validation in encode, decode and load.

See [loaded lifecycle evidence](evidence/pbai_loaded_close-2026-10-05.json) for
actual checks and comparable timing samples. An investigated base-construction
reuse change was not retained: it reduced construction counts, but elapsed-time
measurements did not show a consistent benefit across both installed products.
The original artifact validation and model construction path remains in place.

## Complete file usage and separate timing workloads

The `read` example closes its loaded handle after obtaining the detached prediction.
The `reload` example keeps the old handle usable while a missing/corrupt replacement
is rejected, closes a retired handle only after a valid replacement is assigned,
and closes the final active handle after prediction. Both phases verify that the
saved prediction method rejects reuse, then report `active_closed` and
`active_prediction_blocked`. These are checks of explicit successful example paths,
not exception-safe resource scopes or memory zeroization guarantees.

`bench/pbai_checkpoint_file_origin.sapp` retains its historical load-plus-predict
workload unchanged so previous timing results remain interpretable. It excludes
explicit close and must not be described as a full lifecycle measurement.

Two separate bounded workloads complement it:

- `bench/pbai_loaded_prediction.sapp`: 100 predictions from one already-loaded
  handle per sample. Setup/training/loading and final close are outside timing.
  The saved prediction alias must reject reuse after close.
- `bench/pbai_checkpoint_file_lifecycle.sapp`: five file-load, prediction and close
  operations per sample. Requires a fresh private directory through
  `PBAI_CHECKPOINT_TEST_DIR`; it creates one synthetic checkpoint exclusively.
  Every loaded handle is closed before loading the next. Forty retained prediction
  aliases (including warmup) must reject after timing. A separately loaded reference
  handle is closed before timing. No file is deleted by close.

Both report seven raw elapsed-time samples after one warmup, explicit timing scope,
synthetic artifact identity/size and the same prediction. Lifecycle timing includes
fixed alias retention and output assertions but excludes terminal-alias validation,
imports and training. These workloads do not measure resident-memory release,
real-world model quality, AGI/ASI, or other-platform storage support.

An inline closed-state-guard prototype was rejected: alternating baseline/candidate
runs did not show consistent improvement in either the file or loaded-prediction
workload. The shared neural owner remains unchanged. See
[measurements and lifecycle checks](evidence/pbai_file_usage-2026-10-05.json).
