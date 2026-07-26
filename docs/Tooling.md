# pblang Tooling

This document is the canonical tooling reference for current behavior. Historical milestone sections are kept for context but may reference older phase names.

## CLI commands

- **`pblang init`** — Create `pblang.toml` in the current directory (package name `my-app`, version `0.1.0`, empty dependencies). Fails if the file already exists.
- **`pblang run [file.sapp|dir] [-- arg1 arg2 ...]`** — Execute a script and exit. Arguments after `--` are passed to the script and available via `os.args()` (from `stdlib:os.sapp`). If you pass a directory (or run `pblang run` from inside a manifest dir), pblang resolves the package entry from `pblang.toml`: first `[package].entry`, otherwise `main.sap`, `main.sapp`, `src/main.sap`, `src/main.sapp`.
- **`pblang compile <file.sapp> [--out <name>] [--opt <level>] [--llvm|--wasm]`** — Compile through backend flows. Default is C99 transpiler + native compiler; `--llvm` uses LLVM backend with compatibility fallback, `--wasm` emits `.wat`. `--llvm` and `--wasm` are mutually exclusive.
- **`pblang serve [--port <1-65535>]`** — Start the local IDE backend HTTP server (default `8090`) used by `editors/ide/index.html`.
  - `POST /run` — parse/typecheck/compile/run, returns `{ ok, stage, output, error, diagnostics[], duration_ms }`
  - `POST /check` — parse/typecheck only, returns `{ ok, stage, diagnostics[] }`
  - `POST /format` — normalize/format source for IDE editing, returns `{ ok, code }`
  - `POST /normalize` — apply compatibility normalizations (for example `print(...)` call style → statement form), returns `{ ok, code }`
  - `GET /health` — liveness probe for IDE status indicator
  - `GET /examples` — list available built-in project examples as `{ ok, examples: [{ id, title }] }`
  - `GET /examples/<id>` — fetch example source as `{ ok, id, title, code }` for IDE dropdown loading
  - `GET /manual/ide` — fetch IDE manual markdown as `{ ok, id, title, content }`
  - `GET /manual/project` — fetch project manual markdown as `{ ok, id, title, content }`
  - IDE frontend renders diagnostics as clickable entries that jump the cursor to `(line:col)` in the editor.
  - IDE diagnostics now expose quick-fix actions (per-item and batch auto-fix) for common issues.
  - IDE editor autosaves content in browser `localStorage` and supports manual restore.
  - IDE supports diagnostics keyboard navigation (`F8`/`Shift+F8`) with active-item highlighting and prev/next controls.
  - IDE output panel is resizable via drag splitter; theme, output width, and example filter are persisted in local storage.
  - IDE keeps a local **Run/Check history** (latest 30 entries) with timestamp, status, summary, replay (`Show`) and optional `Load code` snapshot restore.
