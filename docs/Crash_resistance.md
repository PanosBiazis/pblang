# Crash resistance (v0.2)

The VM and builtins should avoid **panic**/unwrap in runtime-critical paths so that embedded scripts cannot crash the host. Instead, we return **RuntimeError** (e.g. via `host.err(...)` or `self.make_err(...)`).

## Current patterns

- **VM**: Error paths use `Err(self.err(...))` or `Err(self.make_err(...))` so that execution failures are reported as `RuntimeError` with message, span, and stack trace.
- **Builtins**: They receive `host: &mut dyn VmHost` and use `host.err("...")` for invalid arguments or denied capabilities; they do not panic on bad input where we can validate.

## Audit scope

For production hardening, the following should be audited and, where possible, converted from panic/unwrap to `RuntimeError`:

- **VM run loop**: Any remaining `unwrap()`/`expect()` on user-dependent data (e.g. stack indexing, constant indexing) should be replaced with explicit bounds checks and `Err(self.err(...))`.
- **Builtins**: I/O and system calls (fs, network, sqlite, ffi) should catch errors and return `Err(host.err(...))` instead of panicking on OS errors.
- **Import resolution**: File not found, parse errors, and cycle detection already produce `RuntimeError`; ensure no panics on malformed paths or permission errors.

## Done (bounded audit)

- **Catch unwind**: In the run loop, when unwinding frames for a try/catch handler, `frames.pop().unwrap()` was replaced with `if let Some(frame) = self.frames.pop() { ... } else { break; }` so a corrupted state does not panic the host.
- **Stdlib init**: When building the optional `json` global (parse/stringify), the two `.find(...).unwrap()` on the builtin list were replaced with `if let (Some(jp), Some(js)) = (...)` so that missing builtins do not panic (the json global is simply not installed in that case).
- **Mutex tables (ffi/db/net)**: All `Mutex::lock().unwrap()` in resource handle tables (ffi, db, net) were replaced with safe helpers `with_ffi_table(host, ...)`, `with_db_table(host, ...)`, `with_net_table(host, ...)` that on poison return `Err(host.err("internal: … table unavailable"))` instead of panicking. Builtins and `gc_collect` use these helpers.
- **Builtin unwrap cleanup**: `freeze(...)` and `require_type(...)` now return cloned input values directly (`args[0].clone()`) rather than taking the first item through an iterator path that can fail on empty input.
- **Memory tracker poison resilience**: `RwLock` access for compatibility mirrors in `memory_safety.rs` now uses poison recovery (`unwrap_or_else(|e| e.into_inner())`) so lock poisoning does not panic the host process.
- **Concurrency internals**: `ThreadPool` worker lock/wait paths and `Timer` epoch time helpers now avoid `unwrap()` panics by recovering poisoned locks and using safe time fallbacks.
- **Remaining**: VM run loop stack/constant indexing is already guarded with `.get(...).ok_or_else(|| self.err(...))` or explicit bounds checks in most places; any remaining direct indexing could be audited if needed.

## Determinism

- **map_keys**: Returns keys in sorted order so that iteration over map keys is deterministic.
- **Golden tests**: `tests/fixtures/*.sap` vs `*.expected` and the determinism test (`same_script_same_output_determinism`) guard against accidental non-determinism in VM/output.

Ongoing: when adding new features, prefer returning errors over panicking so that the host can handle failures gracefully.
