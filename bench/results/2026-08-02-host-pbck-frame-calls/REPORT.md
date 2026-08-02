# Host PBCK flat call-frame benchmark

Date: 2026-08-02

## Scope

This benchmark compares the prepared flat call-frame executor with the generic
PBLang-native PBCK VM. Both execute the same 21-step compiler-shaped program:
local declarations occupy frame stack slots, one closure captures a mutable
integer cell, receives two calls, and the second call uses the tail-call
trampoline. A correctness gate requires both engines to return 42 before timing.

## Environment

- OS: Linux 7.0.0-28-generic x86_64
- CPU: Intel Core i7-1195G7, 4 cores / 8 threads
- PBLang: 0.5.7 (`target/release/pblang`)
- Runtime artifact size: 33,226,888 bytes
- Samples: 9
- Iterations per sample: 500
- Warm state: the PBFP plan and prepared program are built before timing

## Results

| Engine | Median batch | p95 batch | Median per run |
| --- | ---: | ---: | ---: |
| Prepared flat call frames | 62.132 ms | 64.933 ms | 0.124265 ms |
| Generic PBCK VM | 51.645 ms | 57.917 ms | 0.103290 ms |

The prepared flat executor is 20.31% slower than the generic VM on this small
workload. This result is not presented as a speed win. The remaining overhead is
mainly the portable prepared-program shape checks and flat-table/cell dispatch.
It is the next optimization target after the Android generator adopts the shared
contract and produces representative whole-program workloads.

## Reproduction

```sh
./target/release/pblang bench/host_pbck_frame_calls_stages.sapp
```

The benchmark does not measure Android JVM execution, host adapters, startup,
RSS, package size, network transport, graphics, or the complete PBRemote app.
