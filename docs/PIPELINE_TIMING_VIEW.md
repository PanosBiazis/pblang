# Pipeline preparation and timing reader

`devops_pipeline` retains the same limits, validation, snapshot semantics,
Kahn ordering and public exports. Its private preparation used the generic
Workflow builder, whose `add_node` searches every prior node and whose
`connect` searches nodes for both endpoints. The DevOps validator already
checks every identifier, duplicate, dependency and endpoint before constructing
the edges. It now appends validated private node/edge records directly, retaining
exact insertion order. Canonical `Workflow.topo_order` still detects cycles.
No generic Workflow behavior, callback opt-in or target-closure rule changed.

This removes O(V^2 + E*V) builder lookup work from bounded pipeline preparation;
identifier validation, snapshotting and topological traversal remain. The
end-to-end runtime cost includes those remaining operations. No asymptotic
claim is made about callbacks or imported Workflow implementations.

`pipeline_timing_view.create(source, page=0)` calls the existing strict timing
inspector once and returns a renderer-neutral read-only timing view. Each saved
page contains at most eight selectable jobs. Job numbering follows canonical
topological order, IDs/source are omitted, and every selection displays five
precomputed detail lines using portable declarative actions. All 1–32 jobs are
accessible across 1–4 explicit pages, including zero-duration jobs. Page indices
must be integers in range. Invalid source/page throws a fixed error.

Native PBIDE provides a separate inspector with a JSON input and explicit
Analyze action. It retains the prior accepted report after an invalid edit and
uses Previous/Next to reach every job. Close discards the transient input; this
is an inspector with no save contract. Web/Android readers have selection actions
only and do not parse newly entered JSON. No job is executed or authorized.

## Regression and performance evidence

`devops_graph_build_test.sapp` compares exact old/new ordering and selected
prerequisite closures for every forward-edge DAG on four vertices under three
input orders (192 cases), maximum bounded density and malformed graphs.
Existing callback/target tests retain their assertions. The exact previous
implementation is kept in `bench/fixtures/devops_pipeline_before_graph_build.sapp`.

Run `pblang run bench/devops_graph_build.sapp --allow-all` to compare independent,
chain and dense 32-job workloads. It verifies equivalent ordering first, uses
three warmup batches, nine alternating samples and twelve calls per sample,
and emits wall-clock milliseconds per call. Use the median and nearest-rank
p95; with nine samples p95 is the maximum. Concurrent machine load can affect
timings. This is bounded graph preparation, not a claim of faster job execution.

`pipeline_timing_view_test.sapp` verifies masking, selection actions, maximum
pagination, zero durations, invalid inputs and renderer generation.
PBIDE `pipeline_viewer_test.sapp` checks button routing, boundary navigation,
critical-path changes, invalid-input retention and native control bounds.
