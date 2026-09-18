# Retain and select implementations

`stdlib:implementation_switch.sapp` lets a program retain old source while
explicitly choosing a replacement. It contains only PBLang and performs no
imports, environment reads, native loading, process creation or network access.

```pblang
let Switch = import("stdlib:implementation_switch.sapp");
let service = Switch.open(true, "pblang", map{
    pblang: fn() { return make_replacement(); },
    polyglot: fn() { return make_retained_backend(); }
});
```

The constructors above are supplied by the application. For a runnable example,
see [implementation_selection.sapp](../examples/implementation_selection.sapp).
Changing the boolean to `false` refuses construction before inspecting the
registry. Changing the name explicitly restores an available old constructor.
Neither operation requires deleting either implementation.

## Contract

- `open(enabled, selected, factories)` validates selection and invokes exactly
  one zero-argument constructor. It returns its result unchanged, including
  `nil`, `false`, zero or empty values. Exceptions propagate; there is no retry
  or fallback after an uncertain effect.
- `bind(enabled, selected, factories)` performs the same validation and returns
  the callable without invoking it. The caller can retain it for repeated calls
  with its original arguments. Subsequent replacement/removal of a registry
  entry does not change the bound callable. Its own captured state can still
  change. Each call is explicit; this is not caching or automatic retry.
- `enabled` must be a boolean. Names contain 1–64 ASCII lowercase letters,
  digits or underscores. The registry must be a map and the selected value a
  runtime function, closure or builtin. Only the selected entry is inspected.
  These bounds concern implementation names, not program/source size.
- Invalid/disabled selections throw fixed errors without echoing supplied names
  or registry contents. A selected constructor controls its own error messages.

## Integration and limits

Call factories lazily. Do not evaluate a foreign loader while constructing the
registry. Keep foreign imports/loads inside the selected factory where the
loader supports lazy imports. Compiler/module preprocessing may still discover
or load dependencies: this library does not change those mechanisms.

This is **runtime selection**, not conditional compilation, binary dead-code
elimination, revocation, authorization or a sandbox. Retained source must still
parse. Top-level code and effects before selection are not disabled. Existing
objects and previously bound callables remain usable after configuration changes;
stop/close them through their explicit lifecycle before switching live services.
Runtime capabilities, permission checks and isolation remain required.

Active selection is tested on the retained PBLang runtime. The current
self-hosted frontend exposes functions/closures as ordinary maps through `type`;
this module rejects them instead of treating user-constructible map fields as
proof of callability. Its self-hosted regression test verifies rejection and zero
constructor calls, **not active-backend compatibility**. Other compiler backends
and operating systems are not verified by these tests.

PB-MCP retains its existing local selector and pinned releases. Migrating those
release dependency manifests to this standard-library module is a separate,
explicit change. This module alone does not replace GTK, TLS, codecs or the
other foreign libraries in PB-MCP's dependency inventory.

## Validation and cost

The semantic suite covers deferred construction, explicit restoration, captured
selection, malformed inputs, identifier boundaries, inactive throwing factories,
failure without fallback, false-like results and argument/builtin preservation.
The self-hosted boundary suite records the unsupported callable representation.

[Benchmark](../bench/implementation_selection.sapp) compares 10,000 identical
constructor results with a fixed registry. `open` revalidates each call; `bound`
validates once; `direct` is a plain callable reference. This comparison only
applies when the application intentionally keeps the selection fixed. A bound
callable does not notice later policy changes. Measurements are reported
separately; neither zero overhead nor a general language speedup is promised.

On the tested Linux x86-64 retained runtime, three warmup rounds and seven
rotating sample rounds produced these whole-process observations:

| Mode | Median CPU ms | p95 CPU ms | Median peak RSS KiB |
| --- | ---: | ---: | ---: |
| Revalidate with `open` each call | 48.106 | 49.913 | 18528 |
| `bind` once, call repeatedly | 16.451 | 16.956 | 18420 |
| Direct callable | 16.406 | 17.980 | 18432 |

The bound/direct difference is within the observed variation. This does not
establish zero overhead, reduced application RAM, or superiority to the older
PB-MCP selector. Whole-process startup is included. The p95 uses nearest rank;
with seven samples this is the maximum. [Raw measurements and source hashes](IMPLEMENTATION_SELECTION_MEASUREMENTS.json)
include wall time and every sample. Reproduce each mode with
`PBLANG_SELECTION_BENCH_MODE=bound pblang run bench/implementation_selection.sapp`
(substitute `open`/`direct`), using the warmups, order and process accounting
described in the JSON. These are strategy comparisons, not before/after release
benchmarks.

Validation ran in a copied workspace with a refreshed portable dependency lock;
the working-tree lock was preserved. Both new suites, the existing PB-MCP switch
suite, the runnable example, build and touched-file lint passed. Lint reports
an unused-variable warning for `ch`, although the subsequent predicate reads it.
The self-hosted suite is deliberately a refusal test as explained above.
Review found no OS/network/credential operations or fallback path in the new
module. This bounded review is not a whole-runtime security certification.
