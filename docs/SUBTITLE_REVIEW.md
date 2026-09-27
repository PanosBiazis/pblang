# Subtitle review composition

`stdlib:subtitle_review.sapp` combines the existing `subtitles`,
`subtitle_reading`, `subtitle_timing` and `subtitle_clip` libraries.
It introduces no file, process, network, media-decoder or model calls.

```sapp
let R=import("stdlib:subtitle_review.sapp");
let report=R.analyze(document,20);
let excerpt=R.clip_review(document,1000,3000,20,true);
```

`analyze` returns a `pblang.subtitle-review.v1` record with cue rows, the
custom reading limit, counts and the existing interval diagnostics. Each row
contains zero-based cue index, times, duration, nonspace Unicode code-point
count, code points per second, overlap count and ordered reasons:
`reading_rate`, then `overlap`. `attention_count` counts the union, so a cue
with both reasons counts once. Overlap can be intentional; a reason is a
review signal, not a declaration that the subtitle is wrong.

`clip_review` intersects the document with a half-open `[start_ms,end_ms)`
window using the existing clip function, recomputes the report and returns
`source_indices` mapping each output cue to the original zero-based index.
`zero=true` rebases timestamps; false preserves the original time axis.
Changing the axis does not change reading rates. Shortening the visible
duration while preserving cue text can increase reading rate. An empty
intersection throws; it is not reported as a successful empty review.

Existing document limits apply: 1–128 ordered cues, positive integer-ms
durations, timestamps through 359999999 ms, at most four lines and 2048
UTF-8 bytes per cue, and 65536 total cue-text bytes. The entire document is
validated, including cues outside a requested clip. Reading limits are
caller-selected finite numbers from 0 through 1000000; equality is allowed.
The default 20 is a configurable heuristic, not a researched readability
standard. No human readability or accessibility certification is implied.

Counts are Unicode code points, not grapheme clusters or linguistic tokens.
Combining marks count separately; ASCII whitespace is excluded from the rate.
Reports omit cue text but retain timing and metrics, which may themselves be
sensitive. Results are newly constructed and do not mutate the source.

Run `tests/semantics/subtitle_review_test.sapp` and
`examples/discovery_subtitle_recipes.sapp` for synthetic coverage of reading
load, metadata cue sheets, clipping and literal Unicode metrics. A metadata
cue sheet is not support for CD CUE parsing, playback or automatic timing edits.
