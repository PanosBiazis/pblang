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

## Optional ordered X11 input

```pblang
let G = import("stdlib:graphics.sapp");
let window = G.Window(800, 600, "My application", map{ordered_input: true});
print window.input_backend; // x11-ordered-keysyms or legacy
```

The fourth argument is optional. Existing three-argument callers keep their
current backend and permissions. This adapter uses PBLang and the installed
X11/XCB/xkbcommon system libraries; it does not replace rasterization, present,
mouse, resize or close handling. Keyboard snapshots are filtered only after the
owned-window queue is attached successfully, preventing duplicate text.

The initial implementation admits Linux x86_64, a local `:N` X11 display, minifb,
ASCII window titles up to 128 characters and the existing FFI/filesystem grants.
Missing permissions/libraries and unsupported configurations retain `legacy`.
Applications should inspect `input_backend` instead of assuming successful
attachment. No new grants are automatically enabled.

The adapter subscribes briefly to root **creation metadata**, creates the native
window with a random identity, and matches its bounded WM_NAME property through
checked XCB requests. It never subscribes to root keyboard events. The original
ASCII title is restored before normal use. Equal public titles cannot select
another application by name. Each poll processes at most 64 X events; remaining
events stay queued for subsequent polls. Pressed identities are bounded to X11
keycodes 8–255, and focus loss releases tracked keys. No keystroke log, persistent
file, injected key, subprocess, provider or network service is added. A private
CLOEXEC memfd bridges fixed-size native structures to PBLang arrays and is closed
with the observer connection and allocated buffers.

`keydown` may now include `text`, a committed string independent of the physical
`key` name. `gui.sapp` consumes explicit text exactly once (including repeated key
events), preserves case and selection, and accepts a whole commit only if it fits
the widget character limit. Explicit empty/invalid text suppresses fallback key
translation; events without `text` retain the old behavior. Commits are bounded
to 256 Unicode scalars and reject control characters. Enter remains a physical
textarea action. This is a text delivery contract, not an IME implementation.

Support limits remain: keysyms at XKB levels 0/1 with Shift/Caps Lock and group
selection, plus NumLock keypad text and retained keypad shortcut names; no dead-key composition, preedit/IME, AltGr levels, clipboard/paste,
Wayland-native path, macOS/Windows/iOS/Android native proof, or total ordering
between pointer snapshots and queued keys. Ephemeral synthetic keymap rewrites
are not a substitute for a stable installed keyboard layout. The adapter has no
promise to recover input after a process/server failure or an arbitrarily long
blocked application callback. Prior RSS regressions remain open.

`tests/tooling/native_ordered_input.py` compares actual typed text against expected
Latin/Greek sequences, overlapping press order, a zero-delay burst and Ctrl+A,
then checks exact synthetic save/readback. Use `--mode legacy` for an unchanged
backend control, and choose bounded batches with repeated `--case` arguments.
The driver requires an isolated Xvfb display and explicit existing runtime; its
ordered mode fails if the adapter falls back or any expected text is missing.

The 64-event processing limit does **not** bound Xlib's total pending queue memory.
XPending, XSync and XCB reply waits have no intrinsic deadline in this adapter;
an unresponsive display can still block. This is not an untrusted-display sandbox.
During binding, bounded names of newly created windows may be inspected and
immediately discarded; none are logged. XKeysymToString retains the existing
trusted native C-string boundary. Cleanup is attempted once; a denied or failed
native cleanup remains uncertain rather than establishing universal reclamation.
The verified descriptor counts apply only to the measured successful lifecycle.

### Qualified source cases

On the retained Linux runtime, the optional queue delivered exact Latin text at
1/120 ms press durations, overlapping `ba` in order, Greek `αβγ` at 1/20/120 ms,
a 36-character zero-delay burst, Ctrl+A replacement, and NumLock keypad `120.`.
Every case saved and read back the expected value. The legacy control on the
same runtime still returned empty rapid/Greek text and reordered `ba` as `ab`;
its 120 ms Latin case remained correct. These are measured test cases, not
universal timing guarantees. AltGr text is suppressed until higher-level layout
translation is implemented, rather than inserting the base-level character.

Two windows with the same public title retained separate `abcdef` / `αβγ`
values. Fifty already-destroyed test windows produced 100 pending structure
notifications before binding; the checked lookup still attached the owned
window, typed `abcdef`, and saved/read it exactly. The fallback test with
`--deny-ffi` created, polled, presented and closed a legacy window successfully.
A 26-window lifecycle test (25 ordered, one unsupported-title fallback) returned
from 3 to 3 descriptors with no input memfd remaining. No total-memory or
all-failure-path cleanup guarantee follows from those counts.

A runnable developer example is `examples/native_ordered_input.sapp`. It opts
into the queue and the existing `form.text_render="auto"` smooth text renderer,
without changing default GUI rendering or persisting entered text.
