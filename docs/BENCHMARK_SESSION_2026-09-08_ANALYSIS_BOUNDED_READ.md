# Bounded selectable-analysis source reads — 2026-09-08

## Scope and correctness

Base commit: 3f8d0f0. Only production change is tools/pbanalyze.sapp, written in
PBLang. It now calls the existing secure_file.read_bytes with a 16 MiB cap,
catches unexpected read exceptions and retains the existing separate UTF-8
conversion. No new Rust code, unsafe reader fallback or executable replacement.

Previously the complete source was materialized by fs_read_bytes before the cap
check. Now Linux's existing descriptor reader copies at most cap+1 into an
anonymous kernel file and rejects oversize before materializing its source-byte
array in the VM. This remains an algorithmic input bound, not VM-wide hard heap
enforcement. Maps, small buffers and accepted arrays still allocate memory;
an accepted 16 MiB source can occupy much more than 16 MiB as VM values.

A reviewed 3f8d0f0 CLI reference followed a synthetic final symlink and returned
ok/passed with status 0. The patched CLI returns the unchanged two-field generic
read error with status 2. Source/config policy admission still happens first.
The initial test harness required corrections to argument-array construction and
the executable path: under a timeout wrapper, /proc/self/exe names the wrapper,
not PBLang. This harness error was not a product regression.

Linux integration covers final and intermediate symlinks, FIFO (eight-second
child deadline), missing source, directory, malformed UTF-8, unchanged real
source, reviewed relative source and valid Unicode text. A sparse cap+1 file
is created by the host truncate utility, without constructing its payload in the
VM. A tiny one-byte file verifies exact-cap success and zero-cap rejection.
No 16 MiB successful-read stress case was attempted.

Separate traced execution of the sparse 16,777,217-byte file observed exactly:
sendfile(..., 16777217) = 16777217, followed by status-2 rejection.
With strace following child processes/threads, whole supervised run was 6.02s
and peak RSS 36,176 KiB. An earlier trace without child following was not used
as syscall evidence (3.09s / 36,296 KiB). Both measurements include observation
overhead, not ordinary CLI performance. Neither establishes peak heap allocation
or a universal no-allocation guarantee. Private trace paths were not retained.

## Compatibility and remaining reader limits

The two existing public error schemas are retained: generic read failure and
invalid UTF-8. Backend errors and input paths are not copied into rejection
reports. Successful reports still contain their normal paths/findings.
Unexpected backend exceptions are projected to the generic read failure; native
cleanup fault injection was not performed and cleanup guarantees remain open.

Symlinks/special files are intentionally rejected, including performance-only
analysis. Unsupported secure roots no longer receive ordinary path-I/O fallback.
Linux absolute/relative supported paths are exercised. macOS absolute secure
paths are unsupported; Windows secure input is restricted to supported relative
roots. Broader root semantics need implementation and physical verification.
The change provides no post-analysis source/config snapshot verification,
atomic namespace snapshot, VM-typed-FFI guarantee or hostile-code isolation.

## Paired read/decode microbenchmark

bench/analysis_read_pair.sapp compares the previous read+size-check+decode
operation with bounded read+decode on the same 2,432-byte Unicode fixture.
Two warmup pairs, eight retained AB/BA pairs, 100 reads per sample, two series.
The initial diagnostic used ten reads/sample; retained runs were increased to
100 to reduce clock granularity effects, without changing file contents.
Every result must equal the fixture. File creation/deletion and imports occur
outside timed spans. now_ms is not monotonic. No analyzer or CLI startup is timed.

| Series | Legacy median ms/100 | Bounded median ms/100 | Median paired ratio |
| --- | ---: | ---: | ---: |
| initial | 2.446 | 15.162 | 6.046 |
| confirmation | 2.504 | 15.502 | 5.999 |

This is roughly 6x the isolated read cost, about 0.13 ms extra per read on this
small fixture, not a speedup and not a 6x whole-analysis slowdown. The old reader
does not provide equivalent safety. No attempt was made to claim performance
parity between unequal guarantees. Raw samples/p95/variation:
bench/results/analysis-bounded-read-2026-09-08.jsonl.

## Separate numeric benchmark

Unchanged session_compare integer_kernel, one million iterations; all nine rows
status=ok with checksum 474518. First execution, warmup, five retained runs;
compiled paths first compilation plus three repeated compilations.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 267.456 | 275.388 | 8612 | N/A |
| pblang-c99 | 6.542 | 6.974 | 1572 | 14472 |
| pblang-codegen-x64 | 3.269 | 3.488 | 264 | 507 |
| c | 7.277 | 7.678 | 1572 | 14472 |
| cpp | 7.926 | 8.187 | 3380 | 14472 |
| go | 9.596 | 9.873 | 1500 | 1228952 |
| rust | 8.472 | 8.506 | 1968 | 350560 |
| python | 208.491 | 208.750 | 10264 | N/A |
| javascript | 129.178 | 130.513 | 51600 | N/A |

Linux x86_64 only. PBLang's retained interpreter remains slower than Python and
JavaScript here. This workload does not exercise pbanalyze; differences from
earlier sessions do not prove a causal effect of this patch. RSS is residence,
not total allocation; artifact size excludes runtime/dependencies. N/A is not
zero. Short-run CPU zeros reflect measurement resolution. No other physical
device, all-language comparison or embedded-polyglot qualification is claimed.
Raw build/run/CPU/RSS data:
bench/results/session-2026-09-08-analysis-bounded-read.jsonl.

## Validation and incident boundary

Nine targeted suites: bounded-read CLI, source analysis, general security,
dependency analysis, policy preflight, dynamic eval, manifest, current portable
lock, and pbanalyze policy-admission CLI. No full suite. Tests were supervised
sequentially with byte-array stress avoided. Build/native portable-lock refresh
passed; existing dirty work and local lock are excluded from this slice.

Touched-code lint completed with unused-binding/style warnings and no errors.
Owned-file privacy pattern scan and git diff --check passed. Graphify reached its
20-second deadline (124) without completion, so graph freshness is not claimed.
Temporary references, sparse fixtures and traces were removed; no owned test,
benchmark, build or Graphify process remained.

Separately, the user-reported application crashes were confirmed from local
coredump metadata: two ChatGPT renderer processes received SIGILL at 19:59 and
20:01 local time. The inspected kernel journal contained no matching OOM event.
The metadata does not establish the root cause or a causal link to PBLang.
No core payload, browser content, credentials or app state was inspected/changed,
and no crash artifact was deleted. Application crash remediation remains unproven.

Still open for 1.0.0: source/config and concurrent namespace/policy snapshots,
general performance, C99/full-string and compiler/paradigm compatibility,
native self-bootstrap/executable refresh, hard VM heap/time/ownership/cleanup,
typed FFI/raw bypass/sandbox, publisher authenticity/artifact/revocation/rollback,
whole-product regression and physical-platform qualification. NOT READY.
