# Benchmarking PBLang

PBLang benchmarks are evidence for a named workload, host, toolchain, and run.
They are not a basis for claiming that one language wins every category.

## Reproducible competitor core

`stdlib:repro_benchmark.sapp` is the pure-PBLang competitor runner. It owns a
fixed registry for PBLang interpreter, hosted C99, direct Linux x86-64 codegen,
C, C++, Rust, Go, Java, C#/Mono, Python, JavaScript/Node.js, and Zig. It never
executes a caller-supplied command or arbitrary source string.

The initial workloads are deliberately small and auditable:

- `startup`: emit a fixed value and exit;
- `integer_kernel`: execute 1,000,000 equivalent modulo/arithmetic iterations
  and emit checksum `474518`.

Run the full matrix:

```sh
./target/release/pblang bench/reproducible_core_matrix.sapp --allow-all
```

The command reports environment and toolchain versions, explicit coverage for
all requested categories, cold compile/run latency, warm median and p95,
Linux peak RSS, CPU utilization, release artifact bytes, checksum, and all raw
warm samples. Uninstalled toolchains remain visible as `unavailable` rows.

The `pblang-codegen-x64` row is a direct PBLang path, not an alias for C99. Its
fixed PBLang driver imports `stdlib/native/codegen_x64.sapp`, requires successful compilation
with selected runtime checks, validates the ELF header, writes the artifact, and executes
that generated ELF. On unsupported hosts it reports `unavailable`; it never
silently substitutes another backend. Run that row as a standalone example:

```sh
./target/release/pblang examples/repro_codegen_benchmark/main.sapp --allow-all
```

## Compiler bottleneck phase benchmarks

Three pure-PBLang harnesses isolate the optimized surfaces from unrelated
process and toolchain cost:

```sh
./target/release/pblang bench/x64_compile_pipeline_stages.sapp --allow-all
./target/release/pblang bench/c99_modulo_stages.sapp --allow-all
./target/release/pblang bench/tree_interpreter_control_stages.sapp --allow-all
./target/release/pblang bench/x64_batch_compile_stages.sapp --allow-all
./target/release/pblang bench/selfhost_module_cache_stages.sapp --allow-all
```

The x86-64 harness reports startup, backend import, in-process emission,
complete uncached publication, real cache-cold publication, and verified
cache-warm publication separately. The content-addressed cache validates its
dependency key, metadata, digest, byte count, and ELF shape before reuse. A
warm hit is an incremental-build result and must not be compared with another
language's deliberately cold compiler invocation.

The batch harness compares six independent compiler processes with one process
that imports the PBLang-written x86-64 backend once. It reports total and
per-artifact medians for cold artifacts and verified warm artifacts. The module
cache harness separately compares repeated imported-module compilation,
verified cold serialization/publication, and verified warm PBCS loading. Its
main module and VM execution remain in every row, so the warm result is not a
bytecode-deserialization-only microbenchmark.

The C99 harness compares default floating-numeric lowering with the explicit
integer profile on the same integer kernel. Integer mode can expose literal
modulo to the C optimizer, but it is not semantically equivalent for programs
that require fractional values. Dynamic hosted modulo remains checked for zero
and the signed overflow case.

The interpreter harness includes a same-process generic-dispatch control. It
measures the PBLang-written tree interpreter in `stdlib/compiler/interp.sapp`,
not the existing host VM used by the broad `pblang-interpreter` matrix row.

## Protocol

1. Use the same algorithm, iteration count, expected output, and integer range.
2. Use release optimization and remove debug symbols where supported.
3. Record the first invocation separately; do not call it cache-cold unless
   caches were actually controlled.
4. Discard at least one runtime warm-up and retain multiple measured samples.
5. Reject a row on timeout, process failure, or checksum mismatch.
6. Preserve raw samples and report distributions, not only the best sample.
7. Record host, OS, CPU, logical CPUs, PBLang version, and every toolchain.
8. Keep compile, startup, execution, RSS, CPU, and size separate. Do not turn
   unlike units into a weighted marketing score.

The core matrix currently uses three warm compilation samples and seven warm
execution samples. `stdlib:benchmark.sapp` supplies deterministic median and
interpolated percentile calculations.

## Data-store phase benchmark

`bench/data_store_stages.sapp` isolates FIFO/LRU cache hits and the unified
memory/SQLite get/replace paths. Every fixture is filled first, warmed twice,
checked for its expected bounded size, then measured over nine samples with
median, p95, dispersion, and raw aggregate statistics. Redis is explicitly
conditional because a live server, ACL, persistence mode, and network topology
would otherwise make the row non-equivalent.

