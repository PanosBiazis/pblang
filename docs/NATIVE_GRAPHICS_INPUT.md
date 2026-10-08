# Native graphics input delivery

`graphics.run(window, state, update, draw, fps)` and `gui.run` keep their existing
signatures. No application migration or Rust source change is required.

## Backend contract

`graphics.Window` now describes its polling contract with `event_poll_mode`:

- `present-snapshot`: the known minifb backend retains input state refreshed by
  `present()`. The loop reads the initial snapshot and then reads each newly
  presented snapshot once. Repeated polling cannot replay the same key transition.
- `drain`, absent or unknown: keep polling every iteration, preserving the existing
  behavior for queue backends and unmarked/custom adapters.

A custom presentation-backed adapter can opt into `present-snapshot` only if its
`present()` actually refreshes the snapshot. Raw `Window.pollEvents()` is unchanged;
applications that bypass `graphics.run` still own event pumping and consumption.

By default, a minifb loop presents once per requested frame even when `update`
returns false. Drawing is still skipped for unchanged content. This is an input
responsiveness fix: more idle presentations can mean more framebuffer copies and
power use. It is not a general performance improvement. `fps` remains the sampling
budget; a slow application callback or very short press can still lose transitions.

An explicit `PBLANG_GRAPHICS_IDLE_PUMP_MS` retains the existing configurable idle
pump tradeoff (effective range 16–1000 ms). A longer interval can miss brief presses.
Set it in the launcher environment before starting PBLang; do not call `env_set`.
Resize settling, frame/time limits and normal window resource cleanup still apply.
No text composition/IME or universal keyboard-layout support is added here.

## Reproduce the regression tests

```sh
pblang run tests/semantics/graphics_event_delivery_test.sapp --allow-all
PBLANG_GRAPHICS_IDLE_PUMP_MS=1000 pblang run tests/semantics/graphics_event_delivery_test.sapp --allow-all
```

Run with other graphics timing overrides unset. The first mode checks default
input pumping while drawing remains idle. The second checks one-time delivery
across multiple idle polls with no new presentation. Both cover unmarked/queue
adapters, resize settling, and normal stop/close.

For a real isolated Linux/X11 window:

```sh
xvfb-run -a -s '-screen 0 1000x700x24 -nolisten tcp' env XMODIFIERS=@im=none python3 tests/tooling/native_keyboard_delivery.py
```

Requires an existing `target/release/pblang`, Xvfb and xdotool. The driver refuses a
non-Xvfb display. It starts a fresh initially empty form for each 80/120/180 ms
keydown/up duration, uses Tab/Space/text/Enter, and verifies the exact text,
checkbox once, disabled checkbox zero and button once. It closes after 11 seconds.
The result is printed as JSON; no real product data or personal desktop is used.

## Observed scope (2026-10-03)

With the same retained Linux runtime, the previous loop typed no text in the
180 ms fixture and replayed some key releases 3–9 times. The corrected loop typed
`native.pbsheet` exactly at all three tested timings. Each run drew 20 times while
presenting about 330 times over 11 seconds at 30 fps. A separate PBPortions task
also reported exact typing into three empty fields followed by comparison, save,
reader export and native close using the candidate graphics file.

Only Linux/minifb has live native evidence here. SDL2/custom queue behavior has
contract tests; Windows/macOS/iOS/Android native behavior and arbitrarily fast
keystrokes still need platform-specific validation.

## Remaining native boundary (2026-10-08)

`tests/tooling/native_keyboard_boundary.py` records events **before**
`GUI.handle_event`, each resulting edit, and a synthetic file save/readback.
Run with an explicit existing runtime and a fresh evidence directory inside
`xvfb-run -a`; it refuses a personal display. No Rust changes are involved.

In one bounded Linux/minifb run at 30 fps, 120 ms `abc` presses arrived and saved
as `abc`. Six 1 ms `abcdef` presses produced only an `F` key release, so the GUI
remained empty. Overlapping `b`, `a` presses arrived as `A`, `B` and produced
`ab`. Greek `αβ` keysyms held 180 ms produced no keydown/keyup records. All four
save/readbacks exactly matched their GUI values, including the incorrect/empty
ones: successful storage does not establish correct keyboard delivery.

The retained path `gfxpresent` updates minifb state; `gfxpollevents` enumerates
`get_keys_pressed` followed by `get_keys_released`. It exposes physical key names,
not ordered Unicode text/composition events. `keymap.key_to_char` accepts a direct
Unicode scalar when one is supplied, but cannot recover omitted input or original
ordering. PBLang polling changes cannot reconstruct this missing information.
A supported ordered text-input backend is still needed; no faster sampling,
layout/focus correction, Reader zoom shortcut, or save/readback is claimed to fix
the native loss. These are diagnostic samples, not timing guarantees or benchmarks.
