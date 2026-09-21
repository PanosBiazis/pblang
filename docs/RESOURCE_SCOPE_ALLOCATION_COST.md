# Resource-scope allocation cost and desktop integration

The synchronous PBLang resource scope no longer allocates an outcome tuple for
its body and every cleanup callback. Exception state is a boolean field on the
existing private ledger, reset before each callback. Public result envelopes,
reverse registration order, one-attempt cleanup and failure classification are
unchanged. The ledger is separate for nested scopes and is not exposed through
the public facade. Returned nil, booleans, numbers, strings and arrays remain
invalid results rather than exception sentinels.

## Reproduction and measurements

Baseline: `619877b`, compared with the changed `stdlib/resource_scope.sapp`.
Use isolated copies with identical stdlib, retained executable, manifest and
[benchmark](../bench/resource_scope_callbacks.sapp). Build each copy and publish
its lock using `tools/pblock.sapp` before timing. Run:

```
pblang run bench/resource_scope_callbacks.sapp -- success
pblang run bench/resource_scope_callbacks.sapp -- mixed
```

Each process executes 10,000 scopes with 16 callbacks. The success workload
requires 160,000 successful releases; the mixed workload alternates success,
explicit error, exception and malformed result, checking all outcome counts.
The same assertions and source run on both variants. Two warmups and seven
measured processes per variant, alternating order, with GNU time
`%U %S %e %M`. CPU is user plus system time; nearest-rank p95 is the maximum
of seven samples. No tests, builds or Graphify ran alongside these measurements.

[Raw samples](evidence/resource-scope-cost-2026-09-22.json):

| Workload | CPU median / p95 before, s | CPU median / p95 after, s | Peak RSS median before / after, KiB |
| --- | ---: | ---: | ---: |
| Success | 0.81 / 1.24 | 0.72 / 0.85 | 30452 / 21752 |
| Mixed failures | 0.99 / 1.16 | 0.97 / 1.51 | 46556 / 33072 |

The success median CPU decreased about 11%; peak RSS decreased about 29% in
both workloads. Mixed median CPU decreased only about 2%, while its observed
p95 increased about 30%. These process-level results do not prove lower maximum
pause, whole-system memory savings, reduced disk writes or universal speedup.
The retained host's weak allocation registry can contribute to sustained RSS;
removing temporary containers is not a repair of its general cycle collector.

## Validation and focused review

[Validation inventory](evidence/resource-scope-validation-2026-09-22.json):
42 PBLang suites, including 12 kernel emulator suites and five platform/device/
Android emulator suites; retained and self-hosted resource-scope contracts;
real SQLite close after implicit failure; six PB-MCP integration suites; build,
portable lock publication, the SQLite example and lint. Lint has test-only
advisories (two shadowing reports, an explicit boolean comparison and an unused
benchmark loop index); the product library has no lint diagnostics.

The review checked separation of exception and malformed-result classification,
reset between callbacks, nested ledgers, immutable envelope snapshots, preserved
mutable value aliases, exact-once removal before callback invocation, and
registration rejection during/after cleanup. No external effect or permission
was added. This cooperative API is not a sandbox, thread-safe ownership system
or guarantee against allocation failure inside the cleanup machinery.

The real desktop candidate passed authenticated disposable X11 tests for
synthetic enrollment/cancellation, MCP input/capture/streaming, a one-event quota,
Stop, SIGTERM/SIGHUP cleanup, minimized viewer survival and parent-pipe failures.
The fixed personal user service was not restarted or tested: its active session
was preserved. This is not a personal-desktop Codex interaction test.

## Installed desktop scope

The installed release pin is
`7f265fcb5a418767c3baeda5e2839a2724662029740ca9f8a965afac87a056c9`.
Its 93 files total 34,763,001 bytes, 182 bytes more than the previous bundle.
Only `pblang/stdlib/resource_scope.sapp` and the three repository-relative
package locks differ. All other module, native library and launcher bytes are
identical to the prior verified installed release. The closed inventory and
external pin were verified before/after publication; Codex desktop and viewer
registrations now select it for the next connection. Prior release/configuration
backups remain available, using additional disk space. Enrollment file metadata
was unchanged; no local code was read, replaced or embedded in reports.

The viewer imports this resource scope for startup/teardown. It does not call
it on every frame. Therefore this installation does not establish sustained
streaming acceleration. The synthetic candidate's five one-second observations
at requested 2 fps had median summed viewer/operator/worker CPU 510 ms and
summed RSS 288904 KiB; p95 CPU 1040 ms. Shared pages are counted repeatedly;
supervisor and X server are excluded. These are absolute observations, not
before/after desktop improvements. No GPU/NPU/energy measurements were made.

## Remaining work and compatibility finding

Kernel collection's long critical sections, general retained-runtime/FFI root
ownership, broader backends/platforms and polyglot integration remain open.
The desktop package still does not run the kernel collector or self-hosted VM;
this change does not insert either into unrelated host objects. Its existing
foreign system boundaries remain in use.

A minimal existing self-hosted discrepancy was reproduced during this work:

```pblang
fn sample() {
    let caught=false;
    let value=try {throw "synthetic";} catch {caught=true;};
    return caught;
}
print sample();
```

The retained runner prints `true`, but `frontend_run` prints `false` on the
same source. A scalar flag in that catch cannot be relied on across these
frontends yet. Using the already-private mutable ledger preserves behavior
in both, as the expanded contract tests verify. The general compiler issue is
recorded here, not claimed fixed. Original exception diagnostic capture,
cyclic references, hanging callbacks and abrupt termination remain outside this
scope library's guarantees. No EnterpriseOrg server was used.

Graphify AST refresh exceeded its 35-second bound and its owned process group
was terminated. It does not provide semantic `.sapp` coverage.
