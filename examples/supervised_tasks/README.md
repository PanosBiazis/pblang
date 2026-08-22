# Supervised Task Lab

This PBLang-only example migrates an earlier daemon-thread exercise into a
bounded structured lifecycle. One finite worker completes, one background
worker receives cooperative cancellation, both are joined, and their terminal
records are reaped. The source behavior that abandoned a daemon when the main
thread exited is deliberately not retained.

```text
pblang run examples/supervised_tasks/main.sapp
pblang run examples/supervised_tasks/benchmark.sapp
```

The shared responsive UI compiles to Web, native desktop and Android records.
Its buttons inspect lifecycle stages; they do not pretend to start host workers
inside a renderer that lacks the runtime adapter. The backend execution report
is the evidence source for completion and cleanup.

`spawn_cooperative_with_args` passes explicit transferable arguments and a
cancellation token across the VM boundary. Long-running workers poll
`token.requested(token)` at safe interruption points. Every public workload is
validated before spawning and has a finite deadline with cleanup headroom.

The original source is represented by SHA-256
`3631284ad8c23a9d288c06f0ea76147d9f2a381c2fbf770d70db7afedaf39511`
without retaining its personal filesystem path.

PBLang VM threads cannot be force-killed safely. Blocking native calls still
need an independently bounded I/O adapter or disposable-process isolation; the
example reports both capabilities as unsupported instead of claiming a daemon
replacement can provide them.
