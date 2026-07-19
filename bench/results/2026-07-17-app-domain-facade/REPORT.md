# Extended application facade benchmark

Date: 2026-07-17

This benchmark measures the warm PBLang application facade before and after
removing duplicate domain validation and per-validation profile-array
allocation. It is an internal before/after optimization benchmark, not a
cross-language performance claim.

## Environment

- OS: Ubuntu 24.04 family, Linux 6.17.0-40-generic, x86-64
- CPU: Intel Core i7-1195G7, 4 cores / 8 threads
- PBLang revision: `1f7da4e` plus the working-tree changes documented here
- Runner: `target/release/pblang`, 33,220,656 bytes
- Command: `/usr/bin/time -f 'APP_DOMAIN_PROCESS|elapsed_s=%e|max_rss_kb=%M|user_s=%U|sys_s=%S' ./target/release/pblang bench/app_domain_stages.sapp`
- Sampling: seven samples per stage; each sample performs the iteration count
  printed by the benchmark; reported latency is median and p95
- Workload policy: warm module cache, no real filesystem, database, socket, or
  provider I/O

## Results

| Stage | Before median/op | After median/op | Change |
|---|---:|---:|---:|
| Validate all seven domain profiles | 0.3190 ms | 0.2853 ms | -10.6% |
| Plan all seven profiles without execution | 0.6806 ms | 0.3296 ms | -51.6% |
| Hardened REST preview | 0.3105 ms | 0.3076 ms | -1.0% |
| Desktop GUI preview | 0.1384 ms | 0.1390 ms | +0.4% |
| Game action, 16 bounded steps | 0.1978 ms | 0.1516 ms | -23.4% |
| Invert a 16x16 RGBA image | 0.7451 ms | 0.6788 ms | -8.9% |
| Database SQL builder | 0.1151 ms | 0.0651 ms | -43.4% |
| Network request builder | 0.1227 ms | 0.0682 ms | -44.4% |
| Two-node automation workflow | 0.1692 ms | 0.1140 ms | -32.6% |
| AI cosine similarity, four values | 0.1156 ms | 0.0665 ms | -42.5% |
| ML linear regression, four rows | 0.1213 ms | 0.0690 ms | -43.2% |

Whole-process elapsed time fell from 11.91 s to 8.19 s (-31.2%). Peak RSS fell
from 207,088 KiB to 149,388 KiB (-27.9%). These process measurements include the
interpreter, imported libraries, benchmark data, and allocator behavior; they
must not be interpreted as the standalone memory footprint of the facade.

The desktop median changed by +0.4%, which is inside the observed timing noise;
no desktop optimization is claimed. Image p95 increased from 76.51 ms per
100-operation sample to 88.34 ms even though its median improved, so image-tail
latency remains a target for a separate profiling slice.

Raw outputs are preserved in `before.txt` and `after.txt`.
