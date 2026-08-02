# Complete host-PBCK runtime benchmark

Date: 2026-08-01

## Scope

This PBLang-only warm benchmark executes a correctness-gated PBCK-v1 program
that uses the newly completed `pow`, `export`, safe attribute, safe index,
result unwrap and LIFO defer paths. It does not include process startup or
artifact decoding.

## Command

```sh
./target/release/pblang bench/host_pbck_full_runtime_stages.sapp
```

## Result

- Samples: 9
- Executions per sample: 1,000
- Median: 108.533 ms per sample
- p95: 113.079 ms per sample
- Median per execution: 0.108533 ms
- Correctness gate: result and export both equal 256; all 48 opcode IDs present

An earlier run while the semantic suite's Ollama model remained CPU-resident
measured 181.368 ms median. The runner was explicitly stopped and the reported
result above was then collected without PBLang, Graphify or test workers in the
background. This local measurement is not a cross-language performance claim.
