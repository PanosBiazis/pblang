# Embedded Programming

PBLang models embedded systems as:

```text
architecture -> MCU -> board -> capabilities -> pins -> portable IR -> native backend
```

Applications select board aliases such as `D7` or `GPIO23`; lowering resolves
them to physical MCU resources. Capability and pin checks happen before code
generation. Reserved flash pins, input-only outputs, unknown aliases, excessive
delays, and unsupported operations fail closed.

## Current verified slice

The Arduino UNO / ATmega328P backend emits AVR machine code and Intel HEX
directly from PBLang. It does not generate C, C++, Rust, assembly source, or an
Arduino sketch. The 500 ms portable blink program produces a 46-byte native
firmware body. Its timing model is within one AVR cycle of the requested delay.

The current Linux validation hardware is:

| Board | Current port | Test pin | Status |
|---|---|---|---|
| Arduino UNO / ATmega328P | `/dev/ttyUSB0` | `D7`, button `D4`, loopback `D6 -> D2` | Persistent controller flashed; shared protocol and physical input verification |
| ESP32 DevKitC V4 / WROOM-32D | `/dev/ttyUSB1` | `GPIO23`, button `GPIO27`, loopback `GPIO25 -> GPIO33` | Native persistent controller; reset boot and shared protocol verification |

Port names are current test mappings, not hardcoded product identities. Use
`pblang run tools/pbembed.sapp --allow-all -- devices` to inspect bounded,
privacy-redacted discovery. Multiple compatible transports require an explicit
selection. UNO probing and flashing require separate opt-in environment flags
and exact board confirmation.

## Portable source

```pblang
let Embedded = import("stdlib:embedded.sapp");
let program = Embedded.blink("status-led-test", 500);
let uno = Embedded.build(program, "arduino-uno");
let esp = Embedded.build(program, "esp32-devkitc-v4");
```

The same IR validates for both boards. `uno.ok` is currently true. The bounded
ESP GPIO23 artifact is available through `build-esp32-boot` and the compatible
`build-esp32-ram` alias. This is a physically verified persistent boot image,
not a general-purpose ESP32 application backend.

The image is generated, parsed, loaded and flashed by PBLang code. It uses the
classic ESP32 ROM UART protocol directly; it does not invoke Python, esptool,
generated C, assembler, a vendor compiler, or a hidden source fallback. Before
write, the loader verifies the exact classic-ESP32 chip magic and reads the
security eFuses twice. It rejects unsigned plaintext writes when secure boot or
flash encryption is enabled. Writes are fixed to offset `0x1000`, bounded to one
4 KiB sector, read back byte-for-byte and compared with both ROM and local MD5.
No eFuse is written.

The persistent image disables and relocks the ROM-started RTC and Timer Group 0
watchdogs before application I/O. Two independent normal-reset runs produced
10/10 alternating markers each. Their 18 complete phases measured
`496.08..496.64 ms` for a requested `500 ms`, using a strict `+/-75 ms` gate.

```sh
PB_EMBEDDED_PROBE_ENABLE=1 pblang run tools/pbembed.sapp --allow-all -- \
  probe-esp32 /dev/ttyUSB1 --confirm-read-only-probe
PB_EMBEDDED_ESP32_FLASH_ENABLE=1 pblang run tools/pbembed.sapp --allow-all -- \
  flash-esp32-boot /dev/ttyUSB1 --confirm-board=esp32-devkitc-v4 --confirm-flash
PB_EMBEDDED_RESET_ENABLE=1 pblang run tools/pbembed.sapp --allow-all -- \
  reset-esp32 /dev/ttyUSB1 --confirm-normal-reset
PB_EMBEDDED_MONITOR_ENABLE=1 pblang run tools/pbembed.sapp --allow-all -- \
  verify-esp32-markers /dev/ttyUSB1 10 500 75
```

