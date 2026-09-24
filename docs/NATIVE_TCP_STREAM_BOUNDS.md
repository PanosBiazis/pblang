# Native TCP stream bounds and device measurements

The PBLang `native_tcp.sapp` read path now enforces each operation's byte limit
against already-buffered data, as well as newly read bytes. A delimiter must end
inside that limit. When it does not fit, the operation fails and preserves the
pending bytes so the caller can retry with an explicit larger bound or close.
`read_available_bytes` returns at most the requested maximum and retains the tail
for the next read. Poll/read failures preserve accumulated bytes instead of losing
them. These are corrections to the public bounded-read contract.

Within one `read_until_bytes` call, a newly received chunk only rescans the previous
suffix of at most delimiter-length minus one bytes. The implementation then scans
the new chunk. The old path rescanned the entire accumulated response. Delimiters
straddling chunks still match, and returned bytes/order are unchanged for valid
bounded requests. This is not a general incremental parser: a later independent
call starts by inspecting its pending data again. CPU parsing/copying and scheduler
latency still prevent a hard real-time guarantee.

No new Rust, polyglot blocks, native shim or external dependency was added. The
existing libc/libcrypto boundary remains. The HTTP bounded connection manager
uses these methods; MQTT/Modbus contracts are also regression-tested. The installed
PB-MCP desktop release does not contain this module and was not modified. This
change does not establish a PB-MCP Live View speedup.

## Tests and reproduction

```sh
pblang run tests/semantics/native_tcp_pending_bounds_test.sapp --deny-ffi --deny-net --deny-http --deny-fs --deny-exec --deny-spawn
pblang run tests/semantics/native_tcp_fragment_test.sapp --allow-all
pblang run bench/native_tcp_fragment_read.sapp --allow-all
```

The pending-data regression fails on the preceding module because it returns a
delimiter outside the new smaller limit. It passes after the fix, including with
FFI/network/file effects denied. It also checks exact-boundary delimiters, retained
tails, retry, and denied-effect poll failure. A real Linux loopback test sends a
four-byte delimiter across the 4096-byte chunk boundary and checks the following
byte separately. Native TCP remains unsupported on Windows; successful pure
pending-data tests on Windows are not a claim of Windows socket support.

## Resource measurement scope

Measurements use trusted local fixtures only, with no Specisoft server access.
Windows execution uses a dedicated fixture below `C:\exe`, and the uploaded
retained executable's SHA-256 is verified before running. No antivirus exceptions,
driver settings, device power policies or public listeners are changed.

The Linux TCP benchmark transfers 128 KiB plus a four-byte delimiter over loopback,
asserts the entire payload and closes/joins its sockets and child worker. Baseline
and candidate use the identical workload and executable with only the changed
module overlaid. One warmup and seven alternating-order measured samples include
process startup, OS allocation/copying and search. p95 is nearest rank: with seven
samples it is the observed maximum, not a statistically stable tail estimate.

The separate cross-system workload is the existing `nd_projection_stages.sapp`:
256 points, 32 input dimensions, three outputs and 125 calls, one warmup followed
by seven samples per mode. Both machines run the same PBLang source and verify
identical output. Their retained platform executables differ; these measurements
are observations of two configurations, not a controlled ranking of hardware.

Device-counter collection runs separately from timing samples. Linux `/proc` I/O
counters distinguish logical reads from storage reads; the last observed sample
may miss activity immediately before exit. Windows raw process I/O counters and
private working-set samples also are not an exact final/peak measurement. Raw
physical-disk counters are cumulative despite their `Persec` property names.
Whole-system disk deltas include other applications and the diagnostic harness,
so they must not be attributed solely to PBLang.

No GPU engine/fd entries were observed for these CPU workloads. This is not a
measurement of the GPU cost of a GUI, nor evidence that every accelerator is idle.
NPU counters and readable power/energy counters were unavailable. No GPU/NPU,
USB peripheral, energy or device-lifetime saving is claimed.

## Outstanding work

Immediate same-port restart remains unresolved: the retained typed FFI accepts
at most four native arguments, while `setsockopt` requires five. No protection was
weakened and no extra native shim was introduced to conceal that limitation.
Replacing libxcrypt/FFI, reducing account overhead, full application conversion,
interactive multidimensional interfaces, general runtime/GC/FFI integration and
broader platform coverage remain open. These stream fixes do not complete 1.0.

## Results, 2026-09-25

| Linux loopback TCP | CPU median / p95 | Median peak RSS |
| --- | --- | --- |
| Before | 4.70 / 4.95 s | 32,916 KiB |
| After | 2.49 / 2.74 s | 33,256 KiB |

CPU decreased about 47% for this 128 KiB workload. Peak RSS increased about 1%.
This does not establish savings for small requests, all tail latencies, accounts,
GPU/NPU work, or the entire application. The benchmark uses the existing `net`
wrapper for its portable loopback client; both variants use that same wrapper.

| Identical ND source workload | Laptop Linux CPU median / p95 | Corporate Windows CPU median / p95 |
| --- | --- | --- |
| Sparse | 0.60 / 0.72 s | 0.546875 / 0.609375 s |
| Dense | 1.87 / 2.18 s | 0.984375 / 1.046875 s |

Laptop: Intel Core i7-1195G7, 4 cores/8 logical CPUs, internal NVMe plus five USB
storage devices. Corporate workstation: AMD Ryzen 7 8700G, 8 cores/16 logical CPUs,
Radeon 780M, approximately 32 GiB RAM and three SATA storage devices. No drive
serial numbers or personal data were collected for the report.

Linux ND median peak RSS was 21,824 KiB (sparse) / 21,956 KiB (dense). Windows
post-exit peak-working-set queries returned unavailable, not zero. A separate
10-times-longer dense run sampled private working set up to 31,543,296 bytes;
that is a sampled lower bound, not a comparable peak for the shorter benchmark.
Its last sampled logical I/O counters were 4,441 read bytes, 0 write bytes and
3,230 other bytes. Those counters are not physical disk traffic.

Windows whole-system disk counters changed by 32,768 read / 379,904 write bytes on
the system disk during the instrumented sample; the other two disks were unchanged.
Other applications and the harness can account for this activity. Per-process GPU
entries were absent in that sample. Linux per-process DRM entries were also absent;
no accelerator performance gain is inferred. Unsupported/missing NPU and power
counters remain measurement gaps.

Fifteen targeted Linux suites passed after completing the isolated fixture's child
executable and MQTT/Modbus fixture imports; the initial two fixture failures were
not retained as passing tests. Two pure-state/numerical suites passed on real
Windows. The new loopback framing example, lint, entry-only security checks and
build passed. Lint retains existing heuristic warnings; entry-only checks do not
certify the whole dependency tree. The 212 unrelated local file changes were
preserved. No installed PB-MCP deployment was replaced.

Run the readable framing example with:

```sh
pblang run examples/native_tcp_frames.sapp --allow-all
```

In the separate final Linux device sample, the last process snapshot reported
25,778,108 logical bytes read and zero process-attributed physical read/write bytes.
The system NVMe counter increased by 184,320 written bytes; the five USB disks
showed no counter change during that short interval. These are observations, not
proof of zero total storage activity or lifetime/energy savings. No comparable
before/after physical-device reduction was established.

The Graphify update was attempted with a 20-second bound and did not complete;
it was terminated and reaped. Mutable socket state remains caller-owned and is
not made safe for simultaneous reads by this patch. Existing capabilities,
connection deadlines and the dependency boundary are unchanged.
