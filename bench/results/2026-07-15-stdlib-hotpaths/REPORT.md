# pblang standard-library hot paths

Date: 2026-07-15

This slice improves pblang-only library code while preserving the existing
public APIs. The benchmark runs each legacy and optimized algorithm in the same
release-runtime process and verifies equal results before timing.

## Changes

- `strings.sapp`: linear native joining, direct first-match replacement and
  chunked case/reverse/normalization helpers.
- `string_builder.sapp`: bounded incremental output, batch writes, finish,
  clear and take operations.
- `html.sapp`: literal escaping without regex compilation, chunked attrs,
  lists, tables, forms and selects, plus dangerous image-scheme rejection.
- `deque.sapp`: O(1) front/back operations with automatic migration of the
  previous array representation.
- `data_structures.sapp`: O(1) linked LRU get, put, touch and eviction, with
  size/remove/clear APIs and correct zero-capacity behavior.
- `cache.sapp`: cached-nil support, stale-TTL removal, exact expiry boundaries,
  `get_entry`, `get_or_set_ttl` and `touch`.
- `collections.sapp`: the compatibility queue no longer allocates and copies a
  second array on every dequeue; performance-sensitive callers can use deque.

## Measured result

Command:

```sh
./bench/pblang_phase_suite.sh /tmp/pblang-stdlib-phase
```

| Workload | Legacy avg ms | Optimized avg ms | Speedup |
|---|---:|---:|---:|
| Join 3,000 strings | 5.008 | 2.890 | 1.733x |
| Builder batch of 3,000 strings | 5.008 | 2.516 | 1.990x |
| FIFO enqueue/dequeue 3,000 values | 1701.401 | 16.110 | 105.614x |
| LRU 4,096 accesses, capacity 512 | 864.978 | 25.324 | 34.157x |
| HTML table, 1,280 cells | 20.107 | 9.004 | 2.233x |

The stdlib benchmark process used 18,612 KiB peak RSS and completed in 5.63 s.
These numbers characterize this machine and workload; they are not universal
claims about all applications.

## Verification

- All changed modules pass `pblang --check` and targeted semantics tests.
- `string_builder_test`, `deque_hotpath_test`, `cache_stdlib_test`,
  `data_structures_enrich_test`, `string_bytes_test`, both HTML enrichment
  tests, and `collections_test` pass.
- The complete phase suite exits 0, including C99, direct x86-64, i386, ARM64,
  WASM, interpreter, app, polyglot, ecosystem and platform-emulator gates.
- A parallel non-polyglot run passed through `stdlib_warning_sweep_test`, then
  its worker pool waited on a futex with no workers. To close the coverage gap,
  all 83 remaining semantic files from `storage_test` onward and all 9 polyglot
  semantic files were run directly with per-file timeouts and passed. The
  parallel aggregator deadlock is separate from the library results.
