# Bounded student tools and reusable reviewed inference

Both pbAI facades expose `neural_specialist_student_tool(request)`. This is a pure,
synchronous **in-process map protocol** for a caller building a tool adapter. It is
not registration in an MCP server, an HTTP service, a general LLM, or authorization
to invoke other tools. All product implementation is in PBLang.

Each response has exactly `format: "pbai.neural.student-tool.v1"`, `status`,
`result`, and `error`. Successful `status: "result"` responses have `error: nil`.
Failed `status: "error"` responses have `result: nil` and a fixed error code:
`invalid_request`, `invalid_checkpoint`, `unsupported_operation`, or
`computation_failed`. Errors do not echo input, paths, checkpoint contents or raw
exception text. Unsupported training/acceptance operations fail explicitly.

## Exact request shapes

| Operation | Required fields | Result |
| --- | --- | --- |
| `capabilities` | `op` | Supported operations, exact field names, numerical limits and explicit unsupported authority |
| `predict_batch` | `op`, `checkpoint`, `contexts`, `include_logits` | Ordered indexed token predictions; logits only when explicitly requested |
| `compare` | `op`, `baseline`, `candidate`, `examples` | Paired loss/correctness/accuracy and baseline-correct regressions |

Unknown or missing fields are errors. `include_logits` must be a Boolean; it has
no implicit truthy conversion. A prediction request contains 1..32 contexts, each
1..4 integer tokens in 0..7. Duplicate prediction contexts are allowed, and their
order and multiplicity are preserved. All inputs are validated before inference;
malformed later rows never produce a partial successful batch. The batch loads
one validated private model and reuses it for every context.

Comparison uses 1..32 exact `{x,y}` examples, with the same token limits and labels
in 0..7. Duplicate numerical contexts, including signed zero, are rejected. Both
checkpoints must satisfy the existing seven-field fixed-point rank-1 checkpoint
contract and fixed-base binding. The comparison evaluates their exported weights
against the same frozen features. Identical-checkpoint comparison is allowed and
must yield identical metrics and zero regressions.

Comparison is diagnostic: examples can overlap training or previous evaluations,
labels are caller-supplied, and no history was supplied. The result therefore says
`independent_test: false`, `history_overlap_checked: false` and
`artifact_authenticated: false`. No `passed`/`accepted` decision is returned.
Use [reviewed continuation](pbAI_Student_Review.md) for the separate, explicit
retention gate, local acceptance and rollback.

## Reviewed inference reuse

Previously, every `review.predict(ids)` validated/reloaded its active checkpoint
and recreated the fixed model. The reviewed handle now uses the private frozen
model and dequantized parent/candidate adapters already created for its review.
`accept()` switches the adapter together with the active checkpoint/history;
`rollback()` restores all three. Rejected and repeated state transitions leave
inference unchanged. The prediction shape, digest and logits are unchanged.

This is bounded per-handle reuse, not a global model cache. Public checkpoint,
history, report, status and prediction values do not expose the private adapter.
Inference still validates every context. Handles retain the existing single-threaded
contract; this does not provide concurrent mutation synchronization or model storage.

## Privacy and host boundaries

The protocol does not train, save, activate models, ingest chats, read paths, start
processes, invoke callbacks or use the network. Its schema does not accept any of
those authorities. Prediction outputs omit input contexts; compact rows contain
only `index` and `token`, and verbose rows add eight logits. Comparisons return
aggregate metrics and checkpoint identifiers, not labeled examples or weight arrays.
Predictions, logits and digests can still reveal model information; they are not
anonymization. A remote adapter must separately enforce authentication, request-byte,
rate, concurrency, timeout and output policies before exposing this local primitive.
No external transport, encryption or authentication is implied by this release.

## Reproduction

- `tests/semantics/pbai_student_tools_test.sapp`: scalar/loaded-checkpoint oracles,
  batch ordering/bounds, malformed and cyclic input, private-output mutation,
  paired comparison, truthful capability errors, and every reviewed state.
- `bench/pbai_review_inference.sapp`: unchanged before/after workload of 32 calls
  in each of reviewed, accepted and rolled-back states, one warmup and five samples
  per state. Setup is excluded; each sample includes identical JSON serialization.
  Output digests must match before/after and across both products. Five-sample p95
  is the maximum sampled wall time; the desktop load is uncontrolled.
- `examples/pbai_student_tools.sapp` in PBLang, or `demo/pbai_student_tools.sapp` in
  standalone pbAI: synthetic capability, compact prediction and self-comparison.

The contract remains a fixed tiny token specialist. These checks and measurements
are not representative language, AGI/ASI, full-privacy or all-platform evaluations.

## Local measurements (2026-10-03)

The unchanged benchmark produced exactly matching checkpoint and prediction
output digests before/after and between products in all three states.
Median times for 32 predictions were:

| Product/state | Before ms | After ms | Observed reduction |
| --- | ---: | ---: | ---: |
| Embedded/reviewed | 81.09 | 21.05 | 74.0% |
| Embedded/accepted | 101.53 | 19.83 | 80.5% |
| Embedded/rolled-back | 159.38 | 20.07 | 87.4% |
| Standalone/reviewed | 76.13 | 19.36 | 74.6% |
| Standalone/accepted | 78.44 | 20.13 | 74.3% |
| Standalone/rolled-back | 75.12 | 19.77 | 73.7% |

These are observations on Linux x86_64 with the retained PBLang 0.6.0 runtime;
the variable shared-desktop load is visible in the raw samples. They do not
establish an all-workload or all-device speedup, RAM savings, or a quality gain.
See [raw before/after measurements](benchmarks/pbai_student_tools_performance-2026-10-03.json)
and [verification](benchmarks/pbai_student_tools_checks-2026-10-03.json).
