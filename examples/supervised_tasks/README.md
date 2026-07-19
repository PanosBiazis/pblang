# Supervised tasks

This pure-PBLang example runs one bounded task and stops one cooperative worker
through `stdlib:thread.sapp`.

```bash
./target/release/pblang examples/supervised_tasks/main.sapp --allow-all
```

Expected output:

```text
quick=completed
cooperative=cancelled
active=0 detached=0
```

`spawn_cooperative` passes a cancellation token to the worker. Long-running
workers should call `token.requested(token)` at safe interruption points. This
token-local function avoids capturing the imported module across the VM boundary;
the equivalent direct helper is `thread.cancellation_requested(token)`.
The supervisor does not claim to force-kill VM threads. Blocking native calls
still require a timeout at the I/O boundary or isolation in a disposable process.
