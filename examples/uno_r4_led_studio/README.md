# UNO R4 LED Studio

An offline **preview and frame editor** for the Arduino UNO R4 WiFi's 12×8 red
LED matrix, written in PBLang. It never discovers, connects to, flashes or writes
to a board. Existing AVR UNO/ESP32 bridges are intentionally not reused as a
hardware adapter for the different UNO R4 board.

```sh
pblang run examples/uno_r4_led_studio/main.sapp --allow-all -- --gui
pblang run examples/uno_r4_led_studio/main.sapp --allow-all -- --web
pblang run examples/uno_r4_led_studio/main.sapp --allow-all -- --export
```

The Web command prints a loopback URL (`http://127.0.0.1:8768/`) and runs in the
foreground; stop it with Ctrl+C. The CLI export prints a validated initial project
to stdout. No background service or board connection is created.

## Edit and preview

- Click an LED to toggle it, or focus a native button with Tab and press Enter.
  Web pixels have coordinate/state accessibility labels.
- Choose **Heart**, **Border**, **Checker**, **Clear**, or **Invert**.
- **Duplicate** copies the selected frame; **Delete** keeps at least one frame.
  **Previous/Next** select timeline frames without reordering them.
- Enter a duration from **50 to 5000 ms** and apply **Set ms**.
- Enter **1–8 supported characters** and choose **Text / Create scrolling text**.
  This replaces the current timeline with editable scrolling frames at 100 ms each.
- Native **Play once** starts at frame 1 and stops after the timeline, at most
  60 seconds. **Stop** retains the current frame. Editing stops playback.
  The script-free Web edition uses Previous/Next; it does not offer timed playback.
- Native **Export new JSON** creates a new timestamped file beneath
  `build/uno-r4-led-studio/`, using the existing exclusive-write helper. Existing
  files are never replaced. A failed export is explicitly reported and may leave
  an incomplete new file; the application does not claim success on failure.
- Web **Download validated JSON** downloads the current project and its Arduino
  frame words without changing the model or invalidating the current form.

The editor starts with a heart. State is memory-only until explicitly exported.
Closing/restarting discards unexported work. GUI import/resume is not implemented;
programmatic callers can validate the exported `document` using the library.

## Glyph coverage

Reuses `stdlib:bitmap_font.sapp`'s 5×7 glyphs: Latin A–Z/a–z, digits, space,
its punctuation/math symbols, and the explicit Greek subset
**π Π τ φ Φ α β γ δ ε λ μ σ ω Δ Ω**. Examples: `PB`, `12`, `πΩ`.
Some existing symbol/emoji glyphs also work (`←↑→↓`, `❤`, `😀`, `🙂`, `🚀`).
This is not full Greek, Unicode shaping, accents or an emoji font. Unknown glyphs
are rejected before replacing the timeline; they are not silently substituted.
Different lowercase glyphs remain lowercase. No font downloads occur.

## Reuse and boundaries

`stdlib:uno_r4_led_matrix.sapp` owns pixels, packing, validation, timing and glyph
conversion. `model.sapp` owns editor actions; `view.sapp` uses existing GUI/graphics;
`web.sapp` uses existing UI documents, guarded loopback forms and HTML escaping.
There is no handwritten JavaScript runtime and no new Rust. Web forms retain
same-origin, Host, CSRF, stale-form and script-free CSP checks. Each redraw rotates
the form token; stale forms must be refreshed. An overall 120-request/minute budget
survives redraws when the existing crawler policy is on. Policy options are
independently configurable through `Web.create(port, options)`; opt-outs do not
disable origin/CSRF checks. This is a local preview, not multi-user authentication.

Native execution has an X11 acceptance fixture; supported platform adapters are
not equivalent to tested Windows/macOS/mobile execution. Web uses a loopback
PBLang server, not a standalone exported app. Android/iOS adapters and physical
hardware execution are not provided by this example. The display has only binary
red pixels: no RGB, grayscale, physical 3D display, image decoding, arbitrary nD
projection or hardware driver is claimed.

See [the data contract](../../docs/UNO_R4_LED_MATRIX.md) for mapping, limits and
programmatic use. Tests:

```sh
pblang run tests/semantics/uno_r4_led_matrix_test.sapp --allow-all
pblang run tests/semantics/uno_r4_led_studio_test.sapp --allow-all
pblang run tests/semantics/uno_r4_led_studio_web_test.sapp --allow-all
xvfb-run -a -s '-screen 0 1000x1000x24 -nolisten tcp' env XMODIFIERS=@im=none python3 tests/tooling/uno_r4_led_studio_native.py
```
