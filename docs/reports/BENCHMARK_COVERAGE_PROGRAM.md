# Cross-language, backend and polyglot benchmark program

## Requested scope

The user's target is coverage across programming languages, application
categories, operating systems, architectures and devices, including PBLang
native codegen, C99, interpreter and polyglot combinations. This is a continuing,
extensible coverage program, not a claim that an undefined set of all languages
or every possible combination has been implemented or measured.

The existing `repro_benchmark` registry has twelve language/toolchain entries
and twenty-two category records, many explicitly partial or not comparable.
The initial session driver selected six entries and two workloads; the current
driver additionally admits C++, Go and Rust from the existing fixture registry.
Its nine-entry whitelist is a bounded session selection, not the final scope.
See `BENCHMARK_SESSION_2026-09-08.md` for actual evidence. All other combinations
below are pending unless a linked report records execution and exact results.

## Coverage axes

| Axis | Required separation |
| --- | --- |
| Languages | Registered language, implementation, version, flags, dependency versions and historical/current toolchains; add fixtures without inventing results for unavailable tools. |
| PBLang execution | Retained-host interpreter, PBLang-written interpreter/VM where applicable, each direct native target, each C99 numeric/profile target; identify the compiler's own host separately. |
| Workloads | Startup/build, numeric algorithms, collections, parsing/text/JSON, memory/allocation, files/compression, networking, databases, concurrency, GUI/rendering, web/mobile, GPU/AI and FFI/mixed-language calls. |
| Platforms | OS version, architecture, actual device, emulator or VM, acceleration, resource limits; emulation is not physical-device verification. |
| Mixed execution | Language count, fragment count, runtime count, bridge/ABI, compilation and loading mode, call direction, marshaling, ownership, cache state and error propagation. |

Every claimed workload/backend cell needs the same input, output oracle and
observable work, including failure and cleanup behavior where relevant. Keep
unsupported, unimplemented, unavailable, timed-out, wrong-result and passed
cells distinct. A timeout/failure is evidence, not a timing win or a zero value.

## Polyglot and embedded fragments

Start with pairs (PBLang plus one foreign language), then PBLang with two or
more foreign languages, e.g. PBLang + Go + Rust. Exercise multiple fragments of
the same foreign language and fragments of different languages in one PBLang
program. Test sequential pipelines, nested calls/callbacks, fan-out/join where
supported, shared-data versus serialized transfer, repeated calls and boundary
failures. Pairwise coverage does not establish correctness of larger mixtures.

For each actually supported PBLang execution backend, compare:

1. Equivalent PBLang-only work.
2. Standalone foreign implementations.
3. PBLang-orchestrated mixed work with real compiled/loaded foreign artifacts.
4. Bridge-only and end-to-end cost, with cold compilation/loading separated
   from cached execution and with all transfer/serialization costs accounted for.

Contracts, parsers, lint success and placeholder adapters are not polyglot
execution benchmarks. Do not bypass existing execution/trust gates to fill a
cell. Record the missing implementation or verification instead. No new Rust
language-runtime development is implied by using an existing Rust competitor
fixture or by future separately reviewed mixed-language benchmark snippets.

## Non-programming languages and application frameworks

HTML/CSS, markup, styles, data formats and templates require a specified host
engine and operation: parse, validate, transform, layout/render or interact.
SQL requires the same engine/data/schema/durability contract. Compare identical
documents, assets, UI states and rendering/interaction outputs where feasible.
Do not rank markup against numerical machine code or count a static preview as
a complete application. Framework comparisons need functional parity and their
runtime/dependency footprint, not only executable file size.

## Measurement and reporting rules

- Preserve raw samples, exact toolchain/fixture identity, expected outputs,
  sample counts, warmups, order, resource limits, median/percentiles and noise.
- Separate source-to-artifact time, startup, first invocation, warmed work,
  steady-state JIT behavior and build/cache hits; do not label an ordinary
  first run as flushed-cache cold execution.
- Measure wall/CPU time, peak RSS, allocations where instrumentable, artifacts,
  source/generated code and total deployment dependencies separately. Extend to
  throughput, tail latency, I/O, network, GPU memory and energy only with an
  appropriate instrument and stated measurement resolution.
- Preserve failed rows and platform limitations. No unsupported extrapolation,
  selective best sample, changed workload to favor a backend or aggregate
  marketing score. Different numeric semantics need a qualified comparison.
- Run bounded sequential batches to control resource load. A session should
  include relevant benchmark slices, not rerun the entire Cartesian product.
  Full-category/device campaigns need separately available toolchains/devices
  and reviewed fixtures. Avoid waking devices or installing dependencies merely
  to convert an unavailable cell into a measured one without scoped approval.

The current data is local startup/integer microbenchmark evidence only. Native
and C99 GUI/mobile applications, actual mixed-language execution matrices,
historical/future language toolchains and the full platform/category program
remain incomplete. Benchmark success does not itself establish 1.0.0 readiness.

The second local batch is in `BENCHMARK_SESSION_2026-09-08_SUPER_OWNER.md`:
nine numeric-workload rows, including standalone C++, Go and Rust. These do
not establish embedded multi-language/polyglot execution or OOP performance.