- **`pblang --check <file>`** — Parse and typecheck; print diagnostics.
- **`pblang --doc <file> [--html] [out_dir]`** — Print module docs (exports, top-level functions, polyglot blocks). Polyglot blocks now include parsed attrs and code-line counts in both text and HTML output, so the current block contract is more inspectable from tooling. With `--html`, write `out_dir/index.html` (default `docs/`).
- **`pblang --lock [--registry-index \<url\>] [--require-registry-sha256] [path]`** — Generate `pblang.lock` from `pblang.toml`. With **`--registry-index \<url\>`**, registry dependencies are resolved from that index for this invocation instead of relying on the environment. With **`--require-registry-sha256`**, strict checksum mode is enabled explicitly for this command even if the env var is unset. See [Lockfile_contract.md](Lockfile_contract.md).
- **`pblang test [dir|profile] [--profile semantics|llm] [--filter <substr>] [--exact <id>] [--skip <substr>] [--quiet] [--jobs N|auto|smart] [--cache] [--timings] [--slow N] [--fail-fast] [--list]`** — Discover `fn test_*` functions in `.sap`/`.sapp`/`.bpx` files under `dir` (default: `tests/`) and run them with cargo-style output. Files with no `test_*` functions run as one legacy whole-file test. Profiles reduce repeated long commands: `pblang test semantics` expands to `tests/semantics --allow-all --quiet --jobs smart`; `pblang test llm` adds `--filter llm --timings --slow=3`. `--filter` runs matching ids and reports the real `filtered out` count; `--exact` runs only the exact discovered test id; `--skip` excludes matching ids and can be repeated; `--quiet` captures script `print` output while keeping status/failure lines; `--jobs N` runs independent test files in isolated VM workers and prints results in deterministic file order (`--jobs auto` or `--jobs 0` uses available CPU parallelism). `--jobs smart` caps workers by CPU and available RAM (`/proc/meminfo` on Linux), with `PBLANG_TEST_MAX_JOBS`, `PBLANG_TEST_RAM_MB`, and `PBLANG_TEST_WORKER_MB` overrides for old laptops, CI, containers, and memory-heavy tests. `stdlib:test_scheduler.sapp` adds a PBLang-native workload-aware planner with explicit host reserve and conservative `large-product` estimates; product release tools should use it when each worker loads a large independent graph instead of assuming the generic 96 MiB CLI estimate. `--cache` reuses unchanged passing tests; `--timings` prints per-test duration; `--slow N` implies timings and prints the N slowest tests; `--fail-fast` stops after the first failure; `--list` prints discovered test ids without executing code. Accepts the same capability flags as `run`.
- **`pblang run tools/pbportfolio.sapp --allow-all [--max-depth=N] [--max-entries=N] [--max-children=N] [--limit=N] [--lines=N] [--inventory] [roots...]`** — Run the pblang-only metadata inventory and portfolio action planner. It walks directory names only, never reads file contents, skips secret/cache/build paths, caps depth/entry counts, and prints ranked next tasks for pblang, pbAI, pbOS, PBIDE, pb-mcp-server, and PBRemote. With no roots, it uses the current sibling `products/` checkouts from the pblang checkout; pass explicit roots such as `.`, `../pbai`, `./local_media or `~` for broader bounded surveys.
- **`pblang run tools/pbaudit.sapp [--project=name] [--max-depth=N] [--max-entries=N] [--max-children=N] [--max-files=N] [--max-file-bytes=N] [--max-findings=N] [--backlog-limit=N] [--lines=N] [roots...]`** — Run the pblang-only bounded text audit. It first inventories caller-approved roots with the same sensitive/cache/build pruning as `pbportfolio`, then reads only small text-like files and feeds them into `stdlib:project_audit.sapp`. Output is a safe summary/backlog: it does not echo raw finding lines or secret values, and it does not need `--allow-all` because filesystem reads are already in the default trusted CLI capability set. `--backlog-limit=N` deepens the redacted task list without widening filesystem reads. Exit code is nonzero when the quality gate blocks.
- **`pblang run tools/pbscan.sapp <file>`** or **`pblang run tools/pbscan.sapp --text <text>`** — Run the defensive PBLang secret/input scanner. Named GitHub, Slack, AWS, JWT and private-key formats retain explicit detection; generic high-entropy tokens require at least 24 characters. Common source/release artifact suffixes and short mixed protocol labels are excluded to avoid treating filenames such as Android APKs as credentials. Findings are redacted and the CLI returns a nonzero CI exit code.
- **`pblang run tools/pbmigrate.sapp [--json] [--lines=N] <relative-manifest.json>`** — Run the pure-PBLang implementation/migration gate over a bounded `pblang.migration.change_manifest.v1` JSON file. It always rejects new Rust files and added Rust lines, requires exact expiring exceptions for necessary foreign/system boundaries, validates parity evidence before retirement, never invokes git/shell/process helpers, and exits nonzero when blocked. See [PBLANG_ONLY_POLICY.md](PBLANG_ONLY_POLICY.md) and the passing `examples/migration_policy_manifest.json`.
- **`pblang run tools/pbaap_status.sapp [--drill] [--source-gate|--strict-gate] [--no-portfolio] [--max-depth=N] [--max-files=N] [--max-file-bytes=N] [--lines=N]`** — Run one bounded status pass for the current AAP product repositories from the pblang checkout: pblang/PBIDE, pbAI, pbOS, pb-mcp-server, and PBRemote. It composes `project_text_audit` and the metadata-only six-product portfolio planner into one safe summary, never prints raw source lines, includes a ranked `next focus` queue, and exits nonzero only when a project is truly `blocked`. With `--drill`, it appends safe detail for the top-ranked focus plus the recommended follow-up command.
- **`pblang run tools/pbbuild.sapp --allow-all -- [--project] SOURCE_OR_DIR OUTPUT [--goal=balanced|speed|size|debug|portable] [--backend=auto|direct-x64|c99|pbck] [--target=NAME] [--dry-run] [--no-cache] [--no-fallback] [--runtime-fallback]`** — Run the PBLang-native adaptive build orchestrator. Auto mode prefers the safety-certified direct x86-64 backend only on a compatible Linux x86-64 host, reuses the verified content cache, then uses the existing hosted C99 path when fallback is permitted. Hosted builds use `PBLANG_BIN` when explicitly configured; otherwise Linux resolves the current command before consulting `PATH`, preventing an older system installation from being selected silently. A PBCK fallback is never silently substituted for a standalone executable; it must be requested through the portable goal, explicit backend, or `--runtime-fallback`. `--project` validates `pblang.toml`, confines its entry to the project, and builds that entry. Every result includes the selected strategy, failed attempts, target, artifact kind, byte count and content digest.
- **`pblang fmt [--max-line-length=N] [file.sap …]`** — Format one or more `.sap`/`.sapp` files: trim lines, **spacing around binary operators** (`+` `-` `*` `/` `==` `!=` `<` `>` `<=` `>=`), **comma spacing** (space after comma; unchanged inside string literals), indent by brace depth (4 spaces per level), optional **line wrap** when `--max-line-length=N` is set (wraps at last space before N), ensure final newline, and insert a single blank line between top-level `fn` declarations. Does not change content inside string literals. Only overwrites if content changed.
- **`pblang lint [file.sap …]`** — Parse and typecheck each file; print diagnostics (errors and warnings). Reports **unused variable**, **unused import** (bindings never read), and **variable shadows outer binding** (L2-beta) warnings. Exit 1 only on parse or typecheck errors; warnings do not change exit code.

## Error UX (L2-gamma)

- Parse and runtime errors print **`error[<file>:<line>:<col>]: <message>`** when location is available.
- If location is unavailable (e.g. REPL), falls back to **`error[<line>:<col>]: <message>`** or **`error: <message>`**.
- Example: **`error[src/main.sapp:3:5]: unexpected token ';'`**
- Self-hosted type and lint diagnostics use a shared bounded typo engine for
  unknown identifiers, struct fields, and enum variants. It returns structured
  `suggestion`, `help`, and replacement `fix` fields where a close candidate
  exists. Names, candidate counts, and edit distance are capped, and ties are
  deterministic. Project compilation supplies declarations from other source
  files through the bounded `extern_names` typecheck option. A missing global
  inside a protected `try` body remains a catchable runtime operation; outside
  `try`, the same reference is an `undefined-name` compilation error.
- **`pblang build [--registry-index \<url\>] [--require-registry-sha256] [path]`** — Generate `pblang.lock` from manifest dir, then typecheck the resolved package entry. With **`--registry-index \<url\>`**, registry dependencies are resolved explicitly for this build. With **`--require-registry-sha256`**, strict checksum mode is enabled explicitly for this command. Exit 1 on lock or typecheck failure.
- **`pblang install [--registry-index \<url\>] [--require-registry-sha256] [path | <name>@<version>]`** — Resolve dependencies and generate `pblang.lock` when given a manifest **`path`** (existing behavior), or **fetch a single registry package** when invoked as `pblang install <name>@<version>`. With **`--registry-index \<url\>`**, registry dependencies (names starting with `@`) are resolved from that index (file:// or http(s)://). Static registry indexes may point to tarballs with either absolute URLs or paths relative to `index.json`. Git dependencies can use `{ git = \"...\", rev = \"...\" }` and are installed as pinned cached checkouts. Registry tarballs are unpacked with a safe-entry policy: paths must stay relative inside the cache destination, and link entries are rejected. **Verify on install (E4-beta):** when **`PBLANG_SIGNING_KEY`** is set and a `.sha256` sidecar exists at the tarball URL (same URL + `.sha256`), install verifies HMAC-SHA256 of the tarball before unpacking and rejects with an error on mismatch; if the key is set but no sidecar is found, a warning is printed and install continues. Third-party registry/git packages are validated during install: a declared `[package].entry` must resolve to an in-package file, otherwise the package must still contain at least one `.sap`/`.sapp` module. Otherwise uses **`PBLANG_REGISTRY_INDEX`** env var if set for registry deps. **`REQUIRE_REGISTRY_SHA256=1`** (or `true`) or **`--require-registry-sha256`** forces index entries to include sha256. **M4-alpha:** in `<name>@<version>` mode, `pblang install` resolves the highest semver version satisfying `<version>` from the hosted index, downloads and unpacks it into the pblang cache, and prints `Installed <name>@<resolved_version>`.
- **`pblang publish --registry-dir \<dir\> [path]`** — Pack current package (from `pblang.toml` [package] name/version) into \<name\>-\<ver\>.tar.gz, write to \<dir\>, and update \<dir\>/index.json. If `[package].entry` is declared, `publish` validates that it is a relative in-package path and that the target is an existing file before writing any registry artifact. In the current local-registry MVP, published packages must also be **self-contained**: if `[dependencies]` is non-empty, `publish` fails instead of producing a non-portable registry artifact. Packages without explicit `[package].entry` are still allowed, but they must expose at least one pblang module file: either a default bare-import entry (`lib.sapp`, `lib.sap`, `main.sapp`, `main.sap`) or another `.sap`/`.sapp` module for package subpath imports. Other projects can then run `pblang install --registry-index file:///path/to/dir/index.json`.
- **`pblang publish`** — Hosted-style publish flow built on the same packaging and validation rules:
  - Reads **`PBLANG_REGISTRY_INDEX`** as registry base URL. For `file://` URLs pointing at a registry directory, `pblang publish` behaves as a thin wrapper around `pblang publish --registry-dir <dir>` (local file-based registry) so that tests and local workflows can use the same command.
  - For http(s) registries, `pblang publish` builds a tarball of the current package root, computes its sha256, and POSTs a multipart/form-data body to `<registry>/publish` with fields `tarball` (bytes), `sha256` (hex), and `metadata` JSON (`{name, version, description}`).
  - Auth: Bearer token from **`PBLANG_REGISTRY_TOKEN`** or optional CLI flag `--registry-token=<token>`.
- **`pblang search <query>`** — Search a hosted or file-based registry for packages:
  - Reads **`PBLANG_REGISTRY_INDEX`** as registry base URL.
  - For `file://` URLs, reads `<dir>/index.json` directly and returns packages whose names or versions contain `<query>`. Output lines are `name @ version` (description is not stored in the local index).
  - For http(s) URLs, calls `<registry>/search?q=<query>` and expects a JSON array of `{name, version, description}`; prints one line per result as `name @ version — description`, or `No results for '<query>'` when there are no matches.
- Local registry publish is **immutable per package name + version**: republishing the same version with identical content is allowed (idempotent success), but republishing the same version with different content fails.
- After `pblang install`, package imports can use the dependency name directly, e.g. `import("@pblang/http")`, or a package subpath, e.g. `import("@pblang/http:src/router.sapp")`. The resolver loads the installed package root from `pblang.lock`, first honors optional `[package].entry = "src/lib.sapp"` from that package's `pblang.toml`, and otherwise falls back to `lib.sapp` / `lib.sap` / `main.sapp` / `main.sap`. Both `entry` and package subpaths must be relative, stay inside the package root, and resolve to files rather than directories.
- **`pblang pack [--registry-index \<url\>] [--require-registry-sha256] [path] [out_file]`** — Stage A executables: δημιουργεί ένα ενιαίο `.tar.gz` artifact με το περιεχόμενο του πακέτου (manifest dir), συμπεριλαμβανομένων `pblang.toml`, ενημερωμένου `pblang.lock` και τοπικών dependencies. Το default `out_file` είναι `dist/app.tar.gz` μέσα στο manifest dir. **E4-gamma:** Before packing, manifest metadata is validated: `[package].target_platform` (if present) must be one of `any`, `linux`, `macos`, `windows`; `[package].min_runtime` (if present) must be a valid MAJOR.MINOR.PATCH version. Pack fails with a clear error otherwise. Αν το package έχει registry dependencies, το `pack` μπορεί πλέον να ξαναγεννήσει το lockfile με ρητό **`--registry-index \<url\>`** και **`--require-registry-sha256`** χωρίς να απαιτεί exported env var. Το `pack` κάνει validate ότι υπάρχει runnable package entry, ότι τα local path dependencies μένουν μέσα στο package root, και τυπώνει το resolved entry path. Η γραμμή επιτυχίας περιλαμβάνει προαιρετικά **name@version** και **description** από το `[package]` του manifest όταν υπάρχουν, και **οδηγία εκτέλεσης** (run hint): «To run: unpack the tarball and run 'pblang run .' in the unpacked directory.» Για να τρέξεις το app: αποσυμπίεσε το tarball και εκτέλεσε `pblang run` ή `pblang run .` από τον unpacked φάκελο (χρησιμοποιεί το εγκατεστημένο `pblang` binary, δεν χρειάζεται project folder στο origin).
- **`pblang bundle [--registry-index \<url\>] [--require-registry-sha256] [--self-extract] [path] [out_dir]`** — Create a portable folder (default `dist/`) containing the current `pblang` executable, `pblang.toml`, `pblang.lock`, the resolved package entry, and local dependencies. **E4-gamma:** Same manifest metadata validation as `pack` (target_platform, min_runtime) before bundling. The bundled entry now preserves its relative path inside the package (for example `src/app.sapp`), so bundled apps with `[package].entry` work consistently. `bundle` now regenerates `pblang.lock` with the same registry flags as `pack`, so projects with registry dependencies can bundle through an explicit registry URL and strict-checksum CLI policy instead of relying only on env vars. Success output optionally includes **name@version** and **description** from `[package]` when present, and a **run hint**: «To run: cd to the bundle dir and run './pblang run .' (`pblang.exe run .` on Windows).» Local path dependencies must stay inside the package root; escaping or external paths are rejected instead of producing misleading bundles. Run from that folder with `./pblang run .` (or `pblang.exe run .` on Windows). When running a packaged app (`pblang run .` from a manifest dir), **info** lines are printed to stderr when `target_platform` is not `any` or when `min_runtime` is set. **E5-beta:** the optional `--self-extract` flag produces a single self-extracting executable instead of a bundle directory: it stages the package (source, `pblang.toml`, `pblang.lock`) plus the current `pblang` runtime under `bin/`, tars that, and concatenates it onto the `pblang-stub` launcher behind the `PBLANG_BUNDLE_START\x00` sentinel. Output defaults to the `[package].name` (`.exe` on Windows) or the path given by `--output`. Requires `pblang-stub` to be built/installed alongside the `pblang` executable (see `docs/Native_backend_notes.md`).
- **`pblang --dump-bytecode <file>`** — Compile the file and print bytecode disassembly (constants + instructions). For debugging and reverse engineering.
- **`pblang --call-graph <file>`** (R2-alpha) — Print the call graph of a pblang source file as plain-text edges: `caller -> callee` (one per line). Top-level calls attributed to `<main>`. Functions with no outgoing calls print `funcname -> (no calls)`.
- **`pblang lsp`** / **`pblang lsp`** (through L3-gamma) — Start a JSON-RPC 2.0 LSP server on stdin/stdout. Supports **initialize** (completionProvider, textDocumentSync, hoverProvider, definitionProvider), **initialized**, **textDocument/didOpen**/**didChange** (tracks open-file contents; didOpen is parsed with JSON unescaping so newlines in `text` are real line breaks), **textDocument/completion**, **textDocument/hover** (builtins and top-level locals, returned as Markdown content), **textDocument/definition** (jump to `fn`/`let` declaration line for locals), **shutdown**, **exit**.

## L3-beta: LSP scope-aware completion

`pblang lsp` tracks open files via `textDocument/didOpen` and
`textDocument/didChange` and includes local symbols in completion:

- Top-level `fn <name>` declarations from the open file.
- Top-level `let <name>` bindings from the open file (best-effort, line-based scan).
- All VM builtins (unchanged from L2-delta).

Always send `textDocument/didOpen` or `textDocument/didChange` before requesting `textDocument/completion` so that the server sees the latest document text.

## L3-gamma: LSP hover and go-to-definition

- **`textDocument/hover`**: If the identifier under the cursor matches a builtin or a top-level local from the open file, the server returns markdown content (`builtin`, `local fn`, or `local let`). Otherwise `null`. This is implemented, but still intentionally shallow compared with full cross-file/type-aware semantic hover.
- **`textDocument/definition`**: For a top-level local name, returns a `Location` spanning the identifier at its declaration line (UTF-8 byte offsets used as LSP `character` positions — adequate for ASCII sources).
- **didOpen**: Implemented with `serde_json` so string escapes (`\n`, etc.) match normal LSP clients.

## Reverse engineering / bytecode

## Bounded metaprogramming

`stdlib:macro.sapp` provides backward-compatible template macros plus custom
PBLang expanders with explicit arity and output limits. `expand_source_bounded`
supports recursive macro composition with source, output and pass budgets, and
fails when expansion does not converge. `install_record_macro` adds the
Crystal-inspired `@record(constructor, field...)` convenience while emitting
ordinary inspectable PBLang source. Generated source can be checked with
`analyze_generated`; this remains a deterministic library preprocessor, not a
claim of parser-integrated Crystal macro parity.

`stdlib:freeze.sapp` links imports, compiles through the self-hosted frontend,
optimizes the bytecode and atomically publishes a verified PBCK artifact. Use
`tools/pbfreeze.sapp` when repeated startup should avoid parsing normal-sized
source projects. Compiler-sized flattening remains too expensive in the current
interpreter and must be benchmarked before using it as a cold-codegen shortcut.

`stdlib:jvm_classfile.sapp` is a bounded, dependency-free JVM classfile emitter
written in PBLang. It validates class names, descriptors, constant-pool sizes,
method counts and bytecode lengths before emitting deterministic class bytes.
The current surface intentionally targets small launchers and Android D8 input:
UTF-8 constants are strict non-NUL ASCII and the classfile version is 49 so
StackMapTable attributes are not required. It is not yet a general JVM backend.
It now supports bounded field declarations and field references in addition to
interfaces and methods, allowing generated Android launchers to preserve and
update real UI state. `code_builder`, `emit`, `mark`, `branch` and `finish_code`
provide deterministic forward/backward signed-16 branch assembly. They reject
non-branch opcodes, duplicate or unresolved labels, corrupt patch records,
out-of-range targets and method-size overflow before class publication.
Typed `emit_iload`, `emit_istore`, `emit_aload`, `emit_astore`, `emit_iinc` and
`emit_newarray` helpers validate local slots, signed increments and primitive
array tags before encoding compact JVM instructions. Raw `emit` remains available
for advanced backends, so this is backward-compatible rather than a replacement.

`stdlib:android_adb.sapp` provides a reusable, bounded Android device gate. It
discovers the official SDK `adb`, rejects unsafe serials and ambiguous or
unauthorized devices, and runs install, launch, foreground, crash, memory,
accessibility-action and graphics checks through argv-only calls with hard
timeouts. It detects asleep and securely locked screens before UI inspection,
returning an explicit device-state error instead of misclassifying the Android
keyguard as an application accessibility failure. It deliberately does not
bypass USB trust or device credentials and does not collect screenshots.

`stdlib:android_emulator.sapp` is the PBLang-only Android userspace execution
model. It composes `platform_emulator`, `device_emulator`, PBCS v2 and the
self-hosted bounded VM, with package isolation, lifecycle, permissions, private
storage (including uninstall data removal), JSON-bounded explicit services,
foreground input and secure network-policy plans.
`tls13_status(...)` exposes whether the modeled Android runtime guarantees a
platform TLS 1.3 provider. API 29+ returns the Android platform provider; API
21-28 fails closed, and `network_plan(...)` refuses to label unavailable TLS as
`tls-1.3`. Plans also state that no external transport installation is required.
Use `compile_and_run(...)` for source-to-PBCS execution or `run_pbcs(...)` for a
validated artifact. `bench/android_emulator_stages.sapp` records lifecycle and
bytecode stages separately. This is not an ART/Linux-kernel/vendor emulator;
that larger boundary remains explicit in every returned profile and report.

`stdlib:sha3.sapp` provides PBLang-native SHA3-256 for security protocols and
content identifiers. Keccak lanes use exact low/high 32-bit halves and bounded
left-shift steps, avoiding 64-bit rounding differences across interpreter and
codegen backends. FIPS 202 empty, `abc`, multi-block and byte-array vectors are
covered by `tests/semantics/sha3_test.sapp`.

`stdlib:native/compile_session.sapp` is the preferred direct-codegen path for a
project or repeated build. It fingerprints the backend once, validates every
artifact before publication, and loads `codegen_x64.sapp` lazily only after a
real cache miss. Consequently, an all-hit warm session does not parse the 270 KB
backend. `bench/x64_compile_pipeline_stages.sapp` reports process startup,
backend import, emitter, publication and cache stages separately; its derived
rows make clear which latency belongs to code emission and which belongs to
cold orchestration.

`stdlib:build.sapp` now provides `smart_plan`, `smart_session`,
`smart_build_in_session`, `smart_build_many_in_session`, and `smart_project`.
Plans are deterministic and expose why
each backend was selected. Batch builds share one direct-codegen session, so a
warm artifact does not reload the backend. All source/path/job sizes are
bounded, subprocesses use argv rather than shell-concatenated paths, direct
outputs are safety-certified before atomic publication, and manifest entries
cannot escape their project. The legacy `compile` function remains available
but delegates to this real build path; it no longer copies PBLang source into a
fake C file. Automatic artifact execution is deliberately excluded because a
build must not trigger application side effects; callers may add an explicit
execution gate in release workflows. A reusable session intentionally snapshots
the backend dependency fingerprint; create a new session after changing compiler
or backend source files.

**`pblang --dump-bytecode <file>`** compiles a single `.sap` or `.sapp` file (no execution, no package resolution) and prints a human-readable disassembly of the generated bytecode. This supports debugging, inspection of compiler output, and reverse-engineering workflows.

- **Output**: A constants table (literals, names, etc.) followed by the instruction stream with opcodes and operands. The format is implementation-defined and may change with the compiler; use it for inspection and tooling, not as a stable bytecode ABI.
- **Limitations**: Single-file only; imports are not resolved. Safe for untrusted source only in the sense that no code is run.
- **Typical use**: Verify what the compiler emitted for a given script, debug VM behavior, or build analysis tools on top of the disassembly. **R2-alpha:** **`pblang --call-graph <file>`** prints caller→callee edges from the AST. **R2-beta:** **`pblang --annotate <file>`** compiles the file and prints bytecode with source-line annotation: each instruction is preceded by a `-- line N -------  <source line>` header when the source line changes.

## R3-beta: Symbol table dump

```bash
pblang --symbols <file>
```

Prints a simple symbol table for all top-level declarations in the compiled chunk:

- `fn    <name>    arity:<n>    offset:<byte_offset>` — function entry (one per compiled function).
- `let   <name>    offset:const#<k>` — best-effort listing of global names from the constant pool.

Useful for RE workflows: quickly identify exported function names, arities, and approximate
bytecode entry points without reading the full disassembly.

### R3-alpha: Data-flow / use-def analysis

```bash
pblang --dataflow <file>
```

Prints a simple use-def report for all variables in the file:

- **`def <name> line:<n>`** — variable defined here (let/for/assign).
- **`use <name> line:<n>`** — variable read here (identifier occurrences).
- **`WARN <name> assigned but never read`** — variables that are defined but never used.

Complements `pblang lint` (scope-level unused warnings) with a per-variable def/use trace useful for reverse engineering and refactoring workflows.

## Stdlib-style APIs

- **`stdlib:benchmark.sapp`** — timing runner plus robust deterministic evidence: total/average/min/max, correct odd/even median, interpolated p50/p90/p95/p99, population standard deviation, median absolute deviation, IQR, IQR outlier count, coefficient of variation, and `excellent`/`stable`/`noisy`/`unstable` classification. Existing `run`, `time`, `warmup`, `compare`, `ops_per_sec`, `fastest`, and `format` calls remain available.
- **`stdlib:performance_budget.sapp`** — pblang-only fail-closed regression gates. `compare_metric(before, after, lower_is_better)` reports delta/ratio/improvement; `evaluate(baseline, candidate, budgets)` checks multiple latency, memory, storage, throughput, or custom numeric metrics; `assert_within(...)` raises when any budget fails. Missing metrics, invalid bounds, and changed zero baselines cannot pass silently.
- **`stdlib:language_benchmark.sapp`** — bounded correctness-gated six-category fixtures for 15 runtime definitions. Every row retains raw samples and reports the robust `benchmark.sapp` distribution/stability fields; runtime specs are resolved from the internal registry so callers cannot inject process commands.
- **`stdlib:repro_benchmark.sapp`** — bounded pure-PBLang compiled/interpreted competitor harness for the PBLang interpreter, hosted C99, direct Linux x86-64 codegen, C, C++, Rust, Go, Java, C#/Mono, Python, Node.js, and Zig. The direct row safety-certifies and executes the ELF from `stdlib/native/codegen_x64.sapp`; unsupported hosts fail closed as explicit unavailable rows. The harness uses fixed correctness-gated fixtures, separates first/cold and warmed runs, retains raw samples, records median/p95, GNU-time CPU and Linux peak RSS, release artifact bytes, host/toolchain metadata, and explicit unavailable rows. Its 22-category coverage ledger refuses to invent results for categories that do not yet have equivalent cross-language contracts; see `docs/Benchmarking.md`.
- **`json`** — Global object with `json.parse(s)` and `json.stringify(x)` (same as builtins `json_parse`/`json_stringify`). The bytecode VM uses the Rust JSON implementation; the C transpiler runtime also supports dependency-free C99 JSON parse/stringify for nil/null, booleans, numbers, strings, arrays, and string-key maps.
- **`stdlib:strings.sapp`** — String helpers: `chars(s)`, `join(parts, delim)`, native-backed `repeat/pad_left/pad_right`, safe `replace_all/replace_first/count` for empty needles, `starts_with`/`ends_with`/`contains`/`trim`, `lines`, `words`, case conversion, slug/dedent/wrap helpers, `normalize_space`, `collapse_blank_lines`, `remove_empty_lines`, `indent`, `quote`/`unquote`, and prefix/suffix/contains-any/all predicates.
- **`stdlib:array.sapp`** — `push`, `pop`, `map`, `filter`, `reduce`, `contains`, `index_of`, clamped slicing/reversal/flattening/chunking, numeric and predicate helpers, plus one-pass `filter_map`, running `scan`, stable O(n) `distinct_by`, `index_by`, and validated sliding `windows`. **`stdlib:map.sapp`** — immutable copy/merge/default/filter/map/pick/omit operations; grouping/counting and sorted access; plus allocation-saving mutable `merge_into`/`set_many`, batch `get_many`, `compute_if_absent`, and entry `partition`. Mutation is explicit in the API name; existing immutable APIs remain unchanged. **`stdlib:collections.sapp`** (G3-alpha) — stack/queue/set compatibility helpers.
- **`stdlib:path.sapp`** — `join(a, b)`, `dirname(p)`, `basename(p)`, `extension(p)`, `exists(p)`, `is_file(p)`, `is_dir(p)`, `absolute(p)` (resolve relative path against cwd; pass-through for Unix `/` or Windows `X:` style).
- **`stdlib:time.sapp`** — `clock()` (seconds since epoch), `now_ms()` (milliseconds since epoch), `now_iso()` (UTC ISO-8601 string, e.g. `2026-03-10T12:00:00Z`).
- **`stdlib:os.sapp`** — `cwd()`, `exec(cmd, args)`, `sleep_ms(n)`, `env_get(key)`, `args()` (raw CLI arguments), `script_args()` (explicit `--` arguments for direct files and package-directory entries, with backward-compatible direct-file argv fallback), and `exit(code)` (terminate process; sandboxed by exec capability). Note: `env_set` was **removed** in the v0.5 security audit — `std::env::set_var` is unsound in multi-threaded programs — so it is unavailable even with `exec: true` (see `docs/Capability_model.md`).
- **`stdlib:process.sapp`** — `spawn_wait(cmd, args_array?)` → map `{ exit_code, stdout, stderr }` (blocking subprocess; requires allow_exec). Uses VM builtin `proc_spawn_wait`.
- **`stdlib:tool_probe.sapp`** — non-executing trusted-path tool discovery. `describe(name)` classifies commands/daemons and `version_probe(name)` deliberately returns a skipped result: the blocking process API has no pre-emptive timeout/kill handle, and service binaries such as `ydotoold` may start instead of honoring `--help`. Use this module for capability discovery rather than executing help/version flags.
- **`stdlib:csv.sapp`** — **G2-gamma:** **`parse_line(record)`** parses one CSV record with RFC 4180 quoted fields (commas/newlines inside double quotes are literal; `""` is escaped quote). Unquoted records and files use native split fast paths; quoted input retains the full stateful parser. **`split_records(text)`** is quote-aware and handles CRLF without corrupting embedded newlines. **`parse(text)`** returns row arrays or header maps; **`parse_options(text, options)`** combines delimiter/header/typing with `max_chars`, `max_rows`, and `max_columns` guards. **`stringify_options(data, options)`** combines delimiter/header selection, spreadsheet-injection-safe output, and a result-size cap. Header inference is linear in discovered keys and quote escaping uses native split/join assembly.
- **`stdlib:json.sapp`** — JSON parse/stringify, safe fallback and dotted lookup, deep clone/merge/equality, pretty printing, flattening, and JSON Lines. Pretty/JSON Lines output uses chunked assembly; `parse_lines_limit` and `stringify_lines_limit` add explicit record/line/output budgets.
- **`stdlib:set.sapp`** — Map-backed value sets with direct-key union/intersection/difference/symmetric difference, smaller-side intersection/disjoint scans, predicates and transformations, plus allocation-saving `add_all`, `remove_all`, and in-place `retain` batch operations.
- **`stdlib:format.sapp`** — ANSI/color, wrapping, numeric/byte/duration formatting, lists, and hybrid ASCII table rendering. `table_from_maps(rows, columns)` renders record arrays directly without caller-side matrix conversion.
- **`stdlib:text_layout.sapp`** — pblang-native combining-mark/emoji clustering, simple bidirectional runs, code ligatures, visual text, and cell measurement. `shape_many` and `measure_many` provide renderer-friendly batch APIs.
- **`stdlib:dataframe.sapp`** — Table/dataframe helpers over `{ columns, rows, _col_index }`: linear column discovery/normalization, quote-aware CSV I/O, stable O(n log n) `sort_by`/`sort_values`, hash-indexed inner/left joins, one-pass indexed pivot buckets, O(n) rolling means, map-indexed column dropping, aggregation/grouping/reshape/window/statistics, schema, and validation. Join output preserves left order and right duplicate order; left join preserves its first-match contract.
- **`stdlib:serialization.sapp`** — Multi-format conversion helpers with bounded `parse_safe`, `stringify_safe`, and `convert_safe` result APIs; configurable input/output/line/depth/item budgets capped by hard ceilings; strict duplicate/malformed/unsupported checks for the documented native TOML/YAML subsets; safe JSON validation/pretty output; quote-aware CSV reuse; XML text escaping; explicit capability discovery via `formats`; and legacy parse/stringify entry points for trusted-input compatibility. See `docs/Serialization.md` for subset and security boundaries.
- **`stdlib:template.sapp`** — String/report/prompt templates: `render`, `render_file`, `render_strict`, `missing_vars`, and `escape_html`. Supports comments, dotted variables, `{% if %}` with truthy/not/equality checks, `{% for item in list %}` over arrays and maps, isolated loop state (`loop_index`, `loop_number`, `loop_first`, `loop_last`), chained filters (`trim`, `upper`, `lower`, `len`, `json`, `html_escape`, `join`, `default`), and missing-variable validation.
- **`stdlib:validate.sapp`** — Schema validation helpers: `schema(fields, options)`, `check`, `check_field`, `apply_defaults`, `check_with_defaults`, `assert_valid`, `validate_all`, and `explain`. Supports required fields, scalar types, nested map fields, array item schemas, strict unknown-field rejection, string length/pattern/format checks (`email`, `url`), numeric min/max/integer/multiple constraints, enums, defaults, and batch validation.
- **`stdlib:config.sapp`** — Layered app config: explicit values, environment lookup, defaults, typed accessors, bulk `from_map` / `set_many` / `set_defaults`, `unset`, `has`, `keys`, normalized env names (`database.url` → `DATABASE_URL`), `require_int` / `require_bool`, `typed_map(schema)`, and validation bridges `validate` / `validate_with_defaults` through `stdlib:validate.sapp`.
- **`stdlib:router.sapp`** — Routing layer: `route_map()` → empty map; `add_route(map, method, path, handler)` → map (key is `method|path`); `dispatch(map, method, path, req)` → handler result or `{ "status": 404, "body": "Not Found" }`; path params via `/user/:id`-style templates, `req["params"]` set before handler call; `match_path_params(template, actual)` → params map or nil.
- **`stdlib:middleware.sapp`** — Compose handler transforms: `wrap(inner, mw)` (mw(req) returns nil to continue or response map to short-circuit); `chain(inner, mws)` (fold mws around inner); `mw_log()` (log method+path, continue); `mw_require_auth(expected_token)` (req["auth"] must match or 401). Works with router.sapp. The old `mw_log_stub()` / `mw_require_auth_stub()` names remain as compatibility aliases.
- **`stdlib:blocks.sapp`** — Block-programming model and code generator: `workspace()`, `add(ws, block)`, `set_code(ws, handwritten_code)`, expression blocks (`num`, `text`, `var`, `binop`, `call`, `array_expr`, `map_expr`), statement blocks (`let_stmt`, `print_stmt`, `if_stmt`, `while_stmt`, `for_count`, `for_in`, `fn_block`), `raw(code)` / `from_code(code)` for code+blocks projects, `to_code(workspace_or_block)` to generate pblang, `compose(code, blocks)` to combine handwritten source and generated blocks, and `validate(...)` for structural diagnostics. This is the pblang-native foundation for visual blocks programming in the IDE; rendering the drag/drop UI is separate.
- **`stdlib:machine_opt.sapp`** — Machine/device optimization and cross-device play/run planning substrate: `execution_models()`, `execution_model(kind)`, and `verified_execution_model(kind)` expose the bounded executable-model registry; `profile(kind)` covers portable/old/microcontroller/GPU/quantum/WASM targets; `workload(kind, size, dtype)`, `background_task(kind, items, item_bytes, priority)`, `resource_efficiency_plan(profile, task)`, `register_plan(...)`, `memory_plan(...)`, `tile_plan(...)`, `cache_plan(...)`, `binary_plan(...)`, `device_plan(...)`, `full_plan(...)`, `play_target(kind)`, `play_plan(program, target_kind, opts)`, `play_matrix(program, targets)`, and `explain(...)` build portable plans. It chooses honest routes for registers, cache tiling, memory chunking, bytecode/JIT/AOT/WASM/freestanding-C/quantum-protocol backends, and CPU/GPU/NPU/FPGA/quantum/distributed devices. Unknown substrates remain `unverified` until they define an operation set or protocol. Actual device backends must still compile and verify their generated code.
- **`docs/EMULATOR_MATRIX.md`** — Executable cross-device verification matrix for 6502, Z80, AVR, MSP430, MIPS32, PowerPC32, SPARC V8, RISC-V, WebAssembly, and quantum state vectors. The models use real encodings, checked results, and bounded instruction budgets; they are code-generator test foundations, not claims that every production backend is complete.
- **`stdlib:product_health.sapp`** — Shared, pure-pblang product-health contracts for pblang, pbAI, pbOS, PBIDE, pb-mcp-server, and PBRemote. `check` normalizes bounded required/optional checks and latency budgets; `report` produces deterministic status/score/counts; `dependency` and `portfolio` compose products; `release_gate` fails closed for required failures; `transition` reports regressions; and the bounded line renderers feed CLI, IDE, and MCP surfaces. The versioned `pblang.product_health.*.v1` shapes perform no network or process probing themselves.
- **`stdlib:pblang_health.sapp`** — Cheap pblang runtime adapter for the shared health contract. It reports the required interpreter and capability-sandbox baseline, optional JIT/host-platform readiness, and deterministic fallback without invoking compilation, process execution, network access, or device probes.
- **`stdlib:resource_budget.sapp`** — Pure-pblang resource admission and backpressure shared by pblang products. It defines bounded `interactive`, `batch`, `server`, `untrusted`, and `micro` profiles across time, memory, storage, network/output bytes, items, workers, allocations, frames/frame time, tokens, and restarts. Arithmetic is capped to pblang's exact integer range and uses subtraction-first overflow checks. `evaluate` reports remaining capacity and all violations, `reserve`/`release` are transactional, mutable ledgers simplify repeated admission, and `ledger_try_reserve`/`ledger_try_release` provide boolean hot-loop paths without constructing full diagnostics. Unknown or misspelled metrics fail closed and rejected transactions leave state unchanged.
- **`stdlib:resources.sapp` hardening** — Resource snapshots/diffs now have versioned formats, missing VM counters safely default to zero, and `bounded_steps` caps capture/leak loops. Negative captures keep only the baseline and a zero-round leak check returns a non-leaking empty result instead of indexing an empty array.
- **`stdlib:resilience.sapp`** — Shared pure-pblang lifecycle policy for bounded attempts, overflow-safe exponential backoff, cooperative deadlines, explicit cancellation, and circuit opening/recovery. `policy` hard-caps numeric inputs and fails closed on unknown option names; `initial`, `decision`, `begin_attempt`, `record_success`, `record_failure`, and `cancel` form a deterministic state machine that callers can replay without sleeping, spawning, or reading a clock. `backoff_ms` optionally applies caller-supplied deterministic jitter. Deadlines are cooperative: blocking I/O still needs a pre-emptive timeout at its own boundary.
- **`stdlib:thread.sapp` supervisor** — Bounded pure-PBLang worker supervision with validated/clamped deadlines, parent-owned state, success/failure/timeout/cancellation accounting, channel-backed cooperative cancellation tokens, deterministic status snapshots/reaping, and a 1,024-record hard ceiling. A worker is counted as cancelled only when its completion record proves that it observed the signal; otherwise a successful worker remains completed and increments `cancel_not_observed`. `spawn`, `spawn_with_args`, `spawn_cooperative`, and `spawn_cooperative_with_args` accept dot or colon dispatch; explicit argument variants avoid relying on non-transferable worker captures. `kill_all` remains as a compatibility name but truthfully requests cooperative cancellation and reports `preemptive: false`; `stats.detached` exposes timed-out workers that have not exited yet. VM threads cannot be safely force-killed, so blocking native operations still need their own timeout or process boundary. See `examples/supervised_tasks`.
- **`stdlib:retry.sapp` hardening** — The compatibility `run`, `run_or`, and `times` APIs now execute through the shared resilience state machine. Attempts are capped at 100, base delay at 60 seconds, multiplier at 16, maximum delay at 300 seconds, and total cooperative timeout at one hour. `plan` and `schedule` expose normalized policy/delays without executing work, and results carry a versioned status while preserving `nil` as a successful return value.
- **`stdlib:project_survey.sapp`** — Safe metadata-only cross-project planning for pblang, pbAI, pbOS, PBIDE, pb-mcp-server, and PBRemote. It classifies path/name records without reading file contents, skips secret/cache/build paths, and exposes `survey_paths`, `cross_project_plan`, `portfolio_readiness`, and `portfolio_readiness_lines`. It also exposes `category_protocols`, `project_improvement_tasks`, `portfolio_action_plan`, and `portfolio_action_lines` so Management/pbAI/MCP tooling can turn safe local path inventories into prioritized tasks with impact/effort/risk scores, device routes, protocols/algorithms, and verification gates. This is an idea/action planner only; every generated task still needs focused implementation and tests before claiming completion.
- **`stdlib:project_inventory.sapp`** — Bounded metadata-only filesystem inventory for feeding `project_survey`. It exposes `inventory_roots`, `inventory_paths`, `inventory_lines`, and `portfolio_action_from_roots`. It uses directory/file names only, never `fs_read`, accepts caller-approved `.` / `..` roots, skips sensitive/cache/build child paths, caps depth/entries/children, and returns `pblang.project_inventory.report.v1` records plus a portfolio action plan. This is the safe way to look around AAP, `/media`, or `~` for project ideas without ingesting private content or training data.
- **`stdlib:project_audit.sapp`** — Bounded caller-provided text audit for TODO/not-implemented/placeholder/security/reliability/performance markers. It exposes `scan_text_report`, `scan_records`, suppression explanations, project recommendations, source/docs/test/config role counts, source-aware `quality_gate`, `backlog`, `release_readiness`, `compare_reports`, and `progress_gate`. It never walks the filesystem by itself; callers choose which text records to pass and can suppress fixture noise explicitly. HTML `placeholder="..."` attributes are treated as normal UI text, not incomplete implementation markers. Browser-policy block messages, source-only gate option names, roadmap task labels, blocker counters, and pbOS gap-contract helper/rule strings are treated as status/policy text rather than active roadmap findings. Rust-style `#[cfg(test)] mod tests` source regions are suppressible as test-only findings when fixture suppression is enabled.
- **`stdlib:project_text_audit.sapp`** — Explicit safe filesystem-to-audit bridge. It combines `project_inventory` and `project_audit`: `collect_text_records(...)` walks only caller-approved roots, skips sensitive/cache/build/generated-output/local assistant state paths such as `graphify-out`, `.claude`, and `.codex`, skips markdown/text files with explicit `Historical snapshot` / `SUPERSEDED / HISTORICAL` banners by default, accepts only text-like extensions, caps file count/size, and does not print file contents; `audit_roots(...)` returns `pblang.project_text_audit.report.v1`; `audit_lines(...)` renders a redacted summary with role counts suitable for PBIDE Management, pbAI, or MCP wrappers. `tools/pbaudit.sapp` defaults to `--source-gate` so docs/history backlog and low-severity source notes become `needs-review` while high/medium source blockers still fail; pass `--strict-gate` to make every role block.
- **`stdlib:migration_policy.sapp`** — Pure-PBLang, deterministic enforcement for new implementation language and legacy migration records. `classify_path`, `validate_exception`, `validate_migration`, `retirement_ready`, `evaluate_manifest`, and `report_lines` reject unsafe paths and malformed/oversized manifests, prohibit all new/added Rust, require exact dated host-boundary exceptions with verification and removal plans, and enforce the ordered reference/parity/verified/retired evidence contract. It consumes caller-provided records and performs no filesystem, process, shell, git, network, or FFI operation.
- **`stdlib:aap_status.sapp`** — Multi-project status composition for the AAP stack. It exposes `project(...)`, `default_projects()`, `status_report(...)`, `status_lines(...)`, `next_focus_from_reports(...)`, `focus_detail(...)`, and `focus_detail_lines(...)`, running bounded audits for the current sibling repositories pblang/PBIDE, pbAI, pbOS, pb-mcp-server, and PBRemote, then ranking the next project/category to work on and optionally appending metadata-only six-product portfolio action lines. This is the reusable library behind `tools/pbaap_status.sapp`.
- **`stdlib:hwinfo.sapp`** — Machine inventory and compute-use honesty layer: `cpu`, `ram`, `disks`, `gpus`, `npu`, `network`, `sound`, `devices_by_class`, `compute_plan`, `survey`, `vendor`, plus compact `gpu_summary()` / `npu_summary()` status strings for tools and assistants. GPU/NPU summaries use the same structured detector paths as `survey()` and degrade to explicit notes instead of platform-specific "not implemented" placeholders.
- **`stdlib:lang_extras.sapp`** — Cross-language convenience helpers implemented in pure pblang: Python-style `range`, `enumerate`, `zip`, `reversed`; JavaScript-style `keys`, `values`, `entries`, `from_entries`, `pick`, `omit`, `get_path`; Rust/Kotlin-style `unwrap_or`, `expect`, `take_if`, `take_unless`; and functional helpers `tap`, `pipe`, `repeat`, `clamp`. These are library-level commands today, not parser keywords, so they work through normal `import("stdlib:lang_extras.sapp")` and stay portable across VM/native targets.
- **`stdlib:llm_data.sapp`** — LLM corpus helpers: `secret_name`, `text_like`, `read_text_file`, `chunk_text`, `chunk_ids`, `lm_examples`, `split`, `corpus_record`, `record_quality`, `filter_quality_records`, and `filter_records`. It also exposes **`sensitive_line(line)`**, **`sanitize_text(text)`**, and **`sanitize_record(record)`** for deterministic line-level redaction of common token/password/API-key/private-key shapes before text reaches a tokenizer or live-learning corpus. `read_text_file(...)` sanitizes before returning text. Curriculum helpers `repeat_examples(...)` and `interleave_priority_examples(...)` let model projects boost clean/high-signal examples inside a fixed training budget.
- **`stdlib:llm_eval.sapp`** — LLM evaluation helpers: aggregate `evaluate(...)` loss/perplexity/accuracy, sequence-level `sequence_evaluate(...)`, output-shape scoring with `text_shape(...)`, cheap screening records with `regression_proxy(...)`, validation/regression candidate ranking with `selection_score(...)`, `benchmark_generate(...)`, prompt fixtures via `regression_prompt(...)`, generation-quality scoring with `generation_quality(...)` / `max_repetition_run(...)`, `choose_generation(...)` for reranking decoded candidates, teacher-data gates with `teacher_record_quality(...)` / `teacher_dataset_quality(...)`, measurable objective helpers `objective_component(...)` / `objective_distance(...)`, and trial/error helpers `reinforcement_from_objective(...)`, `trial_candidate(...)`, and `select_trial_elites(...)` for small fast reward/punish loops that reduce weighted distance from target metrics.
- **`stdlib:llm_attention.sapp`** — Stacked multi-head decoder substrate for pbAI-Max work: model/config builders, embeddings, multi-head causal attention, Gemma-3-style `gemma3_config(...)` / `attention_plan(...)` for local-window layers plus periodic global layers and grouped-query KV sharing, Gemma-4-style `gemma4_config(...)` planning for PLE/MoE/MTP/system-prompt/thinking/multimodal projection/memory, logits/loss, greedy `generate(...)`, safe anti-collapse `generate_safe(...)`, `repetition_penalized_logits(...)`, and live architecture controls for adding layers, extending context, tuning binary-adapter strength, and policy-gated manipulation.
- **`stdlib:llm_quant.sapp`** — LLM quantization helpers: row-wise quantized inference plus pblang TurboQuant-style vector compression (`tqplus_calibration`, `turboquant_encode`, `turboquant_mse`, `turboquant_prod`, `qjl_residual_signs`, `turboquant_dot`, `turboquant_compression`) and TurboVec-style compressed vector indexing/search (`turbovec_new`, `turbovec_add`, `turbovec_add_with_ids`, `turbovec_search`, `turbovec_remove`, `turbovec_swap_remove`, `turbovec_manifest`). It supports online ingest, lazy dimension locking, stable external ids, allowlist/mask filtered search, and `.tv`/`.tvim`-style metadata. These are pure-pblang substrate helpers inspired by public vector-quantization techniques, not fused vendor production kernels.
- **`stdlib:llm_tiered_store.sapp`** — Pure-PBLang low-memory layer and MoE-expert shard runtime. `model_part(...)` declares exact-size/SHA-256/precision metadata; `create(...)` opens a bounded store with Linux no-follow confinement and optional weighted mirrors; `load(...)` verifies before caching; `record_routes(...)` plus `predict_experts(...)` learn hot experts; `queue_prefetch(...)` / `run_prefetch(...)` provide bounded look-ahead; `rebalance_hot(...)`, `status(...)`, and `close(...)` expose deterministic lifecycle control. RAM bytes and entry count are hard budgets, precision mismatches fail closed, and placement does not alter model semantics. Prefetch reads remain synchronous and GPU upload is not yet supplied by this module.
- **`stdlib:ide_features.sapp`** — Headless IDE logic shared by the web/native editor surfaces: outline and outline filtering (`outline`, `filter_outline`), find/replace, diagnostic normalization, status text, searchable examples, command palette catalog/filtering (`command_catalog`, `filter_commands`) including Problems and diagnostic navigation commands, Quick Open / Files filtering (`quick_open_candidates`) including optional tree display labels, bounded run/check history, Debug/Release build profiles (`build_profile`, `next_build_mode`, `build_mode_summary`), and watcher records (`watcher_new`, `watcher_update`, `watcher_summary`, `watcher_action`, `watcher_badge`, `watcher_lines`) that track buffer hash/revision, dirty state, line count, symbol count, diagnostics, deltas, diagnostic freshness (`unchecked`/`fresh`/`stale`), actionable next steps, compact status badges, and a bounded event log while ignoring unchanged draw-refresh spam. The native IDE uses these for the Mode button, Watch button, `Ctrl+P` command palette, `Ctrl+O` Quick Open, the persistent flat/tree `Files` panel, `Out` symbol panel, `Prob` Problems panel, status bar, and Code side-pane watcher card.
- **`stdlib:quantum_sim.sapp`** — Pure-pblang state-vector simulator: `zero_state(n_qubits)` → array of `[re, im]` amplitudes; `h_gate(state, q)`, `x_gate(state, q)`, `cnot_gate(state, c, t)`; `measure(state, q)` → 0 or 1 (collapses state). Uses builtin `random_float()` for measurement. Q3-beta helpers: `histogram(outcomes_array)` → map from stringified outcomes (e.g. `"0"`, `"1"`) to counts, useful for aggregating repeated measurements or provider results.
- **`stdlib:multipart.sapp`** — **parse(body, boundary)** → array of part maps; each part has `name`, `filename` (nil if not file), `content_type`, `body`. Parses multipart/form-data (delimiter `--` + boundary). Uses builtins `str_index_of`, `str_slice`, `str_lower`.
- **`stdlib:http_server.sapp`** — HTTP server helpers; includes a **static file helper** `static_file_response(root_dir, request_path)` for safe serving of text files under a root directory: returns a response map with `status` 200 or 404, `body`, and `content_type` (inferred from extension: `.html`/`.htm`, `.txt`, `.css`, `.js`, `.json`; default `text/plain`). Cookie helpers: `cookies(req)` parses `Cookie`/`cookie` request headers into a map, and `cookie_get(req, name, fallback)` returns a cookie value with fallback. Paths containing `..` or outside root yield 404. Response headers are sanitized: invalid header names and CR/LF-bearing values are skipped, and unsafe content types fall back to `text/plain`. Body limits: `read_request_limited(stream, max_body_bytes)` and `serve_once_limited` / `serve_forever_limited` reject oversized fixed-length `Content-Length` requests with **413**. **W3-delta:** **chunked_read(socket, on_chunk)** — reads Transfer-Encoding: chunked body frames; calls `on_chunk(data)` per non-empty chunk; uses `tcp_read_until` (size line) and `tcp_read` (chunk body). **Server-path timeouts**: when the host sets **VmLimits.io_timeout_ms**, the VM applies read/write timeout on streams returned by **tcp_accept**. `serve_once_timeout(...)` and `serve_forever_timeout(...)` remain compatibility helpers: `timeout_ms=0` behaves like the normal server, while nonzero values fail loudly because script-level per-socket timeout is not exposed in this build. **sse_event(event, data)** formats an SSE frame (`event: …\ndata: …\n\n`); **res_sse_headers()** returns a 200 response map with `Content-Type: text/event-stream` and `Cache-Control: no-cache`. **CORS:** `cors_headers(origin, allow_methods, allow_headers)`; `res_cors_preflight(...)` returns 204 with CORS headers for OPTIONS. **Rate limiting (W4-beta+)**: `mw_rate_limit(window_ms, max_requests)` returns middleware that tracks requests per instance in a fixed window and returns **429** after the configured number of requests; can be used with `app_use`/`app_get_mw`/`app_post_mw` for global or route-level throttling. **TLS:** See **docs/TLS_notes.md** for host-bridge design (rustls path requires no VM changes).
- **`stdlib:websocket.sapp`** — WebSocket upgrade helpers + frame/message encode/decode. `upgrade_request(req)` returns `{ ok, key }` when request is valid WebSocket GET (Upgrade, Connection, Sec-WebSocket-Key); `upgrade_response(key)` returns HTTP 101 response string with `Sec-WebSocket-Accept` (via builtin `websocket_accept_key`). **Frame parsing:** `parse_frame(data)` decodes one masked/unmasked RFC 6455 frame from a byte-string and supports short/16-bit/64-bit payload length encodings; returns `{ fin, opcode, masked, payload_len, payload, frame_len, next_index }` (or `{ error, ... }`). **Stream parsing:** `parse_frames(data)` decodes concatenated frames. **Fragment reassembly:** `reassemble_fragments(frames)` and `parse_message(data)` rebuild the first complete text/binary message across continuation frames (`FIN=0`/opcode `0`), while skipping control frames. **Frame writing:** `write_frame(opcode, payload)` builds masked client frames with random 4-byte masking keys and supports short/extended length formats (including >125 bytes).
- **`stdlib:quantum.sapp`** (Q2-gamma) — Circuit builder: **circuit(n_qubits)** → `{ n, n_qubits, ops: [] }`; **apply_h(circ, q)**, **apply_x(circ, q)**; **apply_cnot(circ, control, target)**; **measure_all(circ)** (append MEASURE per qubit); **to_qasm(circ)** → QASM 2.0 string. Compatible with **quantum_protocol.submit()**. **Q3-alpha:** **run(circ)** — run circuit locally via quantum_protocol.submit, returns array of measurement outcomes (0/1 per qubit). **measurements_to_string(measurements)** → e.g. `"010"`; **measurements_to_int(measurements)** → little-endian integer. Existing **h**, **cx**, **measure**, **emit_qasm** unchanged.
- **`stdlib:quantum_protocol.sapp`** — Job envelope (submit/poll): `submit(spec)` accepts a circuit spec `{ n, ops }` (same shape as quantum.sapp), runs it via quantum_sim, returns `{ job_id, status: "completed", result: { measurements } }`; `poll(job_id)` returns stored result or `{ status: "pending" }`. In-memory sync execution. **Q2-delta:** **submit_remote(url, circuit_map)** — POSTs JSON-encoded circuit to remote backend; returns `job_id`. **poll_remote(url, job_id)** — GETs `<url>/<job_id>`; returns decoded result map. See **docs/Quantum_providers.md** for provider slot design (Qiskit/Braket/Cirq-style stubs).

## Q3-gamma: BB84 example + quantum error correction stub

- **`examples/quantum_bb84/main.sapp`** — runnable BB84/QKD-style demo wiring together `stdlib:crypto_quantum.sapp` (QKD protocol names), `stdlib:quantum.sapp` (circuit builder + `run`), and `stdlib:quantum_sim.sapp.histogram` to build a simple Bell-state measurement histogram.
- **`stdlib:quantum_sim.sapp.repetition_encode(bit)`** — 3-qubit repetition encode → `[bit, bit, bit]`.
- **`stdlib:quantum_sim.sapp.repetition_decode(bits)`** — majority-vote decoder over the bits array; corrects single physical-bit flip errors in the toy repetition code.
- **`stdlib:tensor.sapp`** (A3-beta) — Typed tensor/buffer contract for AI providers: **tensor(shape, data)** → `{ shape, data, dtype: "f32" }` (validates data length; mismatch returns `{ "error": "..." }`); **zeros(shape)**, **ones(shape)**; **get(t, indices)**, **set(t, indices, value)** (row-major); **shape(t)**, **dtype(t)**; **to_request_body(t)** for JSON in AI requests.
- **A3-gamma:** VM builtin **http_post_auth(url, headers_map, body)** — POST with caller-supplied headers; requires allow_net. **stdlib:http.sapp**: **post_auth(url, body, token)** builds `Authorization: Bearer <token>` for the common API-key case. **examples/ai_http_provider/main.sapp**: **openai_chat_auth(prompt, api_key)** — convenience wrapper using post_auth. **A4-alpha:** **get_stream(url, on_chunk)** and **post_stream(url, body, on_chunk)** — shim: full response then split on `\n\n`, call on_chunk per part (SSE). **parse_sse_line(line)** → `{ kind: "data"|"event", value }` or nil. **openai_chat_stream(prompt, api_key, on_token)** in ai_http_provider parses SSE and calls on_token(delta). See **docs/AI_plugin_pattern.md** (Streaming Inference, ONNX FFI path). **A4-beta:** **http.post_stream_auth(url, body, token, on_chunk)** — authenticated streaming POST shim with Bearer token and **stdlib:provider_registry.sapp** multi-provider AI routing helpers: `register(name, endpoint, api_key)`, `call_best(prompt, model)`, `call_with_fallback(prompt, model)`, `next_provider_name()`, `call_round_robin(prompt, model)`.
- **stdlib:graphics.sapp** (G2-gamma+) — Window wrapper over native graphics builtins plus pure-pblang shape helpers. **`bezier_point(...)`** computes a cubic Bezier point; **`bezier_points(..., segments)`** returns a clamped line approximation; **`draw_polyline(handle, points, color)`** draws connected segments; **`polygon(handle, points, color)`** draws a closed polygon; **`bezier(handle, ..., color, segments)`** draws the approximated cubic curve and returns the generated points; **`draw_text(handle, x, y, text, color)`** renders text through `stdlib:bitmap_font.sapp` using `gfxdrawpoint`. The `Window(...)` object exposes these as `polyline`, `polygon`, `bezier`, and `text` methods. The event loop skips unchanged draw/present work and, after three completely idle frames, expands scheduler waits up to 80 ms; any event or dirty frame immediately restores the requested frame cadence. `idle_frame_delay(...)` exposes the deterministic pacing policy for tests.
- **`stdlib:gui.sapp` / `stdlib:gui_parse.sapp`** — pblang-native GUI toolkit and round-trip parser used by the native Designer. `gui.sapp` provides forms, label/button/textbox/checkbox/radio/select/slider/panel widgets, data binding, mouse handling, `cube3d(...)` and `sphere3d(...)` pseudo-3D widgets, and `fit_form(form, min_w, min_h, pad)` for responsive generated forms. Keyboard-only operation uses deterministic Tab/Shift+Tab focus with wrapping and hidden/disabled skipping, Enter/Space activation, arrows plus Home/End for select/slider controls, Escape focus clearing, visible focus indicators, and key-repeat suppression for discrete actions. `focus_next(form, reverse)` exposes the same traversal to application tests and custom controllers. `gui_parse.sapp` parses Designer-generated `gui.form(...)` / `gui.add(...)` code back into form/widget records, including `cube3d` and `sphere3d`, using the self-hosted parser.
- **`stdlib:app.sapp`** — unified PBLang-only declarative application facade. `run(spec)` validates and launches `cli`, `gui`, `web`, `rest`, `desktop`, `fullstack`, `game`, `multimedia`, `database`, `networking`, `automation`, `ai`, or `ml` applications. Compact helpers expose the same names, while `plan(spec)` reports capabilities and blocking/execution behavior. REST adapts data actions to the hardened `web_framework.sapp` JSON path; desktop adapts to the real `gui.sapp` builder. Full-stack specs contain `cli`, `gui`, and/or `web` maps and use explicit `start: "cli"|"gui"|"web"`, preventing hidden competing event loops. Web binds to loopback unless `allow_public: true` is explicit. `preview: true` exercises GUI/web adapters without opening a display/socket and is plan-only for domain profiles; domain actions run in preview only with explicit `preview_execute: true`. See `examples/one_line_apps/`.
- **`stdlib:app_domains.sapp`** — bounded execution layer behind the game, multimedia, database, networking, automation, AI, and ML helpers. It lazy-loads the real lower-level PBLang modules and supplies them through `ctx.modules`, preserving complete configurable APIs. Actions, names, options, input sizes, step counts, and collected outputs are validated; only game/automation may repeat; database/networking/AI execution fails closed until `allow_io: true`. The current multimedia provider is image processing; audio/video are not claimed as implemented providers. `bench/app_domain_stages.sapp` reports seven-sample median/p95 facade measurements without external I/O.
- **`stdlib:visual_engine.sapp`** — Backend-neutral graphics/media scene engine for pblang. It models safe local/network assets, 2D primitives (rect/line/circle/polygon/mesh/path/group/text/image), 3D primitives (points/meshes/cubes/spheres/surfaces), N-dimensional points/clouds/tensors/items with projection hints, charts (`line_chart`, `bar_chart`, `scatter2d`, `scatter3d`, `heatmap`), custom components, foreign-language metadata components, HTML/CSS/JS web component descriptors, scanner-imported components, and game-engine-style nodes: `camera3d`, `light3d`, `material`, `shader_program`, `physics_body`, `terrain`, `particle_system`, `skybox`, ECS-style `entity` / `component_ref`, `prefab`, and `render_backend` for native/web/OpenGL/Vulkan/Godot/Unity/Unreal-style bridges. It also models image/audio/video nodes, timelines, and keyframes. Network assets reuse `stdlib:browser.sapp` URL policy: only http(s) is allowed and private/local hosts, unsafe ports, credentials, CRLF/control input, and known ad/tracker/malicious URL patterns are blocked unless explicitly opted in where allowed. `compile(scene)` returns a renderer-ready `pblang.visual.plan.v1` for native IDE, web canvas, GPU, or remote-display backends.
- **`stdlib:science.sapp`** — Portable science foundation: physical constants, unit conversion, quantities, vector operations, mechanics/energy/waves/orbital helpers, ideal-gas and photon formulas, chemistry helpers (moles, concentration, pH), geometry helpers (centroid/bounding box), signal helpers (moving average, derivative, trapezoid integration), and experiment-table helpers. This is the base for simulations, scientific charts, pbAI data analysis, and future visual-science panels.
- **`stdlib:browser_render.sapp`** — Optional host-browser snapshot bridge for native tools. `render_args(url, output_path, width, height, family)` builds bounded Chromium/Chrome-compatible or Firefox-compatible screenshot arguments. `render_plan(...)` checks `stdlib:browser.sapp.url_policy(...)` before looking for a renderer, and tries Chromium, Chrome, Brave, Edge, then Firefox. `render_snapshot(...)` runs the renderer through `proc_spawn_wait`, so callers need `--allow-exec`. This is the exact DOM/CSS/JS visual fallback for the native IDE `Host` tab; the safe pblang Live/Reader/Source/Inspect modes remain browser-free.
- **`stdlib:browser.sapp` browser-inspector helpers** — The native IDE Browser uses pblang-native helpers for its live renderer and DevTools-like views: `page_blocks(page)` extracts high-signal render blocks, `source_lines(page, width, limit)` returns numbered HTML source lines, and `inspector_lines(page, width)` returns URL/status/policy/counts/elements/links/images/forms details. These are browser-free and do not execute page JavaScript.
- **`stdlib:math_symbols.sapp`** — Mathematical-symbol bridge for pblang tools and notebooks. `normalize_code(...)` converts safe pasted notation into valid pblang (`π`→`PI`, `Π`/`∏`→`product`, `τ`→`TAU`, `φ`→`PHI`, `√`→`num_sqrt`, `×`/`·`→`*`, `÷`→`/`, `≤`→`<=`, `≥`→`>=`, `≠`→`!=`, `∧`/`∨`/`¬`→`and`/`or`/`not`). `normalize_expr(...)` is for math scratchpads and additionally maps `^` to `**`. `shortcut_table()` / `expand_shortcut(...)` support native IDE compose shortcuts such as `\pi`, `\Pi`/`\prod`, `\sqrt`, `\le`, `\sum`, and `\int`; `\GreekPi` remains available when the literal Greek capital letter is wanted. `shortcuts()` exposes common constants/functions from `stdlib:math.sapp`, including `product(...)`, and `symbol_table()`/`explain_symbol(...)` provide metadata for IDE help/completion.
- **`stdlib:unicode.sapp`** — Unicode scalar helpers for pblang tools and programs. Strings can contain direct Unicode or scalar escapes such as `"\u{1F600}"`; the Rust parser and pblang-written tokenizer both decode `\uXXXX` and `\u{X..XXXXXX}`. Helpers include `codepoint_at`, `from_codepoint`, `codepoints`, `is_valid_scalar`, `is_emoji`, `is_math_symbol`, `classify_codepoint`, `contains_emoji`, `hex_codepoint`, `escape_codepoint`, and `escape_string`. This supports emoji/symbol text in strings/comments/editor buffers; arbitrary emoji identifiers/operators are still intentionally outside the grammar.

## Threads / channels (T4-alpha)

- **channel()** — Returns a map with `sender` and `receiver` endpoints (VM builtins).
- **send(sender_endpoint, value)**, **recv(receiver_endpoint)** — Message passing; sender is single-use per current MVP (send consumes sender).
- **channel_close(ch)** — Marks the channel as closed; subsequent **send** errors with "send on closed channel", subsequent **recv** on empty closed channel returns **nil**.
- **channel_is_closed(ch)** → bool.
- Pattern: drain with `while (let v = recv(ch.receiver)) != nil { ... }`; after **channel_close(ch)**, **recv(ch.receiver)** returns nil when queue is empty.

**T5-alpha (threads stdlib wrapper):** `stdlib:thread.sapp` provides a small ergonomic wrapper around these builtins:
- `thread.spawn(fn)` / `thread.join(handle)` — spawn and join worker threads (VM-per-thread model, no shared heap).
- `thread.channel_new()` / `thread.send_value(sender, value)` / `thread.recv_value(receiver)` — convenience wrappers over `channel`, `send`, `recv`.
- `thread.channel_close_wrapper(ch)` / `thread.channel_is_closed_wrapper(ch)` — explicit close and status check.
- `thread.select_ready(channels_array)` — thin wrapper over `select` for multi-channel receive.
- `thread.supervisor()` — bounded task observation through `spawn`,
  `spawn_with_args`, `spawn_cooperative`, `spawn_cooperative_with_args`,
  `status`, `wait`, `cancel`, `stats`, and `reap`. Cooperative workers poll
  `token.requested(token)` without a module capture (equivalent direct helper:
  `thread.cancellation_requested(token)`); `stats.detached` keeps overdue but
  still-running workers observable.

### T4-beta additions

- **select(channels_array)** → `{ "index": int, "value": any }` — Returns the first ready channel's value (by array order); returns **nil** if all channels are closed and empty; spins (yield_now) until a value is available.
- See **examples/thread_pool/main.sapp** for work-queue pattern with spawn, channel, channel_close, and join.

### Controlled parallel and device execution

- **`stdlib:thread_pool.sapp`** bounds worker groups to 64, rejects submissions
  after shutdown, preserves stable priority order, and performs parent-side
  success/failure accounting from immutable worker outcomes.
- **`stdlib:compute.sapp`** exposes `policy`, `estimate_work`, `select_device`,
  `run_controlled`, and `storage_read_controlled`. It separates hardware
  `present` from generic-runtime `usable`, uses saturating cost estimates and
  adaptive CPU worker counts, rejects accelerator routes whose launch/transfer
  cost is not amortized, and bounds disk-read batches to contiguous worker
  chunks. The cost hints are deterministic relative units, not elapsed-time
  predictions.
- **`stdlib:accelerator.sapp`** accepts validated `gpu`, `npu`, `simd`, and
  `distributed` runtime hooks. `dispatch` reports runtime success, failure, and
  explicit CPU fallback rather than silently changing devices.

See **docs/Controlled_execution.md**, **examples/controlled_compute/main.sapp**,
and **examples/hardware_routing/main.sapp**.

## Math builtins (core)

- **num_floor(x)**, **num_ceil(x)** — floor/ceil (1 argument).

## Math stdlib (G3-beta)

- **`stdlib:math.sapp`** — Structured math helpers across three areas:
  - **Complex numbers**: `math.complex.new/add/sub/mul/conj/norm/div_scalar/arg/exp/ln/pow_scalar/sqrt/abs/abs2/modulus_scalar/to_string`, plus an interactive menu (`math.complex.interactive`). `exp`, `ln`, scalar powers, and principal square root now use standard complex-number formulas rather than component-wise placeholders.
  - **Logic & algorithms**: `math.logic.entailment/conjunction/disjunction/negation/contrapositive/truth_table` and `math.algorithms.gcd/sieve/sequential_search` for basic discrete math operations and demos.
  - **Stats**: `math.stats.sum/mean/median/variance/stddev/min/max/range`; larger numeric arrays can use the VM tensor reductions while keeping the same array API.
  - **Linear algebra & scalars**: `math.la.vec_add/sub/dot/scale`, `la.mat_identity/transpose/mul/mat_vec_mul`, `la.det2/solve2` for 2×2 systems, and **scalar helpers** `math.util.clamp(x, lo, hi)`, `math.util.lerp(a, b, t)`, `math.util.smoothstep(edge0, edge1, x)` for common interpolation/animation-style computations. Larger vector/matrix operations route through native `tensor_*` kernels when that is faster.

## Registry / install env vars

- **`PBLANG_REGISTRY_INDEX`** — URL of the registry index or registry base (file:// or http(s)://). Used by `pblang install`, `pblang --lock`, `pblang build`, `pblang pack`, `pblang bundle`, `pblang publish`, and `pblang search` when `--registry-index` is not passed. Required when the manifest has registry dependencies (names starting with `@`) and no explicit registry URL was given on the command line.
- **`PBLANG_SIGNING_KEY`** — Optional. When set, `pblang pack` writes a `<out_file>.sha256` HMAC sidecar; `pblang verify <tarball>` checks it if the sidecar exists; **`pblang install`** (E4-beta) fetches `<tarball_url>.sha256` for registry tarballs and verifies HMAC before unpacking, rejecting on mismatch.
- **`REQUIRE_REGISTRY_SHA256`** — Set to `1` or `true` to require index entries to include a `sha256` field; resolution fails otherwise. Used by `pblang install`, `pblang --lock`, `pblang build`, `pblang pack`, and `pblang bundle` when the explicit `--require-registry-sha256` flag is not passed.

- **Hosted registry design (M3-gamma/M4-beta):** See **docs/Registry_hosted.md** for API and auth design (GET index/tarball/sha256, POST publish, GET search; bearer token for publish) and the M4-beta client flows for `pblang publish` and `pblang search`.

## Registry semver (M3-alpha, M3-beta)

- **crates/pblang-pm/src/semver.rs**: **Version::parse(s)** → Option&lt;Version&gt;, **satisfies(version, range_str)** → bool. Range syntax: exact `1.2.3`, caret `^1.2.0`, tilde `~1.2.3`, `>=1.0.0`, `<=2.0.0`, `>1.0.0`, `<2.0.0`. Thirteen unit tests; intended for use in dependency resolution.
- **M3-beta: semver wired into resolution** — `resolve_registry_dep` (and thus `pblang install` / `pblang --lock`) now uses **satisfies** to pick the highest available version that matches the requested range. If no candidate satisfies the range, resolution returns an error (e.g. "no version satisfies `^1.0.0`").
---

## SP1-gamma: OOP + functional conformance fixes

**SP1-gamma** closes known failing semantics tests and unblocks the full conformance suite:

- **Class semantics fixes**:
  - Constructor dispatch: `ClassName(args)` → `ClassName.new(args)` (explicit `fn new`).
  - Method resolution with `self` binding in class method table.
  - Auto-constructor with positional field assignment when `fn new` is absent.
  - `init(self, ...)` method called automatically by auto-constructor.
- **Match semantics fixes**:
  - Wildcard fallthrough: `_` matches any value and should be the last arm.
  - Guard evaluation after binding: `case n if n < 0` evaluates guard after binding `n`.
- **Regression tests added**:
  - `class_basic_pass` — basic class with init and methods.
  - `class_method_dispatch_pass` — method dispatch with self binding.
  - `match_exhaustive_pass` — exhaustive match with else clause.
  - `match_guard_pass` — match with guard conditions.
- **Documentation updates**:
  - `docs/Spec_freeze.md` v0.2: OOP semantics, closure capture rules, map/filter/reduce contracts.
  - `docs/Tooling.md` SP1-gamma note (this section).

**Impact**: Keeps conformance suite broadly green, with known ignored parser/thread/env-policy cases documented in `docs/CURRENT_STATUS.md`.

---

## Related docs


- [Language_Semantics.md](Language_Semantics.md) — Core semantics, capabilities, limits.
- [Stability_policy.md](Stability_policy.md) — Breaking changes, versioning.
- [Crash_resistance.md](Crash_resistance.md) — Error handling, determinism.
- [lsp.md](lsp.md) — Language server (parse/typecheck diagnostics).

---

## G3-gamma: nd.sapp linear algebra

`stdlib/nd.sapp` now includes matrix inverse and solve:

- **`nd.mat_inv(A)`** — 2×2 or 3×3 matrix inverse (flat row-major array). Returns `nil` for singular matrices.
- **`nd.mat_solve(A, b)`** — Solve `Ax = b` via Cramer's rule (2×2 and 3×3). Returns solution array or `nil` if singular.

**Tests**: `nd_linalg_inv_solve_pass` in `tests/semantics.rs`.

---

## T5-beta: thread_pool stdlib helper

`stdlib/thread.sapp` includes `thread_pool`:

- **`thread.thread_pool(n_workers, tasks_array, worker_fn)`** — Spawns worker threads, distributes tasks through channels, and collects results.

**Note**: `spawn/join` now uses a send-safe wire format that supports function/closure transfer (for closures whose captured values are transferable), so `thread_pool` worker closures run in child VMs instead of a sequential fallback.

The higher-level `stdlib:thread_pool.sapp` uses closed-upvalue factories before
crossing this boundary. Its queues are deterministic bounded chunks, not a
dynamic work-stealing implementation.

**Tests**: `thread_pool_stdlib_pass` in `tests/semantics.rs` executes `tests/semantics/thread_pool_stdlib_test.sapp` and asserts no `FAIL` markers.

---

## A4-gamma: tensor operations

`stdlib/tensor.sapp` now includes:

- **`tensor.slice(t, start, end)`** — Slice a 1D tensor from flat index `start` to `end` (exclusive).
- **`tensor.reshape(t, new_shape)`** — Reshape tensor to new shape (same total elements).
- **`tensor.dot(a, b)`** — Matrix product `[m×k] × [k×n] → [m×n]` in pure pblang.

See `examples/ai_chat_toolcall/main.sapp` for an OpenAI tool-call request/response example.

**Tests**: `tensor_ops_pass` in `tests/semantics.rs`.

---

## Q3-delta: quantum noise model + Bloch sphere

`stdlib/quantum_sim.sapp` additions:
- **`qsim.y_gate(state, q)`** — Y Pauli gate (bit flip + phase flip)
- **`qsim.z_gate(state, q)`** — Z Pauli gate (phase flip on |1⟩)
- **`qsim.add_depolarizing_noise(circ, p)`** — Add depolarizing noise (probability `p` of X/Y/Z error per gate)

`stdlib/quantum_viz.sapp` (new):
- **`qviz.bloch_angles(state_2d)`** — Convert 2-qubit state vector to Bloch sphere angles `{theta, phi}`
- **`qviz.bloch_to_svg(theta, phi)`** — Generate SVG string of Bloch sphere with state vector arrow

**Tests**: `quantum_noise_depolarizing_pass` in `tests/semantics.rs`.

---

## W4-gamma: TLS integration (implemented)

`stdlib/tls.sapp` exports the TLS API surface and delegates to VM TLS builtins:

- **`tls.listen_tls(addr, cert_path, key_path)`** — creates a TLS listener via `tcp_listen_tls`
- **`tls.serve_once_tls(addr, cert_path, key_path, handler)`** — accepts and serves one TLS request
- **`tls.serve_forever_tls(addr, cert_path, key_path, handler)`** — serves TLS requests in a loop
- **`tls.wrap_tls(tcp_handle, cert_path, key_path)`** — upgrades a plain TCP stream to TLS server mode

TLS client verification uses trust roots from `webpki-roots` plus best-effort native roots. See `docs/TLS_notes.md`.

**Tests**: `tls_stub_exports_pass` in `tests/semantics.rs`.

---

## PL1-alpha: Polyglot block execution bridge

`stdlib/polyglot.sapp`:
- **`polyglot.run_block(lang, src)`** — Execute code in Python/Node/Bash/Ruby/Perl via subprocess.
  Returns `map{ ok, stdout, stderr, exit_code }`. Requires `allow_exec` capability.
- **`polyglot.detect(refresh = false)`** — Discover supported interpreters and
  compilers with a bounded PBLang registry and `PATH` lookup. Detection never
  starts a discovered tool, so interactive commands such as `tclsh` cannot
  block on standard input. The result is retained as one process-local map;
  pass `true` only when the host `PATH` changed and a fresh lookup is required.

Supported languages: `"python"`, `"python3"`, `"node"`, `"js"`, `"javascript"`, `"bash"`, `"ruby"`, `"perl"`.

**Tests**: `polyglot_exec_python_pass` in `tests/semantics.rs` (skips gracefully if Python not found), plus `tests/semantics/polyglot_native_test.sapp` for bounded, non-executing detection and cache reuse.

`stdlib:polyglot_policy.sapp` adds runner validation, exact language allowlists,
and a hard-bounded source size for every mode. `stdlib:ffi_universal.sapp` adds
exact-path/exact-symbol checked FFI decisions and raw-pointer opt-in. These
PBLang controls only narrow the native capability and `FfiPolicy`; they cannot
grant host access. See **docs/Controlled_execution.md**.

---

## OS1-alpha: os.sapp expansion

`stdlib/os.sapp` additions:
- **`os.env_set(key, val)`** — **Removed** in the v0.5 security audit (`std::env::set_var` is unsound in multi-threaded programs); unavailable even with `exec`/`allow_exec`. Read-only `env_get` remains.
- **`os.signal_trap(sig, handler_fn)`** — Register signal handler stub (SIGINT/SIGTERM)
- **`os.args()`** — Returns CLI args array (passed after `--` in `pblang run script.sapp -- arg1 arg2`)
- **`os.exit(code)`** — Terminate process with exit code

**Tests**: `os_env_set_pass` in `tests/semantics.rs` is kept for documentation but `#[ignore]`d (it is the suite's only ignored test).

---

## L4-alpha: LSP semantic tokens + hover

The LSP server (`pblang-lsp`) now advertises and implements two additional
capabilities alongside the existing `publishDiagnostics` support.

### `textDocument/semanticTokens/full`

A byte-level source scanner walks the document without requiring a full AST
parse.  It handles nested block comments and escape sequences inside strings,
then delta-encodes the result according to the LSP specification.

| Index | Token type  | What is classified |
|-------|-------------|--------------------|
| 0     | `keyword`   | `let`, `const`, `fn`, `if`, `else`, `while`, `for`, `return`, `match`, `case`, `import`, `export`, `try`, `catch`, `defer`, `in`, `nil`, `true`, `false`, `and`, `or`, `not` |
| 1     | `function`  | The identifier immediately after a `fn` keyword |
| 2     | `variable`  | The identifier immediately after `let`, `const`, or `for` (the loop binder) |
| 3     | `string`    | `"..."` double-quoted string literals (single-line only) |
| 4     | `number`    | Integer and float literals (`42`, `3.14`) |
| 5–8   | `comment`, `operator`, `type`, `parameter` | Registered in the legend for future use; **not emitted** in this version |

The lookahead flags (`next_is_fn`, `next_is_var`) are preserved across
whitespace and comments, so constructs like `fn   /* doc */ name` and
`let\n  x` classify correctly.  Any punctuation (parentheses, braces,
operators) resets the flags.

### `textDocument/hover`

When the cursor rests on an identifier the server looks it up in three places
in priority order:

1. **Built-in function signatures** — 22 built-ins are recognised, including
   `len`, `print`, `array_push`, `map_keys`, `json_parse`, `http_get`,
   `tcp_listen_tls`, `spawn`, `channel`, `send`, `recv`, and others.
2. **Locally-defined functions** — top-level `fn` declarations in the current
   file.  The signature is reconstructed from the parsed AST, including
   parameter types and return type when annotated.
   Format: `` fn name(param: type, …) -> return_type ``
3. **Import module names** — `import "stdlib/math"` binds the name `math`;
   hovering over `math` shows `` import "stdlib/math" ``.

All hover responses are rendered as Markdown fenced code blocks.

> LSP now includes completions, go-to-definition, and semantic tokens. Remaining work is deeper scope/type fidelity and cross-file intelligence.
