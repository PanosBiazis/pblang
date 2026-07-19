# pblang complete vertical slice - 2026-07-15

## Scope

This cycle is implemented in pblang. It covers the self-hosted tree interpreter,
one-action CLI/native-GUI/Web applications, shared product performance gates,
pblang/pbAI/pbOS/pb-mcp-server health integration, PBIDE example discovery,
polyglot policy, and the bounded platform emulator. No Rust source was changed.

## Accepted A/B changes

| Area | Controlled before | After | Result |
|---|---:|---:|---:|
| Interpreter plain loop, 10,000 iterations | 86.351 ms median with signal checks | 81.312 ms median with prepared signal-free path | 5.8% lower |
| Universal CLI+GUI+Web preview | about 34.0 ms/call with per-call template import | 0.387 ms/call with module-cached template | about 99% lower |
| 1 MiB virtual memory construction | 170.526 ms/call contiguous | 0.0264 ms/call sparse | about 6,460x faster for sparse workloads |

The interpreter analysis is conservative: any reachable return, break, or
continue retains the previous control-signal path. Sparse memory is logically
zero-filled, bounds checked, stores only nonzero cells, and automatically keeps
the previous contiguous representation for memories at or below 64 KiB.

## Cross-language category matrix

The correctly capability-enabled run produced 54 passing and 36 unavailable
rows across 15 declared runtimes and six categories. Nine runtimes were
installed. pblang's median and installed-runtime position were:

| Category | pblang median | Position |
|---|---:|---:|
| startup | 1.609 ms | 5 / 9 |
| recursion | 8.537 ms | 4 / 9 |
| integer loop | 28.973 ms | 5 / 9 |
| array | 12.647 ms | 5 / 9 |
| string | 2.905 ms | 4 / 9 |
| hash map | 7.033 ms | 4 / 9 |

This evidence does not support calling pblang the fastest language overall.
The next measured priorities are interpreter integer loops/arrays and startup;
the direct x86-64 target remains a distinct execution tier.

## Backend and integration evidence

`bench/pblang_phase_suite.sh /tmp/pblang-slice-phase-20260715` passed all gates.
Selected results:

- C99 hosted transpile: 0.575 ms average; size-profile artifact 14,464 bytes.
- direct x86-64 source codegen: 1.195 ms average; verified artifact 405 bytes,
  peak RSS 264 KiB.
- i386 IR codegen: 0.103 ms average.
- ARM64 IR/source codegen: 0.136 / 1.126 ms average.
- WASM source codegen: 0.851 ms average.
- universal app preview: 0.385 ms average inside the integrated ecosystem run.
- secure polyglot plan: 0.050 ms average.
- four-metric performance contract: 0.034 ms average.
- 16-target OS/ABI profile round, bounded memory/syscalls, and all ISA/device
  gates completed with exact checksums.

## Regression suites

- pblang semantics: 661 passed, 0 failed.
- pbAI semantics: 22 passed, 0 failed.
- pbOS tests: 17 passed, 0 failed.
- pb-mcp-server semantics: 9 passed, 0 failed.

The product health adapters now accept caller-measured performance samples.
A supplied regression is required and blocks release; an omitted sample performs
only a cheap contract-engine self-check and never fabricates benchmark data.
