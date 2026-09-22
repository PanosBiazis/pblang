# PBCS writer optimization: preserved contract, reduced repeated work

Baseline `4fa005655a7583c92f89ccf944d5c1b573efdfea`; additional comparison against
pre-arity-fix `125152fe86631fde54c773f785506cd345907e68`. Product implementation
and new fixtures are PBLang. The codec stays at v3; no function-arity metadata,
admission limit, reader validation or VM accounting was removed.

## Change and correctness argument

`_w_u32` still validates its input once as an integer in `0..4294967295`.
Its four existing shift/mask expressions produce bytes in `0..255`. Those
already-derived values now go directly to the output array, eliminating four
`_w_u8` calls, four additional `_uint` calls and four duplicate masks per u32.
No untrusted byte is admitted through that shortcut. The original writer
validation, all reader checks, byte order and artifact size remain unchanged.

This is synchronous value packing, not a new shared mutable cache or deferred
validation scheme. It adds no callback, lock, ownership transfer or foreign
boundary. It does not establish whole-codec race freedom or a complete bytecode
verifier. General mutable-input and host/runtime safety guarantees are unchanged.

## Checks

- **40 suites passed** in a clean archived checkout: serializer/loader/bundle,
  native translation, module cache, defaults, closure/try behavior, VM budgets,
  optimizer and five emulator suites. These are not guest-OS or physical-device
  certification.
- The new u32 suite checks **529 exact encodings** with an independent arithmetic
  quotient/remainder oracle, including byte carries, the signed boundary and
  the maximum unsigned value. It rejects **32 invalid inputs**: malformed writer
  operands and malformed reader bytes at every position of the operand field.
- **12 complete artifact fingerprints** agree between baseline v3 and optimized
  v3: plain/optimized source, defaults, captured closures, try/catch, Unicode,
  loops, large function/upvalue fields and legacy absent-minimum metadata.
- Existing minimum-arity regression tests remain green, including equality of
  native translation bytes before and after PBCS roundtrips.
- Build passed; its lock was normalized using `tools/pblock.sapp` in the clean
  test checkout. Lint passed with only two existing NaN self-comparison warnings.
  No warning was suppressed. Touched-file whitespace/privacy checks passed.

Reproduce the exact encoding test with:

```sh
target/release/pblang run tests/semantics/selfhost_pbck_u32_test.sapp --allow-all
```

Run `tests/fixtures/selfhost_pbcs_codec_corpus.sapp` on both v3 revisions and
compare all 12 output lines. They hash the serialized byte arrays without writing
artifacts. [Suite evidence](evidence/pbcs-writer-validation-2026-09-22.json) and
[artifact fingerprints](evidence/pbcs-writer-parity-2026-09-22.json) are retained.

## Same-workload measurements

The unchanged `bench/selfhost_pbcs_arity.sapp` runs 600 compile-once,
serialize/deserialize/execute iterations, checking the result each iteration.
Each side uses the same retained executable (hash recorded in evidence), two
warmups and seven measured processes per workload, rotating the three versions.
There were no simultaneous test/build/Graphify jobs during timing. CPU is
user+system process time; peak RSS includes the retained host and compilation.
Nearest-rank p95 is the maximum at seven samples, not a worst-case guarantee.

```sh
target/release/pblang run bench/selfhost_pbcs_arity.sapp --allow-all -- functions 600
target/release/pblang run bench/selfhost_pbcs_arity.sapp --allow-all -- plain 600
```

| Workload/version | CPU median / p95 seconds | Peak RSS median / p95 KiB | Artifact bytes |
| --- | --- | --- | --- |
| Functions, old v2 | 2.19 / 2.29 | 39,064 / 39,180 | 462 |
| Functions, baseline v3 | 2.25 / 2.28 | 39,568 / 39,844 | 477 |
| Functions, optimized v3 | 2.00 / 2.07 | 39,508 / 39,624 | 477 |
| Plain, old v2 | 1.11 / 1.37 | 36,504 / 36,816 | 109 |
| Plain, baseline v3 | 1.11 / 1.14 | 36,524 / 36,676 | 109 |
| Plain, optimized v3 | 1.06 / 1.13 | 36,592 / 36,784 | 109 |

Versus baseline v3 this is **11.1% less median CPU** with functions and **4.5%
less** in the plain control. Versus old v2, the function workload is **8.7%
less CPU**, while retaining the correctness fix. Thus the previous small CPU
regression is more than offset on these workloads, not universally.

RAM is effectively unchanged versus v3 (about −0.15% / +0.19%). Function-workload
RSS is still **about 1.14% above v2**. The earlier +1.8% RSS measurement is not a
constant for every run. This optimization does not remove the extra metadata,
prove general RAM savings or change the artifact's five-byte cost per function
with minimum arity. [All 42 samples](evidence/pbcs-writer-cost-2026-09-22.json)
include wall time and variation. No GPU/NPU, energy, whole-system resource or
live-streaming improvement is claimed.

## Integration and remaining work

The configured installed PB-MCP desktop bundle was checked read-only: it includes
`resource_scope.sapp` and the GTK viewer, but neither `selfhost_pbck.sapp` nor
`compiler/vm.sapp`. Its launcher still uses the retained runtime/OS boundary.
Replacing this absent codec in that bundle would not speed up the viewer.
The PB-MCP source and installed bundle were not changed or restarted, and saved
permissions were not modified. There was no Specisoft-server access.

Large collector pauses and the rooted-collector overhead remain open; kernel
collection is not the same as the retained desktop runtime. Scalar sibling
closures still need shared capture/lifetime semantics, including escaping
closures, exception paths and optimizer compatibility. General runtime/FFI and
polyglot integration and broader platform tests remain open. These require
separate implementations and acceptance tests; this writer optimization does
not complete them. The [1.0 release criteria](STABLE_1_0_READINESS.md) remain.

Graphify update reached its 35-second bound and was terminated. No semantic
`.sapp` graph coverage is claimed. The 212 pre-existing modified/untracked files
were checked by hash and preserved; only this slice's explicit paths were staged.
