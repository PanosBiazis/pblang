# Performance contracts

`stdlib/performance_budget.sapp` is the low-level fail-closed metric comparator.
`stdlib/product_performance.sapp` adds a versioned product contract for latency,
memory, artifact size, throughput, and caller-defined measured metrics.

```pblang
let perf = import("stdlib:product_performance.sapp");
let result = perf.resolve("my-product", map{
    baseline: map{ latency_ms: 10, memory_bytes: 1000 },
    candidate: map{ latency_ms: 9, memory_bytes: 1020 },
    budgets: [
        map{ metric: "latency_ms", max_regression_percent: 5 },
        map{ metric: "memory_bytes", max_regression_percent: 5 }
    ]
});
if not result.ok { throw perf.summary(result); }
```

The pblang, pbAI, pbOS, and pb-mcp-server health adapters accept this sample
shape. Omitting a sample runs only a deterministic, side-effect-free contract
self-check. Supplying a sample makes the performance check required; missing
metrics or a regression beyond budget blocks the product release gate.

Health collection never starts a benchmark, probes the network, trains a model,
or invents measurements. Benchmark runners own measurement; product health owns
validation and release policy.

Run the integrated pblang-only fixtures with:

```sh
target/release/pblang run bench/benchmark_quality_stages.sapp --allow-all
target/release/pblang run bench/ecosystem_vertical_stages.sapp --allow-all
target/release/pblang run bench/stdlib_hotpaths_stages.sapp --allow-all
target/release/pblang run bench/stdlib_pipeline_hotpaths_stages.sapp --allow-all
target/release/pblang run bench/stdlib_data_pipeline_hotpaths_stages.sapp --allow-all
target/release/pblang run bench/stdlib_safety_resource_stages.sapp --allow-all
target/release/pblang run bench/product_runtime_stages.sapp --allow-all
target/release/pblang run bench/cache_render_stages.sapp --allow-all
target/release/pblang run bench/platform_resource_stages.sapp --allow-all
./bench/pblang_phase_suite.sh
```

`stdlib_hotpaths_stages.sapp` correctness-gates and measures legacy versus
optimized pblang-only implementations for incremental strings, FIFO queues,
LRU caches, and HTML generation. The optimized contracts are linear chunked
string assembly, O(1) deque ends, and O(1) LRU get/put/eviction. Timings are
reported separately for each path; a single run is evidence for that machine,
not a universal performance claim.

`stdlib_pipeline_hotpaths_stages.sapp` extends the same correctness-gated
before/after method to RFC-compatible CSV serialization and unquoted parsing,
JSON Lines, large ASCII tables, and set union. The corresponding library
contracts are native split fast paths only when quotes are absent, linear
header discovery, native split/join quote escaping, hybrid small/direct versus
large/chunked ASCII tables, chunked JSON Lines/pretty output, and set algebra
over backing-map keys without temporary value arrays. Every phase emits
baseline and candidate timings separately.

`stdlib_data_pipeline_hotpaths_stages.sapp` correctness-gates stable keyed
deduplication, mutable map accumulation, DataFrame sorting, rolling windows,
joins, and pivots against their previous pblang algorithms. The optimized
contracts are map-backed membership, explicit in-place batch merge, stable
bottom-up merge sort, sliding sums with gap tracking, and first-pass hash
indexes that preserve legacy row ordering. Benchmarks remain machine-specific
evidence rather than universal performance claims.

`stdlib_safety_resource_stages.sapp` measures resource rejection rather than
unchecked happy-path work. It compares late rejection after all speculative
allocations with bounded arenas that stop at quota, and compares full
diagnostic ledger reservations with the boolean hot-loop API. Both routes are
correctness-gated and report normalized timings; results remain specific to the
machine and fixture.

`product_runtime_stages.sapp` correctness-gates the shared product admission
layer and compares detailed reservations with the boolean hot path. On the
local 2026-07-16 five-run sample (2,000 admit/release cycles per phase), median
times were about 300.82 ms detailed and 55.75 ms fast, approximately 5.4x for
this fixture. These are local measurements, not a cross-machine guarantee.

`cache_render_stages.sapp` measures legacy `len(map_keys(...))` size discovery
against the O(1) counter in `stdlib:cache.sapp`, FIFO churn under hard quotas,
and eager per-mark dirty-region coalescing against deferred frame consumption.
On the local 2026-07-16 run, 5,000 size reads took 14,369.07 ms versus 1.49 ms,
and 60 render fixtures took 804.72 ms eager versus 87.64 ms deferred (about
9.2x). These ratios are fixture- and machine-specific; capacity, eviction, and
render-equivalence assertions are the portable contracts.

`platform_resource_stages.sapp` compares sparse-memory `map_keys` materialization
with the platform emulator's O(1) resident-cell counter, measures quota-accounted
virtual-file writes, and verifies transactional rejection of a malformed byte
batch. On the local 2026-07-16 fixture, 2,000 resident-count reads took 6,023.42
ms through `map_keys` and 2.26 ms through the counter; 256 quota-accounted file
writes took 3.93 ms. It asserts count/byte quotas separately from timing, and
these machine-specific timings are not universal target claims.

The phase suite also verifies C99, direct x86-64, i386, ARM64, WASM, interpreter,
polyglot, app, OS/ABI, and ISA/device paths. Cross-language rows remain separate
because no microbenchmark establishes universal language superiority.
