# Optional category-count margins

The existing TABLE_GROUPING_AND_TALLY / TABLE_AXIS_SWAP documentation describes
literal grouping and same-axis reuse. This extension adds optional record-count
margins, reusing that implementation. No new general disk scan or private customer
source was ingested. Existing grouping/index optimizations are not new work here.

New table_group_totals.counts and cross return standard detached Sheet documents.
They first derive trusted counts with table_group, then append a grand-total row
and, for cross-tab, row and column margins. No caller cell is evaluated as a formula
or coerced into a measurement. All input categories retain exact order/text. The
last row/column is computed; labels choose Total or the first unused Total [N]
independently on each axis to avoid collision with literal category labels.

The source remains bounded32×8 and8192 cell-text bytes. Totals reserve output
capacity: at most30 count categories or30×6 cross categories. Added labels/counts
are included in complete output validation. In particular, a valid base cross-tab
near the text budget can reject after totals. No truncation or partial in-memory
result is exposed. Header-only data produces explicit zero totals.

PBTally0.2 exposes both modes in CLI/native UI, preserving previous modes and swap
behavior. Changed settings block saving until calculation succeeds. Failed totals
or swap retain previous results/settings. The summary records the computed mode
rather than reflecting pending mode edits. Simple statement control flow is used
for that summary to remain compatible with the retained bootstrap runtime.

Privacy/data flow: source tables are read locally, category labels are retained
in output and selected-row Web/Android snapshots, and explicit new copies use the
existing bounded/exclusive table adapters. No authentication, network service,
telemetry, model provider, credentials or new external dependency is introduced.
Outputs are user-retained documents; there is no automatic expiry/deletion. CSV
labels may be interpreted differently by third-party spreadsheet importers. This
is not a whole-runtime filesystem/backend audit or a full vulnerability scan.

Validation covers independent count-conservation oracles, literal blanks/Greek
text/formula-looking categories, Total label collisions, empty data, same axes,
maximum shape, one-over bounds, byte overflow, source detachment, native pending
state, failed/successful swaps, exclusive file copies, CLI and generated views.
Physical Windows/macOS/Android execution is not established by these checks.
