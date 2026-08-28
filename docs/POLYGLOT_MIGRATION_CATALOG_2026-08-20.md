# PBLang and Polyglot Migration Catalog

Date: 2026-08-20

## Scope and boundary

This catalog records a read-only metadata inventory. No discovered program,
archive, binary, model, database, installer, or script was executed. Personal
documents, credentials, logs, runtime state, and private datasets were not
opened. The `sw` tree is idea-only and must never be copied or modified by this
migration.

The catalog is not a license or ownership decision. Every candidate requires a
separate provenance check before source is read or reimplemented.

## Source groups

### Primary workspace

- `products`: active PBLang products and the first migration destination.
- `codes`: small language exercises and experiments suitable for bounded review.
- `projects`: 42 project families, including product candidates and duplicates.
- `others`: mostly educational, personal, generated, archived, and configuration
  material; default quarantine applies.

### Other mounted user-data disks

- `BACKUP`: backup-oriented content. Treat matching projects as historical
  snapshots until a hash and revision comparison proves otherwise.
- `Ventoy`: contains apparent older copies of PBLang, pbAI, pb-mcp-server, and
  other projects. Deduplicate; do not merge it directly into active repositories.
- `TESTER`: primarily Delphi and vendor utility material. Use only documented
  behavior as architectural inspiration after ownership review.
- `SPSSDT7`: primarily installed games and platform-managed content. Excluded
  from migration unless a separately owned source directory is explicitly
  approved.
- System filesystems are excluded from broad project discovery.

## Classification

### PBLang examples

- Small algorithms, mathematics, parsing, and language exercises.
- SQL examples with generated local fixtures and no real database contents.
- Raspberry Pi and LED-controller concepts using the shared embedded HAL.
- Small calculators and web exercises.
- Assembly, C, C++, Go, Lua, Pascal, Fortran, and COBOL behavior examples after
  individual safety and provenance review.

Do not create a duplicate example when an equivalent maintained PBLang example
already exists. Extend its tests and platform matrix instead.

### Polyglot conformance projects

- Equivalent algorithms implemented in PBLang and one foreign language.
- Rust/Python experiments retained only as reference inputs, never as new
  PBLang implementation dependencies.
- Web-framework and graphics comparisons with identical inputs and observable
  behavior.
- ABI, serialization, error, cancellation, and resource-limit conformance.

A mixed-language directory is not sufficient. A conformance project must define
the same contract, inputs, expected outputs, resource bounds, and evidence for
each implementation.

### Product candidates

- SpotIt.
- A consolidated WordDetect product after divergent copies are compared.
- TesterStudio after tracked-source ownership is established.
- Selected CRUD and web applications after version consolidation and removal of
  legacy authentication, SQL, and CSRF defects.
- Financial automation only after credentials, account state, logs, compliance
  rules, and broker side effects are isolated behind a reviewed supervisor.
- AI/model projects only from clean source architecture. Never deserialize or
  import discovered model objects during migration.

### Idea-only sources

- `sw`: accounting, budgeting, sales, CRM, networking, reporting, ORM, memory,
  and component-system ideas only.
- Third-party frameworks and vendor libraries.
- Reverse-engineering material and unknown-provenance archives.
- Educational collections and private datasets.

## Immediate quarantine rules

Do not migrate automatically:

- authentication, recovery, key, token, account, or credential artifacts;
- configuration exports, logs, databases, financial state, or user content;
- pickle files, serialized models, model caches, or generated AI output;
- archives, installers, executables, libraries, object files, compiled classes,
  IDE caches, or build output;
- CMS vendor/content trees until source and generated content are separated;
- system, network, installation, flashing, recovery, or destructive scripts;
- any source with unknown ownership or incompatible licensing.

## Duplicate handling

- Treat byte-identical copies as one provenance unit, not multiple projects.
- Preserve divergent CRUD, WordDetect, web, and finance versions until behavior
  and data migrations are compared.
- Metadata equality is not byte equality. Hash only approved source candidates.
- Never overwrite an active child repository from a backup copy.

