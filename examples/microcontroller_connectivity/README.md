# Microcontroller connectivity

One PBLang status read over USB, followed by one MQTT 3.1.1 QoS 1 publication
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
message but disconnect/cleanup failed. Do not automatically retry: even QoS 1
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
