# UNO R4 WiFi: PBLang project to physical matrix

This additive route preserves the existing native/Web editor, v1 exports, v2 saved
projects, and offline preview. It targets **Arduino UNO R4 WiFi only**. It neither
scans devices nor uploads firmware automatically. Exported authored frames are used;
a saved counter configuration must first be captured into frames in the editor.

The application is actual `device_firmware.sapp`, compiled by PBLang's
`stdlib/compiler/ctranspile.sapp` numeric C backend. `device_export.sapp` adds numeric
project/digest/literal accessors and invokes that compiler. The generated
`pblang_firmware.h` is built with the official Arduino ARM toolchain. The temporary
thin SDK adapter supplies fixed storage, milliseconds, serial bytes and matrix
I/O only. This is **not** a completed native RA4M1 machine-code backend. No existing
compiler/runtime or legacy AVR/ESP protocol is replaced.

## Export and build

Run from a retained PBLang package root containing the source modules and runtime:

```sh
pblang run tools/pbleddevice.sapp --allow-all -- export /absolute/project.json /absolute/new_sketch 4
arduino-cli compile --fqbn arduino:renesas_uno:unor4wifi /absolute/new_sketch
```

The new sketch directory must not exist; its parent must exist. Its basename must
be an Arduino identifier (letters/digits/underscore, beginning with a letter,
maximum 64 characters). Input reads are bounded and reject symbolic links. Output
uses existing PBLang Linux directory APIs, private directory permissions and
exclusive files. An interrupted write leaves an explicitly partial private folder;
rerun with a different new directory. Nothing silently overwrites an existing sketch.
The export result explicitly says `compiled_for_board:false, uploaded:false`.

Files: matching `.ino`, generated `pblang_firmware.h`, full compiled `firmware.sapp`,
validated `plan.json`, and `build.json` with source/code/adapter hashes and target.
Keep project exports local: frame content is embedded in the firmware/source.

Upload requires separately identifying the actual UNO R4 WiFi and selecting its
port with the official Arduino uploader. A CP2102 serial adapter is not evidence
of UNO R4 identity. The editor's live preview never implies a connected device.

## Explicit commands after upload

```sh
pblang run tools/pbleddevice.sapp --allow-all -- status /absolute/project.json /dev/ttyACM0 4 --confirm-uno-r4-wifi
pblang run tools/pbleddevice.sapp --allow-all -- play /absolute/project.json /dev/ttyACM0 4 --confirm-uno-r4-wifi
pblang run tools/pbleddevice.sapp --allow-all -- stop /absolute/project.json /dev/ttyACM0 4 --confirm-uno-r4-wifi
```

Replace the example port with the explicitly identified device. Use the same saved
project and repetition count as the compiled sketch. The Linux host reuses
`stdlib/linux_serial.sapp` at 115200 baud. It verifies `PBLED1`, board, exact project
digest, frame count, repetitions, total duration and a fresh request ID before Play.
This handshake detects mismatches; it is not cryptographic device authentication.

Every operation closes the serial handle. A failed/partial Play is never replayed;
a confirmed handshake permits a bounded cleanup Stop. Inspect `stop_confirmed`,
`output_uncertain`, `close_ok`, and `termios_restored`; a failed cleanup is not proof
that the device stopped. Opening the port can reset some boards/USB setups; an
unconfirmed handshake makes no playback claim. The current shared serial close API
reports termios restoration; do not infer a separately tested modem-line restoration.

Firmware boots blank, starts only on Play, and clears after the finite authored
sequence: 1–8 repetitions, at most 60 seconds and 128 displayed frames. Stop clears
the matrix. Status does not restart playback. Repeated immediately preceding command
IDs acknowledge the existing state; reuse for a different command is rejected.
Earlier request IDs are not retained indefinitely.

## Bounded protocol and scheduling

Requests: `HELLO|PLAY|STOP|STATUS`, one space, 16 lowercase hex request characters,
newline; trailing CR before newline is accepted. Firmware lines are limited to 31
characters, partial input expires after 500 ms, and overflow/control bytes discard
through the next newline. The host adds a synchronization newline for handshake and
failure cleanup, accepting only a matching response within a 2-second/2048-byte budget.
Normal operation rejects mismatched responses immediately. Writes are bounded to 1 second.

Replies are JSON lines under 512 bytes with exactly thirteen fields:
`protocol, board, digest, command, request, ok, error, state, frame, remaining_ms,
frames, repetitions, total_ms`. Frame zero means cleared. The host strictly parses
JSON and rejects duplicate keys. Replies are snapshots, not a live stream.

Firmware receives at most 32 bytes per loop iteration and transmits at most 32
bytes per flush (at most two flushes, or 64 bytes, per loop). On the
UNO R4 WiFi target, the official core defines `NO_USB`: `Serial` is UART1 through the
ESP USB bridge. Renesas 1.6.0 UART inherits an always-zero `availableForWrite()`;
the thin adapter instead permits its synchronous, interrupt-paced one-byte writes.
The USB implementation retains its real readiness check. The UART driver waits for
a transmit interrupt, so this is a bounded application quota, not a hard real-time
guarantee if the SDK/hardware interrupt path stalls. With a functioning driver,
playback ticks even if the host stops reading. It waits for queued output to drain before
reading another command; therefore Stop acknowledgement depends on a reading host.
Synthetic output-backpressure and input-flood tests verify the finite deadline in
the application; they do not prove the SDK can never block.
The host reports uncertainty if confirmation cannot be obtained.

`millis()` and packed words are represented exactly as doubles; uint32 casts happen
only in the SDK adapter. Signed-long modulo is restricted to bounded nonnegative durations/decimal digits;
decimal division subtracts the remainder first to preserve native PBLang parity. The firmware uses nested ordinary `if/else`:
older retained compiler versions omitted some `elif` bodies. The separate compiler
fix is also retained when integrating current main.

## Verification commands (synthetic; no ports)

```sh
pblang run tests/fixtures/uno_r4_led_studio_device_host.sapp --allow-all
pblang run tests/fixtures/uno_r4_led_studio_device_export.sapp --allow-all
c++ -std=c++17 -Wall -Wextra -Werror -fsanitize=address,undefined -I build/pblang_led_device tests/graphics/uno_r4_device_firmware.cpp -o /tmp/pbled-firmware-test
/tmp/pbled-firmware-test > /tmp/pbled-firmware-responses.tsv
python3 tests/graphics/uno_r4_device_assert.py /tmp/pbled-firmware-responses.tsv
python3 tests/graphics/uno_r4_device_oracle.py build/pblang_led_device/firmware.sapp build/pblang_led_device/oracle.sapp
pblang run build/pblang_led_device/oracle.sapp --allow-all > /tmp/pbled-native-oracle.tsv
python3 tests/graphics/uno_r4_device_compare.py /tmp/pbled-firmware-responses.tsv /tmp/pbled-native-oracle.tsv
```

Tests use synthetic state/byte/clock callbacks. They do not open serial devices or
PTYs. Host simulation is separate from generated-C behavior, official ARM compile,
and actual LED observation; each requires its own evidence. The host transport is
Linux-only in this slice. Windows/macOS/mobile retain existing graphics support;
this route does not claim physical-device transport on those platforms or ESP32.

Official SDK references: [matrix API](https://github.com/arduino/ArduinoCore-renesas/blob/main/libraries/Arduino_LED_Matrix/src/Arduino_LED_Matrix.h)
and [board target definitions](https://github.com/arduino/ArduinoCore-renesas/blob/main/boards.txt).
