# Retained join mappings and Writer reviews

Existing TABLE_JOIN_WORKFLOWS.md / DISCOVERY_RECIPES.md identify literal catalog,
order and course enrichment. The previous join displayed match flags only in its
in-memory report; ordinary CSV/Sheets exports cannot distinguish a matched blank
from a missing reference. This slice adds an explicit Writer review, without new
disk scans, third-party code ingestion or real customer data.

The existing indexed join is retained. Its result gains reference_rows aligned
with output rows (nil for missing), and unmatched_source_rows covering all missing
left rows, including inner-mode omissions. Original fields, literal matching,
ordering, validation and value-only exports stay compatible. New metadata uses
linear bounded output storage; no benchmarked speed improvement is asserted.

New table_lookup_review validates mapping structure, count/coverage consistency,
strictly increasing source/missing positions, Boolean flags, and reference bounds.
It accepts reports from the updated lookup, not older reports lacking metadata.
It does not establish source authenticity or verify positions against absent source
files. to_document emits one summary plus one paragraph per left data row, including
inner omissions. Maximum32 Writer blocks cover all31 supported data rows. No source
cell/header strings or filenames are copied by the default title/content path.
Counts/positions can remain sensitive. User-supplied titles are copied literally,
with a 256 UTF-8 byte limit and a nonblank requirement; oversized titles reject
before trimming. No anonymization claim is made.

The common file adapter adds save_review and review (open/join/save). PBJoin0.2
adds native and CLI review export. PBDataLab adoption is not part of this delivery. Native saving
uses the retained join snapshot and rejects changed inputs/settings until a new
successful join. Disk changes do not silently recompute that snapshot. Output is
an ordinary editable .pbtext for Writer/PBBrief, not a signed audit trail, evidence
bundle or live link. New-copy write failures may leave incomplete files.

The shared Writer reader supplies four blocks/page, native/Web/Android generation
and Show/Hide display actions. Complete reviewed mappings remain embedded. The new
Web tool exports selected pages1–8 into generated build paths. Physical Android,
Windows/macOS and browser/mobile editing parity remain unverified.

Three original Greek examples cover a matched blank and missing catalog entry,
case-sensitive developer identifiers with inner omission, and repeated learning
module keys. The review kit is reusable handoff material for the existing three
commercial directions. It does not demonstrate demand, revenue or support savings.

Validation includes malformed mappings, invalid flags/counts/order/coverage,
header-only and zero-output cases, max31 source rows, original lookup regressions,
new-copy collisions, stale settings and snapshot retention in PBJoin. The existing PBJoin/source modules are concurrent project work; only the
mapping extension/review integration and new examples originate in this slice.
