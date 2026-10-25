# Relative timelines and subtitle diagnostics — 2026-09-27

This slice extends PBMedia's timing workflow and adds the independently versioned PBTimeline 0.1.0-alpha. Product direction comes from the timeline-view gap in `PORTFOLIO_MEDIA_CIRCUITS.md`, the presentation/lesson workflow in `PORTFOLIO_LESSONS.md` and the separate model/presentation idea in `IDEA_INTAKE_CATALOG_2026-09-17.md`. No fresh disk-content scan, external source copying, private corpus import or automatic learning occurred. PBTimeline is a newly authored composition of these existing ideas.

## Reusable PBLang libraries

| Module | Responsibility | Bounds |
|---|---|---|
| `interval_analysis.sapp` | Endpoint sorting and binary-search overlap counts, coverage/gaps/peak via two-pointer sweep | 0–512 exact `{start_ms,end_ms}` intervals; integer 0..359999999; strictly positive durations |
| `subtitle_timing.sapp` | Adapts existing subtitle cues; finds next overlapping cue with wraparound | Existing 128-cue subtitle contract; preserves legacy statistics |
| `timeline_document.sapp` | Detached edit/append/remove/rename, strict JSON persistence and analysis | 1–64 entries; 80 characters/256 UTF-8 bytes per label/title; 65536 serialized bytes |
| `timeline_adapters.sapp` | Explicit-duration slide agenda and timing-only SRT conversion | Rejects title/time overflow or >64 cues; no truncation |
| `timeline_view.sapp` | Portable activity selection with complete per-activity details | Eight activities per page; page must exist; full-document aggregates |

The analyzer's work is O(n log n) plus output, using O(n) storage, subject to host sorting/collection costs. It never materializes the potentially quadratic pair list. Per-entry counts use `count(start < end_i) - count(end <= start_i) - 1`. In the sweep, ends win equal timestamps; thus touching intervals do not overlap. Each segment contributes once to occupied/overlap time. Gaps exclude time outside the global span. The original caller order and values are unchanged. Results are data, not permission to schedule or execute anything.

## Product behavior

PBMedia adds a detailed timing CLI and native jump-to-overlap action, preserving drafts and the existing inspection contract. PBTimeline implements local title/activity/time editing, eight-step undo, explicit new-copy saving and reopen, CLI inspection/conversion, and portable page readers. Slide import consumes PBOffice/PBLesson native slide documents; subtitle import consumes PBMedia's supported SRT dialect. The native editor uses the existing file adapter without changing its write guarantees.

Pure interval and document logic has no platform-specific calls. A portable source does not establish execution on every OS. Linux runtime/native X11 and Web interaction evidence are distinct from generated Android class/manifest evidence. No physical Windows/macOS/Android validation, packaged mobile editor, native installer, calendar service, media playback or complete cross-platform editing parity is claimed.

## Verification and measurements

Independent quadratic-pair and discrete-time coverage oracles cover 40 deterministic varied interval sets; malformed data, duplicates, nesting, touching endpoints, input preservation and the 512-entry maximum are tested. Document tests include Unicode, strict reconstruction, rejection of duplicate/unknown fields, 64-entry roundtrip, conversion overflow and action compilation on the last portable page. Product tests cover applied/unapplied edits, undo, no-overwrite file roundtrips, conversion/CLI parity, and native event loops.

`bench/interval_analysis.sapp` records two warmups and seven samples for the same 32/128/512 interval overlap-count workload. The quadratic reference computes only counts; the indexed analyzer also computes coverage/gaps/peak. This compares algorithms, not old/new application releases. Median/p95 milliseconds on one shared Linux host:

| Intervals | Quadratic reference | Indexed full analysis |
|---|---:|---:|
| 32 | 0.924 / 1.546 | 0.603 / 0.888 |
| 128 | 13.380 / 14.645 | 1.974 / 2.357 |
| 512 | 379.449 / 449.708 | 18.810 / 21.962 |

Timing varies with runtime and concurrent host load. p95 is nearest-rank (maximum of seven). No overall product speedup, hardware-acceleration benefit or cross-platform timing guarantee is inferred.
