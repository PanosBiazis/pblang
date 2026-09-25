# Audit remediation status — 2026-09-25

This is an implementation ledger following the source audit, not certification
of the whole language. Tests on one retained Linux host do not qualify all
backends, devices, products, or deployed installations.

| Audit issue | Current result | Remaining dependency |
| --- | --- | --- |
| Self-hosted analysis accepted invalid arity/names/types | Fixed in `frontend_analyze`; focused regressions | Full checker conformance and retained CLI exit status |
| Diagnostic failures returned process success | `pblang check` and `tools/pbcheck.sapp` fail on diagnostics; no status-file write | Existing Rust `--check --via-pblang` driver remains unchanged |
| Race waits for all workers | Fixed in `async.await_race`; loser handles remain caller-owned | Host-level cancellation and uncatchable failures |
| Repeated settlement consumes a channel twice | Cached success/failure shared across async APIs | Concurrent cross-VM future ownership is unsupported |
| User result resembles internal error sentinel | `go` wraps success separately from payload | Similar APIs in other modules still need review |
| Invalid pool sizes can hang | Validated before channel/worker allocation | Global admission control and queue budgets |
| `gather` loses workers after task/start failure | Existing owned workers settled before error | Nonterminating workers cannot be preempted safely |
| Legacy `timeout(ms, fn)` exceeds deadline | Replaced with structured ready/ok/value/future result; caller retains ownership | Breaking alpha API migration; automatic cancellation/reaping still unavailable |
| Worker startup copies unrelated thread library | Isolated worker module; removed wrapper-only thread import | Host channel registry reclamation and full closure transfer remain open |
| LLVM unsupported constructs / backend differences | Not changed | Fail-closed compilation and backend conformance |
| FFI ABI, external resource cleanup, cycles/GC pauses | Not changed | Ownership, ABI proofs, bounded cleanup and platform testing |
| Windows lock publication and release authenticity | Not changed | Atomic OS primitives and real Windows qualification |
| Native listener restart / TLS / libxcrypt replacement | Not changed | OS interface and cryptographic implementation verification |
| PB-MCP, PBIDE, form conversion, ND interactions | Not deployed or expanded in this slice | Integrate only verified library contracts; product-level tests |
| GPU/NPU and cross-device resource optimization | Not changed | Actual dispatch, output parity and measured device workloads |
| Tooling, documentation consistency and packages | Focused new checker and lifecycle example | Broader debugger/LSP, package trust and release gates |

See `ASYNC_LIFECYCLE_AND_CHECKING.md` for usage, compatibility changes and limits.
No claim is made that all audit findings are fixed or that 1.0.0 stable is ready.
