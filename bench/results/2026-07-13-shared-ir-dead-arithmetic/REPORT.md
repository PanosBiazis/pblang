# Shared native IR dead-arithmetic elimination

The pure-pblang shared register-IR optimizer now removes `add`, `sub`, `mul`,
and `addi` instructions when backward liveness proves that their destination is
dead. These operations have no IR-visible side effects. Stack/local operations,
syscalls, branch conditions, loop counters, and loop-carried values remain
conservative barriers or dependencies.

## Method

Seven fresh release-runtime processes were pinned to logical CPU 3. Each process
optimized the same correctness-gated loop fixture 1,000 times and generated each
native artifact 500 times. Medians are reported. The baseline is the immediately
preceding optimizer, which already folded one adjacent `seti` + `addi` pair.

| Metric | Baseline | Final | Change |
|---|---:|---:|---:|
| Optimized loop-body instructions | 5 | 1 | **80.00% lower** |
| Optimizer average | 0.065963 ms | 0.057750 ms | **12.45% lower** |
| x86-64 ELF | 170 B | 153 B | **10.00% lower** |
| i386 ELF | 126 B | 111 B | **11.90% lower** |
| ARM64 ELF | 168 B | 152 B | **9.52% lower** |
| Portable C99 source | 220 B | 167 B | **24.09% lower** |
| x86-64 codegen average | 0.166851 ms | 0.134932 ms | **19.13% lower** |
| i386 codegen average | 0.133812 ms | 0.107735 ms | **19.49% lower** |
| ARM64 codegen average | 0.182050 ms | 0.146673 ms | **19.43% lower** |

The generated program result remains 21 in the portable evaluator. Regression
coverage also verifies that arithmetic feeding a later branch and values carried
between loop iterations are retained. These are fixture-specific local results,
not a universal cross-language performance claim.
