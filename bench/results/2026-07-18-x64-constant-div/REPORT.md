# Direct x86-64 constant division optimization

Date: 2026-07-18

## Scope

This slice optimizes constant unsigned division and remainder in the
PBLang-written Linux x86-64 backend. It also fixes a full-suite reproducibility
failure discovered during validation: pbAI lazy training modified a locked
stdlib model file while tests were running.

This report covers one fixed integer microbenchmark. It does not claim that
PBLang is universally faster than any other language.

Host: Linux 7.0.0-28-generic x86-64, Intel Core i7-1195G7, 8 logical CPUs,
PBLang 0.5.7. The kernel performs 1,000,000 iterations and must emit checksum
`474518`.

## Implementation

- Conservative range facts prove only non-negative values below `2^32`.
- Constant `n / d` and `n % d` use an exact reciprocal multiply plus one
  correction when the proof applies.
- A proven `n < d` performs no reduction; a proven `n < 2*d` uses one compare
  and at most one subtraction.
- Power-of-two constants keep shift/mask lowering.
- Unknown parameters, negative/overflowing loop values, zero divisors, and
  unsupported constants use the original 64-bit `div` sequence.
- Branch and loop-carried writes invalidate facts unless the induction or
  modulo invariant is proved.

## Before and after

The same-turn pre-change direct artifact used two hardware divisions in every
iteration and measured 5.676 ms median at 487 bytes. The final 19-sample focused
run measured 1.924 ms and 507 bytes: 2.95x faster, a 66.11% latency reduction,
with a 20-byte (4.11%) artifact increase.

Times below are warm medians in milliseconds. RSS is median peak KiB.

| Language | Compile | Run | Run p95 | RSS | Artifact bytes | Checksum |
|---|---:|---:|---:|---:|---:|---:|
| PBLang direct x86-64 | 53.611 | 1.924 | 2.105 | 264 | 507 | 474518 |
| C / GCC | 26.899 | 3.988 | 4.154 | 1,496 | 14,472 | 474518 |
| C++ / G++ | 201.340 | 4.375 | 4.757 | 3,460 | 14,472 | 474518 |
| Rust | 46.014 | 4.311 | 4.409 | 1,996 | 350,560 | 474518 |
| Go | 22.113 | 4.797 | 4.915 | 1,500 | 1,228,952 | 474518 |

On this fixture, the generated PBLang executable was 2.07x faster than C,
2.27x faster than C++, 2.24x faster than Rust, and 2.49x faster than Go. It was
96.50% smaller than the C artifact and used 82.35% less measured peak RSS.
However, its complete warm compile path was 1.99x slower than C and 2.42x
slower than Go. Unlike the C-family compilers, the measured PBLang compile path
includes driver startup, PBLang module imports, safety certification, ELF
validation/write, and permission setup.

The refreshed broad matrix retained the result with fewer repetitions: direct
codegen measured 1.910 ms median, 1.998 ms p95, 264 KiB RSS, and 507 bytes. It
produced 22 correct rows, 2 explicit Zig-unavailable rows, and 0 failures.

## Correctness and stability

- `codegen_x64_const_div_test.sapp` executes correction boundaries, the
  one-million-iteration checksum, optimized opcode checks, and fallback cases.
- `codegen_x64_hardening_test.sapp`, `codegen_x64_compile_test.sapp`, and the
  reproducible benchmark contract pass.
- New `model_cache_test.sapp` rejects stale dimensions, versions, malformed
  rows, and malformed JSON, and verifies staged publication.
- New `pbai_model_cache_test.sapp` verifies classification and proves the
  bundled model remains byte-for-byte unchanged.
- Canonical semantic suite: 692 passed, 0 failed, 0 ignored, 90.06 seconds,
  1,885,584 KiB whole-runner peak RSS.
- `stdlib/pbai_model.json` retained SHA-256
  `2079a063835daa21d698a3ebf315b3b8b83961e21066619186a637f8a26a5447`
  and the same mtime before and after the parallel suite.

## Limits and next target

The optimized backend accepts a smaller language subset and emits Linux x86-64
ELF directly. The benchmark is modulo-heavy and therefore favors this specific
change; GUI, web, database, floating-point, allocation, and multicore workloads
can rank differently. Artifact comparisons also include different runtime and
linkage choices.

The next measured targets are the end-to-end direct compile/import overhead,
hosted C99 modulo lowering (15.810 ms here), and interpreter numeric/loop
dispatch (277.964 ms in the broad matrix). Each needs a separate phase profile
and correctness gate before changing code.

Exact samples and the refreshed matrix are in [`raw.txt`](raw.txt).
