# Compile, C99 modulo, and interpreter dispatch report

Date: 2026-07-18

Host recorded by the matrix: Linux 7.0.0-28-generic, x86-64, Intel
i7-1195G7, 8 logical CPUs, PBLang 0.5.7.

## Scope

This slice addresses three measured bottlenecks without adding Rust code:

1. direct Linux x86-64 compile/import and publication overhead;
2. hosted C99 modulo lowering for integer workloads;
3. numeric/loop dispatch in the PBLang-written tree interpreter.

The raw output files beside this report are authoritative. Absolute process
times changed materially between runs, including unchanged C and host-
interpreter controls, so cross-run latency is not treated as causal evidence.

## Direct x86-64 compilation

`stdlib/native/artifact_cache.sapp` adds a bounded content-addressed cache. Its
key covers source, profile, and the tokenizer/parser/ELF/backend inputs. A hit
validates metadata, SHA-256 content, byte count, and the ELF64 Linux/x86-64
header before staged publication. A miss retains the complete safety-certified
backend path. The cache detects corruption and stale dependencies; it is not a
trust boundary against a malicious process running as the same user.

The initial same-turn phase samples moved complete uncached publication from
120.057 ms to 78.954 ms median, 34.24% lower. The independently saved rerun
measured 97.660 ms uncached and 23.998 ms on a verified warm hit, 75.43% lower
within that run. Its backend import alone was 103.395 ms on the loaded rerun,
showing that cold import remains the dominant unresolved phase.

## Hosted C99 modulo

Integer-profile modulo now keeps nonzero literals visible to the C optimizer
and avoids redundant casts. Dynamic hosted modulo calls `pb_mod_checked`, which
rejects division by zero and `LONG_MIN % -1` instead of invoking C undefined
behavior. Literal zero is rejected during transpilation. Default floating-
numeric PBLang semantics remain the default profile.

The paired 19-sample rerun measured 14.675 ms median for the floating profile
and 5.915 ms for the integer profile, a 59.69% reduction. Both stripped
artifacts were 16,008 bytes. This is a profile comparison on the fixed integer
kernel, not a claim that integer mode is semantically interchangeable with all
PBLang programs.

## PBLang-written tree interpreter

AST preparation now gives safe number/identifier binary leaves a compact plan.
Plain assignment statements and while conditions consume that plan directly,
while nested expressions, short-circuit operators, strings, closures, and
control-flow edge cases retain the generic evaluator fallback.

The saved same-process control measured 170.536 ms for the optimized 10,000-
iteration loop, 223.439 ms with plan metadata disabled, and 203.573 ms through
the prior signal-check path. The optimized path was 23.68% below the controlled
generic path. The first before/after acceptance sample moved the loop from
136.348 to 117.261 ms (14.00%) and `fib18` from 138.138 to 126.196 ms (8.65%);
the controlled comparison is preferred because absolute frequency/load drifted.

The broad `pblang-interpreter` matrix row still executes the existing host VM,
not `stdlib/compiler/interp.sapp`. It measured 682.928 ms in the saved rerun,
while C and Python also regressed substantially under that system load. No host
VM claim is derived from this PBLang-only change.

## Cross-language snapshot

The saved fixed integer matrix completed with 22 successful rows, 2 explicit
Zig-unavailable rows, and 0 failures. In that particular run:

| Path | Compile median | Run median | RSS KiB | Artifact bytes |
|---|---:|---:|---:|---:|
| PBLang direct x86-64 | 78.707 ms | 4.269 ms | 264 | 507 |
| PBLang hosted C99 integer | 132.716 ms | 9.811 ms | 1,496 | 14,472 |
| C/GCC | 59.645 ms | 9.197 ms | 1,496 | 14,472 |
| Rust | 96.947 ms | 10.411 ms | 2,052 | 350,560 |
| Go | 53.646 ms | 10.224 ms | 1,628 | 1,228,952 |

Direct compilation remained 1.32x slower than C compilation and its execution
median was 53.58% lower on this fixed checksum kernel. C99 compilation remained
2.23x slower than C and C99 execution was 6.67% slower in this rerun. An earlier
sample reversed the small C99/C runtime ordering, so no winner claim is made.

## Verification

- `./target/release/pblang build .`: passed.
- Focused cache, C99 modulo, codegen, and interpreter semantic tests: passed.
- `bash bench/pblang_phase_suite.sh`: every reported phase suite passed.
- `cargo test --release --test semantics -- --test-threads=8`: 203 passed,
  0 failed, 1 documented security ignore.
- `./target/release/pblang test tests/semantics --allow-all --jobs 8 --quiet`:
  695 passed, 0 failed, 0 ignored.
- Reproducible core matrix: 22 ok, 2 unavailable, 0 failed.

## Remaining work

- Cold direct compilation still pays the large module-import cost. Removing it
  requires a validated PBLang module/bytecode cache or self-hosted loader path.
- The broad host VM numeric loop remains separate and was not changed because
  this slice introduced no Rust code.
- Direct cached artifacts currently target Linux x86-64 only.
- Freestanding C99 does not use the hosted checked-modulo helper.
- Zig was not installed, so its two rows remain unavailable.

