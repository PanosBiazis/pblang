# pblang application facade benchmark

`stdlib/app.sapp` provides one declarative pblang entry point for CLI, GUI, and
web applications. The facade uses lazy adapters so a CLI process does not parse
or initialize GUI, HTTP, routing, and security modules.

## Cold startup

Twenty independent executions of `examples/one_line_apps/main.sapp` were timed
with nanosecond wall-clock timestamps around the complete process loop.

| Implementation | 20 processes | Average |
|---|---:|---:|
| Eager GUI/web/security imports | 1,182 ms | 59.1 ms |
| Lazy kind-specific imports | 77 ms | 3.85 ms |
| Change | -93.5% | -93.5% |

## Warm in-process phases

| Phase | Iterations | Average |
|---|---:|---:|
| Validate CLI spec | 1,000 | 0.0042 ms |
| Plan GUI capabilities | 500 | 0.0086 ms |
| Dispatch CLI handler | 500 | 0.0107 ms |
| Build three-widget GUI preview | 100 | 0.0811 ms |
| Build hardened web preview | 50 | 0.1481 ms |

Every benchmark path performs a correctness probe before timing. Raw output is
in `metrics.txt`; the runnable pblang benchmark is
`bench/app_facade_stages.sapp`.
