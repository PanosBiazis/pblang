# Windows publication: held destination parent and native schema reuse

## Fixed behavior

The original `windows_dirfs.move` passed a multi-component destination to
`NtSetInformationFile`. An owned test junction in that destination was followed:
the source disappeared and a file outside the selected root was replaced.
This was reproduced on the authorized Windows workstation using only disposable
fixture data. The fixed code opens the final parent with the existing no-reparse
NtCreateFile policy, rejects a reparse-tagged final parent, holds that handle, and
passes only a leaf name to the rename call. Normal replacement/no-replace behavior
is retained. A literal destination component named `root` is no longer confused
with the directory-listing alias.

Both exclusive and staged writes now validate all bytes before creating parents
or files. Previously an invalid byte returned an error but left an empty file.
Exclusive creations also request DELETE access so a later write failure can attempt
handle-based cleanup. This is a deliberate permission tightening: a directory that
allows creation but denies deletion can reject a write earlier.

If destination-parent close fails after a successful rename, the result reports
`ok:false, published:true`. Staged-write cleanup must not delete that published
file. A deterministic fake-ABI regression executes the actual owning module to
exercise this transition and pre-publication failure; it is not an induced native
CloseHandle failure. Generic cleanup, exception and concurrent-writer guarantees
remain incomplete elsewhere in the module/runtime.

## Resource cost

These are historical results for the protected-parent fix. The subsequent
[call-local frame optimization](Windows_open_frame.md) retains these protections
and reports a new, separately measured comparison.

Path conversion uses the existing validated batch byte writer. The sixteen fixed
native call specifications are now one private module-level map. No API returns
that map or mutates it. The public call still validates name, argument count,
closed state, numeric arguments and owned buffers; this is not a cache of mutable
filesystem state, directory authorization or user approvals.

The benchmark performs 1,024 identical three-byte atomic publications per fresh
process, then verifies bytes and removes the owned file. One warmup and seven
measured samples per variant/workload, alternating order. Both variants use the
same retained executable and generated DLL; the baseline uses both original
modules. CPU is process user+kernel time. Working set is the largest reported peak
sampled while running every 10 ms; it is not private/unique memory. No computer
power mode, AV setting or personal desktop permission was changed.

| Path workload | Median CPU ms before → after | p95 CPU ms | Sampled peak working set KiB | Median wall ms |
|---|---:|---:|---:|---:|
| short | 765.625 → 843.750 | 953.125 → 1093.750 | 53748 → 44268 | 2551.1 → 2660.0 |
| long | 1250.000 → 750.000 | 1625.000 → 1062.500 | 82108 → 46852 | 3656.9 → 2880.0 |

On long paths median CPU falls 40% and sampled peak working set about 43%.
On short paths median CPU increases about 10.2%, while working set falls about
17.6%. The short-path CPU p95 also increases. This is a security/correctness fix
with workload-dependent performance, not a general speedup. The added held-parent
checks remain enabled even where they cost more CPU. No GPU/NPU, disk-byte-saving,
whole Live View or generic runtime-memory result follows from this benchmark.

The initial 24-operation pilot was below useful CPU timing resolution, and reading
working set after process exit yielded unavailable zeros. It was not used for RAM
claims. Both variants were increased equally to 1,024 operations and sampled live.
An earlier safety-only run identified repeated schema allocation; it is retained
separately in the raw report and not mixed with final paired samples.

## Validation and reproduction

- Four native Windows suites passed: directory filesystem, new publication
  regression, ABI byte/canary tests and fixed-schema reuse/closed-state tests.
- Six Linux suites passed: ABI generation/inspection, dirfs contract, fake-ABI
  publication fault paths, portable lock, current lock and quoted-path lock.
- `examples/windows_atomic_publication.sapp` passed on Windows. It uses the
  existing resource scope to attempt root cleanup automatically and treats an
  already-published cleanup failure as non-retryable.
- Eight touched PBLang files passed lint with existing analyzer warnings.
  The isolated package built and its lock was published through native `pblock`.
- The generated PE bridge bytes/digest and runtime were unchanged. Test inputs,
  executable and bridge stayed in an owned private test directory and were removed.
  No Specisoft server was used. The current installed PB-MCP was not updated.

Run the live tests only in an explicitly owned Windows test directory with the
reviewed packaged ABI and `PBLANG_WINDOWS_ABI_BRIDGE` configured. For the example,
set `PB_WINDOWS_EXAMPLE_ROOT` to that owned directory. The benchmark selects
`PB_DIRFS_WORKLOAD=short|long`; run each variant in a separate fresh process.
Fault tests generate a temporary module with only its native dependency replaced;
run them from an isolated copy of the repository. Production has no fake-ABI seam.

## Outstanding work

Safe Windows lockfile publication still needs verified package/private-root ACLs,
load-time artifact identity guarantees and Windows root handling integrated into
`portable_lock`. Its readiness flags remain false. Held-parent rename protects the
tested destination traversal; this is not a proof against all concurrent namespace
changes or untrusted owners. Secure-file private parents on Windows remain refused.
General native cleanup reporting, GC cycles/pauses, runtime/FFI integration, foreign
boundary replacement and broader devices/platforms remain open. Graphify timed out
after 35 seconds and was stopped; it does not supply semantic PBLang coverage.

The design uses Microsoft's [FILE_RENAME_INFORMATION root-directory contract](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information)
and [NtCreateFile access/options contract](https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntcreatefile).
[Raw measurements and regression observations](Windows_PUBLICATION_MEASUREMENTS.json)