The ESP32-WROOM-32D target records the module's 4 MiB external SPI flash and the
chip's 448 KiB ROM, 520 KiB SRAM, 8 KiB RTC FAST SRAM and 8 KiB RTC SLOW SRAM.
Flash is distinct from volatile SRAM and can hold boot firmware, partition
tables, applications, OTA slots, NVS and filesystems. This slice writes only the
single verified boot sector. It deliberately does not publish a partition table,
NVS, OTA slots or a filesystem yet.

## Persistent wired controller

`embedded_protocol.sapp` defines one fixed 12-byte, checksum-protected and
sequence-bound command contract for `status`, capability negotiation, `on`,
`off`, `toggle`, finite or continuous `blink`, `stop`, two-phase `loopback`,
`adc`, and read-only `button`. Independent PBLang-native AVR and Xtensa LX6
firmware emitters implement that contract. Board descriptors own baud and
post-open reset timing, so the shared controller contains no UNO/ESP timing
special case. Boards without a physical controller backend fail closed.

The physical verifier sends positive and negative frames. On 2026-08-19 both
connected boards passed synchronization, capability negotiation, out-of-order
rejection, duplicate replay rejection, unsupported-opcode rejection,
invalid-interval rejection, physical LOW/HIGH loopback, checksum-corruption
silence, overlapping-magic parser recovery, `255 -> 1` sequence rollover, safe final `off`, and termios
restoration. UNO required two bounded idempotent status attempts after its
USB-triggered reset; ESP32 required one. These are protocol/transport results,
not proof that every peripheral is implemented.

The verified loopback circuits are `UNO D6 -> 1 kOhm -> D2` with a 10 kOhm
pull-down on D2, and `ESP32 GPIO25 -> 1 kOhm -> GPIO33` with a 10 kOhm pull-down
on GPIO33. The command reads LOW, drives HIGH, reads HIGH, then always leaves
the output LOW. Its two result bits preserve the existing LED/blink state.

Controller protocol version 1 also publishes the extended `analog-follow`
capability. On UNO, A0 is calibrated for the shared 3.3 V potentiometer and
updates Timer0 OC0A/D6 at approximately 976 Hz. On classic ESP32, ADC1 GPIO34
updates the 8-bit DAC1 output on GPIO25. Temporary digital loopback explicitly
returns GPIO25 from RTC/DAC routing to the digital GPIO matrix and restores DAC
mode afterward; a physical LOW/HIGH regression test protects this transition.

For visual wiring diagnosis, the ordered `output-test` command holds the main
and loopback outputs HIGH (`D7` plus `D6`, or `GPIO23` plus `GPIO25`) until
`outputs-off`, reset, or firmware replacement. It never configures ESP32
`GPIO0`, which remains a boot-strapping input outside the application profile.

```sh
PB_EMBEDDED_PROBE_ENABLE=1 pblang run tools/pbembed.sapp --allow-all -- \
  control arduino-uno /dev/ttyUSB0 outputs-on --confirm-control
PB_EMBEDDED_PROBE_ENABLE=1 pblang run tools/pbembed.sapp --allow-all -- \
  control esp32-devkitc-v4 /dev/ttyUSB1 outputs-on --confirm-control
```

The 2026-08-19 physical wiring run observed all five user-visible LEDs lit:
the four declared outputs above plus the already-lit GPIO0-connected indicator.
Only the four declared outputs were driven by PBLang; GPIO0 was deliberately
left unchanged.

