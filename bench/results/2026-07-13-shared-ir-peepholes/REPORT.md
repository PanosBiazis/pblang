# pblang shared register-IR optimization report

The pure-pblang register-IR optimizer now removes self-register copies, folds
adjacent `seti` + `addi`, resolves an immediately known structured `ifnz`, and
recurses through loop/branch bodies. Validation runs before optimization.

`syscall` is now explicitly non-foldable. This closes a correctness gap where a
loop-free effectful program could be sent to the compile-time reference evaluator,
which neither models nor may execute external effects.

## Release results

Five fresh processes ran `bench/codegen_ir_optimizer_stages.sapp`; timings are
medians. Artifact comparisons use one semantically identical structured kernel.

| Metric | Before | After | Change |
|---|---:|---:|---:|
| IR instructions | 6 | 5 | -16.7% |
| x86-64 ELF | 182 B | 163 B | -10.4% |
| i386 ELF | 136 B | 121 B | -11.0% |
| ARM64 ELF | 180 B | 160 B | -11.1% |
| Optimizer latency | - | 0.0365 ms | median |
| x86-64 codegen | - | 0.1301 ms | median |
| i386 codegen | - | 0.1032 ms | median |
| ARM64 codegen | - | 0.1379 ms | median |

Reference evaluation and all three backend artifact tests agree after the
rewrite. This improves the shared classical path; emulator conformance models
remain separately verified and are not represented as complete production
backends.
