# pblang x86-64 frame-encoding optimization report

This snapshot measures compact stack-frame and stack-argument encodings in the
two x86-64 backends implemented in pblang.

## Accepted implementation

- Zero-byte shared-IR frames omit `sub rsp,0`.
- Stack adjustments up to 127 bytes use `sub/add rsp,imm8`; larger values keep
  the safe `imm32` representation.
- The source backend computes framed-function size before real emission and
  fails if the sizing and emission passes drift.
- Shared-IR epilogues use `leave; ret`; a direct terminal `ret` prevents an
  unreachable duplicate compatibility epilogue.

No ABI, register-allocation, parser, optimizer, or language semantic changed.

## Method

`bench/x64_frame_encoding_stages.sapp` generates 32 framed source functions
with ten arguments and 32 shared-IR functions with eight local slots. Each
reported latency sample is itself an average of 20 source compilations or 100
IR compilations. Five independent samples are compared by median. The emitted
ELFs are executed after each implementation state; expected exit codes are 37
and 41.

## Results

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Source ELF | 8,124 B | 8,022 B | -102 B (-1.26%) |
| Shared-IR ELF | 1,943 B | 1,584 B | -359 B (-18.48%) |
| Source codegen median | 69.462 ms | 70.629 ms | +1.7% |
| Shared-IR codegen median | 2.261 ms | 2.032 ms | -10.1% |
| Source executable result | 37 | 37 | unchanged |
| Shared-IR executable result | 41 | 41 | unchanged |

The source latency does not establish a speed improvement; the small increase
is reported rather than normalized away. Its accepted benefit is deterministic
artifact and instruction-footprint reduction. One after sample was a visible
host outlier (105.181 ms source, 2.987 ms IR), which is why medians were chosen
before measurement.

## Verification gates

- Compact and large-frame byte contracts in
  `tests/semantics/codegen_x64_hardening_test.sapp`.
- Executable shared-IR calling-convention tests.
- Executable source backend smoke matrix, including >6 arguments and recursion:
  254/254 cases passed.
- Semantic corpus: 644 tests passed in the parallel run after filtering the
  three polyglot watchdog cases; those three passed in a serial run, for
  647/647 executed test files.
- The pblang phase suite passed across tokenization, parsing, semantics,
  typechecking, bytecode, tree interpretation, C99, x86-64, i386, ARM64, and
  WASM generation.
- Ten pblang emulator gates passed: 6502, AVR, Z80, MIPS, MSP430, PowerPC,
  RISC-V, SPARC, WASM, and the quantum state-vector model.

The canonical all-parallel semantic command can still trip its worker watchdog
on the three polyglot tests even though each completes serially in about two
seconds or less. Also, `polyglot_cdylib_per_lang_test.sapp` currently prints D
and Haskell adapter failures but exits successfully; that pre-existing test
contract must be strengthened before those two adapters can be claimed as
passing. This optimization did not modify Rust or any non-pblang implementation
source.
