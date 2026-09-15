# Data storage, caches, and Redis

PBLang has separate layers for transient caches, durable key/value data,
relational data, packed files, and external database tools. They are not aliases
for one another, and their guarantees should not be mixed.

## Canonical layers

| Need | Module | Current contract |
|---|---|---|
| Process-local cache | `stdlib:cache.sapp` | TTL, cached `nil`, hard entry/unit quotas, FIFO by default, opt-in LRU, O(1) lookup and size, metrics |
| Public HTTP/CDN cache | `stdlib:cdn_cache.sapp` | Bounded edge/regional hierarchy, Cache-Control, ETag/304, SWR, stale-if-error, promotion, purge, metrics |
| One key/value API | `stdlib:data_store.sapp` | Namespaced memory, SQLite, and Redis backends with versioned JSON value envelopes |
| Relational database | `stdlib:db.sapp` | Native capability-gated SQLite, prepared value parameters, transactions, savepoints, migrations |
| Relational convenience | `stdlib:orm.sapp` | CRUD, pagination, upsert, batch insert, transactions; developer-authored raw SQL remains trusted input |
| Redis protocol | `stdlib:redis.sapp` | Shell-free RESP2 client over bounded TCP or certificate-verified TLS, AUTH, SELECT, pipeline, SCAN |
| Packed file container | `stdlib:storage.sapp` | Per-entry compression, CRC32 integrity, bounds-checked load, traversal-safe unpack |
| External database CLI bridge | `stdlib:polyglot_db.sapp` | Bounded, shell-free compatibility bridge with typed connections, fixed executables, private stdin, redacted diagnostics, and fail-closed platform behavior; not a native driver or connection pool |
| pbAI derived cache | `stdlib/pbai_cache.sapp` inside pbAI | Persistent SHA-256 keyed prompt/training artifacts with metadata verification, TTL, quotas, staged publish, and recovery |

## Process-local lookup behavior

`cache.get` and `cache.get_entry` count hits/misses and refresh opt-in LRU order.
`cache.has` and `cache.peek` do neither; all four reject and remove elapsed entries.
A stored `nil` is present: use `has` or `get_entry().found` to distinguish it from
an absent key. Each `get_entry` result is a fresh map. Reads without an expiry
deadline avoid clock work; entries with a deadline still check it on every read.

[Local memoization example](../examples/cache_memoize.sapp) uses the existing
`get_or_set` callback to avoid recomputation. Its two-entry quota is chosen only
for the example. Cache units are caller-defined accounting, not deep RAM usage;
mutable values and cache objects do not gain cross-thread synchronization.

[Lookup measurements](CACHE_LOOKUP_VALIDATION_2026-09-15.md) record the earlier
optimization. The subsequent [self-hosted time fix](SELFHOST_TIME.md) enables
positive TTL, touch, keys and expiry sweeping on the tested self-hosted frontend.
The underlying time remains host wall time and is not monotonic; other backends
and deterministic/time-isolated execution require separate validation.

## Unified key/value API

The same operations work on all three `data_store` backends:

```pblang
let stores = import("stdlib:data_store.sapp");
let cache = stores.open(map{
    backend: "memory", namespace: "sessions",
    max_entries: 4096, max_units: 67108864
});

stores.set(cache, "user-42", map{ role: "developer" }, map{ ttl_ms: 60000 });
let entry = stores.get_entry(cache, "user-42");
stores.close(cache);
```

Use `backend: "sqlite", path: "app-cache.db"` for disk persistence. Use
`backend: "redis", host: "cache.example", port: 6380, tls: true` for Redis.
Redis credentials can be supplied as `username` and `password`; applications
should source secrets from their capability-controlled configuration rather than
hard-code them.

`get_entry` distinguishes a missing value from a cached `nil`. `clear` removes
only the selected namespace. `touch` changes the TTL, and `get_or_set` evaluates
its producer only after a miss. Memory defaults to FIFO because it has lower hit
overhead; select `policy: "lru"` only when recency eviction is worth that cost.

## Limits and security

- Memory and SQLite stores enforce per-store `max_entries`, `max_units`, and
  `max_value_units`. Admission failure does not partially replace a value.
- Redis requests and responses have hard byte, array, line, and nesting limits.
  Redis-wide memory limits and eviction policy are server configuration, not a
  guarantee a client library can provide. `data_store.stats()` says so instead
  of reporting a fake quota.
- Redis commands are RESP arrays. No key, value, host, or password is inserted
  into a shell command.