```sh
pblang run tools/pbembed.sapp --allow-all -- build-controller-uno
PB_EMBEDDED_FLASH_ENABLE=1 PB_EMBEDDED_PROBE_ENABLE=1 \
  pblang run tools/pbembed.sapp --allow-all -- \
  flash-controller-uno /dev/ttyUSB0 --confirm-write

pblang run tools/pbembed.sapp --allow-all -- build-controller-esp32
PB_EMBEDDED_RAM_LOAD_ENABLE=1 pblang run tools/pbembed.sapp --allow-all -- \
  load-controller-esp32 /dev/ttyUSB1 --confirm-ram-load
PB_EMBEDDED_ESP32_FLASH_ENABLE=1 pblang run tools/pbembed.sapp --allow-all -- \
  flash-controller-esp32 /dev/ttyUSB1 \
  --confirm-board=esp32-devkitc-v4 --confirm-flash

pblang run tools/pbembed.sapp --allow-all -- \
  control esp32-devkitc-v4 /dev/ttyUSB1 loopback --confirm-control
PB_EMBEDDED_PROBE_ENABLE=1 PB_EMBEDDED_RESET_ENABLE=1 \
PB_EMBEDDED_PROTOCOL_VERIFY_ENABLE=1 \
  pblang run tools/pbembed.sapp --allow-all -- \
  verify-controller-protocol esp32-devkitc-v4 /dev/ttyUSB1 --confirm-protocol-test
```

The verifier requires an exact MCU signature/magic probe compatible with the
requested controller profile and records the expected generated artifact
digest separately. It does not observe that digest from the running firmware
and does not prove the exact carrier board. This is protocol conformance on a
verified MCU class, not cryptographic device or firmware attestation. The
checksum protects serial framing/corruption only; the direct protocol is not
authenticated and must not be exposed directly to a LAN or the Internet.

## Inspector and technical manual

The target database describes every exposed I/O alias in the current UNO,
classic ESP32 DevKitC and Pico-family board definitions. The inspector reports:

- board, MCU, architecture, ABI, clocks and memory capacities;
- logic and supply voltage;
- physical pin/port/bit and all aliases;
- digital input/output, ADC, DAC, PWM, interrupt and peripheral roles;
- reserved, boot-strapping, input-only, UART and ADC/Wi-Fi hazards;
- physical hardware capability separately from native compiler support;
- upload protocol, baud, firmware format and current verification status.

```sh
pblang run tools/pbembed.sapp --allow-all -- manual arduino-uno
pblang run tools/pbembed.sapp --allow-all -- inspect-pin arduino-uno D7
pblang run tools/pbembed.sapp --allow-all -- inspect-pin esp32-devkitc-v4 GPIO25
pblang run tools/pbembed.sapp --allow-all -- inspect "" /dev/ttyUSB1
pblang run tools/pbembed.sapp --allow-all -- inspect esp32-devkitc-v4 /dev/ttyUSB1
```

Supplying only a board works on every platform because it reads the portable
manual. Supplying a port invokes the current platform discovery adapter. Linux
is implemented; Windows, macOS, Android and BSD remain explicitly incomplete.
Ambiguous VID/PID matches require the programmer to select a board. Serial
numbers and serial-bearing stable paths remain redacted by default and are
never persisted by the inspector.

High-level IR constructors now include `digital_read`, `digital_write`,
`analog_read`, `analog_write`, and `pwm_write`. Validation accepts an operation
only where the selected physical pin supports it. A valid hardware operation
can still have `compiler_status:"incomplete"`; code generation must then fail
instead of silently omitting it.

## Circuit and controller state

`stdlib/embedded_circuit.sapp` provides a bounded model shared by physical and
emulated devices:

- up to 32 board instances and 256 outstanding hardware intents;
- deterministic LED set, toggle, finite/continuous blink, and scheduler ticks;
- automatic grid placement plus explicit programmer coordinates;
- ambiguity-safe USB discovery and manual port overrides;
- no persistent USB serial number or other machine identity;
- immediate verified state in emulator mode;
- pending state in wired mode until the transport acknowledges actual output;
- consumed acknowledgements are removed, so replay cannot confirm a command.

This separation keeps GUI, web, and Android renderers unprivileged. They send
bounded controller intents; a platform adapter owns device access and returns
verified observations.

## Raspberry Pi microcontrollers and broader families

