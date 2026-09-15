# Dependency traversal and resource behavior

The compatibility APIs `inline_source`, `inline_source_prefixed`, `inline_file`
and `inline_file_prefixed` in `stdlib:source_inliner.sapp` collect dependencies
with an explicit work stack. Each frame retains its source, discovered imports
and next child index. A frame is completed only after its dependencies finish.

Resolver callbacks still run in source-order depth-first traversal. Completed
canonical modules are read once and emitted in dependency-first order. Invalid
paths, missing modules and cycles retain their existing diagnostics and stop
before later sibling resolver calls. Resolver exceptions continue to propagate.
The explicit stack removes a language call per import depth; it does not make
source storage, output construction or memory exhaustion disappear. The old
implementation also completed the measured 12,000-module chain: this is not a
claim that a previously reproduced stack crash was repaired.

No new source-size, project-count or duration limit is introduced. The bounded
`link_modules` API and its `_collect_bounded` traversal are unchanged. Tools that
accept untrusted projects still need their existing admission and work budgets.
This slice changes no compiler backend, scheduler, allocator or FFI boundary.

Run the compatibility and deep-chain regression with:

```sh
pblang run tests/semantics/source_inliner_traversal_test.sapp --allow-all
```

`bench/source_inliner_deep.sapp` generates a deterministic trusted-local import
chain. For comparisons, use the same file and count in each checkout, retain the
same bootstrap executable, and refresh each checkout's local dependency lock.
Use one warmup per version/count and at least five alternating process samples.
Record medians and nearest-rank p95 for process user/system CPU time, elapsed
time and peak RSS. Compare both printed hashes before interpreting timings.
The printed `link_ms` interval includes linking, result validation and the two
identity hashes; it excludes process startup. It is not a pure traversal timer.
The fixture links source; it does not execute a 12,000-deep function call chain.

For the original comparison, use source_inliner from commit `9177307` as the
baseline and set count identically to 40, 2000 or 12000. Run a local process
measurement tool around `pblang run bench/source_inliner_deep.sapp --allow-all`.
Do not compare an interpreter run with a compiled executable or change the
fixture, protection settings or output requirements between versions.

See [the measured validation report](DEEP_LINK_RESOURCE_VALIDATION_2026-09-15.md)
for results, noise and coverage limits.

## Automatic memory cleanup

The top-level `memory.scope` and `memory.scope_result` APIs create a VM lifetime.
Borrows associated with that lifetime are released when the scope returns or
catches a failure. Scoped owner handles become inaccessible, but the current VM
retains their owner/value entries until explicit `memory.free(handle)`. Access
invalidation is not automatic storage reclamation. An owner outside the scope remains
usable after the scope releases its borrow. Returning an owner handle created
inside the scope does not extend that owner's lifetime.

`memory.SafeRef()` is an explicit map-based guard. Dropping its variable at a
lexical function boundary does not call a destructor. Use its explicit release
operation or the top-level lifetime API; do not assume lexical automatic cleanup.
The tutorial now demonstrates the actual lifetime API. Its regression checks
successful, failing, nested and repeated scopes as well as the manual guard.

This documents existing behavior; it does not add universal garbage collection,
automatic native-resource disposal or full protection across foreign runtimes.

## Disk consumption

Measure final artifact bytes, allocated filesystem blocks, peak simultaneous
temporary space, and total bytes written separately. Source linking takes place
in memory; downstream generators and packaging steps determine artifact writes.
An unchanged artifact hash implies unchanged logical bytes, not unchanged disk
allocation, physical device writes or peak temporary space.

Potential follow-up work includes avoiding identical output rewrites, reusing
validated build results and streaming large outputs. These require separate
correctness and resource measurements; they are not implemented by this slice.
In particular, writing a replacement before atomically publishing it can require
temporary space for both versions. Reducing that space must not silently weaken
failure recovery. No general disk-saving percentage follows from source size.
