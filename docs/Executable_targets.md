# Executable targets

PBLang separates an operating-system profile from an executable backend. The
profile describes a userspace/device contract; only `stdlib:target_artifact.sapp`
may claim that target bytes can actually be emitted.

## Verified backend matrix

| OS family | Minimum version | CPU floor | Format | Input | Verification |
|---|---:|---|---|---|---|
| DOS | 2.0 | 8086 | COM, 16-bit tiny model | register IR | bounded 8086 execution model |
| CP/M | 2.2 | Z80 | COM, transient program | register IR | bounded Z80 execution model |
| Linux | 2.6 | i386 | ELF32 | register IR | run on current host |
| Linux | 2.6 | x86-64 | ELF64 | register IR | run on current host |
| Linux | 3.7 | ARMv8-A | ELF64 | register IR | instruction execution model |
| FreeBSD | 12 | x86-64 | ELF64, FreeBSD OSABI | register IR | format and syscall ABI |
| Windows | 5.2 | x86-64 | PE32+ | register IR | headers, `kernel32!ExitProcess` import table, RX `.text` and RW `.idata` |
| Windows | 5.1 | i386 | PE32 | register IR | headers and imports checked with `objdump`, RX `.text` and RW `.idata` |
| macOS | 10.6 | x86-64 | Mach-O 64 | register IR | container format; target signing/run still required |
| WASI | 1 | wasm32 | WebAssembly | PBLang source | runtime verification |

`plan(os, version, arch)` rejects a version older than the backend floor. It
also rejects catalog entries such as Android/ARM64 for which no native artifact
emitter exists. This is deliberate: an ELF container alone is not an Android
backend, and a platform profile is not a kernel, driver set, SDK, signer, or
proprietary framework implementation.

Unsupported native targets return `portable_fallbacks: ["c99-source"]`.
`emit_c99(ir)` emits source that still needs a real target C99 compiler and
runtime; it is never reported as a ready native executable.

## Old systems

`stdlib:codegen_dos.sapp` emits raw 8086 instructions without an assembler or
linker. It supports arithmetic, multiplication with preserved virtual-register
semantics, moves, conditionals, and short/near loops. It enforces 16-bit
immediates, the COM 64 KiB image limit, and DOS `INT 21h/AH=4Ch` termination.
Because it uses the 8086 instruction subset, the output remains usable on later
x86 processors under compatible DOS environments.

`stdlib:codegen_cpm.sapp` emits raw Z80 instructions for CP/M's 0100h transient
program area. It implements the same arithmetic/move/conditional/loop IR subset,
keeps four 16-bit virtual registers in bounded image slots, and terminates through
the CP/M warm-boot vector at 0000h. CP/M has no portable process exit status, so
the backend loads r0 into HL before termination. Its semantic tests execute every
emitted opcode in a bounded Z80 model, including multiplication and nested loops.

The Windows i386 backend emits PE32 rather than re-labelling PE32+ bytes. It uses
4-byte import thunks, the i386 COFF machine identifier, Win32 stack/heap fields,
and a fixed-base indirect call to `kernel32!ExitProcess`. Independent GNU
`objdump -x` verification identifies the output as `pei-i386`, resolves the
import, and confirms read-only executable `.text` plus non-executable `.idata`.

Supporting every historical OS release is not a single format switch. Each new
row requires its loader format, CPU ABI, syscall/application ABI, version floor,
negative tests, and either target execution or a bounded instruction model.
Targets with proprietary APIs additionally require the relevant SDK and signing
rules. They remain unavailable until that evidence exists.

## Native Linux input

`stdlib:linux_uinput.sapp` replaces the external `ydotoold` dependency for
approved key, click, relative-pointer, absolute-pointer, and bounded ASCII text
actions on Linux x86-64 and ARM64. It encodes
the kernel's 92-byte `uinput_setup`, 24-byte `input_event`, and exact ioctl
request numbers, including 28-byte `uinput_abs_setup`, then emits an RX-only
static ELF using direct
`open/ioctl/write/close` syscalls. Building the ELF is inert. Running it injects
input and is only wired through `input_control` after its environment opt-in,
master-control session, nonce approval, audit, and kill switch.

The x86-64 artifact is ABI-byte verified. The ARM64 artifact is now also run
through a bounded PBLang instruction executor which consumes the exact generated
ELF code and verifies the `openat/ioctl/write/close/exit` register ABI, branches,
ioctl sequence, and packet length. This is execution-model verification, not a
live ARM64 host run; real ARM64 `/dev/uinput` verification remains required before
the status can be upgraded to run-verified. Native uinput text intentionally
rejects non-ASCII input because evdev keycodes do not encode Unicode or a keyboard
layout.

`stdlib:x11_input.sapp` replaces `xdotool` for key, click, relative-pointer and
text events through
trusted absolute-path `libX11`/`libXtst` system APIs. It does not dynamically
search attacker-controlled library paths. Text injection handles common ASCII
modifiers directly and maps other Unicode scalars to standard X11 `Uxxxx`
keysyms. A Unicode scalar succeeds only if the current server keymap has a
keycode for it; the adapter never mutates the user's keyboard layout. Absolute
pointer movement still uses a reviewed external fallback because the current
bounded typed FFI supports at most four arguments while
`XTestFakeMotionEvent` requires five.

`stdlib:input_daemon.sapp` adds a persistent, bounded loopback service contract
with HMAC-SHA256 authentication, constant-time tag checks, strictly increasing
sequence numbers, replay rejection, frame limits and request quotas. The
`tools/pbinputd.sapp` entrypoint binds this contract to native X11 only after
`PB_INPUT_DAEMON_ENABLE=1` and a 32+ character token. It is not described as a
persistent uinput device: the runtime still lacks a streaming process/native-fd
API capable of keeping `/dev/uinput` open across requests.

`stdlib:tool_probe.sapp` performs metadata-only discovery in trusted system
directories. It never runs `--help` or `--version`: `ydotoold --help` starts the
daemon and a blocking process API would wait forever. The module classifies such
services as daemons and fails closed until the runtime exposes a pre-emptive
process timeout and kill handle.

Live input was verified only inside a disposable Xephyr server, never on the
active desktop. The pointer moved from `(320,240)` to `(357,269)`, and independent
`xev` output recorded 11 key presses for `PBLang!`, including Shift state. The
default Xephyr keymap had no Greek alpha keycode, and the backend correctly
reported that Unicode request as unavailable. Semantic tests additionally cover
ABI bytes, ARM64 execution, capability rejection, ELF permissions, authenticated
daemon tamper/replay/quota behavior, planner behavior, and approval gates.

## Usage

```pblang
let T = import("stdlib:target_artifact.sapp");
let p = T.plan("dos", "6.22", "x86");
let a = T.build_ir(p, [["seti", 0, 42]]);
T.write("answer.com", a);
```

For CP/M use `T.plan("cpm", "2.2", "z80")`; for 32-bit Windows use
`T.plan("windows", "5.1", "x86")`.

Run `examples/cross_target_artifacts/main.sapp` for an inert matrix demo.
