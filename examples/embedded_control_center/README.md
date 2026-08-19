# Embedded Control Center

Current example release: **1.3.0** (`versionCode 4` on Android).

This PBLang-only example connects the embedded stack from low-level controller
operations to one responsive UI definition for desktop, Web, and Android.

The complete breadboard wiring, component list, safety boundaries and expected
behavior are documented in [`CIRCUIT.md`](CIRCUIT.md).

## Layers

- `low_level.sapp` opens a bounded controller session and reads ADC directly.
- `model.sapp` exposes board-safe `on`, `off`, `toggle`, `blink`, `adc`,
  `button`, `loopback`, `follow`, and `status` operations without exposing serial handles
  to UI code.
- `view.sapp` is the single responsive component tree.
- `desktop_app.sapp` binds the common view to Linux device discovery, bounded
  controller commands, telemetry updates, inspection, and cooperative cleanup.
- `main.sapp` and `desktop.sapp` open the real native GUI. `web.sapp` serves
  the same component tree through an authenticated loopback hardware bridge.
- `android.sapp` builds a PBLang-generated WebView APK and `android_live.sapp`
  installs it, creates the ADB reverse tunnel, launches it, and keeps the same
  foreground hardware bridge active.
- `test.sapp` checks controller behavior, native GUI interactions,
  compact/wide parity, and fail-closed wired configuration.

The common model provides `manual`, `live`, and `periodic` refresh policies.
Live intervals are bounded to 100-5000 ms, periodic intervals to 250 ms-1 hour,
polls cannot overlap, and inactive/background lifecycle state pauses polling.

The controller firmware follows the shared potentiometer without a host process:

- Arduino UNO reads `A0` every 4 ms and drives `D6` with 8-bit, approximately
  976 Hz hardware PWM. The 3.3 V circuit endpoint is calibrated to raw 650.
- ESP32 reads ADC1 channel 6 on `GPIO34` and drives the optional LED on
  `GPIO25` through its 8-bit DAC. No software PWM loop is used.
- `D7` and `GPIO23` remain independent button/on/off/blink outputs.

## Run

```sh
pblang run examples/embedded_control_center/test.sapp --allow-all
pblang run examples/embedded_control_center/main.sapp --allow-all
pblang run examples/embedded_control_center/desktop.sapp --allow-all
pblang run examples/embedded_control_center/web.sapp --allow-all
pblang run examples/embedded_control_center/android.sapp --allow-all
PBLANG_ANDROID_SERIAL=<adb-serial> pblang run examples/embedded_control_center/android_live.sapp --allow-all
```

The native desktop window starts disconnected. Select the board and detected
USB serial port, then press **Connect**. Commands operate only on that explicit
selection. Changing the board or port closes the active serial session first.
The **Open inspector** button reports board, port, refresh policy, ADC, LED,
button, poll, and failure state without persisting USB serial identities.

Manual refresh is the default. Live and periodic modes are explicit choices,
with bounded intervals, because the current serial transaction primitive is
blocking even though every operation has a timeout and retry limit.

The Web command is a foreground application: keep its terminal open while the
browser is in use and stop it with `Ctrl+C`. The Android live command is the
one-step workflow for the currently verified topology where the boards are
connected to the Linux host and the Android device is connected through ADB.
It reserves the loopback listener before launching the Activity, avoiding the
startup race that previously produced `ERR_SOCKET_NOT_CONNECTED`.

## Release artifacts

- Native Linux GUI source launcher: `desktop.sapp` (also the default `main.sapp`).
- Android 5.0+ APK: `release/Embedded-Control-Center-debug.apk`.
- Authenticated responsive Web application: `web.sapp`.

The native GUI is real and launch-verified, but it is not currently published
as a standalone PBCK. The PBLang-native whole-program freeze reaches the GUI
dependency graph and then fails closed in compiler literal lowering with
`parse_number(): expected string`. No partial PBCK is retained or advertised.

The current physical USB/serial adapter is Linux-only. In the verified Android
workflow the APK controls the real boards through an ephemeral, authenticated
ADB reverse loopback bridge to that Linux adapter. Opening the installed APK by
itself cannot control boards that remain physically attached to the laptop.
Direct Android USB-OTG control remains a separate backend that must be verified
with the board connected to the phone. Unsupported boards and missing ports
fail closed.

Release 1.3.0 records its version in the Android APK evidence. It adds the real
authenticated Web/Android hardware path, per-operation browser deadlines,
busy-state protection, pre-bound listener startup, and removes the obsolete
text-only static Web bundle. The Android 9 device was live-verified selecting,
connecting, turning on, and turning off both the UNO on `/dev/ttyUSB0` and the
ESP32 on `/dev/ttyUSB1`; each result was read back through the firmware status
protocol. Final brightness tracking still requires a person to rotate the
physical potentiometer while observing both LEDs; software cannot move that
component.

The current controller firmware also binds one physical active-high pushbutton
per verified board profile: UNO D4 and ESP32 GPIO27. A debounced press stops any
active blink and toggles the main D7/GPIO23 LED exactly once; release rearms the
button. This board-local behavior remains available when no host UI is running.
