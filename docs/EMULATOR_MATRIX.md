# pblang execution-model matrix

This matrix records executable conformance models, not claims of complete target
backends. Every verified row has a pure-pblang encoder/interpreter test that runs
real instruction encodings to a checked result. Instruction-driven models stop
after at most 1,000,000 instructions so malformed input fails predictably instead
of monopolizing the host; the quantum checks use finite, fixed circuits.

| Model | Representative systems | Verified surface |
|---|---|---|
| MOS 6502 | Apple II, C64, NES | 8-bit arithmetic, flags, branches, loops |
| Z80 | calculators, CP/M, retro systems | loads, arithmetic, relative branches |
| AVR/ATmega | Arduino, appliances | register encoding, arithmetic, multiply, loops |
| MSP430 | sensors, low-power controllers | word operations, jumps, CPUOFF halt |
| MIPS32 | routers, SGI, PlayStation | arithmetic, multiply, branch delay slots |
| PowerPC32 | classic Macs, consoles, embedded and space systems | canonical encodings, arithmetic, multiply, CR branch, syscall |
| SPARC V8 | workstations, LEON/ERC32 satellites | arithmetic, traps, delay slots, annul |
| RISC-V RV64I+M subset | open embedded through server CPUs | arithmetic, branches, jump, ecall |
| WebAssembly | browsers and portable runtimes | LEB128, stack/local arithmetic, structured control flow |
| Quantum state vector | QASM-oriented simulation | gates, superposition, Bell/GHZ state checks |

Run the matrix with:

```sh
./scripts/t emu_6502 emu_z80 emu_avr emu_msp430 emu_mips emu_powerpc emu_sparc emu_riscv emu_wasm emu_quantum
```

## OS, version, ABI, and process model

`stdlib:platform_emulator.sapp` adds a second, orthogonal layer. ISA models
answer "how are instructions encoded and executed?"; platform profiles answer
"which ABI, executable format, paths, libc, pointer size, and userspace services
does a target expose?".

| Family | Verified version floor | Validated architecture families | ABI / format |
|---|---:|---|---|
| Linux | 2.6 | x86, x86-64, ARMv7/64, MIPS32, PowerPC32, RISC-V64, SPARC V8 | SysV / ELF; glibc, musl, or freestanding |
| Windows NT | 5.1 | x86, x86-64; ARM64 from 10 | Windows ABI / PE |
| macOS | 10.6 | x86-64; ARM64 from 11 | Darwin / Mach-O |
| Android | API 21 | ARMv7/64, x86/x86-64 | Bionic / ELF |
| iOS | 11 | ARM64 and checked simulator profiles | Darwin mobile / Mach-O |
| FreeBSD | 12 | x86-64, ARM64, RISC-V64 | FreeBSD / ELF |
| WASI | preview 1 | WASM32 | WASI / WebAssembly |
| DOS / CP/M / OS/2 / AmigaOS | documented family floors | x86, Z80, m68k, PowerPC32 as applicable | MZ, COM, LX, Hunk |
| Plan 9 / QNX | 4 / 6.5 | x86/64, ARMv7/64 | Plan 9 a.out / ELF |
| FreeRTOS / Zephyr / NuttX | 8 / 2 / 10 | AVR, MSP430, ARM, x86, RISC-V as applicable | firmware or ELF |
| VxWorks / bare metal | 6 / contract 1 | verified historical and embedded ISA families | ELF or firmware |
| AIX / HP-UX / IRIX | 7 / 11 / 6.5 | PowerPC, PA-RISC/IA64, MIPS32 | XCOFF or ELF |
| ChromeOS / Fuchsia | contract 1 | x86-64, ARM64 | ELF |
| Tizen / webOS | 2 / 3 | x86/64 and ARMv7/64 as applicable | Linux-derived ELF userspace |

