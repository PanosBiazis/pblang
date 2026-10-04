# Microcontroller connectivity

By default, one PBLang status read over USB, followed by one MQTT 3.1.1 QoS 1 publication
to an already running broker on **127.0.0.1**. Nothing runs on import; the CLI
requires explicit board, port and confirmation. It does not upload firmware,
start a broker, discover devices, change outputs or keep a background worker.

## Run

From the PBLang package root, using its matching runtime and dependency lock:

```sh
pblang --allow-all examples/microcontroller_connectivity/main.sapp
pblang --allow-all examples/microcontroller_connectivity/main.sapp -- uno-r4-wifi /absolute/export.json /dev/ttyACM0 1883 4 1 --confirmed
pblang --allow-all examples/microcontroller_connectivity/main.sapp -- esp32-wroom-32d /dev/ttyUSB0 1883 1 --confirmed
```

Replace the example paths and ports with the explicitly selected device.
The final numeric argument is the caller's sequence number (0–2147483647).
The UNO arguments before it are broker port and installed repetition count
(1–8, subject to the LED Studio duration/frame limits). Use the exported JSON
and repetition count matching the firmware installed by LED Studio. A project
mismatch prevents publication. Close other serial monitors before running.
`--allow-all` grants broad host permissions; use only with reviewed local code.

The ESP32 must already run the compatible PBLang embedded-controller firmware.
This example reads its controller status; it does not assume that the board has
an onboard LED. Status fields describe that protocol, not unmeasured sensors.

## Finite monitoring

Prefix the same explicit board arguments with `--monitor SAMPLES INTERVAL_MS`:

```sh
pblang --allow-all examples/microcontroller_connectivity/main.sapp -- --monitor 3 1000 uno-r4-wifi /absolute/export.json /dev/ttyACM0 1883 4 1 --confirmed
```

This requests three separate status messages, starting with sequence 1. Each
sample closes its serial and network connection. The CLI sleeps between samples
and returns a final counter summary; it does not leave a service running.
The exported UNO project is read and validated once before acquisition begins.
Changing the file during a run does not silently change the expected firmware.

Limits: 1–32 samples, 1000–30000 ms minimum interval, and a total scheduling
budget of `(samples - 1) * interval + samples * 1000 + 6000`, at most 60000 ms. Choose a smaller
count/interval if this combination exceeds the budget. Sequence overflow is
rejected before device access. The interval is a minimum wait after completing
a sample, not a promise of a fixed sampling frequency or simultaneous snapshots.

A failed read or unconfirmed publish stops the session; it never catches up
by sending a burst or retries an uncertain delivery. `acknowledged` counts broker
acknowledgements even if later cleanup fails; `published` counts fully successful
exchanges. Neither is a subscriber counter.
Status summaries do not retain a history of device payloads or private paths.
The selected broker may have its own independent storage policy.

Applications can import `stdlib:embedded_telemetry_monitor.sapp` and drive one
`poll(session, read_snapshot, publish, clock)` at a time from their own event loop.
`new(options)` has no device or network effects. The read callback receives
`(sequence, remaining_ms)` and must return a validated telemetry snapshot.
The optional publisher receives `(snapshot, port, timeout_ms)`; omitting it uses
the local MQTT publisher. `status`, `cancel` and `close` support an application's
own progress display and Stop control. The CLI currently has no interactive Stop
UI; cooperative cancellation is an API capability.
Use the returned `wait_ms` to schedule the next poll instead of repeatedly polling
while waiting. `status()` reports the last observed clock; it does not read a
device, advance time or perform I/O.

Custom publisher reports must contain boolean `ok`, `acknowledged`, `cleanup_ok`,
`publish_attempted` and `outcome_uncertain` fields. An acknowledgement requires
an attempted publication; uncertainty must mean an attempt without acknowledgement;
`ok:true` also requires acknowledgement and successful cleanup. A malformed or
contradictory report ends the monitor without retry, counts no acknowledgement or
publication, and leaves delivery uncertain. The explicitly reported cleanup result
remains separate. Valid acknowledgements survive cancellation and cleanup failures.
These checks validate the callback contract, not the truth of a broker response.

Callbacks are trusted application code. They must bound their own execution and
release their resources. Cancellation is checked at callback boundaries, not a
forced native interruption. Existing USB acquisition can overrun the scheduling
budget; a late result is not published and no later acquisition starts. Process
termination or native failures do not guarantee serial-settings restoration.

| Board | Topic | Published status |
| --- | --- | --- |
| UNO R4 WiFi | `pblang/boards/uno-r4-wifi/status` | playback state, frame, remaining milliseconds, frame count |
| ESP32-WROOM-32D | `pblang/boards/esp32-wroom-32d/status` | output state, blinking, interval, remaining milliseconds |

Topics are per board type, not per physical device. The current example is for
one board of each type; it does not distinguish multiple boards of the same type.
Payloads include the schema version, board type, sequence and validation label,
but exclude USB paths, source paths, project hashes, request nonces and credentials.
The label records validated local adapter results; it is not cryptographic
attestation. No retained message is requested. Broker/subscriber logging and
retention remain their own configuration responsibilities.

## Library API

```sapp
let Telemetry=import("stdlib:embedded_telemetry.sapp");
// observed is a confirmed device_host.run(..., "STATUS", true) result.
let snapshot=Telemetry.from_uno(observed, 1);
let delivered=Telemetry.publish_local(snapshot, 1883, 1000);
```

For ESP32, use `from_esp(session, status_result, close_result, sequence)` after
closing the wired session. Emulator results are rejected. Both adapters copy
only the bounded status fields. `publish_local` validates exact snapshot keys,
limits payloads to 1024 bytes and uses one shared 250–5000 ms exchange budget.
It checks exact CONNACK/PUBACK packets, performs no retry and closes the socket
after recoverable exchange errors. Random session identifiers avoid client-ID
collisions without exposing host identity.
The native clock is normalized to whole milliseconds; a backward clock change
aborts the exchange. This host clock is not a hard real-time guarantee.

Results distinguish `acknowledged`, `cleanup_ok` and `outcome_uncertain`.
`ok:false` with `acknowledged:true` means the broker already acknowledged the
message but the subsequent clock check, disconnect or cleanup failed. An exact
PUBACK received at/after the deadline remains acknowledged, while `ok` stays
false and no further protocol write is attempted. Invalid/truncated or wrong-ID
acknowledgements never confirm delivery. The fixed `post-ack-clock-failed` error
identifies a clock failure after a valid PUBACK. Do not automatically retry: even QoS 1
can deliver duplicates. A broker ACK does not prove a subscriber displayed it.
Use the caller sequence to detect repeats when appropriate.
`cleanup_ok` reports the native transport's close result; the existing primitive
does not independently verify kernel descriptor closure.

## Boundaries

This is **USB → Linux PBLang host → local MQTT**, not MCU Wi-Fi/TCP firmware.
There is no remote broker, TLS, credential handling, network discovery, remote
command execution, guaranteed real-time scheduling or automatic reconnect.
Native socket/serial primitives remain host boundaries. Android/iOS/Windows/macOS
wired execution and physical ESP32 telemetry are not established by this example.
The network budget excludes earlier USB acquisition and cannot guarantee recovery
from a native crash or hung system call. Use a separately configured authenticated
broker bridge when a deployment needs access beyond the local computer.
