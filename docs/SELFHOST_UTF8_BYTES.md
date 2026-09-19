# UTF-8 conversion in the self-hosted VM

The self-hosted VM now recognizes and dispatches `str_to_bytes(text)`, including
direct calls, first-class aliases and explicit user shadowing. The compiler's
host builtin inventory already included this name, but the self-hosted runtime
subset did not; calling it previously failed instead of returning UTF-8 bytes.

The implementation and admission logic are PBLang. The final conversion still
uses the retained host's UTF-8 primitive, like other supported string builtins.
No new Rust source, foreign block or OS capability was added. This is not a
claim that the whole retained runtime has been replaced.

## Allocation and work admission

Normal `run` and `frontend_run` gain no new text-size or execution-time limit.
An ordinary conversion larger than 1 MiB is covered by the regression suite.

For explicitly bounded execution, `max_value_items` limits the number of bytes
in the resulting array. This is a per-value item count, **not** a hard limit on
total RAM in bytes: array representation, compiler state and other values also
consume memory. Kernel/driver/FFI isolation is unchanged.

The guard rejects a scalar count already above that budget. If four bytes per
scalar fit, conversion takes the direct path. Otherwise it counts exact UTF-8
widths before allocating the output byte array, accepting ASCII at the exact
budget rather than applying a pessimistic four-times limit. This preflight is
charged against the same sticky `max_steps` budget as the guest program; guest
try/catch cannot reset an exhausted budget.

Long strings are divided into smaller strings before scalar indexing. Leaves
contain at most 128 scalars. That is an algorithm threshold, not an input cap:
it avoids repeatedly indexing from the start of one very long UTF-8 string.
The preflight allocates substring temporaries, but not a full output byte array.
Incorrect argument counts and non-string values fail with content-free errors.

## Evidence

`tests/semantics/selfhost_utf8_bytes_test.sapp` compares retained/self-hosted UTF-8
bytes for empty, ASCII, Greek, combining, three-/four-byte and NUL text, plus a
long mixed string. It checks exact inclusive limits, one-byte overflow, aliases,
shadowing, malformed calls, sticky failure, scan-work accounting and an ordinary
conversion above 1 MiB.

23 focused suites pass: the new suite, existing frontend/codegen/optimizer/VM,
ownership libraries, Unicode encoding and five emulator suites. Two fixture
dependencies initially absent from the isolated staging tree were supplied and
the affected memory suites then passed. The matching baseline also failed with
the incomplete staging tree; no runtime change was made to hide those failures.

PB-MCP additionally verifies 96 existing service-core metadata responses through
the real self-hosted frontend, in addition to 96 provider-projection comparisons
and seven rejection cases. Inputs and transport flags are synthetic; no provider,
credential, personal desktop or production server was accessed.

## Resource measurement

`bench/selfhost_utf8_control.sapp` compiles and repeatedly runs an unchanged
`len`/`bit_shr` workload. It intentionally performs no UTF-8 conversions, to
measure possible effects on existing VM usage. Fresh Linux x86_64 processes,
two warmups and seven samples per mode, alternating order, compared the same
checkout with only `vm.sapp`/`vm_builtins.sapp` restored from baseline
`a87389fbd2665974f6454013cae0168859b67281`. Compilation and startup are included;
outputs must be identical. All guest OS effects except imports are denied.

| Measurement | Baseline median | Candidate median | Baseline p95 | Candidate p95 |
| --- | --- | --- | --- | --- |
| CPU ms | 1645.448 | 1710.167 | 1895.161 | 2458.646 |
| Wall ms | 1646.574 | 1712.488 | 1896.887 | 2461.419 |
| Peak RSS KiB | 24028 | 24216 | 24260 | 24340 |

The observed medians are +3.9% CPU and +0.8% peak RSS with timing variation.
No speedup or zero-overhead claim is supported. The new conversion had no working
self-hosted baseline, so its throughput is not presented as an optimization.
Raw samples, nearest-rank p95, exact workload and runtime/source digests are in
`bench/results/2026-09-20-selfhost-utf8/control.json`. No GPU/NPU/disk/network or
physical guest-OS performance is inferred from this CPU-only workload.

Error-preserving cleanup for every direct VM failure, composite ownership,
identity-safe callable representation, missing host builtins such as JSON parsing,
full native/FFI replacement and wider platform verification remain outstanding.
