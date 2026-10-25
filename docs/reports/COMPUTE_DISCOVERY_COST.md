# Compute discovery and device validation — 2026-09-22

`compute.run_controlled` now avoids a full GPU/NPU/hardware survey when the
validated policy already determines the route: empty work, explicit CPU or
sequential execution, disabled accelerators, or work below the accelerator
threshold. The change is PBLang-only and automatic for existing callers,
including `run` and small `run_chunked` batches. No new program-size limit,
background service, device cache, or hardware setting was introduced.

The private helper keeps the supplied-device path small. This is a structural
choice, not evidence that this path became faster. The existing selector still
owns route decisions and diagnostics. Supplied device lists, policies, and
workers retain validation. Strict accelerator requests remain blocked when
appropriate; callback failures still propagate. Eligible calls observe runtime
registration and removal on the next call.

## Linux measurements

Baseline: `b4f47ff64550ab7a40cfb88899816709230d6112`. Clean archived packages
used the same retained Linux executable and identical benchmark source for
each comparison. One warmup per variant, nine measured pairs in alternating
order. CPU is process user plus system time, including waited-for discovery
children. Startup and import verification are included. p95 is nearest rank;
nine samples do not establish a worst-case latency bound.

| Workload | Rounds | CPU median before / after | CPU p95 before / after | Peak RSS median before / after |
| --- | ---: | ---: | ---: | ---: |
| Default small batch | 100 | 5.55 / 1.37 s | 5.77 / 1.61 s | 39,892 / 40,036 KiB |
| Explicit CPU | 100 | 6.67 / 1.58 s | 7.08 / 2.09 s | 39,848 / 40,068 KiB |
| Supplied device list | 1,000 | 1.87 / 1.98 s | 2.58 / 2.52 s | 39,976 / 40,036 KiB |

The two discovery-heavy workloads used approximately 75–76% less CPU. The
supplied-device control instead measured 5.9% more median CPU. A longer
5,000-round confirmation on one fixed logical CPU measured 6.98 / 7.37 s
(+5.6%), with p95 9.00 / 9.07 s. Samples varied considerably on this interactive
laptop; affinity did not isolate CPU frequency, thermals, or other processes.
These are observed differences, not proof of a universal causal regression or
improvement. There is no general RAM-saving claim, and the remaining cost in
the supplied-device path has not been resolved. Preliminary inline-layout
measurements are retained in the evidence rather than silently discarded.

Reproduce each variant from a clean package named `pblang`, prepare its lock
with the retained build and `tools/pblock.sapp`, then time these identical
commands with warmups and alternating sample order:

```text
pblang run bench/compute_discovery.sapp --allow-all -- small 100
pblang run bench/compute_discovery.sapp --allow-all -- cpu 100
pblang run bench/compute_discovery.sapp --allow-all -- injected 1000
```

## Devices and coverage

The laptop has an Intel Core i7-1195G7 and Iris Xe. OpenCL and Vulkan enumerate
the GPU, but the tested retained PBLang executable reports `gpu_feature=false`
and `gpu_enabled=false`; its tensor backend reports eight CPU threads. No GPU
or NPU execution or speedup was verified.

The storage harness creates eight synthetic 64 KiB files per selected volume,
then `bench/device_storage_reads.sapp` verifies exact contents for serial and
four-worker reads. One warmup and six measured batches per mode completed on
the internal NVMe and four USB volumes. One additional USB volume was read-only
and was skipped without changing its mount. All created fixtures were removed.
These warm-cache reads test scheduling and filesystem compatibility, not
physical disk bandwidth, endurance, power consumption, or a cold-I/O speedup.
Serial reads were faster on the four tested USB volumes; the NVMe sample
favored parallel reads. No global storage policy was changed from this small
experiment.

The authorized Windows desktop was reached through the user's existing SSH
connection. Inventory identified Windows 11 Pro, a Ryzen 7 8700G (8 cores,
16 logical processors), approximately 32 GB RAM, Radeon 780M, an NPU driver
reporting OK, and three storage devices. Driver presence is not a
working PBLang NPU backend. Seven actual PBLang suites passed in a dedicated
directory under the user's requested `C:/exe` location.

The Windows package builds passed, but the PBLang-native portable lock writer
reported that its secure file backend was unavailable. The Linux-generated
dependency hash was also rejected on Windows. Tests therefore used freshly
generated **local Windows test locks**, retaining content verification. No
Windows release lock was published. Cross-platform lock generation remains a
real compatibility gap.

Windows timing used one warmup and five alternating measured pairs with the
same before/after source and retained executable. Ten routing calls per sample
took 9,844.5 / 2,004.3 ms median wall time for the small-batch case and
9,731.2 / 2,012.1 ms for explicit CPU (approximately 79–80% less elapsed time).
Their wall-time p95 values were 10,697.7 / 2,050.6 ms and
10,392.3 / 2,065.0 ms respectively. The 1,000-call supplied-device control was
approximately unchanged: 1,968.1 / 1,963.6 ms median. Windows parent-process
CPU excludes discovery children and cannot be compared with Linux total CPU.
Peak working-set measurement was unavailable after process exit; null values
are not zero memory consumption. Raw samples include these limits explicitly.

## Windows antivirus observation

Avast quarantined the retained Windows executable during the first attempt in
the temporary directory, reporting `IDP.HELU.PSD11` (confirmed by the user's
screenshot). Its SHA-256 is
`78035e133f1cd515d5571279094fe2a3bbd03d325fe2f5ad91c2348005d1801a`.
This is an older retained artifact dated 2026-08-24, not a newly compiled
executable from this change. Its PE Authenticode security directory is empty;
source-to-binary reproducibility was not established in this turn.

A local ClamAV 1.5.4 scan with database 28131, dated 2026-09-22, completed with
zero infected files and no reported scan-limit alert. The same Windows artifact
was subsequently hash-verified and ran at the user's explicitly requested
`C:/exe` location. Antivirus settings and the existing quarantine entry were
not changed. These observations do not confirm either malware or a false
positive; the vendor disagreement remains unresolved. No executable was
uploaded to an external scanning service.

## Correctness and remaining work

Twenty-one Linux suites and seven Windows suites passed, covering compute,
accelerator registration, strict fallback, tensors, resources, policy and FFI
contracts, and five Linux emulator suites. Linux lint completed with existing
module warnings and an analyzer unused-variable warning for benchmark arguments;
the relevant build and portable lock refresh passed.

A temporary instrumented copy of `devices()` counted entry calls while running
the regression suite: 18 before, 3 after. The baseline fails the expected count,
while the new implementation passes. The three retained surveys are the
eligible calls before registration, after registration, and after removal.
Instrumentation is not part of the shipped module. Input validation occurs
before the shortcut, no shared mutable cache is introduced, and no protection
or fallback requirement is relaxed. Concurrent runtime-hook mutation and device
hotplug during dispatch are existing separate synchronization concerns.

This slice does not change the kernel collector, VM cycle handling, allocation
recovery, pause duration, or generic runtime/FFI/polyglot integration. Earlier
collector and VM overheads remain unresolved. No real-time guarantee is made.
The inspected installed PB-MCP desktop package contains neither `compute.sapp`
nor an import of it, so its installation was not updated and no live-view
speedup is claimed. Broader platform, GPU/NPU, energy and peripheral validation
remain open. Graphify exceeded its 35-second refresh limit and was stopped.

Raw measurements and validation:

- [Cost samples](evidence/compute-discovery-cost-2026-09-22.json)
- [Device and validation results](evidence/compute-discovery-validation-2026-09-22.json)
