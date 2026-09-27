# Palette ordering and strip construction

`palette_document.move_swatch(doc, source, target)` uses zero-based integer
indices into the current 1–8 swatches. It returns a fully detached document,
moves the selected swatch to the target position, and retains the relative order
of all other swatches. Title, background, label/HEX pairs and document format do
not change. Invalid input or either index rejects without mutating the source.
Moving to the same index returns an equal, detached document.

PBPalette adds Move left / Move right. The selected color follows its new position;
Undo restores both prior document and selection. Pending field edits block moves.
Boundary moves are no-ops and do not consume history. Applied moves participate
in the existing eight-entry history, unsaved-close/open guards and new-copy save
workflow. Identical document content does not add an undo entry.

CLI: `move <input.pbpalette> <from 1-8> <to 1-8> <new.pbpalette>` uses one-based
indices and a new output path. The actual document length bounds both positions.
Read the returned JSON `ok` field, not only the process exit status. Existing
analysis, matrix, image, table and portable reader operations use stored order.
Web/Android readers display that order but do not reorder or persist edits.

`strip` previously parsed each swatch once per image row (sixteen times).
It now parses each swatch once after document validation, then reuses those RGB
values while allocating every pixel and image row independently. Output remains
16 pixels high and 16 pixels per swatch, maximum 128 by 16. No shared mutable
pixel arrays, persistent cache, new pixel format or color-formula change is used.
The final validation still rejects malformed/canonical input as before.

`palette_move_test.sapp` checks all 204 source/target combinations for sizes 1–8,
inverse moves, source preservation, detached outputs, serialization, invalid
indices and consistency across analysis, matrix, PNG source pixels, tables and
portable readers. Product tests cover actual callbacks, draft/dirty guards,
boundary moves, selected-index restoration and bounded history. Existing store
tests decode exported PNG and compare exact pixels and refusal to overwrite.

`bench/palette_strip.sapp` compares the exact pre-change module retained under
`bench/fixtures/palette_document_before_strip.sapp`. It checks identical pixels
and measures 1/4/8 swatches with three warmups, nine alternating-order samples,
and five calls per sample. Output is wall-clock milliseconds per strip call;
median and nearest-rank p95 (maximum of nine samples) describe that run only.
PNG encoding, disk I/O and complete editor refresh are outside this benchmark.
