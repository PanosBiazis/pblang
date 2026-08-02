# Host PBCK frame scalar benchmark

Date: 2026-08-02

## Scope

This PBLang-only benchmark compares the new prepared flat scalar executor with
the generic recursive PBCK VM. Both engines execute the same 21-instruction
program, sum integers 1 through 100, and must return the exact checksum `5050`
before timing starts. It does not compare process startup, artifact decoding,
platform adapters, calls, collections, Android execution or other languages.

Command:

```sh
./target/release/pblang bench/host_pbck_frame_scalar_stages.sapp
```

Each stage uses 9 samples. Preparation uses 500 iterations per sample; execution
uses 100 iterations per sample. The reported p95 is the slowest of nine samples,
matching the existing small PBLang benchmark convention.

## Results

| Stage | Iterations/sample | Median/sample | p95/sample | Median/run |
|---|---:|---:|---:|---:|
| prepare | 500 | 64.505 ms | 103.579 ms | 0.129010 ms |
| prepared flat run | 100 | 285.834 ms | 457.976 ms | 2.858345 ms |
| generic VM run | 100 | 404.753 ms | 634.713 ms | 4.047527 ms |

The prepared scalar median is 29.38% lower than the generic VM median for this
closed scalar/local/control-flow workload. This is not a claim that it is faster
for unsupported PBCK operations or on Android. The accepted optimization replaced
per-instruction linear opcode membership scans with a fixed 48-entry dispatch
mask; semantic parity and negative resource tests pass after the change.
