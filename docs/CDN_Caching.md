# CDN caching in PBLang

`stdlib:cdn_cache.sapp` is a pure-PBLang, provider-neutral HTTP cache and policy
layer. It implements the behavior required inside edge and regional cache
processes. A single local process is useful for development and origin
shielding, but does not by itself create geographically distributed Points of
Presence. Production geography requires deploying the handler at those nodes
or adapting the same contracts to a CDN provider.

## Cache hierarchy

The intended request order is:

1. **Browser cache.** `Cache-Control` and `ETag` let the client reuse content or
   issue a conditional request.
2. **Edge cache.** The bounded cache nearest the request serves fresh content.
3. **Regional cache/origin shield.** An edge miss checks the larger regional
   cache and promotes a hit back to edge.
4. **Application cache.** The origin may use `data_store.sapp`, Redis, or SQLite
   for computed/session/database results. This is separate from public HTTP
   response caching.
5. **Origin.** The authoritative handler runs only after all usable shared
   cache tiers miss or require revalidation.

## One-line static site

```pblang
let site = import("stdlib:cdn_cache.sapp").static_site("public");
```

`site.handle(request)` safely serves files through bounded edge and regional
caches. `site.purge_path(...)`, `site.purge_all()`, and `site.stats()` expose
invalidation and observability. `cdn.static_files("public")` returns middleware
compatible with the PBLang HTTP/web handler contract.

Static names containing a conservative 8-64 digit hexadecimal content hash,
such as `app.34f9a123.js`, automatically receive a one-year `immutable` policy.
Ordinary files use the configured short policy. Disable this with
`auto_immutable: false` or set `immutable_ttl_ms` explicitly.

## Configurable hierarchy

```pblang
let cdn = import("stdlib:cdn_cache.sapp");
let layer = cdn.create(map{
    allowed_hosts: ["example.com"],
    edge_max_entries: 4096,
    regional_max_entries: 16384,
    edge_max_units: 268435456,
    regional_max_units: 1073741824,
    max_object_units: 16777216,
    policy: map{
        scope: "public",
        browser_ttl_ms: 60000,
        shared_ttl_ms: 300000,
        stale_while_revalidate_ms: 30000,
        stale_if_error_ms: 300000
    }
});

let response = cdn.fetch(layer, request, origin_handler);
```

The module supports:

- deterministic `Cache-Control` generation and bounded parsing of `public`,
  `private`, `no-store`, `no-cache`, `max-age`, `s-maxage`, `immutable`,
  `must-revalidate`, `stale-while-revalidate`, and `stale-if-error`;
- SHA-256 cache keys over method, scheme, host, path, sorted query pairs, and
  configured `Vary` request headers;
- a bounded canonical-key memo that avoids recomputing pure-PBLang SHA-256 on
  hot requests while preserving SHA-256 as the final key;
- generated or origin-provided ETags and weak `If-None-Match` comparison for
  GET/HEAD `304 Not Modified` responses;
- edge hits, regional shielding and promotion, origin admission, stale serving,
  explicit revalidation, and stale-if-error fallback;
- exact-request, exact/prefix-path, tag/surrogate-key, and full-cache purge;
- entry, byte-unit, key, query, tag, object, and header bounds plus metrics.

## Stale-while-revalidate

The default `stale_mode: "serve"` returns stale content immediately with
`response.cdn.needs_revalidate: true` and a versioned revalidation token. A
server with a task queue can enqueue:

```pblang
cdn.revalidate(layer, request, origin_handler);
```

Single-threaded servers can select `stale_mode: "blocking"`; they refresh before
returning. This is an intentional fallback. The current minimal HTTP server does
not silently create a background network worker, because worker VM capabilities
and shared cache mutation must be controlled by the embedding host. The library
never reports background revalidation that did not actually occur.

## Security defaults

- Only GET and HEAD are eligible. A HEAD miss is not stored, preventing an empty
  HEAD body from poisoning a later GET; a HEAD hit may reuse GET metadata.
- Requests containing `Authorization`, `Cookie`, `Range`, or `If-Range` bypass
  shared storage by default. Sensitive bypass responses are forced to
  `Cache-Control: private, no-store`.
- Responses with `private`, `no-store`, `Set-Cookie`, an unsupported status, an
  oversized body, or a `Vary` dimension absent from the configured cache key are
  not admitted.
- Host allowlists prevent Host-header cache pollution when configured. Paths,
  hosts, headers, tags, queries, and objects are bounded and reject control
  characters.
- Hop-by-hop headers and origin `Content-Length` are removed. The HTTP serializer
  computes `Content-Length` from UTF-8 bytes and has the correct 304 status text.
- Purge operations scan only the already bounded edge and regional stores.

Provider purge endpoints still need authentication, authorization, rate limits,
and audit logs at the product/deployment boundary. TLS termination, DDoS
protection, anycast routing, cross-node replication, and global consistency are
infrastructure responsibilities, not claims made by this local library.

## Verification and benchmark

```sh
./target/release/pblang tests/semantics/cdn_cache_test.sapp
./target/release/pblang run examples/cdn_cache/main.sapp --allow-all
./target/release/pblang bench/cdn_cache_stages.sapp
```

On the recorded 2026-07-18 development-host run, bounded key memoization reduced
the edge-hit median from about `3.68 ms/op` to `0.108 ms/op` while retaining the
portable SHA-256 final key. The final nine-sample medians were `0.108 ms/op` for
edge hits, `0.129 ms/op` for regional hit plus promotion, `0.119 ms/op` for
conditional 304, and `1.114 ms/op` for origin miss plus normalization, hashing,
ETag generation, and two-tier storage. These are interpreter/local-process
microbenchmarks, not public-network latency or global CDN throughput claims.
