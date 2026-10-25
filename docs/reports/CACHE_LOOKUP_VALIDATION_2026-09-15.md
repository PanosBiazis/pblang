# Cache lookup validation — 2026-09-15

Scope: remove temporary result maps from `get`, `has` and `peek`, and skip
clock/conversion work for entries without deadlines. `get_entry` still creates
an independent `{found,value}` map for each hit or miss. Baseline commit:
`05515fc7765ec89f18c72b99664039087fd91bf8`.

Presence remains independent of stored nil/false/zero/empty strings. Each public
operation calls the private lookup once; counting reads retain their previous
hit/miss and LRU behavior. Expired entries still release metadata and accounting
units, count expiry once, and cannot be returned. No new quota, compiler limit,
thread-safety guarantee or change to the existing wall-clock TTL policy was added.

## Reproduction and measurement boundaries

`bench/cache_lookup.sapp` arguments are `count repeats API TTL policy outcome`.
API is get/peek/has/get_entry, TTL is 0 or 600000 milliseconds, policy is fifo/lru,
and outcome is hit/miss. Example:

```sh
pblang run bench/cache_lookup.sapp --allow-all -- 16 20000 get 0 fifo hit
```

For paired runs, extract `stdlib/cache.sapp` from the baseline commit and copy the
current module into sibling scratch files; use two copies of this same benchmark
with equivalent relative module imports. Both variants use the same retained
executable SHA-256:
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.

One warmup per variant, seven measured samples, alternating order for every case.
`clock()` times the lookup loop, excluding cache preparation, expected-result
construction and output hashing. GNU time records whole-process user/system CPU
and peak RSS, including startup. p95 is nearest rank (maximum of seven samples).
All before/after results required matching value/statistics SHA-256 and exact hit,
miss, size and expiry expectations. Benchmarking ran serially, before the broad
tests/build/Graphify; unrelated host activity was not controlled. Python/GNU time
only orchestrated measurements; the benchmark and product code are PBLang.

[Raw samples and hashes](CACHE_LOOKUP_BENCHMARK_2026-09-15.json).

| API / policy / outcome | Entries / TTL ms | Before median / p95 ms | After median / p95 ms | Median time change | Before / after median RSS KiB |
| --- | --- | --- | --- | --- | --- |
| get / fifo / hit | 16 / 0 | 152.045 / 204.169 | 62.689 / 86.627 | -58.77% | 11892 / 10356 |
| get / fifo / hit | 16 / 600000 | 140.982 / 143.383 | 127.069 / 131.074 | -9.87% | 11864 / 10456 |
| peek / fifo / hit | 16 / 0 | 155.530 / 169.656 | 55.885 / 81.667 | -64.07% | 11920 / 10312 |
| peek / fifo / hit | 16 / 600000 | 278.738 / 828.062 | 249.815 / 482.100 | -10.38% | 12132 / 10240 |
| has / fifo / hit | 16 / 0 | 147.209 / 279.933 | 46.451 / 56.355 | -68.45% | 12016 / 10332 |
| has / fifo / hit | 16 / 600000 | 147.498 / 198.043 | 150.078 / 155.571 | +1.75% | 12000 / 10372 |
| get_entry / fifo / hit | 16 / 0 | 146.092 / 198.583 | 75.723 / 98.424 | -48.17% | 12068 / 12068 |
| get_entry / fifo / hit | 16 / 600000 | 154.992 / 163.639 | 160.634 / 166.254 | +3.64% | 12012 / 12088 |
| get / lru / hit | 16 / 0 | 280.625 / 296.960 | 170.697 / 215.734 | -39.17% | 14844 / 12444 |
| get / lru / hit | 16 / 600000 | 275.986 / 315.275 | 273.944 / 302.551 | -0.74% | 14568 / 12664 |
| get_entry / lru / hit | 16 / 0 | 255.854 / 261.201 | 174.510 / 182.227 | -31.79% | 14676 / 14828 |
| get_entry / lru / hit | 16 / 600000 | 260.341 / 263.054 | 269.316 / 272.158 | +3.45% | 14752 / 14716 |
| get / fifo / miss | 16 / 0 | 48.120 / 48.890 | 33.844 / 37.638 | -29.67% | 12072 / 10360 |
| has / fifo / miss | 16 / 0 | 44.895 / 48.627 | 29.156 / 31.530 | -35.06% | 12056 / 10384 |
| get_entry / fifo / miss | 16 / 0 | 49.580 / 51.391 | 48.930 / 52.362 | -1.31% | 11976 / 12012 |
| get / fifo / hit | 1 / 0 | 143.230 / 145.587 | 56.967 / 62.413 | -60.23% | 11924 / 10492 |
| get / fifo / hit | 4096 / 0 | 206.895 / 352.805 | 84.063 / 156.291 | -59.37% | 16308 / 14588 |

