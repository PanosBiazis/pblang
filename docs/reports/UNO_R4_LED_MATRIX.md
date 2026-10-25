# UNO R4 WiFi LED matrix contract

This additive PBLang module models and previews the **12 columns × 8 rows of
binary red LEDs** on the UNO R4 WiFi. It has no I/O or board authority.

## Coordinates and Arduino words

Coordinates use top-left `(0,0)`, x increasing right and y increasing down. A frame
is exactly 96 numeric 0/1 values, row-major at `pixels[y*12+x]`.
`pack(frame)` returns three unsigned 32-bit numeric words. Pixel 0 maps to bit 31
of word 0; pixel 31 to bit 0 of word 0; pixel 32 to bit 31 of word 1; pixel 95 to
bit 0 of word 2. `unpack(words)` reverses this mapping. These are numeric words,
not a host-endian binary file. No integer signed-shift behavior is assumed;
packing uses multiply/add and unpacking uses bounded arithmetic.

The mapping follows the official Arduino implementation's `renderBitmap`,
`loadPixelsToBuffer`, `loadFrame` and `next` behavior. Its internal byte/bit reversal
is the Arduino library's responsibility, not another reversal in our export:

- [Official UNO R4 WiFi LED matrix tutorial](https://docs.arduino.cc/tutorials/uno-r4-wifi/led-matrix/)
- [Official Arduino_LED_Matrix source](https://github.com/arduino/ArduinoCore-renesas/blob/main/libraries/Arduino_LED_Matrix/src/Arduino_LED_Matrix.h)
- [Official gallery fixture](https://github.com/arduino/ArduinoCore-renesas/blob/main/libraries/Arduino_LED_Matrix/src/gallery.h)
- [UNO R4 WiFi datasheet](https://docs.arduino.cc/resources/datasheets/ABX00087-datasheet.pdf)

Source inspected 2026-10-03. The test includes independent literal row strings
for `LEDMATRIX_HEART_BIG`: words `0x3184a444`, `0x44042081`, `0x100a0040`.
It also checks every one of the 96 single-bit positions and all-on unsigned words.
This is source-level mapping evidence, not a board execution result.

## Bounded model

```pblang
let LED=import("stdlib:uno_r4_led_matrix.sapp");
let pixels=LED.pixel(LED.blank(),11,7,1);
assert(LED.pack(pixels)==[0,0,1]);
let animation=LED.animation([
    map{pixels:pixels,duration_ms:250},
    map{pixels:LED.preset("heart"),duration_ms:500}
]);
let text=LED.encode(animation);
assert(LED.decode(text)==animation);
let arduino=LED.arduino_sequence(animation); // each row: word0, word1, word2, ms
let position=LED.sample(animation,250);     // second frame, not finished
```

- Format: `pblang.uno-r4-led-matrix.v1`; keys `format`, `width`, `height`, `frames`,
  `total_ms` only; width 12 and height 8.
- 1–64 frames, each exactly `{pixels, duration_ms}`; frame duration 50–5000 ms;
  total duration at most 60000 ms. Empty, unknown-field and malformed data fail.
- Pixels must be numeric 0 or 1; booleans, strings, fractions, NaN and infinities
  are not accepted. Words must be integers 0–4294967295.
- Text JSON import is at most 65536 UTF-8 bytes and 65536 characters. Successful
  validation copies all frame arrays; caller mutations cannot alter previous data.
- `sample` uses half-open frame intervals: an elapsed value exactly at a boundary
  selects the next frame. At/after the total it returns the last frame and
  `finished:true`; no implicit looping or unbounded catch-up loop.
- `text_animation(text, duration_ms=100)` accepts 1–8 existing bitmap glyphs and
  creates at most 61 frames. Unsupported glyphs or excessive total duration fail
  before a new editor timeline is committed. Glyph coverage is documented in the
  [example](../examples/uno_r4_led_studio/README.md).

The example's download format `pblang.uno-r4-led-export.v1` contains the canonical
`document`, a mapping label, and `arduino_frames` (three numeric words plus duration
per row). It is a validated data export, not a firmware image or transport protocol.
The Arduino adapter/board initialization and physical orientation check remain
future work. Import the canonical document via `LED.validate(exported.document)`;
`LED.decode` accepts canonical document JSON, not the outer download envelope.


## Studio export import and playback

The studio `examples/uno_r4_led_studio/exchange.sapp` accepts only the existing
export envelope, with exactly `format`, `mapping`, `document`, `arduino_frames`.
`format` is `pblang.uno-r4-led-export.v1`; `mapping` must be
`row-major; pixel y*12+x; MSB-first uint32 words`. Every four-number Arduino row
must equal the three packed pixel words and duration from its corresponding
canonical frame. Canonical document validation remains unchanged.

```pblang
let Model=import("../examples/uno_r4_led_studio/model.sapp");
let state=Model.create();
let saved=Model.export_document(state);
Model.apply(state,"import",saved);
```

Import is atomic: validate the complete envelope before replacing state. It resets
selection to frame 1, stops playback, and clears transient text/export flags.
Malformed import leaves those fields unchanged. Native uses `secure_file.read_text`
with 131072-byte maximum; Web uses explicit paste plus the existing guarded form.
Limits are 131072 UTF-8 bytes, 5 JSON nesting levels, 14000 parser items and 16384
lines, with duplicate keys rejected before the ordinary JSON parser. This larger
*envelope* limit does not change the library's 64 KiB canonical decode contract.
No extra fields or raw document-only import are accepted.

Static text is a separate selected-frame operation (`Model.apply(state,
"static_text", "PB")`): 1–2 complete centered 5×7 glyphs fit without cropping.
It preserves the selected duration and every other frame. Scrolling text remains
an explicit whole-timeline replacement. `Model.apply(state, "timing_all", 200)`
changes all frame durations atomically within the existing 60-second document
budget. Switching Web Draw/Text/Animation workspaces changes no project data.

`Model.apply(state, "play", 3, num_floor(now_ms()))` starts three preview passes
from frame 1. Repetitions must be 1–8, with at most 60 seconds and 128 displayed
frames per run. These transient settings do not change the document or export
schema. Stop resolves the elapsed pass/frame; invalid actions preserve clocks,
selection, playback and document. Import resets repetitions to 1.

Web playback is a finite SVG timeline produced in PBLang. Each remaining frame
has a timed visibility interval; the final frame freezes and a completion label
appears. Stop and successful edits/import navigate to a response without the
animation tree. There is no JavaScript, setInterval, recurring request or infinite
repeat. Browser presentation and server wall clock are not synchronized hardware
clocks: server state catches up on the next valid page/action request. Background
tab scheduling, navigation/history and other viewers are browser-local; a shared
server edit does not push updates to already open tabs. Use one active editor tab.

The mechanism uses SVG [`set`](https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Element/set)
visibility with finite durations. Actual tested browser support is reported in the
release evidence; this source contract is not a universal platform test.


## Local counter generator and project envelope

The Studio's pure `counter.sapp` derives values from elapsed time without creating
an authored frame sequence. Its strict config is `{kind:"counter",start,end,
interval_ms}` with integer start/end0–99, whole-second intervals1000–10000ms,
maximum30 values and60000ms. Count direction follows the endpoints. Native uses
the existing tick loop; Web renders current server samples via explicitly started,
bounded same-origin refreshes. This is not hardware or an external sensor feed.

`Model.apply(state,"counter_start",config,now)` validates atomically; Stop retains
the last sampled value and returns to unchanged authored frames. Only
`counter_capture` copies pixels into the selected authored frame. `counter_settings`
saves without starting; `counter_clear` removes settings. Sequence playback and
counter playback do not run concurrently. Clock reversal stops the active source.

`Model.export_project` emits `pblang.uno-r4-led-project.v2` with exactly five keys:
format, mapping, document, arduino_frames, generator. The first four retain the
legacy authored-frame mapping; generator is null or a validated counter config.
Runtime clocks, active playback and last sample are deliberately not serialized.
`Exchange.decode_project` accepts v1 or v2 and returns detached document/settings.
The legacy `Exchange.decode` remains v1-only to avoid silently discarding generator
configuration. Importing v1 clears absent settings; v2 imports stopped at its start.
`Model.export_document` and `/frames.json` stay byte-compatible v1 exports. The
new `/project.json` endpoint and native Project action persist both parts.

Both formats retain the128KiB strict duplicate/depth/item limits and independent
validation of all Arduino words against authored pixels. No new renderer/runtime,
external resource access, JavaScript or Rust is required by this generator.
