# pblang execution-model matrix

This matrix records executable conformance models, not claims of complete target
backends. Every verified row has a pure-pblang encoder/interpreter test that runs
real instruction encodings to a checked result. Instruction-driven models stop
after at most 1,000,000 instructions so malformed input fails predictably instead
of monopolizing the host; the quantum checks use finite, fixed circuits.

| Model | Representative systems | Verified surface |
|---|---|---|
| MOS 6502 | Apple II, C64, NES | reusable bounded stdlib model; immediate arithmetic, flags, branches, malformed-stream rejection |
| Z80 | calculators, CP/M, retro systems | reusable bounded stdlib model; loads, arithmetic, flags, relative branches, malformed-stream rejection |
| AVR/ATmega | Arduino, appliances | reusable bounded stdlib model; checked register encoding, arithmetic, multiply, branches, malformed-stream rejection |
| MSP430 | sensors, low-power controllers | reusable bounded stdlib model; checked word operations, immediate mode, jumps, CPUOFF halt, malformed-stream rejection |
| MIPS32 | routers, SGI, PlayStation | reusable bounded stdlib model; checked signed/unsigned arithmetic, multiply, branch delay slots, aligned sparse word stores, snippet fallthrough, overflow and malformed-control rejection; bounded source assembly supports ADD/ADDU/ADDI/ADDIU/SW/NOP/SYSCALL |
| PowerPC32 | classic Macs, consoles, embedded and space systems | reusable bounded stdlib model; checked encodings, arithmetic, multiply, CR branch, syscall, malformed-control rejection |
| SPARC V8 | workstations, LEON/ERC32 satellites | arithmetic, traps, delay slots, annul |
| RISC-V RV64I+M subset | open embedded through server CPUs | reusable bounded stdlib safe-integer model; checked arithmetic, branches, jump, ecall, malformed-control rejection |
| WebAssembly | browsers and portable runtimes | reusable bounded stdlib model; checked LEB128, i32 stack/local arithmetic, structured control, malformed-stream rejection |
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

Output, environment and virtual-file quotas are measured in encoded UTF-8
bytes, not character counts, so multibyte text cannot bypass memory/storage
limits. The output byte counter is maintained in O(1); overwrites and deletes
release the exact encoded-byte amount. Portable environment keys and values
reject embedded NUL. The Android composition preserves an already granted
permission when event publication cannot proceed and returns a fresh decoded
service response for every call, preventing callers from mutating registered
service state through a shared map alias.

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

Differential reports use `pblang.emulator-differential.v2`. Legacy v1 reports
are rejected: their independence flag cannot establish the new evidence
contract. Regenerate reports with the current tool; renaming a format field is
not a migration. Every declared comparison field must exist on both sides;
absent fields fail even when both are missing, while explicit `nil` remains a
valid scalar value.

`stdlib:emulator_diff.sapp` separates per-case differential comparison from
cross-target claim coverage. `report(...)` binds the caller's backend/oracle,
source digest and platform/architecture/ABI claim to an exact digest, but does
not authenticate the runner or verify independence. `coverage_matrix(...)`
validates and detaches every bounded case before hashing, re-verifies the digest
and pass count, rejects duplicate case and backend/oracle/target identities,
and requires one passing claim for every requested target tuple. External cases
must bind input, artifact, exit, stdout and stderr digests. All reports in one
matrix must share the same ordered campaign input-state digest; target-specific
artifacts remain bound by each report digest. Scalar strings,
canonical cases, reports and matrices have explicit byte ceilings. The matrix
is bounded to 128 reports and 32 target keys. A full set is only structurally
complete and self-reported; `ok`,
`complete` and `independent_execution_verified` remain false until a future
trusted runner verifies provenance. It never claims physical device testing,
complete kernel/driver emulation, or production readiness.

