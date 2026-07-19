# Controlled execution benchmark

Date: 2026-07-18

This report measures the PBLang-written policy/control plane added for bounded
parallel work, device routing, safe FFI decisions, and polyglot limits. Every
stage has a checksum or structural assertion; no foreign toolchain or real GPU
is invoked.

## Environment

- PBLang: 0.5.7, release binary
- Git base: `1f7da4e` with a dirty multi-slice worktree
- OS: Ubuntu 24.04-derived Linux, kernel 6.17.0-40-generic, x86-64
- CPU: Intel Core i7-1195G7, 4 cores / 8 threads
- Cache: 192 KiB L1d, 128 KiB L1i, 5 MiB L2, 12 MiB L3
- Timer: PBLang `now_ms()` for stages; `/usr/bin/time` for process wall/RSS
- Samples: seven fresh processes; p95 uses nearest-rank and is therefore the
  maximum with seven samples

Command:

```bash
for i in 1 2 3 4 5 6 7; do
  /usr/bin/time -f 'wall=%e rss_kib=%M' \
    ./target/release/pblang bench/controlled_execution_stages.sapp
done
```

## Controlled suite

| Stage | Work per process | Median ms | p95 ms | Median per operation |
|---|---:|---:|---:|---:|
| CPU route | 20,000 plans | 246.536 | 251.628 | 0.012327 ms |
| Unusable GPU to CPU fallback plan | 20,000 plans | 151.150 | 159.858 | 0.007557 ms |
| Allowed + denied FFI decision pair | 20,000 pairs | 442.472 | 457.759 | 0.022124 ms |
| Bounded polyglot report | 2,000 scans | 104.720 | 108.485 | 0.052360 ms |
| 128-item bounded four-worker map | 1 map | 38.599 | 57.933 | 38.599 ms/map |

Whole controlled-suite process wall time was 1.92 s median and 2.41 s p95.
Peak RSS was 58,888 KiB median and 59,244 KiB p95. Every run printed
`CONTROLLED_EXECUTION_SUITE|ok`.

## Existing polyglot benchmark

The existing `bench/polyglot_control_plane_stages.sapp` was also run seven
times after the change. Its process median was 0.80 s and median peak RSS was
17,452 KiB. The only pre-change same-turn baseline was one process (1.18 s,
17,148 KiB), so process-level deltas are too weak for a performance claim.

The directly comparable `policy_report` phase took 100.755 ms in that one
pre-change sample and 103.820 ms after the change at seven-process median,
about 3.0% slower. This is the measured cost of validating the versioned
config and retaining code/language limits; it is disclosed rather than hidden.
The post-change p95 was 106.276 ms. Registry, guard, and route checks remained
correct in all runs.

## Interpretation

These numbers measure policy decisions and PBLang VM thread orchestration on
one machine. They do not measure GPU/NPU throughput, distributed execution,
foreign-language runtime speed, FFI call latency, or compiler automatic
parallelization. Such claims require executable equivalent kernels, matching
inputs, warm-up rules, and target-specific measurements.

The final post-change canonical semantic run also passed 682/682 tests in
163.68 s with 1,698,940 KiB process peak RSS using eight isolated workers. That RSS is a
whole-suite concurrency result dominated by overlapping large fixtures, not the
memory cost of this benchmark or one worker.
