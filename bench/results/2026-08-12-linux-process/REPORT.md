# PBLang Linux process supervision benchmark

Date: 2026-08-12

## Scope

This benchmark measures one complete warm `linux_process.run` operation. Each
sample includes argv-image and supervisor generation, private mode-0700 state
setup, launch, wait, bounded stdout/stderr loading, process-group termination
verification and cleanup. Compiler startup and module imports are outside the
sample and are not presented as subprocess latency.

## Command

```sh
target/release/pblang run bench/linux_process_stages.sapp --allow-all
```

## Result

- Samples: 15
- Median: 8.6543 ms
- p95: 9.4580 ms
- Minimum: 8.0950 ms
- Maximum: 9.5598 ms
- CV: 5.07%
- Stability: stable

Every sample correctness-gated direct argv execution, exact output and verified
process-group termination. The security contract is covered separately by
`tests/semantics/linux_process_test.sapp`, including inert shell metacharacters,
aggregate output limits, large-output truncation and descendant cancellation.

## Boundaries

The current generated supervisor and argv trampoline target Linux x86-64. The
generic API owns and verifies a private process group. A child explicitly
permitted to create another session can leave that group; untrusted workloads
must additionally use a PID namespace such as pb-mcp-server `sandbox_exec`.
