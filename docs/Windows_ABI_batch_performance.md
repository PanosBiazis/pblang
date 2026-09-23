# Windows ABI validated batch writes

The byte preflight introduced in `bc83c6d` remains in place: every byte must be
an integral number in 0–255 before any destination write. The optimization
removes repeated checks of invariant values inside `write_bytes` only.
The exported scalar write functions retain their original validation.

## Implementation and retained checks

The pointer, initial offset and total length pass `_owned_range` before the
loop. The count and pointer lookup key are computed once. Each four-byte word
is an exact uint32 because its four components have already been validated.
The loop therefore calls the same fixed native write symbol without going
through scalar conversion/validation again. Tail bytes use the same approach.

Every native write still rechecks the mutable allocation registry and its
size, then `_call_bridge` checks closed state and catches bridge exceptions.
For the already validated nonnegative integer positions, checking that the
write end is within the current allocation is equivalent to the prior
offset/remaining-width comparisons. Missing or shrunken allocations stop the
next write. Invalid internal numeric metadata retains the existing comparison
error; it does not authorize another native call. This is not a concurrency
lock, a hostile-state sandbox, or a guarantee against a concurrent free.

No new buffer, native export, machine-code implementation, capability, input
limit or automatic retry is added. A failure after valid prefix writes still
leaves that prefix modified, as before. Invalid input bytes fail before the
first native call. Call count and emitted DLL bytes are unchanged.

## Tests

The real module is tested through a recording FFI replacement. In addition to
the existing 95 malformed buffers, 18 lengths, 10 rejected input records and
10 free-result cases, the suite checks all 256 byte values in packed and tail
positions, six injected native failures (return error or exception at each
write), and four state changes between writes (revoke, shrink, close, invalid
size). Both baseline and candidate pass the expanded behavioral tests.

The final candidate passed 23 selected Linux suites, three touched-file lint
checks, an isolated build and PBLang-native portable lock publication. On the
authorized Windows workstation, the mock suite, actual heap/canary suite and
filesystem live suite passed. Actual input injection was not exercised.

An initial test setup did not apply its intended protected ACL because the
remote PowerShell input was not submitted as a complete script block. The
post-setup check detected this. The ACL was applied and checked on the owned
test root, executable and DLL, artifact hashes were matched to local copies,
and the three Windows suites were rerun before measurements. This operator
setup is not automatic PBLang ACL verification. No AV policy or unrelated
directory permissions were changed.

## Performance method

The existing `bench/windows_abi_write_bytes.sapp` and measurement harness were
unchanged. Each comparison used one excluded warmup and seven alternating
samples per source version on the same Windows machine and retained executable.
The native DLL was identical. CPU includes startup, imports, load, writes,
readback and cleanup. Seven-sample nearest-rank p95 is the sample maximum,
not evidence of a guaranteed upper latency bound. Working-set measurements
were unavailable; there is no measured RAM-saving claim.

An initial candidate only removed the scalar wrapper and cached the loop
length. Its median CPU was unchanged in both workloads. The final candidate
also hoists invariant pointer/offset checks and pointer-key formatting while
retaining mutable allocation checks. All measurements, including the initial
candidate, are retained in the [evidence record](evidence/windows-abi-batch-2026-09-23.json).

| Workload | Median CPU baseline / final | Median elapsed baseline / final | Elapsed p95 baseline / final |
|---|---:|---:|---:|
| 64 bytes × 256 writes | 296.875 / 265.625 ms | 317.498 / 310.796 ms | 323.096 / 313.854 ms |
| 4096 bytes × 32 writes | 468.750 / 390.625 ms | 487.160 / 417.544 ms | 528.055 / 459.003 ms |

The larger workload uses 16.7% less median CPU than the validated baseline;
the smaller uses 10.5% less, with coarse CPU timing granularity. This does not
establish a general speedup or a bound on worst-case latency.

A separate alternating comparison used the pre-validation ABI source from
`e4eac5e`, with the same remaining package files and benchmark. The benchmark
supplies only known valid bytes; that historical source is not being restored.

| Workload | Median CPU legacy / final | Median elapsed legacy / final | Elapsed p95 legacy / final |
|---|---:|---:|---:|
| 64 bytes × 256 writes | 281.250 / 281.250 ms | 313.436 / 309.726 ms | 316.857 / 317.260 ms |
| 4096 bytes × 32 writes | 390.625 / 375.000 ms | 436.749 / 414.899 ms | 440.971 / 422.010 ms |

In this fresh comparison, the earlier validation overhead is covered: the
larger workload uses 4% less median CPU than the historical source and the
smaller has equal CPU. The smaller elapsed p95 worsens by about 0.13%; not
every measurement improves. These narrow results do not resolve unrelated
VM, GC or device-selector overheads.

## Outstanding work

Secure Windows lockfile publication remains disabled. `portable_lock.publish`
still needs a Windows dependency-root implementation and verified private
root/package handling; the live filesystem test does not satisfy those
requirements. No readiness flag was enabled.

This work does not complete general runtime cycle collection, reduce GC
pauses, implement remaining runtime/FFI/polyglot integration, or establish
GPU/NPU acceleration. It does not prove previous device-selector regressions
resolved. The installed PB-MCP was not changed, and no live-view improvement
was measured. Graphify update exceeded its 35-second bound and was terminated.
