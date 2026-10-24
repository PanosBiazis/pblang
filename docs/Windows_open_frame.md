# Windows filesystem call-local frame

## Implementation

`windows_dirfs._open_relative` now owns one zeroed 88-byte native allocation for
four simultaneously live Win64 records. Previously it allocated 16, 48, 16 and
8 bytes separately. The requested record payload remains 88 bytes; allocator
metadata and registry operations are reduced. UTF-8 and UTF-16 conversion buffers
remain separate. This changes six explicit allocations to three per open,
excluding the ABI call gate's own argument buffer.

| Record | Byte range | Alignment |
|---|---|---|
| UNICODE_STRING | 0–15 | 8 |
| OBJECT_ATTRIBUTES | 16–63 | 8 |
| IO_STATUS_BLOCK | 64–79 | 8 |
| returned HANDLE | 80–87 | 8 |

Checked memory operations still use the allocation base plus a checked offset.
Only the native call receives interior pointers. Reserved fields remain zero;
access, sharing, disposition, synchronous operation, reparse rejection and the
held-destination-parent rename protection are unchanged. The frame is local to
one invocation and never cached across calls or roots. It is released after
success, preparation failure and returned native failure. General exceptions,
failed HeapFree/CloseHandle reporting and shared-state concurrency are not solved
by this allocation change. No Rust, C or new polyglot product source was added.

## Measurements

Baseline is commit `3e97e628a1d9985c80cfa9b3ede74bbe8630c81b`, including the preceding
security fixes. The identical `bench/windows_dirfs_publication.sapp` performs
1,024 atomic three-byte writes, verifies bytes and removes its test files. Both
variants use the same retained executable and generated DLL. Each of two batches
uses one warmup and seven samples per variant/workload, alternating execution
order. Runs were serial on the authorized Windows workstation. Only disposable
owned files beneath the authorized executable directory were used.

| Batch / workload | CPU median ms before → after | CPU p95 ms before → after | Sampled peak working set KiB before → after |
|---|---:|---:|---:|
| 1 / short | 828.125 → 812.500 | 1031.250 → 968.750 | 44308 → 43624 |
| 2 / short | 687.500 → 671.875 | 875.000 → 1000.000 | 44268 → 43576 |
| 1 / long | 937.500 → 703.125 | 1031.250 → 937.500 | 46840 → 46068 |
| 2 / long | 750.000 → 703.125 | 890.625 → 1015.625 | 46852 → 46056 |

Short-path median CPU fell 1.9–2.3%; sampled peak working set fell 1.5–1.6%.
Long-path CPU fell 6.3–25%, with substantial variation; working set fell 1.6–1.7%.
The CPU timer has 15.625 ms resolution, so the short-path median change is one
quantum. Seven samples do not establish a stable tail-latency estimate: p95 here
is the largest sample, and it worsened in the second batch. RAM is the maximum
observed running PeakWorkingSet64 sampled every 10 ms, not private/unique memory.
The previous +10% result used an older baseline: this comparison does **not**
prove that every historical overhead is eliminated. No whole Live View, GPU/NPU,
disk-byte or general memory-management benefit was measured.

[Raw samples, hashes and both batches](Windows_OPEN_FRAME_MEASUREMENTS.json).

## Verification and reproduction

- Five native Windows suites passed: existing directory operations, junction and
  publication failures, ABI canaries, schema/state contracts, and the new frame
  lifetime/Unicode test. The last test performs 96 publications across two roots
  with ASCII, Greek, Chinese and supplementary Unicode names, verifies bytes and
  checks that each root's ABI allocation registry is empty after the calls.
- Seven Linux suites passed: native ABI generation, dirfs contract, publication
  faults, frame faults and three portable-lock suites. Fault tests replace only
  the native dependency in an isolated copy of the actual owning module.
- The frame model checks all seven initialized record fields, reserved zeros,
  pointer offsets/alignment, unchanged flags, each of three allocation failures,
  each of nine preparation-write failures, native errors, invalid handles,
  malformed paths, closed roots and independent subsequent roots. No incomplete
  record reaches the mocked NT call and no modeled buffer remains allocated.
- Five emulator suites passed: platform, device, emulator differences, Android and Android cleanup. These are emulator evidence, not physical device certification.
- The existing Windows publication example now demonstrates a Greek UTF-8 name
  and passed with automatic root cleanup. Five touched PBLang files passed lint
  with analyzer warnings; isolated build and PBLang-native `pblock` passed.

Run `windows_open_frame_test.sapp` only from an isolated repository copy: it writes
and removes its temporary subject under `tests/fixtures`. The live test skips
on non-Windows rather than reporting a Windows pass. Configure the reviewed
packaged bridge through `PBLANG_WINDOWS_ABI_BRIDGE` and use an owned working
directory. For the example, set `PB_WINDOWS_EXAMPLE_ROOT` to that directory.
Benchmark `PB_DIRFS_WORKLOAD=short|long` in separate fresh processes; retain both
batches and do not compare only the fastest samples.

## Remaining integration work

| Area | Still required |
|---|---|
| Windows lock publication | Package/private-root ACL verification, loader identity across load, Windows dependency-root handling in portable_lock. Readiness remains false. |
| PBRemote consumer | The companion PBRemote change preserves confirmed publication through upload/copy, queue history and dispatcher errors, consumes completed upload IDs and releases retained buffers. End-to-end deployed Windows validation and crash/lost-response idempotency remain open. |
| Runtime / FFI | General error-preserving cleanup, shared/cyclic references and collection pause control across backends. |
| Foreign boundaries | Complete replacements and behavioral parity before disabling remaining polyglot paths. |
| Devices | Actual GPU/NPU execution, output parity, device-specific resource measurements and deterministic fallbacks. |
| Products / platforms | Current-artifact integration and tests on declared platforms. This Windows source change does not update the installed Linux PB-MCP or prove a Live View benefit. |

No EnterpriseCore server, personal desktop control or antivirus setting was used or
changed. The implementation is a tested reduction in per-call allocation work,
not completion of the wider 1.0.0 program.

Graphify update reached the 35-second budget in PBLang and its process group was
stopped. PBRemote update completed; neither result certifies semantic `.sapp`
coverage. Owned Windows fixtures were removed after verifying no fixture process
or junction remained. Pre-existing local changes were preserved.
