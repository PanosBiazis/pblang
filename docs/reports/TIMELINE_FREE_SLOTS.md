# Explicit free-window search

The relative-timeline portfolio already supports internal gaps, overlaps and clock
agendas. The new PBSlotFinder workflow adds explicit search-window boundaries and a
minimum duration. PBTimeline exposes the same CLI and a separate native workspace.

`interval_analysis.free_windows(intervals,start_ms,end_ms,minimum_ms=1)` validates
all original intervals and bounded integer parameters, clips intersections, and
reuses analyze for their union and internal gaps. It adds only boundary gaps and
filters durations inclusively. This preserves existing analyze semantics and avoids
a second sorting/sweep implementation. No measured speedup is claimed.

Returned format pblang.free-windows.v1 includes start_ms, end_ms, minimum_ms,
occupied_ms, free_ms, matching_ms and sorted detached slots with start_ms, end_ms,
duration_ms. Free+occupied equals window length; matching<=free. Empty occupancy
frees the entire window; no zero-duration slots. Out-of-window malformed entries
still reject. Up to512 busy intervals produce at most513 free slots. Time maximum
remains359999999ms. A positive minimum larger than the window yields no matches.

`timeline_slots.find` validates a full timeline then projects timings only.
`to_timeline` recomputes and exports generic-labeled independent copies, all matches
or one explicit zero-based index. Source title/labels never enter reports/exports.
Timings are still sensitive. The source64-activity cap can yield65 gaps: report all,
reject export-all, allow any single selection. Zero matches cannot become a valid
nonempty timeline and export rejects. No new persisted format or truncation.

`timeline_slots_view` supplies the native reader and a portable selected-slot view.
Invalid operations retain prior state; pending bounds block stale exports. Inputs
use existing timeline_files with bounded no-follow reads and exclusive-new writes.
No network, telemetry, scheduling, booking, hidden storage or automatic cleanup.
Save failures may leave partial new copies. Show/Hide is visual only.

Tests use independent discrete occupancy oracles across40 varied interval sets,
clipped/touching/nested/empty/max cases,512-to513 and64-to65 boundaries, generic
labels, source immutability and native/file integration. Platform evidence remains
separate: Linux CLI/native, Web actions, Android generation; installed Windows,
macOS/Android and signed releases are unverified.
