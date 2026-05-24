# Tutorial — Concurrency

A minimal worker-pool pattern using pblang's spawn + channels.

## Run

```bash
pblang run examples/tutorial_concurrency/main.sapp
```

Expected output:

```
result=144
```

## What you learn

- `stdlib:thread.sapp` — `thread.spawn(fn() { ... })` runs the closure in a
  separate VM (VM-per-thread isolation, Go/Erlang model).
- `thread.channel_new()` returns a `{ sender, receiver }` pair. Channels are
  the only safe way to pass values between threads — only **transferable
  values** (numbers, booleans, nil, strings, deeply-cloneable arrays/maps)
  cross the boundary.
- `thread.send_value(sender, v)` and `thread.recv_value(receiver)` — blocking
  send/receive.
- `thread.join(handle)` — wait for the worker to finish.

## Safety model

pblang threads do **not** share state. Every spawned thread gets its own VM,
its own GC arena, and its own globals. The only channel between them is the
explicit `channel_new()` queue. This eliminates whole classes of bugs
(data races, lock ordering issues) at the cost of giving up shared memory.

For cases where you genuinely need shared atomic counters / mutex-protected
state, see `stdlib:concurrency.sapp` (Mutex, RwLock, Atomic, Semaphore,
WaitGroup wrappers).
