# Controlled pbAI adapters

Both facades expose `modular_catalog()`, `modular_probe(adapter)` and
`modular_run(adapter, request)` through the shared PBLang
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
The 15 cases include missing providers, mismatched exports/contracts, invalid source,
incorrect numeric output, throwing probes/execution, callable metadata never invoked,
resolver failure and catalog/invalid-ID paths that never import optional providers.
These are Linux fixture results; other platforms require their own runtime evidence.
