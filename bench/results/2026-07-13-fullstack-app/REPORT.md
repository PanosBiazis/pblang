# pblang full-stack application report

`stdlib/app.sapp` now composes CLI, native GUI, and hardened web components in
one pblang `fullstack` spec. Preview uses the real builders without opening a
window or socket. Live mode requires `start: "cli"|"gui"|"web"`; only that
surface may block, preventing hidden server/window lifetimes.

## Performance

Five independent `target/release/pblang` process runs were recorded; this table
reports the median of each phase after memoization.

| Release warm phase | Median average |
|---|---:|
| Validate CLI | 0.0053 ms |
| Plan GUI | 0.0179 ms |
| Dispatch CLI | 0.0123 ms |
| Build GUI preview | 0.0634 ms |
| Build hardened web preview | 0.1242 ms |
| Build CLI + GUI + web full-stack preview | 0.2373 ms |

Module getters remain lazy for cold CLI startup and now memoize the resolved
module inside the process. The controlled before/after comparison below used
the same debug binary for both sides so build mode could not bias the delta.

| Debug warm phase | Before memoization | After memoization | Change |
|---|---:|---:|---:|
| Validate CLI | 0.0880 ms | 0.0867 ms | -1.5% |
| Plan GUI | 0.2512 ms | 0.2557 ms | +1.8% |
| Dispatch CLI | 0.1619 ms | 0.1619 ms | unchanged |
| Build GUI preview | 0.7561 ms | 0.7109 ms | -6.0% |
| Build hardened web preview | 1.1868 ms | 1.1440 ms | -3.6% |
| Build CLI + GUI + web full-stack preview | 2.4210 ms | 2.3853 ms | -1.5% |

An earlier snapshot mixed build-mode context with the phase output. It has been
replaced by explicitly labeled release medians and a build-matched optimization
comparison rather than retained as ambiguous performance evidence.

The earlier lazy-import result remains 77 ms for twenty cold CLI processes,
down from 1,182 ms with eager GUI/web/security imports.

## Rejected VM experiments

Two pure-pblang VM hot-path changes were measured and reverted:

- Inline truthiness checks: `fib(12)` median regressed from 8.5676 ms to
  8.9071 ms (4.0% slower).
- `try locals[arg] catch nil` local reads: median 8.5781 ms versus 8.5676 ms,
  no demonstrated improvement.

The existing `_truthy` call and explicit local bounds check remain. This report
does not claim universal language leadership; it records accepted and rejected
work so later optimization rounds begin from evidence.
