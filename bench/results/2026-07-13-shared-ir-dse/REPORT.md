# pblang structured register-IR dead-store elimination

The pure-pblang shared register IR now runs backward register liveness after
peephole optimization. It removes only dead, side-effect-free `seti` and `movr`
writes. Branches merge both live-in sets, loops converge over loop-carried
registers, syscalls retain every argument and overwrite `r0`, and stack/local
operations retain their ordering and register dependencies.

The added stateful regression also found and fixed a reference-evaluator bug:
numeric IR local slots are now normalized to string map keys for `lset/lget`.

## Incremental release result

These figures compare the accepted peephole optimizer with peephole + DSE. Five
fresh release processes were recorded; latency values are medians.

| Metric | Peepholes | Peepholes + DSE | Change |
|---|---:|---:|---:|
| IR instructions | 5 | 3 | -40.0% |
| x86-64 ELF | 163 B | 154 B | -5.5% |
| i386 ELF | 121 B | 113 B | -6.6% |
| ARM64 ELF | 160 B | 152 B | -5.0% |
| Optimizer latency | 0.0365 ms | 0.0515 ms | +0.0149 ms |
| x86-64 codegen | 0.1301 ms | 0.1229 ms | -5.5% |
| i386 codegen | 0.1032 ms | 0.0973 ms | -5.8% |
| ARM64 codegen | 0.1379 ms | 0.1246 ms | -9.6% |

Against the original unoptimized kernel, final artifacts are 15.4% smaller on
x86-64, 16.9% on i386, and 15.6% on ARM64. This is a structured four-register
analysis, not an unsafe whole-program assumption.