## Required migration pipeline

1. Allowlist one directory.
2. Inventory metadata only, with personal identifiers redacted.
3. Apply privacy, provenance, ownership, and license gates.
4. Separate source from binaries, state, content, dependencies, and generated data.
5. Review complete owning modules and observable behavior.
6. Classify as PBLang example, polyglot conformance project, product, or idea-only.
7. Reimplement in a clean staging child repository using PBLang source.
8. Add positive, negative, malformed-input, resource-bound, privacy, and
   compatibility tests.
9. Run the default PBLang security gate and platform-specific verification.
10. Publish only current evidence; unsupported behavior must remain fail-closed.

## Current limitations

### Polyglot execution security status

- JSON bridges for Python, JavaScript, and Ruby now validate bounded JSON and
  transfer it exclusively through stdin. Caller JSON is never interpolated into
  generated foreign-language source. Unsupported bridge languages and malformed
  or oversized JSON fail before execution. Parsing uses the bounded serializer
  once; the public plan never exposes the retained parsed value used by the
  private executor.
- Foreign program source remains explicitly executable code. The stdin contract
  prevents data-to-source injection; it is not a filesystem, network, syscall,
  compiler-plugin, or package-build sandbox.
- `polyglot_build` now publishes generated source through no-follow secure-file
  operations, creates a fresh private output session for each source build, and
  executes fixed compiler candidates through the descriptor-held bounded process
  facade. Every exported language builder creates its own random private child
  session, so a direct builder call cannot target a planted deterministic output
  symlink. Recursive project discovery uses no-follow directory roots and rejects
  symlinked source/output components on Linux.
- The current Linux descriptor backend still invokes a fixed `/usr/bin/stat`
  or `/bin/stat` candidate to verify held-descriptor owner and mode. Therefore
  these secure publication paths currently need both the FFI and execution
  capabilities. This is not shell execution and the path is a validated held
  `/proc/<pid>/fd/<fd>` identity, but an FFI-only backend remains incomplete
  until PBLang decodes bounded `fstat`/`statx` results for each supported ABI.
- Directory enumeration still has two explicit platform limits: Linux/macOS
  backends truncate returned entries only after the host `fs_listdir` allocation,
  and Windows root opening does not yet prove every intermediate component is not
  a junction. Whole-project polyglot discovery must therefore remain disabled for
  hostile giant directories and untrusted Windows trees until streaming native
  enumeration and component-wise Windows root opening are implemented.
- `polyglot_db.sapp` no longer constructs shell commands or calls raw process
  execution. It now requires typed network connections, fixed executable
  candidates, bounded private stdin, explicit resource ceilings, redacted
  diagnostics, deterministic no-follow migrations, and truthful failure
  propagation. Ambiguous legacy MySQL/Redis flag strings fail closed.
- `polyglot_web.sapp` now publishes HTML through exclusive/atomic no-follow
  secure-file operations. Generated pages carry restrictive CSP and referrer
  policy defaults. Local browser launch and Node/Deno execution use fixed
  bounded launchers/processes, explicit environments, private stdin, and no raw
  fallback. Browser status explicitly reports that browser execution and file
  identity are not bounded; the same-UID path is a trust boundary. Trusted
  executable fragments remain separate from the escaped, script-free
  `bundle_text_html` API. Node/Deno resource bounds are not an OS sandbox.