Board descriptors exist for Raspberry Pi Pico/Pico W (RP2040) and Pico 2/Pico 2
W (RP2350), including ARM and alternate Hazard3 RISC-V architecture identities.
Their native code generators and UF2 packaging are not yet implemented. These
boards therefore fail closed instead of using a hidden C SDK fallback.

The architecture catalog also reserves explicit families for megaAVR, ARM
Cortex-M/SAM/SAMD/STM32/nRF, Renesas RA, Espressif Xtensa and RISC-V, MSP430,
8051, and PIC. Catalog presence is not an implementation claim. Universal MCU
support requires a native backend, startup/runtime, linker/image format,
flashing transport, emulator tests, and real hardware evidence per family.

## Complete-MCU and complete-project direction

The product target is not an LED-only DSL. PBLang projects must be able to
combine bare-metal firmware, reusable drivers, a host service and responsive
GUI/web/Android clients under one package/build/test/security model. Each MCU
family is implemented in verified layers:

1. CPU ISA, ABI, startup, interrupt vectors, memory/link layout and artifact format.
2. Clocks, GPIO, timers, interrupts, watchdog and safe raw-register access.
3. ADC/DAC/PWM, UART, I2C, SPI, DMA, USB and persistent storage.
4. Power modes, networking/radios where present, secure boot, signed update and OTA.
5. Board pin aliases, voltage/reserved-pin diagnostics, flashing/debug transport,
   execution model and physical hardware matrix.

Hardware capability and compiler implementation remain separate. UNO currently
has native general pin-mode/digital-write/delay/loop codegen plus the persistent
UART controller, ADC0, D6-to-D2 loopback and debounced D4 input. ESP32 currently
has the persistent GPIO23 controller, ADC1/GPIO34, GPIO25-to-GPIO33 loopback and
debounced GPIO27 input. General pin-selectable digital input, DAC/PWM,
interrupt delivery, buses, DMA, NVS/filesystem, signed firmware and OTA are not
yet native-complete. RP2040/RP2350 and UNO R4 are truthful descriptors only.

## ADC controller and shared application example

The persistent UNO and classic ESP32 controller artifacts implement the same
bounded `adc` command. UNO samples ADC0/A0 at 10 bits using AVcc as reference.
Classic ESP32 samples ADC1 channel 6/GPIO34 at 12 bits with 12 dB attenuation.
The response contains both the raw sample and its target-specific maximum, so a
portable client consumes a normalized ratio instead of assuming one ADC width.
For diagnosis, the UNO backend can generate a temporary artifact restricted to
the ATmega328P internal 1.1 V or ground source. This proves the ADC core without
claiming that the external A0 wiring is valid. Restore the normal controller
immediately after the measurement.

The physical pushbuttons are active-high with external 10 kOhm pull-downs.
Firmware samples D4 on UNO and GPIO27 on ESP32 on the controller timer, requires
12 consecutive samples for both press and release, and performs one
`stop-blink-and-toggle-led` action per fully debounced cycle. A one-sample LOW
glitch while held cannot re-arm the button. The UNO threshold is approximately
12 ms; ESP32 still uses a bounded but uncalibrated controller-loop cadence, so
its exact milliseconds remain a separate hardware-timer task. It does not block
UART while debouncing. GPIO0 is not an application LED or button in this
profile; it is released after the ESP32 flashing sequence so normal boot is not
held in download mode.

The GPIO27 self-test is input-only. It disables and verifies the output-enable
bit before sampling the electrical level, and never drives HIGH or LOW against
the external pushbutton circuit.

```sh
pblang run tools/pbembed.sapp --allow-all -- \
  control arduino-uno /dev/ttyUSB0 adc --confirm-control
pblang run tools/pbembed.sapp --allow-all -- \
  control esp32-devkitc-v4 /dev/ttyUSB1 adc --confirm-control
pblang run tools/pbembed.sapp --allow-all -- \
  control esp32-devkitc-v4 /dev/ttyUSB1 button --confirm-control

PB_EMBEDDED_FLASH_ENABLE=1 PB_EMBEDDED_PROBE_ENABLE=1 \
  pblang run tools/pbembed.sapp --allow-all -- \
  flash-controller-uno-diagnostic /dev/ttyUSB0 internal-1v1 \
  --confirm-diagnostic-write
```

