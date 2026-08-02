# Cold x86-64 Codegen Driver Report

Date: 2026-07-29

## Scope

This slice measures the direct Linux x86-64 driver separately from generated
artifact execution. It reduces the cost of loading the PBLang-written tokenizer,
parser, ELF writer and code generator in a fresh process. It does not present a
warm cache hit as cold compilation.

## Implementation

- `stdlib/source_inliner.sapp` now scans ASCII PBLang syntax through UTF-8 byte
  indexes instead of allocating one-character strings throughout
  compiler-sized inputs. Strings and comments remain byte-preserved.
- `tools/pbcodegen_cache.sapp` links the five driver modules once, emits a
  content-addressed PBCK, validates its container and requires it to generate a
  structurally valid x86-64 ELF before atomic publication.
- Cache v2 binds the source graph, linked source, compiler version and exact
  SHA-256 of the compiler executable. `sha256sum` is invoked with a fixed
  executable and argv, never through a shell; a bounded PBLang byte-hash
  fallback is retained.
- Warm lookup validates the dependency graph and PBCK digest without relinking.

No Rust source was added.

## Reproduction

```sh
./target/release/pblang tools/pbcodegen_cache.sapp --allow-all
./target/release/pblang bench/x64_compile_pipeline_stages.sapp --allow-all
```

The first pre-existing generic freeze attempt was bounded at 90 seconds and
produced no artifact. Before the byte scanner change, the dedicated linker was
also bounded at 120 seconds and produced no artifact.

## Results

Host compiler: `pblang 0.5.7`

| Phase | Median | p95 |
|---|---:|---:|
| Process startup | 1.783 ms | 1.975 ms |
| Source backend import | 40.259 ms | 43.588 ms |
| In-process machine-code emission | 1.624 ms | 1.797 ms |
| Full source-import publication | 43.804 ms | 47.179 ms |
| Verified warm artifact-cache publication | 9.826 ms | 10.419 ms |
| Precompiled-driver full publication | 7.169 ms | 7.349 ms |

The precompiled driver is **6.11x faster** than source-import publication,
reducing measured latency by **83.63%**. Peak RSS moved from **19,472 KiB** to
**12,720 KiB**, a **34.68%** reduction. The generated direct ELF remains
unchanged.

The verified PBCK driver is 716,463 bytes. Its cache key is
`d5fa6c1122545bb9dddfd39dea49c4cd7c7da8ea4c502fad96154c0cc798df79`;
its artifact SHA-256 is
`d4d5b8d2b738586b7d18f92b2f16e2acd93deabfef60ae6b7005f19b9ab8c9bc`.
A first v2 build took 1.12 seconds and 77,100 KiB peak RSS. A verified indexed
lookup took 0.10 seconds and 41,552 KiB peak RSS. This publication cost is
amortized by subsequent direct compilations.

## Limits

The driver consumes 716,463 bytes of cache storage. Cache construction still
uses the current host `emit-pbck` command as an explicit bootstrap boundary.
The interpreter and host loader remain outside this optimization. Results apply
to this host and fixture and do not establish that PBLang is universally faster
than other languages.
