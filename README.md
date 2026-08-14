# pblang

**A fast, secure, embeddable programming language with a pblang-written self-hosted toolchain.**

[![Version](https://img.shields.io/badge/version-0.5.7-blue)]() [![License](https://img.shields.io/badge/license-MIT-green)]() [![Tests](https://img.shields.io/badge/tests-passing-brightgreen)]()

pblang is a general-purpose programming language and bytecode VM designed for embedded scripting and standalone tooling. The project includes a Rust compiler/VM toolchain, a pblang standard library, package tooling, LSP support, a native pblang IDE, and a self-hosted compiler written in pblang.

---

## What works today

### Stable / primary surface

- **Stack-based bytecode VM** with closures, modules, errors, arrays/maps, GC support, and runtime diagnostics.
- **Parser, compiler, type checker, linter, formatter, and CLI**.
- **Capability-based embedding controls** through `StdlibConfig`, `VmLimits`, and `FfiPolicy`.
- **Default-on PBLang-native build security analysis** with audited scoped
  suppressions, `protect`/`strict`/`audit` modes, bounded custom rules and an
  explicit opt-out. Selectable quality/performance/completeness analysis is
  available separately through `pbanalyze`; see
  [source security and analysis](docs/Security_Analysis.md).
- **Package manager** with manifests, lockfiles, local and registry dependencies, checksums, and HMAC helpers.
- **Standard library** for files, paths, JSON, strings, arrays, maps, HTTP helpers, bounded memory/SQLite/Redis data stores, provider-neutral edge/regional CDN caching, native SQLite, bounded native Parquet metadata/PLAIN-page reading, crypto, threading, async-style helpers, testing, templates, logging, regex, tensors, and more. `stdlib:onnx.sapp` provides bounded deterministic ONNX protobuf decoding and PBLang-native CPU inference for a documented operator subset, failing closed on unsupported models without Python or ONNX Runtime. `stdlib:command_argv.sapp` converts legacy command text to bounded literal argv without shell evaluation and rejects operators, redirects, substitutions, comments and controls; new APIs should accept structured argv directly. See [data storage](docs/Data_Storage.md) and [CDN caching](docs/CDN_Caching.md).
- **Bounded identity and organization policy** through `stdlib:iam.sapp`:
  users, organizations, memberships, system administrators,
  `owner`/`admin`/`member`/`viewer` RBAC, explicit permissions, tenant-scoped
  queries, and last-owner/last-admin invariants. Authentication and persistence
  remain product-owned trust boundaries; the library never accepts a
  caller-asserted principal as proof of identity.
- **PBLang-native Linux x86-64 process supervision** through `stdlib:linux_process.sapp`: direct `execve(argv)` without shell parsing, private process groups, wall/address-space limits, an independent 64 MiB child-file ceiling, separately bounded retained stdout/stderr, asynchronous `start/poll/cancel/finish`, descriptor-held fixed executable fallback through `run_fixed`, and verified process-group termination. See `examples/bounded_process/main.sapp`. A child that is permitted to create a new session leaves this generic group boundary; sandbox such workloads in a PID namespace.
- **Strict cron expressions** through `stdlib:cron.sapp`: complete bounded
  five-field grammar/domain validation, field-specific diagnostics, Sunday
  `0`/`7` compatibility and deterministic matching. Invalid list members,
  ranges, values and steps fail closed.
- **Carrier-independent transport guard** through
  `stdlib:transport_security.sapp`: identity/certificate/channel pins,
  downgrade and replay rejection, charge-only USB data blocking, and redacted
  anomaly signals without blanket distrust of a carrier type.
- **LSP server** with diagnostics, completions, hover, goto-definition, workspace symbols, and semantic tokens.
- **Native pblang IDE** via `pblang --ide`: code editor, terminal, visual designer, pbAI panel, and pblang-native browser panel.
- **Self-hosted compiler pipeline** under `stdlib/compiler/`, including tokenizer, parser, semantic analysis, type checking, IR, optimizer, codegen, and a pblang bytecode interpreter.
- **Pure-pblang tree interpreter** with prepared AST metadata, cached lexical
  depth, specialized small-arity calls, and compact fixed-slot closures.

### Experimental / preview surface

- **Cranelift JIT** for eligible numeric functions, with fallback to the bytecode VM.
- **C99 transpilers, LLVM IR emitter, and WAT/WASM backend**. The pblang-written
  C99 numeric subset now emits complete hosted programs and can build/run them;
  dynamic language features are still outside that backend subset.
- **AI and quantum integrations**. These are bridges and helpers, not the minimal core language.
- **Polyglot execution**. Useful for trusted workflows; keep it behind `exec` capability.

### Roadmap / research goals

- A fully self-hosted visual Studio written in pblang.
- Bare-metal / OS-development support.
- pbAI as a native pblang model/tooling family.

---

## Quick start

### Prerequisites

- Rust 1.70+

### Build

```bash
git clone https://github.com/PanosBiazis/pblang.git
cd pblang
cargo build --release
```

### Hello World

Create `hello.sapp`:

```pblang
print "Hello, World!";

fn add(a, b) {
    return a + b;
}

print add(2, 3);
```

Run it:

```bash
cargo run -- run hello.sapp
```

### One-line applications

The pblang-only `stdlib/app.sapp` facade builds CLI, native GUI, hardened web,
or composed full-stack
applications from one declarative entry point. A complete CLI application fits
on one physical source line:

```pblang
let app = import("stdlib/app.sapp"); app.run(map{ kind: "cli", handler: fn(ctx) { return "Hello from pblang"; } });
```

The compact helpers remove the envelope as well:

```pblang
import("stdlib/app.sapp").cli("Hello from pblang");
```

Use `kind: "gui"` with a `widgets` array, `kind: "web"` with `routes`, or
`kind: "fullstack"` with `cli`, `gui`, and `web` component maps. Full-stack
`start` explicitly selects the live blocking surface; the other surfaces use
their real preview builders.
`preview: true` builds and exercises the real form/router without opening a
window or socket. Runnable examples are under `examples/one_line_apps/` and are
included in the PBIDE Examples catalog.

For responsive, themeable applications, `stdlib/modern_ui.sapp` adds
row/column/grid/card layouts and modern controls while retaining access to the
lower-level GUI form:

```pblang
let ui = import("stdlib/modern_ui.sapp"); import("stdlib/app.sapp").surface("Tasks", ui.screen([ui.heading("Tasks"), ui.button("Add")]), map{ mode: "auto" });
```

The same validated view tree runs on native desktop and builds a signed Android
APK from PBLang-generated JVM bytecode and hardened responsive Web artifacts
through the same shared components and selectable `app.surface(...)` mode.
iOS, Web and embedded remain explicit adapter contracts and fail clearly until
their real runtimes are linked. Native `gui.button(...)` controls also accept
semantic `primary`, `secondary`, `navigation`, `ghost` and `danger` variants;
older calls remain primary by default and custom palettes can override the
corresponding selected, focus, danger and on-accent roles.
See [`docs/MODERN_UI.md`](docs/MODERN_UI.md).

### One-line CDN-backed static site

The pure-PBLang CDN control layer combines a safe static origin with bounded
edge and regional caches, ETag/304 handling, SWR, stale-if-error, and purge:

```pblang
let site = import("stdlib:cdn_cache.sapp").static_site("public");
```

This is executable cache behavior for an edge/origin-shield process; actual
geographic POP deployment remains an infrastructure concern. See
[`docs/CDN_Caching.md`](docs/CDN_Caching.md) and `examples/cdn_cache/`.

### Self-hosted hosted C99

`stdlib/compiler/ctranspile.sapp` can now produce a complete userspace
translation unit and an optimized executable without using the Rust
transpiler:

```pblang
let c99 = import("stdlib/compiler/ctranspile.sapp");
let result = c99.build_program("fn sq(x) { return x*x; } print sq(9);", "square", map{ profile: "speed" });
```

`run_program(...)` adds a bounded watchdog and temporary-artifact cleanup.
Normal mode preserves f64 arithmetic. `numeric: "integer"` is explicit and
opt-in because integer division is not equivalent to normal pblang semantics.

Adapters load lazily by application kind. On the recorded development host,
twenty cold CLI processes fell from 1,182 ms with eager GUI/web imports to 77 ms
with lazy loading. Reproduce the warm component costs with
`pblang run bench/app_facade_stages.sapp --allow-all`.

For bounded Linux control-plane connections, `stdlib:native_tcp.sapp` provides
nonblocking IPv4 connect/accept/read/write deadlines, ephemeral listeners,
independently owned accepted sockets, close-on-exec descriptors and
binary-safe UTF-8 copying. Unsupported ABIs fail closed; this is not a TLS
layer. See [`docs/NATIVE_TCP.md`](docs/NATIVE_TCP.md).

---

## Common commands

| Command | Description |
|---|---|
| `pblang run <file|dir>` | Run a script or package. |
| `pblang <file|dir>` | Short `run` alias; recognized source/bytecode paths never enter the REPL. |
| `pblang test [dir]` | Run pblang tests. |
| `pblang fmt [files...]` | Format source files. |
| `pblang lint [files...]` | Parse, type-check, and lint files. |
| `pblang build [path]` | Generate lockfile and type-check the package entry. |
| `pblang run tools/pbbuild.sapp --allow-all -- SOURCE OUTPUT` | Plan and run a cached adaptive native build with explicit fallbacks. |
| `pblang run tools/pbsecure.sapp --allow-all -- SOURCE` | Dependency-wide policy and interprocedural semantic taint/control-flow audit. |
| `pblang run tools/pbaudit.sapp --allow-all -- --module-by-module --resume --json-output=audit.json roots...` | Atomic resumable per-module repository triage without a shared finding early-stop or raw source lines in reports. |
| `pblang init` | Create a `pblang.toml`. |
| `pblang new <name>` | Scaffold a new project. |
| `pblang install` | Install dependencies. |
| `pblang publish` | Publish a package. |
| `pblang search <query>` | Search the package registry. |
| `pblang bundle [--self-extract]` | Build a portable bundle or self-extracting executable. |
| `pblang lsp` | Start the language server. |
| `pblang --ide` | Launch the native pblang IDE. |
| `pblang --check <file>` | Parse and type-check only. |
| `pblang --dump-bytecode <file>` | Print bytecode disassembly. |
| `pblang --cfg <file>` | Dump a control-flow graph. |

Large product test runners can use `stdlib:test_scheduler.sapp` to cap workers
by CPU and available memory. Physical-device and sustained-soak profiles always
serialize access to each device and expose adaptive, fail-closed battery,
temperature and memory policy. Warm test caches remain an optimization, not an
evidence substitute; product runners must invalidate them from the complete
import/source graph.

---

## Language overview

```pblang
let name = "Alice";
let nums = [1, 2, 3];
let config = map{host: "localhost", port: 8080};

fn greet(name) {
    return "Hello, " + name + "!";
}

if len(nums) > 0 {
    print greet(name);
}

for x in nums {
    print x;
}

let result = match 200 {
    case 200: "OK",
    case 404: "Not Found",
    case _: "Unknown"
};

print result;
```

The PBLang-native compiler also accepts value-producing `elif` chains:

```pblang
let class_name = if score >= 90 { "excellent" }
    elif score >= 75 { "good" }
    elif score >= 50 { "pass" }
    else { "retry" };
```

`elif` and the older `else if` spelling lower to the same conditional AST, so
branch selection, type widening, bytecode and C99 output remain compatible.
Chains are bounded to 256 arms. During bootstrap migration, compile this exact
expression spelling through the PBLang-native `tools/pbselfc.sapp`; the retained
host parser continues to accept the equivalent `else if` spelling.

---

## Tensor compute and hardware acceleration

pblang ships CPU tensor kernels (the `tensor_*` builtins). Matrix multiply is the hot
path and has two backends:

- **CPU (default).** `tensor_matmul` runs **multi-core** via rayon, parallelised over
  output rows. It is **deterministic and exact (f64)**: identical inputs produce
  bit-identical results regardless of core count, so the default build is fully
  reproducible.
- **GPU (opt-in).** Build with the `gpu` feature to compile a wgpu compute-shader matmul,
  then enable it at runtime:

  ```bash
  cargo build --release --features gpu
  PBLANG_GPU=1 pblang run your_script.sapp
  ```

  The GPU path is **f32** (GPUs lack f64 in compute shaders), so it trades precision for
  throughput on large matmuls — which is exactly why it is opt-in and the default stays
  exact f64 on the CPU. A `gpu`-enabled binary still runs everything on the CPU unless
  `PBLANG_GPU=1` is set, and only large matmuls are offloaded.

Query what the running binary actually supports with the `tensor_backend()` builtin:

```pblang
print tensor_backend();
// { cpu_threads, parallel_matmul,
//   gpu_feature, gpu_available, gpu_enabled, gpu_adapter, note }
```

`gpu_feature` is `true` only when built with `--features gpu`; `gpu_enabled` is `true`
only when offload actually ran (feature on + `PBLANG_GPU=1` + a matmul large enough to
offload). With the `gpu` feature, `tensor_backend().gpu_adapters` lists **every** GPU wgpu
can see (name/type/backend — discrete, integrated, and software). Only one adapter is used
at a time (pblang does not split a matmul across GPUs); choose which with
`PBLANG_GPU_ADAPTER` — an index, a device type (`integrated`/`discrete`/`cpu`), or a name
substring — so e.g. an integrated GPU can be selected explicitly. There is **no NPU
compute backend**: NPUs are driven by vendor stacks
(ONNX Runtime + Vitis AI / OpenVINO / QNN, on quantized graphs) that don't map to
pblang's f64 tensor kernels, so a present NPU is detectable but not usable for compute
from pblang.

> Windows build notes: if cargo fails TLS with `CRYPT_E_NO_REVOCATION_CHECK`, set
> `CARGO_HTTP_CHECK_REVOKE=false`. The `gpu` feature needs the `HighPerformance` adapter
> to pick a discrete GPU rather than the software (WARP) renderer — pblang already
> requests it.

### Arbitrary-width floats (`fN`)

`stdlib/floatn.sapp` is a pblang-native engine for representing and quantizing floats in
**any** bit width — `f0`, `f1`, `f2`, `f4`, `f8` (`f8e4m3`/`f8e5m2`), `f16`, `bf16`,
`tf32`, `f32`, `f64`, `f128`, and arbitrary custom `fN`. It powers low-bit ML quantization
(shrinking model weights), minifloats for old/embedded targets, and is the foundation for
future language-level `fN` types.

```pblang
let F = import("stdlib:floatn.sapp");
F.quantize(3.14159, "bf16")    // 3.140625   (nearest bf16 value, as f64)
F.info("f8e4m3").max_finite    // 448
F.matmul(A, B, "f16")          // emulated low-precision matmul (rounds each result to f16)
let bytes = F.pack(weights, "f8e4m3")   // 4x smaller than f64 on disk
```

**Honest model:** hardware computes only in f32/f64, so the engine computes in f64 and
uses `fN` as a representation/quantization layer (`fN` arithmetic = compute in f64, round
to `fN`). f64 is the precision ceiling: wider formats (`f128`, big `fN`) are *described*
and quantized within f64 precision, but their exact >52-bit patterns can't be assembled in
an f64 — `info().encodable`/`faithful` report this and `encode()` raises rather than lie.

### Machine inventory (`hwinfo`)

`stdlib/hwinfo.sapp` detects **every device** on the machine — CPU, RAM, drives (SSD/HDD),
every GPU (integrated + discrete), NPU, network and sound cards, and a class histogram of
all other PnP devices — **vendor-agnostically** (NVIDIA, AMD, Intel, Apple, Qualcomm, …;
`vendor(name)` classifies any device). `survey()` returns the whole inventory and
`compute_plan()` auto-selects the best compute path.

```pblang
let hw = import("stdlib:hwinfo.sapp");
hw.survey();         // full inventory (needs --allow-exec for live probing)
hw.compute_plan();   // best CPU/GPU config; lists everything else as not-compute
```

**Honest by construction:** only the **CPU** (all cores) and a **GPU** (opt-in, one adapter
at a time) can be *used for compute*. NPU/RAM/disks/network/sound are detected but are
storage and I/O devices, not processing units — the library says so rather than pretend
otherwise. Detection uses `proc_spawn_wait` + CIM on Windows; other platforms degrade to
notes.

---

## Project layout

```text
pblang/
├── src/main.rs              # CLI entry point and command dispatch
├── src/cli/                 # CLI helpers and reports
├── crates/                  # Rust workspace crates
│   ├── pblang-core/         # Parser and AST
│   ├── pblang-ir/           # Bytecode, opcodes, values
│   ├── pblang-vm/           # VM, builtins, GC, sandbox hooks
│   ├── pblang-compiler/     # AST → bytecode compiler
│   ├── pblang-typeck/       # Type checker
│   ├── pblang-lint/         # Lints
│   ├── pblang-pm/           # Package manager
│   ├── pblang-lsp/          # Language server
│   ├── pblang-cranelift/    # JIT / AOT prototype backend
│   ├── pblang-ctranspiler/  # C99 transpiler
│   ├── pblang-llvm/         # LLVM IR emitter
│   ├── pblang-wasm/         # WAT/WASM emitter
│   ├── pblang-ai/           # AI provider helpers
│   ├── pblang-quantum/      # Quantum bridge
│   └── pblang-stub/         # Self-extracting bundle launcher
├── stdlib/                  # pblang standard library modules
├── stdlib/compiler/         # self-hosted compiler written in pblang
├── editors/ide/             # native pblang IDE
├── tests/                   # Rust and pblang tests
├── examples/                # example projects
├── docs/                    # design, safety, tooling, and status docs
└── benches/                 # benchmarks
```

`tools/pblock.sapp` is the PBLang-native publication step for local path
dependencies while the retained bootstrap package manager remains available.
It validates `pblang.toml`, preserves content digests, rejects absolute or
mismatched dependency roots, and atomically rewrites only local lock paths. Run
it as trusted maintenance code with
`pblang run tools/pblock.sapp --allow-all -- <project>`. Nested source
dependencies should not ship their own bootstrap lock
until the self-hosted loader owns nested relative-path resolution end to end.

See [`STRUCTURE.md`](STRUCTURE.md) for the crate map and [`docs/CURRENT_STATUS.md`](docs/CURRENT_STATUS.md) for the current test/status snapshot.

---

## Embedding safely in Rust

```rust
use pblang_core::parse;
use pblang_compiler::compile;
use pblang_vm::{StdlibConfig, VmLimits, VM};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"print "Hello from embedded pblang!";"#;
    let ast = parse(source)?;
    let chunk = compile(ast)?;

    let config = StdlibConfig {
        core: true,
        fs: false,
        net: false,
        http: false,
        json: true,
        crypto: true,
        sqlite: false,
        ffi: false,
        exec: false,
        gfx: false,
        spawn: false,
        env: false,
    };

    let mut vm = VM::with_stdlib(config);
    vm.set_limits(Some(VmLimits {
        max_instructions: Some(100_000),
        max_stack_depth: Some(64),
        ..Default::default()
    }));

    vm.run(&chunk)?;
    Ok(())
}
```

Use `fs`, `net`, `http`, `exec`, `ffi`, and `env` only for trusted scripts or inside an OS-level sandbox.

The standard library also includes bounded native model-container support:
`stdlib/onnx.sapp` executes a deterministic CPU subset of ONNX without Python
or ONNX Runtime, while `stdlib/gguf.sapp` converts dense F32/F16/BF16
Safetensors snapshots to deterministic GGUF v3 and validates the complete
published container. Unsupported operators, tensor types, dynamic shapes,
malformed offsets, and exceeded resource budgets fail closed.

`stdlib/linux_pidfd.sapp` provides race-resistant Linux process identity and
signalling through `pidfd_open` and `pidfd_send_signal`. It exposes no classic
PID-based fallback: unsupported kernels/libcs, PID 1, invalid signals, stale
handles, and closed handles fail closed.

---

## Documentation

- [`docs/USAGE_GUIDE.md`](docs/USAGE_GUIDE.md)
- [`docs/TUTORIALS.md`](docs/TUTORIALS.md)
- [`docs/Language_Semantics.md`](docs/Language_Semantics.md)
- [`docs/Tooling.md`](docs/Tooling.md)
- [`docs/Safety.md`](docs/Safety.md)
- [`docs/Capability_model.md`](docs/Capability_model.md)
- [`docs/PB_IDE.md`](docs/PB_IDE.md)
- [`docs/Visual_Designer.md`](docs/Visual_Designer.md)
- [`docs/Performance_Contracts.md`](docs/Performance_Contracts.md)
- [`docs/CURRENT_STATUS.md`](docs/CURRENT_STATUS.md)
- [`docs/Tasks.md`](docs/Tasks.md)

---

## Testing

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo build --workspace --examples
```

---

## License

MIT. See [`LICENSE`](LICENSE).
