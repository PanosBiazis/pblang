# Caption excerpts — wave 13

The existing media/timeline roadmap establishes SRT editing, whole-document shifts and timing diagnostics. This slice adds temporal selection: PBMedia clip exports and new PBCaptionClip. It reuses the validated cue model and existing file adapter; no second subtitle parser, media decoder, external content copying or AI transcription.

`subtitle_clip.extract(doc,start_ms,end_ms,zero=true)` validates the complete source and strict integer bounds, selects half-open interval intersections, clamps timings to the window and optionally subtracts its start. Original text/order remain unchanged. The ordered cue scan can stop once starts reach the exclusive end, but full validation occurs first. Empty results throw because a valid subtitle document needs at least one cue. Only selected cues are copied. No measured speedup claim.

`subtitle_files` extracts PBMedia's existing bounded open/new-copy SRT/VTT routines unchanged. PBMedia's store facade preserves callers. Native cue-window export includes overlapping cues and leaves dirty baseline/history intact. PBCaptionClip applies range/mode transactionally and blocks stale-setting export. Its portable reader splits one cue into bounded 256-character pages, retaining text and supporting Show/Hide.

Tests use an independent overlap oracle across all small integer windows, plus 128-cue/65536-byte limits, max-time one-millisecond intervals, whole-source rejection before early stop, literal Unicode/VTT transport, no-output gaps, dirty/draft guards and source/existing-output preservation. Native Linux, browser actions and generated Android are separate evidence from physical Windows/macOS/mobile or signed packaging. No media-file editing or standards-compliance claim.
