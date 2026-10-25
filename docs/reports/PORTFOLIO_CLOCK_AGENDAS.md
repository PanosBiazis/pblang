# PBTimeline clock agendas and reusable delivery material

Direction: the existing PORTFOLIO_TIMELINES.md describes relative agendas derived
from presentations, workshops and subtitles. PORTFOLIO_BRIEFS.md supplies the
standard Writer handoff route. This slice connects those existing ideas; no new
whole-disk scan, third-party code or real client document was ingested.

The original PBTimeline planner, timeline_document, timeline_adapters, timeline_view
and optimized interval_analysis come from concurrent workspace development. They
are included as supporting code, not claimed as newly authored in this slice.

New timeline_agenda.clock_ms validates an exact five-byte ASCII HH:MM origin.
project returns fixed-day D+N HH:MM:SS.mmm endpoints, source indices/labels, exact
millisecond durations, per-entry overlap counts and pair/internal-gap totals. Input
order is preserved even when entries are not chronological. This is a projection,
not an automatic schedule repair. Days are exactly86400000ms; no calendar, timezone,
DST or wall-clock service is involved. Rollover is explicit up to the existing
maximum interval offset359999999ms. The existing O(n log n) analyzer is reused,
with one linear pass for output. This is no new benchmarked speed improvement.

New timeline_agenda.to_document emits a standard Writer paragraph per activity.
Assumptions and totals prefix the first paragraph, leaving capacity for all64
activities. Existing Writer bounds validate the complete output. Labels, source
order and overlaps remain untouched, and returned values are detached. Long Unicode
labels and the maximum entry count are tested without truncation.

The original product store now delegates to new shared timeline_files, retaining
open/save/import_document contracts and adding export_agenda through canonical
text_document_files.save_copy. This allows other products to reuse the conversion
and file policy without importing a product's internals. Failures may leave an
incomplete new file; atomic visibility and storage durability are not promised.

PBTimeline0.2 adds native and CLI Writer export. Pending document fields block
export until Apply/Reset. Exporting applied unsaved edits neither updates the saved
baseline nor clears undo; a generated delivery copy is not a saved editable source.
The explicit clock/output path are export options, not persisted timeline fields.
Read-only Web/Android views reuse document_excerpt_view. The new Web exporter uses
four activities/page, up to16 pages, and overwrites only generated build artifacts.
Show/Hide never removes embedded content. Cross-platform generation is distinct from
physical execution, installable APKs and full native editing parity.

Original examples cover a business workshop with an internal break, a developer
session with overlapping tasks and midnight rollover, and a short learning session.
The editable timelines, Writer documents, plain text and six readers form an
Agenda Kit for the three requested audiences. These are synthetic editable examples,
not certified operational/educational guidance or evidence of demand/revenue.