The resolver rejects impossible historical combinations, compares normalized
versions, and exposes explicit same-family backward-compatibility checks. The
machine model provides bounded byte memory, virtual files/environment,
platform-aware normalized paths, captured stdout/stderr, deterministic
clock/random services, a one-million-syscall budget, and fail-closed unknown
operations. Sparse resident cells, files, storage bytes, environment entries,
and environment bytes use O(1) counters with hard quotas. Memory writes validate
the entire byte batch before mutation; address bounds use subtraction-first
arithmetic. `machine_stats()` exposes these counters, while `unsetenv`,
`delete_file`, `listenv`, and `list_files` provide quota-safe lifecycle APIs.
Mobile and WASI paths are confined beneath a virtual sandbox root.

Run the contract and performance matrix with:

```sh
./scripts/t platform_emulator
target/release/pblang run bench/platform_emulator_matrix_stages.sapp --allow-all
target/release/pblang run bench/platform_resource_stages.sapp --allow-all
```

These models are verification foundations for future code generators. A model does
not mean the production compiler already emits complete binaries for that ISA.
The platform model does not emulate complete kernels, drivers, graphics stacks,
or proprietary frameworks. Individual historical releases inherit a family
contract only when they meet the documented version and architecture gates.
Biological, protein, analog-signal, FPGA, NPU, and other non-ISA substrates
require a defined operation/protocol contract before executable emulation can
be honest.

## Device contracts and portable input

`stdlib:device_emulator.sapp` composes a platform machine with bounded input,
sensor/signal channels, accelerator metadata, deterministic time, and explicit
resource quotas. Its catalog covers desktop, server, smartphone, tablet,
smartwatch, smart TV, console, router, microcontroller, sensor, satellite,
automotive, appliance, calculator, browser, and quantum-controller classes.
Impossible known device/OS pairings are rejected. `custom_profile()` exists for
new research substrates, but labels them unverified until conformance tests are
provided.

`stdlib:input_events.sapp` is the PBLang-native portable event and virtual-input
implementation. `stdlib:linux_uinput.sapp` now emits an RX-only x86-64 ELF that
uses the real Linux uinput ioctl/write ABI, replacing ydotoold for approved key,
click, and relative-pointer actions. `stdlib:x11_input.sapp` uses trusted-path
libX11/libXtst APIs for the same X11 subset without xdotool. Its text path uses
reversible empty-keycode batches via trusted `xmodmap` argv and XTest, providing
layout-independent Unicode on X11; mapping restoration is mandatory before
success. A bounded per-user kernel `flock` serializes mapping batches across
cooperating processes and is released automatically on process death. The
external boundary emits structured argv, never shell commands. Raw
uinput remains a physical-key backend, not a Unicode text backend. Every real
action still passes the existing human opt-in, approval, audit, and kill switch
in `input_control.sapp`.

Executable platform coverage is tracked separately in
`docs/Executable_targets.md`. A new DOS COM backend emits an 8086-compatible
artifact and is checked by bounded instruction execution. Platform rows without
an artifact backend continue to fail closed rather than borrowing a container
format from a different OS ABI.

`tools/pbinput.sapp` provides a PBLang-only CLI for backend discovery, event
validation, structured argv planning, and virtual replay. It never actuates the
live desktop; actual actuation remains exclusively behind `input_control`'s
separate opt-in and human approval.

Run the device contract and benchmark with:

```sh
target/release/pblang run tests/semantics/device_emulator_test.sapp --allow-all
target/release/pblang run tests/semantics/input_events_test.sapp --allow-all
target/release/pblang run bench/device_emulator_stages.sapp --allow-all
target/release/pblang run examples/device_emulation/main.sapp --allow-all
target/release/pblang run tools/pbinput.sapp --allow-all -- status
target/release/pblang run tools/pbinput.sapp --allow-all -- plan-delta ydotool 10 -5
```

On the 2026-07-19 verification host, indexing the immutable platform catalog by
canonical OS reduced 32,000 profile resolutions from 3,683.22 ms to 662.28 ms
(5.56x faster, 82.0% lower latency) with the same `1,616,000` checksum. The
composed six-device resolver fell from 965.20 ms to 452.64 ms (2.13x faster)
with the same `3,768,544,000` checksum. These are local component measurements,
not cross-language or full-system emulator claims.
