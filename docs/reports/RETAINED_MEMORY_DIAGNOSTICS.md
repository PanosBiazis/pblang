# Retained runtime memory diagnostics

`stdlib:runtime_gc.sapp` reports the retained host's limitations explicitly:
`weak_registry_auto_pruned:false`,
`allocation_counters_are_live_usage:false`, and
`long_running_registry_growth_possible:true`.

The host tracks arrays, maps and modules in thread-local weak registries.
Dropping a value does not itself remove its registry entry. Repeated short-lived
allocations can therefore grow registry metadata even after payloads are no
longer needed. Host allocation counters are cumulative observations, not the
current live heap or a module's private memory consumption.

A destructive collection from a nested VM cannot prove it sees all roots of
its suspended caller on the same thread. The standard library consequently
keeps `collect()` unavailable and returns an explicit unsuccessful result.
Enabling this globally would risk correctness. A trusted outer desktop event
loop with eager imports has a narrower maintenance contract; that specialized
integration does not establish safety for arbitrary imports or applications.

The new fields diagnose an existing limitation; they do not implement a new
collector or prevent every out-of-memory failure. Large live values, retained
references and copies still need separate workload measurements. Portable
owner-scoped runtime reclamation and complete nested-VM root tracking remain
unfinished. No new program-size limit is introduced by these diagnostics.

Validation: `tests/semantics/runtime_gc_test.sapp` and the resource cleanup
suite in an isolated package using the retained executable. The former checks
the status contract and denied collection; it is not a full leak detector.