The normal controller images enable autonomous potentiometer LED following at
boot. UNO maps the shared 3.3 V potentiometer on A0 to D6 through 8-bit Timer0
hardware PWM at approximately 976 Hz. Its endpoint is calibrated at raw 650
because AVcc remains the ADC reference. Classic ESP32 maps GPIO34 ADC1 channel
6 to the 8-bit DAC1 output on GPIO25. Both routes refresh every four controller
ticks and leave the independent D7/GPIO23 button LEDs unchanged.

```sh
pblang run tools/pbembed.sapp --allow-all -- \
  control arduino-uno /dev/ttyUSB0 follow-on --confirm-control
pblang run tools/pbembed.sapp --allow-all -- \
  control esp32-devkitc-v4 /dev/ttyUSB1 follow-on --confirm-control
```

`follow-off` disconnects PWM/DAC ownership and drives the analog LED output
LOW. Digital loopback temporarily owns the same output and restores analog
following with the last bounded duty value.

`examples/embedded_control_center` demonstrates direct controller use, a shared
bounded application model, and one declarative view compiled for responsive
desktop, Web, and Android surfaces. Physical USB control remains enabled only
on a linked and verified platform transport; rendering an Android UI does not
silently grant it serial-device access.

The shared model supports manual refresh, bounded live refresh at 100..5000 ms,
and periodic refresh at 250..3600000 ms. Each sample serializes status, ADC and
button reads, prevents overlapping polls, and pauses automatically while the
application lifecycle is inactive. Desktop/Web/Android adapters consume the
same policy; physical live data still requires a verified USB-host or network
bridge on that platform.

Physical evidence on 2026-08-19: a 100-sample live input sweep observed both
GPIO27 button levels with 51 press/release transitions. The button therefore
passed its physical active-high input, pull-down, RTC digital-mux, debounce,
protocol, and GPIO23 toggle path on the connected ESP32-WROOM-32D board.

The UNO internal 1.1 V source read 209/1023, validating its ADC core and native
codegen. Isolated physical A0 boundary tests then read 1022/1023 through a
current-limiting connection to 5 V and 0/1023 through a current-limiting
connection to ground. After restoring a common 3.3 V potentiometer with shared
ground and independent 1 kOhm ADC branches, the final bounded ADC initialization
read 4095/4095 on ESP32 GPIO34 and 651/1023 (63.64%) on UNO A0 at the physical
maximum. This difference is expected: the potentiometer is powered from 3.3 V,
while UNO uses 5 V AVcc as its ADC reference. The ESP32 setup now preserves the
adjacent GPIO35 register fields, selects the RTC analog mux, uses increasing
12-bit data, and disables the Hall/LNA amplifier paths before conversion. The
final firmware's low endpoint still requires a second physical reading before
the shared external ADC test is considered fully closed. The full shared
protocol previously passed 14/14 checks on both boards.

## Safety boundaries

- UNO uses 5 V logic; ESP32 and Pico GPIO use 3.3 V logic.
- USB discovery identifies transports first. VID/PID alone is not always an
  exact board or MCU identity.
- Flashing is disabled by default and requires explicit confirmation.
- Hardware state is not marked successful merely because a write was queued.
- External tools may temporarily provide flashing/debug transport for targets
  without a native transport. The verified classic-ESP32 path is PBLang-native;
  external tools are not compiler, HAL, or runtime implementations.

The next implementation slice is a versioned general I/O HAL/command contract
for board-validated pin selection, followed by native PWM and interrupt-event
reporting on UNO and classic ESP32. Partition-table, NVS, OTA, filesystem and
signed-firmware support remain separate fail-closed storage/security slices.