Permanent FIFO hits improved about 48–68%; LRU hits about 32–39%. FIFO get with
one and 4096 entries improved about 60% and 59%. Active-TTL results range from
about 10% faster to 4% slower: the extra expiry-map guard still costs work when a
deadline exists, and get_entry still allocates its public result. Misses improved
about 30% for get and 35% for has; get_entry misses were nearly unchanged.

Peak-RSS median changes ranged from -16.2% to +1.0%. These are
whole-process measurements, not allocation counters or live-heap measurements;
no general RAM-saving claim is established. The source removes intermediate
maps on three paths, but actual allocator impact is not measured. No improvement
to executable size, disk writes, GPU/NPU, full compiler throughput or general
runtime isolation was measured. There is no zero-overhead claim.

## Validation

39 selected suites passed with both exit status zero and their declared final
success marker: cache_stdlib, cache_bounded, cache_lookup, selfhost_cache_lookup,
cdn_cache, data_store, pbai_ensemble_cache, pbai_voter_cache, the previous 14
ORM/database/predicate/resource suites, and 17 emulator/input suites. The data
store test used synthetic local data, and CDN used a mock origin; no remote
server or AI service was used. The example `examples/cache_memoize.sapp` passed.

Five owned PBLang files passed via-PBLang check, lint and entry-only strict
pbsecure (15 gates; zero reported security findings). Lint retains existing
unused-variable warnings. An independent daybreak review found no blocker in
the cache diff. These are bounded checks, not a full security audit. Build and
portable-lock publication passed. Graphify update completed with 9,069 nodes;
its AST extraction still does not cover .sapp, so this is not validation of
PBLang source semantics. Generated graph output is not part of this commit.

## Confirmed self-hosted gap

The new self-hosted test verifies permanent get/get_entry/has/peek behavior,
cached nil/false, miss accounting and LRU eviction. It does not verify all cache
operations. With the same retained executable and unchanged frontend:

```pblang
let F=import("stdlib:compiler/selfhost_frontend.sapp");
let result=F.frontend_run("print now_ms();");
```

This throws `member access only supported on modules/maps`. The same failure
occurs when cache entries have a forced elapsed deadline and with positive-TTL
insertion, using both the baseline and changed cache module. This is pre-existing;
it was not reclassified as a passing TTL test. `keys()` and `evict_expired()`
also call the clock even when the expiry map is empty. Native TTL tests passed;
self-hosted clock/TTL parity remains open. Mutable cache objects/values are not
an isolation boundary, and shared/concurrent use needs a separate design.

## Remaining 1.0.0 work

Prioritize the self-hosted clock/TTL gap with backend parity tests; then complex
and shared-value lifetime management, native runtime/FFI and C99 allocation
correctness, authenticated audit/release evidence, PBIDE Run/Designer and actual
guest-OS validation. Entry quotas/accounting units do not measure deep live RAM.
This slice does not complete 1.0.0 or verify universal platform/device support.
No EnterpriseCore/Hetzner server was contacted or used for execution.