```sh
./target/release/pblang bench/data_store_stages.sapp --allow-all
```

Use this benchmark to locate local hot paths. Do not compare its in-process
memory operation count directly with a network Redis deployment or present it
as a database throughput claim.

## CDN cache phase benchmark

`bench/cdn_cache_stages.sapp` separately measures direct origin callback, edge
hit, regional hit plus promotion, conditional 304, and origin miss plus
two-level admission. Each phase warms twice, records nine samples, checks the
expected cache state on every operation, and reports median/p95/dispersion.

```sh
./target/release/pblang bench/cdn_cache_stages.sapp
```

The final 2026-07-18 local interpreter run measured medians of 0.108 ms/op for
edge hit, 0.129 ms/op for regional hit plus promotion, 0.119 ms/op for
conditional 304, and 1.114 ms/op for origin miss plus normalization, portable SHA-256/ETag,
and two-tier storage. The pre-optimization edge baseline was about 3.68 ms/op;
bounded canonical-key memoization removed repeated pure-PBLang SHA-256 work.
These are local control-layer microbenchmarks, not geographic latency,
multi-node throughput, provider pricing, or internet performance.

The same verification run measured the data-store phases at 3.60 us/op for FIFO
cache hit, 7.61 us/op for LRU hit, 9.29 us/op for the unified memory-store get,
and 23.89 us/op for in-memory SQLite get. Redis remained explicitly conditional
because no equivalent provisioned service was available.

## Product contract benchmark

`bench/product_contract_stages.sapp` measures the digest-bound release-evidence
pipeline and the renderer-neutral UI document pipeline independently:

```sh
./target/release/pblang bench/product_contract_stages.sapp
```

Every phase uses three warmups and nine retained samples, reports raw timings,
median and p95, and rejects a sample set whose correctness checksum changes.
The UI workload contains exactly 512 widgets, the current public contract
limit. Release-evidence results remain consistency checks only: benchmark
success is not trust verification and never authorizes publication.

## Interpretation limits

- PBLang C99 compilation is end-to-end transpilation plus C compilation. A
  direct C row measures a different pipeline boundary, so both are useful but
  not same-phase compiler measurements.
- Direct-codegen compilation includes PBLang process startup, module imports,
  selected runtime checks, ELF validation/write, and `chmod`. It currently targets
  Linux x86-64 and the source subset implemented by that backend. The phase
  harness above now complements this end-to-end measurement and keeps cold,
  uncached, and verified warm-cache boundaries explicit.
- Native binaries, VM bytecode, managed assemblies, interpreter source, and
  deployment bundles are different storage surfaces. Report the artifact kind.
- Peak RSS from short processes and CPU time rounded by an external tool are
  noisy. Repeat runs and inspect raw samples.
- A microbenchmark can locate a hot path, but an optimization is accepted only
  after focused correctness tests and broader semantic regressions.

## Coverage policy

The runner's `coverage()` ledger makes incompleteness explicit. A category may
be `runnable`, `partial`, `conditional`, or `not-comparable`, with a reason for
every non-runnable state. A category moves to `runnable` only when it has:

- equivalent inputs, outputs, lifecycle, and correctness checks;
- bounded setup and cleanup;
- cold/warm protocol and repeated samples;
- resource metrics appropriate to that category;
- at least one available PBLang path and one valid comparison path.

Network, JSON, web-server, and database comparisons must first define loopback
protocol/payload, parser semantics, server lifecycle, schema/durability, and
cache-state contracts. GPU rows additionally require the same kernel and result
on real registered hardware plus a CPU fallback; a planning simulation is not
a GPU performance result.

## Evidence

The verified 2026-07-18 baseline and full methodology are in
`bench/results/2026-07-18-reproducible-core-matrix/REPORT.md`; raw pipe-delimited
evidence is adjacent in `raw.txt`.

The subsequent compile/import, C99 modulo, and PBLang-written interpreter
dispatch work, including independent raw reruns and current limitations, is in
`bench/results/2026-07-18-compile-c99-interpreter/REPORT.md`.

The PBCS v2 hardening, persistent self-hosted module cache, x86-64 multi-file
compile session, raw samples, and completion evidence are in
`bench/results/2026-07-19-compiler-caches/REPORT.md`.