- Plain Redis TCP remains available for trusted local deployments. Use TLS for
  traffic outside that boundary. TLS uses the runtime's certificate validation.
- SQLite values are parameterized. Dynamic SQL identifiers must use
  `db.quote_ident`; raw ORM where/order/schema fragments are developer-authored
  SQL and must not be taken directly from untrusted users.
- `storage.sapp` and SQLite currently read or serialize complete values in
  memory. They are bounded by callers and store quotas, but they are not a
  streaming large-object API.
- TCP, SQLite, and Redis handles are explicitly closed. They cannot be shared
  blindly across PBLang VM threads.
- `polyglot_db` accepts a path only for SQLite. Network adapters require the
  typed maps returned by `postgres_conn`, `mysql_conn`, `mongodb_conn`, or
  `redis_conn`; legacy strings containing CLI flags are rejected.
- PostgreSQL, MySQL, and Redis passwords stay out of argv and returned plans.
  PostgreSQL and MySQL use random mode-0600 client credential files. After the
  bounded process, secret content is securely scrubbed and unlink is attempted;
  result metadata reports both outcomes. A result is withheld if neither scrub
  nor unlink succeeds. Authenticated Redis and
  MongoDB CLI paths fail closed because their available CLI contracts cannot
  provide the same no-argv/no-secret-environment guarantee; use the native
  `redis.sapp` RESP/TLS client for authenticated Redis.
- Database scripts use bounded private stdin. Redis commands are arrays such as
  `["SET", key, value]`, so shell metacharacters remain ordinary data. External
  adapters are available only where `process.run_fixed_bounded` is available;
  there is no raw process fallback.
- Migration discovery and reads use no-follow secure-file operations, enforce
  entry and byte ceilings without enumerating the directory. Reviewed filenames
  and caller-pinned SHA-256 hashes define the exact in-memory snapshot; execution
  runs in deterministic filename order and stops at the first failed migration.

## pbAI prompt and artifact cache

New pbAI cache entries use a domain-separated SHA-256 digest of kind plus input.
Reads verify the expected kind, key, and digest before returning the value.
Entries can use `ttl_ms`, `max_entries`, `max_units`, and `max_entry_units` via
`put_options`. Writes use a unique staged file and rename; when a platform
cannot replace an existing target by rename, the previous entry is retained as
a recoverable backup until publication succeeds.

Legacy v1 entries remain readable to avoid deleting existing derived data, but
`get_entry` reports `legacy: true, verified: false` because their old 31-bit key
cannot provide collision-resistant verification. Every new write uses v2 and
removes the corresponding legacy entry.

This cache is safe to clear. Checkpoints, weights, curated data, and live lessons
remain outside it.

## CDN versus application data cache

`cdn_cache.sapp` stores public HTTP responses by a normalized request key and
emits browser/shared cache policy. `data_store.sapp` stores application values
such as sessions, computed API data, and database results. Redis is normally an
application/regional data cache; it is not automatically a public CDN and must
not receive private response data under a public key.

The CDN request sequence is browser -> edge -> regional/origin shield -> origin.
The origin may then consult `data_store`/Redis/SQLite before querying its source
of truth. See `docs/CDN_Caching.md` for cacheability and invalidation rules.

## Verification

```sh
./target/release/pblang tests/semantics/cache_bounded_test.sapp --allow-all
./target/release/pblang tests/semantics/redis_resp_test.sapp --allow-all
./target/release/pblang tests/semantics/data_store_test.sapp --allow-all
./target/release/pblang examples/data_store/main.sapp --allow-all
./target/release/pblang bench/data_store_stages.sapp --allow-all
./target/release/pblang tests/semantics/cdn_cache_test.sapp
./target/release/pblang run examples/cdn_cache/main.sapp --allow-all
./target/release/pblang bench/cdn_cache_stages.sapp
./target/release/pblang run tests/semantics/polyglot_db_security_test.sapp --allow-all
./target/release/pblang run tests/semantics/polyglot_db_bounded_test.sapp --allow-all
./target/release/pblang run tests/semantics/polyglot_hardening_suite.sapp --allow-all
```

The Redis semantic test validates the complete RESP2 codec without a service.
Live connection behavior requires an explicitly provisioned Redis service and
is therefore an integration condition, not a fake passing unit test.
The bounded database smoke test performs a real in-memory query when a fixed
`sqlite3` executable is installed; otherwise it records only the portable
contract test and does not claim live CLI coverage.
