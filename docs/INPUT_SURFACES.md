# Touch, pen and interactive-board input data

`stdlib:input_surface.sapp` is a PBLang-only, device-independent computation
library. It does not enumerate hardware, open a device, inject input, change
calibration or grant input authority. The host still uses its existing runtime.

`transform(surface, points)` accepts width/height1..1048576, optional rotation
0/90/180/270 and boolean mirror_x/mirror_y. Up to256 points must be inside the
declared extent. Reflection precedes clockwise rotation. Returned coordinates
are normalized0..1, inclusive; a pixel-index adapter must multiply by
`pixel_count-1`, not by `pixel_count`.

`analyze(events)` accepts up to256 ordered events with contact_id0..31,
kind touch/pen/mouse, phase start/move/end/cancel, time_ms0..86400000 and
normalized x/y. A contact must start before it moves/ends/cancels. IDs may be
reused after release. Duplicate starts, kind changes, non-monotonic time,
non-finite/out-of-range numbers and unknown fields are rejected. Pressure0..1
and tilt_x/tilt_y-90..90 are optional pen-only reported measurements. Missing
measurements are not inferred. Cancel is per contact. Open contacts produce
`complete:false`, allowing partial live traces.

The summary contains counts/timing and pressure/tilt aggregates, never source
coordinates or contact IDs. These derived statistics can still be sensitive;
they are not anonymous biometric measurements. Nothing is persisted.

Run `examples/input_surface.sapp` for synthetic examples. The bounded
`examples/input_surface_stream.sapp` reads JSONL frames of up to32 events,
prints a summary after each frame, and stops at EOF or32 frames. Total events
are limited to256. Stream frame size is checked after read_line allocation;
apply OS memory/CPU limits to untrusted producers. No device adapter or hard
real-time scheduling guarantee is supplied by this example.

Adapters must establish hardware/OS capabilities independently. A passive
capacitive tip may arrive as touch; absent pressure/tilt/buttons must remain
absent. Do not identify a physical stylus from generic pointer events. A
Baseus Smooth Writing2 product name alone is insufficient: variants differ,
and the active screen protocol must match the actual screen. There is no
universal hardware compatibility promise, LED/battery control, palm rejection,
handwriting recognition or native driver supplied by this module.

Validation: semantic positive/negative tests and synthetic streamed input.
Physical pens, touchscreens, boards and other OS backends require separate
acceptance evidence. See `tests/semantics/input_surface_test.sapp`.

The user-confirmed device is **Baseus BS-PS011, Active/Passive version**. Model
label alone is not runtime capability discovery. Represent passive contacts as
ordinary touch only when delivered by the platform; use pen events only when
reported by a compatible active screen/driver. Test both modes independently.
