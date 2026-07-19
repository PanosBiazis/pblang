# Hardware cost routing benchmark

Date: 2026-07-18

This is a local, correctness-gated subsystem benchmark. It compares PBLang's
cost-aware route with an intentionally forced parallel route for the same
128-element `x + 1` kernel. It is not a cross-language or whole-compiler claim.

## Environment

- PBLang: 0.5.7 (`target/release/pblang`)
- OS: Linux 6.17.0-40-generic x86_64
- CPU: Intel Core i7-1195G7, 4 cores / 8 threads
- Maximum CPU frequency reported by `lscpu`: 5.0 GHz

## Command

```sh
/usr/bin/time -f 'BENCH_PROCESS elapsed=%e user=%U system=%S max_rss_kib=%M exit=%x' \
  ./target/release/pblang bench/hardware_cost_routing_stages.sapp --allow-all
```

Each execution row contains nine same-process samples after one warm-up per
route. Sample order alternates each round to reduce systematic cache, frequency,
and ordering bias. The benchmark verifies result length and endpoint values
before reporting. p95 uses the nearest-rank ninth sample.

## Results

| Workload | Median | p95 | Minimum |
|---|---:|---:|---:|
| Cost-aware generic (serial) | 42.3972 ms | 52.8840 ms | 36.5759 ms |
| Forced parallel (4 workers) | 43.5327 ms | 53.4041 ms | 35.3755 ms |

The cost-aware route reduced median time by about 2.6% for this cheap kernel.
The forced-parallel minimum was lower, so the result is specifically a median
and p95 observation, not a claim that every sample is faster. The complete
benchmark process took 3.15 seconds, 1.49 user seconds, 1.64 system seconds, and
64,064 KiB peak RSS.

The route-only phases (20,000 correctness-checked decisions each) measured:

| Route | Total | Per decision |
|---|---:|---:|
| Cheap CPU serial | 340.0234 ms | 0.01700 ms |
| Heavy CPU parallel | 343.5198 ms | 0.01718 ms |
| Tiny GPU rejection | 656.1455 ms | 0.03281 ms |

These values include VM and scheduler overhead and may vary with CPU frequency,
system load, and kernel/runtime changes. Re-run the command on the target
machine before tuning policy hints.
