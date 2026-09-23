# Windows ABI byte validation and release results

The historical measurements below describe the initial correctness fix.
See [batch-write optimization](Windows_ABI_batch_performance.md) for the
subsequent implementation, new measurements and remaining limits.

## Behavior

`windows_native_abi.write_bytes` validates every input element as an integral
number in 0–255 before its first native write. Previously, packing four values
into one word could conceal invalid elements: `[-1, 1, 0, 0]` became 255 and
`[256, 0, 0, 0]` became 256. An invalid suffix could also follow a successfully
written prefix. Malformed input now returns false without a native call or
destination modification. Existing owned-range checks and four-byte batching
remain; no second complete buffer or new input-size limit was introduced.

`send_input_records` uses the same validation before loading the input library.
The tests exercise rejected records only; no real desktop input is generated.

`free` interprets the numeric Win32 BOOL returned by the retained typed FFI.
Previously, a successful numeric result removed allocation ownership but was
reported as false because it was compared directly with a PBLang boolean.
Successful numeric results now return true and remove ownership; zero,
malformed results and exceptions retain ownership and return false. Repeating
a successful release cannot issue another native free through this state.
See Microsoft's [HeapFree return contract](https://learn.microsoft.com/en-us/windows/win32/api/heapapi/nf-heapapi-heapfree).

## Validation, 2026-09-23

Baseline: `e4eac5e0a7784d108cf773619c880d3fc349c1cd`. The retained Linux/Windows
executables and generated Win64 bridge were unchanged; only PBLang source and
tests changed. Exact artifact digests and results are in the
[evidence record](evidence/windows-abi-bytes-2026-09-23.json).

- The baseline fails the malformed-byte regression. The candidate passes 95
  malformed buffers, 18 valid lengths with surrounding canaries, 10 invalid
  input records, and 10 native release-result cases using a recording mock.
- All 23 selected Linux suites passed, including FFI contracts, controlled
  execution, resource budgets, device routing, GC, emulator and portable-lock
  regressions. Five touched PBLang files passed lint (with existing/fixture
  warnings). Isolated build and PBLang-native portable lock publication passed.
- On an authorized Windows workstation, the mock suite, actual heap/canary
  suite and existing `windows_dirfs_live_test` passed. The latter exercises
  read/write, exclusive creation, bounded reads, atomic rename, descriptor
  locking, traversal and junction rejection. It does not certify the generic
  Windows lockfile publisher.
- The actual heap test checks seven invalid buffers, valid unaligned writes,
  range rejection, successful free and rejected write after free.
- Graphify update was stopped after 35 seconds without completion.

The Windows test root was operator-prepared beneath the authorized executable
directory with a protected ACL for its owner, SYSTEM and Administrators. This
is test setup, not evidence that PBLang automatically verifies root/package
ACLs. No server, antivirus setting or desktop approval was changed.

## Measured cost

`bench/windows_abi_write_bytes.sapp` was identical for both source revisions.
Each workload used one excluded warmup per revision, then seven alternating
samples per revision on the same Windows machine. CPU and elapsed time include
process startup, imports, loading, writes, final byte parity and cleanup.
The older incorrect free return is ignored in both benchmark variants, but
allocation ownership removal and close are asserted.

The test harness `bench/windows_abi_write_bytes_measure.ps1` records the samples.
Set `PB_ABI_BENCH_ROOT` to a private, operator-prepared test directory containing
`before/pblang` and `after/pblang` source packages, the identical retained
`pblang.exe`, and `pblang-win64-abi.dll`. Place the identical benchmark script in
both packages, then run the harness in PowerShell. It writes `measurements.json`
inside that test directory. It is test infrastructure; product logic is PBLang.

| Workload | Median CPU before / after | Median elapsed before / after | Elapsed p95 before / after |
|---|---:|---:|---:|
| 64 bytes × 256 writes | 281.250 / 265.625 ms | 314.245 / 318.957 ms | 347.893 / 350.940 ms |
| 4096 bytes × 32 writes | 406.250 / 468.750 ms | 440.739 / 490.439 ms | 446.333 / 505.505 ms |

The larger workload costs **15.4% more median CPU** and **11.3% more elapsed
time**. The smaller CPU difference is one timing tick and is not evidence of
a general speedup. Nearest-rank p95 with seven samples is the sample maximum,
not a reliable tail-latency guarantee. Peak working-set readings were
unavailable, so there is no RAM-saving claim. Prior device-selector/VM/GC
regressions were not benchmarked away by this change.

## Remaining limits

Validation and writing assume sequential access to the input and owned state;
concurrent mutation or freeing requires synchronization not provided here.
Native failure while writing otherwise valid data can still leave a partial
write. Caller-controlled state is not an untrusted-code boundary. Pointer
precision, exact FFI widths, DLL identity/ACL verification and load-time
pathname races remain governed by the limitations in the
[FFI contract](FFI_contract.md).

Windows secure lockfile publication remains disabled pending automatic,
verified private-root/package handling and integration of its filesystem
backend into the portable publisher. No readiness flag was changed.
General cycle collection, long collection pauses, runtime/FFI/polyglot
integration and measured GPU/NPU execution remain separate outstanding work.
The installed PB-MCP was not updated; this change establishes no live-view
performance benefit and does not complete the 1.0 stable requirements.
