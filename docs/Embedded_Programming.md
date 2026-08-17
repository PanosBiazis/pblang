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
| Arduino UNO / ATmega328P | `/dev/ttyUSB0` | `D7` | MCU signature, flash and read-back verified |
| ESP32 DevKitC V4 / WROOM-32D | `/dev/ttyUSB1` | `GPIO23` | Native RAM blink and UART timing verified; persistent flash incomplete |

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

The same IR validates for both boards. `uno.ok` is currently true. The public
ESP build remains fail-closed because only the bounded RAM-only GPIO23 blink
artifact has physical evidence. It is available through `build-esp32-ram`; it
does not imply a persistent, general-purpose ESP32 backend.

The RAM artifact was generated in PBLang, independently decoded as an ESP image,
loaded into the connected classic ESP32 and measured through alternating UART
markers. Eight markers alternated correctly; the seven complete phases measured
`495.34..495.83 ms` for a requested `500 ms`. The board was hard-reset after
the test. The artifact does not use generated C, assembler, a vendor compiler,
or a hidden source fallback. The temporary external boundary was the ROM serial
loading transport only.

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

## Safety boundaries

- UNO uses 5 V logic; ESP32 and Pico GPIO use 3.3 V logic.
- USB discovery identifies transports first. VID/PID alone is not always an
  exact board or MCU identity.
- Flashing is disabled by default and requires explicit confirmation.
- Hardware state is not marked successful merely because a write was queued.
- External tools may temporarily provide flashing/debug transport only. They
  are not compiler, HAL, or runtime implementations.

The next product slice is the shared responsive LED-controller view and its
desktop/web/Android adapters. Real wired live control additionally needs a
bounded device protocol and persistent native UART firmware support.
