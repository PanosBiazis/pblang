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
