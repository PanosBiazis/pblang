# Bounded source linker

`stdlib:source_inliner.sapp` keeps the legacy resolver APIs for compatibility and
adds `link_modules(main_source, modules, options)` for product/compiler use. The
new API consumes an immutable map of virtual path to pblang source and performs
no filesystem, process, or network operation.

## Contract

Paths are canonicalized relative to the importing virtual file. Project paths
and the `stdlib:` virtual scheme are supported. Dependencies are emitted before
their importers, and each module receives a deterministic `__pbiN_` namespace so
independent modules can define the same names without collision. Imports and
exports are removed only after string/comment-aware scanning.

Successful results include flattened `source`, dependency order, reachable and
supplied byte counts, per-module byte counts, output bytes, entry path,
namespace strategy, and effective limits. `ctranspile.link_program(...)` adds a
hosted-C99 compatibility gate; `ctranspile.transpile_program_linked(...)` returns
portable C source plus compact link metadata.

## Safety limits

Defaults and hard maxima are:

| Limit | Default | Hard maximum |
|---|---:|---:|
| modules | 256 | 4,096 |
| dependency depth | 64 | 256 |
| source bytes per module | 1 MiB | 16 MiB |
| total supplied project bytes | 8 MiB | 64 MiB |
| flattened output bytes | 16 MiB | 128 MiB |
| virtual path bytes | 1,024 | 4,096 |

The linker rejects absolute paths, backslashes, foreign schemes, control
characters, root traversal, duplicate canonical paths, unresolved imports,
cycles, nested imports, bare module-binding use, generated `__pbi*` top-level
names, and non-text module values. Both reachable and unused supplied modules
are charged to snapshot limits.

## Hosted C99 integration

The generic linker preserves module top-level values and records which modules
define them. Hosted C99 emits those names as static translation-unit values and
initializes them once in dependency/source order from generated `main`. Module
functions can read and mutate persistent globals; module closures can capture
initialized values; global closure handles and named function values use the
checked dynamic-call runtime. Float and explicit integer profiles share this
lowering. Generated-runtime/C-name collisions are rejected before returning C.

The current hosted representation remains the backend's numeric value subset;
it is not yet a general tagged/GC-managed value runtime or a thread-safe module
initialization protocol. A project loader may read approved files before calling
the linker, but the linker itself never crosses that capability boundary.

Verification: `source_linker_bounded_test.sapp`, `source_inliner_test.sapp`,
`source_inliner_comment_import_test.sapp`, `source_inliner_prefixed_test.sapp`,
`ctranspile_hosted_test.sapp`, `ctranspile_module_globals_test.sapp`,
`execution_pipeline_test.sapp`, and
`ide_source_build_test.sapp`. Phase metrics are in
`bench/selfhost_c99_stages.sapp`.
