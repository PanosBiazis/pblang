# PBLang low-memory LLM tiered-store benchmark

Date: 2026-07-25

## Reproduction

```sh
./target/release/pblang bench/llm_tiered_store_stages.sapp --allow-all
```

The benchmark creates 16 deterministic 16 KiB expert shards, records exact
length and SHA-256 metadata, and reports seven independent cold verified reads
and seven warm RAM-cache reads. Results are milliseconds.

## Host

- OS: Linux 7.0.0-28-generic x86_64
- CPU: Intel Core i7-1195G7, 4 cores / 8 threads
- PBLang executable: `target/release/pblang`

## Results

| Stage | Median | p95 |
| --- | ---: | ---: |
| Cold confined read + length/SHA-256 verification | 4.792 ms | 5.044 ms |
| Warm bounded RAM-cache hit | 0.087 ms | 0.093 ms |

Raw cold samples:

```text
4.996, 4.685, 4.440, 4.327, 5.047, 5.044, 4.792
```

Raw warm samples:

```text
0.088, 0.125, 0.082, 0.093, 0.087, 0.085, 0.085
```

These numbers measure the PBLang storage substrate, not token generation or
GPU inference. The current prefetch executor is synchronous.
