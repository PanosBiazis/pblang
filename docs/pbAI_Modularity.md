# Controlled pbAI adapters

Both facades expose `modular_catalog()`, `modular_probe(adapter)`,
`modular_run(adapter, request)` and `modular_bind(adapter, provider, resolver)` through the shared PBLang
`stdlib:pbai_modularity.sapp`. Supported IDs are exactly `sensor-v1` and
`student-v1`. No arbitrary source paths, callbacks, GPU/NPU providers or plugin
installation are admitted. Existing direct functions remain compatible.

## Separate description, observation and execution

`modular_catalog()` is a fresh static declaration. It loads neither optional
provider, runs no probe and initializes no model. `runtime_checked` is false.
Changing the returned descriptor never changes execution policy.

`modular_probe(id)` explicitly loads only the selected packaged provider:

- Sensor: exact versioned contract, callable exports and one fixed four-value
  numerical oracle (linear trend, slope and residual checks).
- Student: exact capabilities/limits and one fixed unsupported-operation error.
  No checkpoint, weights, training, prediction or worker is initialized by this
  probe. `numeric_inference_checked` is false for the student provider.

The bridge reuses `controlled_modularity.resolve` with exact package/interface
version 1.0.0 and one optional dependency. Missing/unloadable dependencies produce
`dependency_unavailable` and a disabled binding. Imported providers with incompatible
exports/contracts or failed probes fail closed without a substitute provider.
An import error is not proof that a file was absent: permissions, integrity checks
and malformed source can also make a dependency unavailable.

The resolver's `execution_performed:false` describes static dependency resolution,
not the separately performed explicit probe. Its capability claims are not OS
permissions. Our manifest describes this bridge's interface version, not the
PBLang executable release version. The current trusted package content lock remains
responsible for source integrity; neither the probe nor an unsigned content digest
establishes publisher authenticity or isolation from hostile code.

`modular_run(id, request)` verifies the selected provider and uses that same private
module handle for one synchronous request. No successful-probe cache or public mutable
activation registry is retained. The outer `pbai.modular.result.v1` status describes
bridge admission/execution; `output` preserves the direct provider response, including
its own input errors. Always inspect the inner provider status before using a result.
Probe/import/execution exceptions are reduced to fixed error codes without raw
exception text, paths or rejected IDs. Provider outputs retain the privacy semantics
of the [sensor](pbAI_Sensors.md) and [student tool](pbAI_Student_Tools.md) interfaces.

## Limits and operations

The path is serial, pure-PBLang CPU computation. It launches no workers, performs no
hardware enumeration or network access, saves nothing and authorizes no external
actions. `workers_per_call:1` is a synchronous-call description, not a global
concurrency limiter; an embedding server must bound concurrent callers and transport
bytes/time separately. Trusted provider code runs in-process: this is not a sandbox
for arbitrary malicious modules or callbacks.

```pblang
let AI=import("stdlib/pbai.sapp");
let probe=AI.modular_probe("sensor-v1");
assert(probe.ok,"sensor provider admitted");
let result=AI.modular_run("sensor-v1",map{
 values:[2,4,6,100],period_seconds:1,sigma:3,absolute_tolerance:1
});
assert(result.status=="result" and result.output.status=="result","request succeeded");
assert(result.output.value.outlier,"synthetic deviation");
```

Run `tests/semantics/pbai_modularity_test.sapp` from either product root.
Tests cover direct-output parity (including real student predictions/logits from an
explicitly constructed synthetic checkpoint), exact static limits, both actual provider probes,
mutation isolation, malformed IDs/cycles/functions and input privacy. Dedicated
minimal dependency fixtures additionally exercise absent, incompatible and throwing
providers and detect unrequested imports. These fixtures do not prove a full installed
product release or support on every operating system.

Future independently reviewed adapters require their own exact interface, trusted
loading boundary, numerical reference test, resource limits and platform evidence.
GPU/parallel execution is not enabled by editing returned metadata.

The Linux integration suite is `tests/semantics/pbai_modularity_dependency_test.sapp`.
Set `PBLANG_BIN` to a trusted executable and run it with `--allow-all --deny-net
--deny-http` (one line). The outer test creates only UUID-named disposable fixtures,
uses bounded subprocesses and cleans them afterward. Each child denies network,
HTTP, process execution, FFI, spawning and SQLite. Both products use the same test.
The 20 cases include missing providers, mismatched exports/contracts, invalid source,
incorrect numeric output, throwing probes/execution, callable metadata never invoked,
resolver failure and catalog/invalid-ID paths that never import optional providers.
These are Linux fixture results; other platforms require their own runtime evidence.

## Installed execution and the retained function-origin limit

The retained runtime does not preserve the declaring file's import origin for
all exported function chunks. A lazy `stdlib:` import inside a function can therefore
resolve against the caller's working directory. Installed dependency locks alone
cannot repair this missing origin. The convenience `modular_probe` / `modular_run`
wrappers require the trusted installed product directory as cwd on this runtime.
An unrelated directory without a manifest produces `dependency_unavailable`; a
caller-owned manifest can instead redirect a legacy lazy import. Do not use these
legacy wrappers from an untrusted cwd. The general runtime defect
is not fixed by this module.

For foreign-cwd callers, use explicit **trusted-host binding**:

```pblang
let AI=import("stdlib/pbai.sapp");
let SENSOR=import("stdlib:pbai_sensor.sapp");
let RESOLVER=import("stdlib:controlled_modularity.sapp");
let binding=AI.modular_bind("sensor-v1",SENSOR,RESOLVER);
assert(binding.report().ok,"selected provider passed bounded ABI/probe checks");
let reply=binding.run(map{values:[2,4,6,100],period_seconds:1,sigma:3,absolute_tolerance:1});
```