The 6502, Z80, AVR, MSP430, MIPS32, PowerPC32, SPARC V8/LEON, RISC-V, and
WebAssembly subsets
are available to applications as `stdlib:emulator_6502.sapp`,
`stdlib:emulator_z80.sapp`, `stdlib:emulator_avr.sapp`,
`stdlib:emulator_msp430.sapp`, `stdlib:emulator_mips32.sapp`,
`stdlib:emulator_powerpc32.sapp`, `stdlib:emulator_sparc32.sapp`,
`stdlib:emulator_riscv64.sapp`, and `stdlib:emulator_wasm.sapp`; they are
no longer test-local code. `run()`
validates the complete byte/word stream before execution, rejects truncated or
unknown instructions with structured errors, enforces each model's address
and operand limits, and applies a caller-selectable instruction budget. Each
module documents the unimplemented machine surfaces that remain outside its
verified subset.

`platform_emulator.release_conformance()` separates family-level userspace/ABI
modeling from exact OS-release evidence. An exact claim requires matching
OS/version/architecture, artifact and report SHA-256 digests, a named backend
and runner, bounded duration, and successful loader, ABI, memory, filesystem,
process, negative-path, quota, and cleanup checks. Missing or mismatched
evidence fails closed. Even successful exact evidence deliberately does not
claim complete kernel, driver, firmware, or proprietary-framework emulation.

## Device contracts and portable input

`stdlib:device_emulator.sapp` composes a platform machine with bounded input,
sensor/signal channels, accelerator metadata, deterministic time, and explicit
resource quotas. Its catalog covers desktop, server, smartphone, tablet,
smartwatch, smart TV, console, router, microcontroller, sensor, satellite,
automotive, appliance, calculator, browser, and quantum-controller classes.
Impossible known device/OS pairings are rejected. `custom_profile()` exists for
new research substrates, but labels them unverified until conformance tests are
provided.

`Device.tick(machine, milliseconds = 1)` accepts integer numbers from 0 through
86,400,000 milliseconds, including zero. It returns the new device time only
when the composed platform clock accepts the step. A platform error (including
exit or exhausted syscall budget) throws and leaves the device clock unchanged;
the platform clock also remains unchanged for these rejections. Existing signal
timestamps remain unchanged. Fixing the simulated platform condition permits a
subsequent valid tick without adding the previously rejected duration.

The platform `clock_ms` syscall uses the same integer range and returns an
`ok: false` result for an invalid advance. Fractional advances are now rejected
instead of silently rounded down; strings, booleans, nil and non-finite values
are rejected too. Default and valid integer behavior is unchanged. Failed
platform calls retain existing syscall-attempt accounting; clock preservation
does not roll back the syscall counter. Device argument rejection occurs before
the platform call. Direct platform ticks and device ticks remain separate entry
points: callers should use `Device.tick` when advancing a composed device.
These are deterministic in-memory models, not wall-clock synchronization or
hardware timing guarantees; externally corrupted machine maps are not a general
transactional state API.

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
target/release/pblang run tests/semantics/device_clock_test.sapp --allow-all
target/release/pblang run tests/semantics/input_events_test.sapp --allow-all
target/release/pblang run bench/device_emulator_stages.sapp --allow-all
target/release/pblang run examples/device_emulation/main.sapp --allow-all
target/release/pblang run examples/android_emulator_security/main.sapp --allow-all
target/release/pblang run examples/isa_emulation/main.sapp --allow-all
target/release/pblang run tools/pbinput.sapp --allow-all -- status
target/release/pblang run tools/pbinput.sapp --allow-all -- plan-delta ydotool 10 -5
```

On the 2026-07-19 verification host, indexing the immutable platform catalog by
canonical OS reduced 32,000 profile resolutions from 3,683.22 ms to 662.28 ms
(5.56x faster, 82.0% lower latency) with the same `1,616,000` checksum. The
composed six-device resolver fell from 965.20 ms to 452.64 ms (2.13x faster)
with the same `3,768,544,000` checksum. These are local component measurements,
not cross-language or full-system emulator claims.
