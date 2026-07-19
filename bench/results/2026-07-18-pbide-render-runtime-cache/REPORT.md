# PBIDE render-runtime cache benchmark

Date: 2026-07-18

## Scope

This headless benchmark measures the PBLang-side adaptive render planning work.
It does not open a window and does not claim whole-IDE FPS, GPU throughput, input
latency, or framebuffer memory improvements.

The correctness gate first verifies that the direct quality/runtime path chooses
the same throttle, idle cadence, draw gap, and input grace as the full frame
planner. It then models 1,200 event-loop ticks (20 seconds at 60 Hz), with a new
completed draw sample every ten ticks:

- `legacy_plan_per_tick`: builds the full dirty/batch/device/frame plan each tick.
- `cached_runtime_policy`: runs the bounded cache decision each tick and refreshes
  the equivalent direct runtime policy at most once per second.

## Command

```bash
for run in 1 2 3 4 5 6 7; do
  taskset -c 3 ./target/release/pblang run \
    bench/pbide_render_runtime_cache.sapp --allow-all
done
```

## Environment

- PBLang: `0.5.7` (`target/release/pblang`)
- OS: Ubuntu 24.04, Linux `6.17.0-40-generic`, x86-64
- CPU: Intel Core i7-1195G7, 4 cores / 8 threads
- Affinity: logical CPU 3
- Samples: 7 fresh processes, 1,200 ticks per phase
- CPU frequency scaling and unrelated host activity were not disabled.

## Results

| Phase | Median total (ms) | p95 total (ms) | Median ms/tick | p95 ms/tick |
|---|---:|---:|---:|---:|
| Full frame plan per tick | 979.713 | 1007.773 | 0.816428 | 0.839811 |
| Cached runtime policy | 10.336 | 10.536 | 0.008614 | 0.008780 |

The local median planning cost was `94.8x` lower (`98.95%` reduction). Each
candidate sample performed 20 runtime refreshes and avoided 1,180 full-plan
constructions. The retained cache is one versioned policy map; peak process RSS
was not used to claim a memory reduction because interpreter/runtime startup
dominates this short microbenchmark.

## Raw totals

```text
legacy_ms: 982.279 972.110 948.068 965.245 984.763 1007.773 979.713
cached_ms: 10.452 10.255 9.679 10.493 10.536 10.120 10.336
refreshes: 20 20 20 20 20 20 20
saved_full_plans: 1180 1180 1180 1180 1180 1180 1180
```

## Interpretation

The result supports moving adaptive runtime planning out of PBIDE's idle hot
path. It does not prove that PBIDE is faster than another IDE, that SDL2 always
beats minifb, or that GPU presentation accelerates CPU-side UI rasterization.

## Regression verification

```text
./target/release/pblang test tests/semantics --allow-all --jobs 8 --quiet
test result: ok. 683 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
elapsed=309.63 s; max_rss=1,761,296 KiB
```

The peak RSS is for the complete parallel semantic test runner and is not used
as a PBIDE process-memory claim.