Top-level package imports retain the application's source context and normal lock
validation. Import only the selected provider. The binding accepts native module
handles from trusted application setup, not paths, JSON/MCP fields, maps or supplied
callbacks. It is not a new capability grant or authenticated module provenance.
The resolver result must match the exact selected two-module plan. Failed admission
produces a binding whose `run` always refuses; mutating a returned report cannot
activate it. A successful binding captures the checked callable and a private report
snapshot. It does not reload code or repeat the probe on every request. The already
trusted provider's own mutable internal state, if any, remains outside this snapshot;
this is not isolation for malicious modules.

The updated `pbai_modularity.sapp` example follows this pattern, requiring no cwd
mutation, subprocess bootstrap, absolute-path imports or unselected provider load.
For an installed layout with a materialized local lock:

```sh
cd /tmp
"$PBLANG_ROOT/bin/pblang" run "$PBLANG_ROOT/examples/pbai_modularity.sapp" --deny-net --deny-http --deny-exec --deny-ffi --deny-spawn --deny-sqlite
```

Set `PBLANG_ROOT` to the reviewed installed directory. The standalone equivalent
uses its `demo/pbai_modularity.sapp` with its installed dependency lock. For legacy
lazy-wrapper applications, run from their installed project directory instead.

`tests/semantics/pbai_modularity_installed_test.sapp` tests both actual facade copies
with a trusted `PBLANG_BIN`, Linux `bwrap` and the sibling source checkout layout.
It creates private installed fixtures, removes the unselected student module,
materializes local fixture locks, hides both source checkouts and optionally the
`PBAI_SOURCE_HIDE_ROOT` tree, then runs from a foreign cwd containing a decoy provider, both without a manifest
and with a valid caller-owned dependency lock.
It verifies both facade results, legacy project-cwd success and the known foreign-cwd
legacy refusal. Run the outer test with `--allow-all --deny-net --deny-http`; its
numerical children deny process/FFI/network capabilities. This is tested Linux
installed-fixture evidence, not proof of universal platform support or a global
runtime-origin repair. The test does not alter an installed launcher or release.

## Private operational measurements

An explicit binding adds `metrics()` and `clear_metrics()`; existing run/report
results are unchanged. Metrics cover only that binding's `run` calls after creation
or the latest clear. Contract probes, direct provider calls, legacy `modular_run`,
other bindings and model quality are outside the observation scope.

```pblang
let metrics=binding.metrics();
assert(metrics.executed_calls==1,"one actual adapter call observed");
// Detaching or editing this snapshot never changes counters or admission.
binding.clear_metrics();
assert(binding.metrics().observed_calls==0,"new empty observation window");
```

The fixed `pbai.modular.metrics.v1` snapshot contains these separate counts:

| Field | Observation |
| --- | --- |
| `observed_calls` | Calls included in the current bounded window |
| `rejected_calls` | Binding admission refused; no provider execution or latency |
| `executed_calls` | Checked callable was invoked |
| `provider_results` | Reply declared `status:"result"`; not proof of answer correctness |
| `provider_errors` | Reply declared `status:"error"`, including rejected provider input |
| `execution_errors` | Callable threw or returned nil; bridge emits `execution_failed` |
| `unclassified_replies` | Other non-nil replies; not counted as successful results |

Within a completed synchronous window, `observed_calls` equals rejected plus
executed calls. Executed calls equal the four outcome categories. Replies are not
copied, serialized or semantically validated for monitoring; only the top-level
status is classified. Cyclic payloads and exception text are never retained.

Storage is constant per live binding. The first **1,024** calls are observed;
`window_full` becomes true at that boundary. A subsequent call sets
`observations_dropped:true` and runs normally without new counters/timing. These
are partial-window aggregates, not lifetime totals. `clear_metrics()` discards
aggregates and starts a fresh window without changing checked callables, admission
or report. The binding and its counters live in process memory until released.
This is logical retention control, not guaranteed secure memory erasure.

`duration` uses two actual `now_ms()` readings around the checked call. It reports
source/unit, `monotonic:false`, valid `samples`, `invalid_samples`, and total/min/max
milliseconds. No valid sample means min/max are nil, not an invented zero; an actual
zero-ms sample is allowed because the clock is coarse. Missing/nonfinite readings,
backward movement and elapsed values above 86,400,000ms are marked invalid and
excluded. This upper bound limits numeric aggregation; it is **not a timeout**.
Undetected forward wall-clock adjustments can still distort a valid sample. Duration
includes dispatch/error handling between readings; it excludes admission probes and
post-call metric aggregation. These are operational observations, not a monotonic
benchmark, latency guarantee, confidence score or AGI/ASI evidence.

No raw inputs, checkpoints, outputs, timestamps, paths, caller identifiers or free-text
labels are stored. Nothing is written to disk or sent over the network. Metrics do
not grant rights, activate models or authenticate artifacts. A host that exports
snapshots must define its own retention and access policy; counts/timing may still
reveal usage patterns. This synchronous binding is not a cross-thread monitor or a
global concurrency limiter. The implementation reuses the runtime clock directly;
the shared profiler's global arbitrary-name stores and trace list do not provide
this bounded per-binding retention model. No optional provider imports were added.

`pbai_modularity_metrics_test.sapp` covers real sensor calls, status categories,
1024/1025 saturation, reset/refusal isolation, detached nested snapshots and separate
student counters. The existing modularity suite also checks a real student prediction
from a synthetic checkpoint. Five additional isolated dependency cases exercise
throwing providers, cyclic unclassified outputs, backward/unavailable/oversized clock
samples. The source-hidden installed test runs the updated examples' metric assertions
from a foreign cwd without loading the unselected student provider.
