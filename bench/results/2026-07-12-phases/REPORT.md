# pblang step-by-step phase baseline

Date: 2026-07-12 UTC. Host details are in `environment.txt`. Every row passed
its correctness gate. Imports are warmed before internal phase timing; process
startup and aggregate RSS are measured separately.

## Internal phases

| Phase | Average | Output | Primary metric |
|---|---:|---:|---:|
| Tokenize | 0.618 ms | 86 tokens | 330,345 source B/s |
| Parse | 0.612 ms | 4 statements | 140,423 tokens/s |
| Semantic analysis | 0.098 ms | 7 symbols | 40,798 statements/s |
| Type check | 0.031 ms | 0 diagnostics | 128,301 statements/s |
| Bytecode generation | 0.172 ms | 14 instructions | 23,223 statements/s |
| Safe bytecode optimization | 0.110 ms | 14 instructions | 126,960 instructions/s |
| Self-host VM `fib(12)` | 8.397 ms | 14 instructions | 119 runs/s |
| Tree interpreter `fib(18)` | 109.461 ms | 102 source B | 9.14 runs/s |
| Pure-pblang C99 transpile | 0.708 ms | 269 B C | 138,342 source B/s |
| Source to x86-64 | 1.685 ms | 425 B ELF | 49,261 source B/s |
| Register IR to x86-64 | 0.131 ms | 153 B ELF | 22,880 IR ops/s |
| Register IR to i386 | 0.117 ms | 112 B ELF | 25,629 IR ops/s |
| Register IR to ARM64 | 0.144 ms | 152 B ELF | 20,803 IR ops/s |
| Source to ARM64 | 1.331 ms | 424 B ELF | 73,647 source B/s |
| Source to WASM | 1.073 ms | 86 B module | 91,307 source B/s |

## Process and artifacts

| Target | Compile | Runtime RSS | Artifact |
|---|---:|---:|---:|
| Direct x86-64 | 1.685 ms internal average | 264 KB | 425 B |
| CLI C99 route | 218 ms end to end | 1,944 KB | 25,240 B |
| Entire warmed phase process | 1.25 s wall | 45,324 KB peak | n/a |

All ten emulator/model processes passed. Peak RSS spans 9,404-9,840 KB;
elapsed times were below `/usr/bin/time`'s 0.01 s resolution.

## Optimization order

1. Tree interpreter call/environment/expression dispatch.
2. Self-host VM call frames, stack representation, and opcode dispatch.
3. CLI C99 compile latency and runtime/artifact overhead.
4. Source-level x86-64/ARM64/WASM analysis and lowering passes.
5. Tokenizer/parser throughput on larger multi-file workloads.
6. Emulator instruction throughput with longer calibrated programs; current
   smoke programs are too short for meaningful elapsed-time ranking.

This report does not claim universal language superiority. These are diagnostic
fixtures, not replacements for applications, energy, parallel scaling, latency
distributions, or runtime-input competitor suites.