- `interop.sapp` now limits trusted source to 3,584 UTF-8 bytes and the final
  generated `-c`/`-e` wrapper to the Linux supervisor's 4,096-byte per-argument
  ceiling. PBLang's bounded serializer performs item, depth, key, scalar and
  estimated-output checks before JSON construction, using a linear byte scan
  without repeated character slicing. Private values travel only through stdin.
  Fixed Python/JavaScript/Ruby interpreters run through the bounded process
  facade with timeout, output, file and memory ceilings plus descriptor-held
  executable identity. Python additionally uses isolated, no-site mode (`-I -S`)
  so the current directory and user-installed packages cannot hijack imports.
  `cross_call` accepts only the explicit `main` entrypoint;
  unsupported pairs, oversized input and oversized benchmarks fail before
  execution. Process, pipe-stage, and benchmark results distinguish caller
  acknowledgement from runtime authority and separately preserve observed
  execution, timeout, truncation and boundedness state. Every executable path
  now also requires `unsafe_polyglot:true` plus a SHA-256 bound to the exact
  source bytes. `trusted_source:true` remains a separate risk acknowledgement;
  neither option grants host capability or
  prove sandbox isolation. The
  registry JSON planner uses the same size
  contract and `input_data` name as its bounded executor. Compatibility
  serializers publish canonical JSON data and never executable literals. Every
  public executor (`run_filter`, `call`, `pipe`, `benchmark_compare`, and
  `cross_call`) requires the hash-bound unsafe acknowledgement and the exact
  option `trusted_source: true` before it can
  reach a process. Missing, false, string, and numeric values fail before spawn.
- `polyglot_registry.sapp` no longer uses shell availability probes, shared
  temporary source files, bare executable names, raw `proc_spawn_wait`, or
  generic in-process FFI. Public registry metadata is detached from the private
  executable database. Eight direct interpreter adapters and the executable
  JSON bridge require the exact hash-bound unsafe acknowledgement plus boolean
  `trusted_source: true` and use
  fixed descriptor-held bounded processes;
  unsupported direct languages fail closed. Direct compilation requires
  explicit trusted-source and toolchain-effect acknowledgements, reuses the canonical
  `polyglot_build.sapp` dispatch, retains artifacts only under private output
  sessions, and publishes a verified byte count and SHA-256. Resource bounds
  are not filesystem/network isolation, so results state `sandboxed:false` and
  `effects_confined:false`. Generic registry FFI and raw artifact loading are
  disabled; checked native artifact loading requires an exact FFI policy.
- Legacy `polyglot.sapp` is now a PBLang-owned fail-closed compatibility facade.
  Direct interpreted execution requires explicit trusted source and routes only
  through fixed descriptor-held bounded adapters; stdin has a 1 MiB private
  descriptor limit. Shared temporary files, ambient compiler execution, the
  custom D cache, raw compile-and-run, in-process `compile_cached`, and native
  cache mutation were removed from this public path. Artifact compilation is a
  separate explicitly authorized operation and never loads the result. The raw
  native VM polyglot builtins remain bootstrap/reference code and are still an
  internal migration slice; the facade no longer calls them. The bounded process
  backend is currently Linux x86-64 and other platforms fail closed.
- Router plans now distinguish successful analysis from executable authority.
  They always return `allowed:false` and `executable:false`; a compatibility
  `sandbox_ready=true` argument is recorded only as `sandbox_declared:true`,
  never as verified isolation. When analysis would permit execution, the reason
  is `execution_authority_required` and the caller must invoke a capability-gated
  executor separately.
- `secure` polyglot policy uses pattern scanning for diagnostics but no longer
  treats that scan as isolation. It requires an explicit sandbox runner before
  scan-safe source can execute. Generic callbacks report only
  `sandbox_runner_used:true`; they remain `sandbox_verified:false`,
  `sandboxed:false`, and `effects_confined:false` until a concrete backend can
  provide evidence. Policy state and callbacks live in a bounded private registry;
  public opaque handles and detached descriptions cannot mutate authority.
- Guard source, language, dependency and allowlist inputs are byte/count bounded
  before canonicalization or comparison. Oversized or malformed requests fail
  closed and report incomplete analysis instead of returning partial approval.
- SQL polyglot builds never execute source for validation. Until a bounded,
  non-executing parser is available, SQL is published as an explicit source-only
  fallback and aggregate reports set `complete:false`.

This catalog does not claim that every file was semantically reviewed or that
every project can legally or technically be converted. The other-disk pass was
intentionally metadata-only. Full conversion is a multi-project program and
must proceed in independently buildable, reviewable slices.
