# Selfhost data-library execution

The PBLang bytecode VM supports `range`, `json_parse`, `json_stringify`,
`bytes_to_str`, `regex_match` and `regex_replace_all`. This completes the primitive
path used by the unchanged synthetic sensory fixture in
`tests/fixtures/selfhost_sensory/original.sapp`: its result must be exactly
`[2000, 2, false]` (mean, replay count, acquisition performed).

The fixture runs imported source through the PBLang frontend and bytecode VM.
The existing pure native JSON, UTF-8 and regex primitives still provide their
structural/encoding operations; this is not a fallback to native execution of
the guest program. `json_decode.sapp` continues to own corrected binary64 JSON
number conversion and `serialization.sapp` continues to own duplicate-key checks.
No physical sensor acquisition or universal library/backend parity is implied.

## Contracts and bounds

- `range(end)`, `range(start,end)` and `range(start,end,step)` produce an array,
  excluding the end, using repeated floating-point addition. Signed and fractional
  steps work. Inputs must be finite; zero step and non-progressing addition reject.
  These restrictions deliberately replace native cases that could run forever.
  At most 1,048,576 items are emitted; bounded runs also obey their smaller
  `max_value_items` and shared step budget, checked before each push.
- JSON input/output admission uses a hard 1,048,576-byte ceiling, depth ceiling
  128 and at most 1,048,576 visited values/keys. Smaller runtime value/work limits
  apply in bounded execution. Host syntax and parse depth validation still apply.
  Serialized size estimates conservatively allow six escape bytes per input byte,
  plus quotes/punctuation and scalar bounds. A near-limit value can therefore be
  rejected even when its eventual JSON would fit. Cycles and repeated subtree
  expansion terminate at depth/visit/work limits; preflight invokes no guest methods.
- `json_stringify` rejects maps containing `__bi` or `__fn`, regardless of the
  marker's value, and maps with a map-valued `chunk`. The VM represents callable
  objects as forgeable mutable maps: identically shaped ordinary user maps and
  parsed JSON cannot be distinguished and also reject, including nested values.
  `chunk: 7`, `chunk: "text"` and unrelated keys such as `__fnx` remain ordinary
  data. JSON parsing itself does not reserve these shapes. This is a deliberate
  selfhost serialization restriction, not full native JSON-domain parity.
- Exact boxed numbers retain the native string representation. **Retained
  limitation:** `str(bigint/rational/decimal)` materializes a temporary decimal
  representation before its size is available. Visit, work and depth admission
  precede this one conversion per scalar visit; UTF-8/escaped-output admission
  precedes final host JSON serialization. This limitation also applies to
  `frontend_run_bounded`: no preconversion digit-size, CPU or RSS bound is claimed.
- `bytes_to_str` accepts only arrays of integer bytes 0..255 and valid UTF-8,
  with the same hard item ceiling and smaller bounded value/work limits.
- Regex patterns are limited to 4,096 UTF-8 bytes; text/replacement to 1,048,576.
  Before replacement allocation, the conservative bound is
  `B + (C+1)*L + R*B`, where B is input bytes, C input Unicode scalar count,
  L all replacement bytes and R every dollar sign. Products are checked against
  remaining capacity before multiplication. Capture and dollar syntax stays
  host-owned; the bound intentionally overcounts. Pattern/text limits and charged
  work estimates are **not** a hard deadline for the host regex engine.

Ordinary type/domain errors follow existing catch and owned-memory cleanup
semantics. Bounded resource failures are sticky: a guest catch cannot resume the
same exhausted budget. In an otherwise unbounded run, a hard data admission
rejection is catchable and preserves ownership when handled, like ordinary errors.
Direct calls, first-class aliases and small-arity fallback
all use the same new admission helpers. `range` has a static 1..3-argument signature;
previously gradual helper signatures stay gradual because the typechecker does
not yet narrow every optional value inside guarded branches. Runtime arity and
type checks remain mandatory.

## Local dependency roots

`frontend_run(source)` uses the caller's `pblang.toml` declaration when imported
source is discovered. It supports this deliberately restricted local form:

```toml
[dependencies]
stdlib = { path = "../pblang/stdlib", paths = ["stdlib"] }
```

`paths` is optional. The resolver accepts single-line double-quoted path literals,
whitespace and quote-aware comments. It rejects duplicate declarations/paths,
unsupported explicit forms (including git/version declarations, quoted/dotted
keys and `[dependencies.stdlib]`), escapes/control characters in paths and
ambiguous declarations. It does not implement general TOML. Other dependency
values do not cause network or package resolution.

An explicit caller option `stdlib_root` takes precedence. It is available in
`frontend_run(source, options)`, `frontend_run_file(path, options)` and the existing
bounded entrypoint options. It does not grant filesystem authority. Unbounded
entrypoint options select imports only; bounded runtime limits belong to the
bounded entrypoints.

Both `stdlib:module.sapp` and `stdlib/module.sapp` use the selected root. Suffixes
cannot contain empty, dot, dot-dot, backslash or control components. Explicit
sibling roots are allowed because the caller's declaration is the authority.
Other local imports retain their existing working-directory semantics. No
upward search, environment lookup, downloads or working-directory mutation occurs.
Without a declaration, the legacy working-directory `stdlib` mapping remains.
An unavailable selected module never falls back to a shadow local copy. Missing
or lexically invalid modules retain empty exports, cycle guards and export filters.

Import-free source with no discovered import markers never reads a manifest.
The existing import scanner is textual, not a full AST import-origin resolver.
Retained `fs_read` allocates before manifest/source size admission: this is
caller-owned filesystem access, **not** a hard descriptor read bound or symlink
containment guarantee. Manifest admission is 65,536 bytes, 1,024 lines and 4,096
characters per line; paths are at most 4,096 bytes. No FFI or read permission is
silently added.

## Persistent cache identity

Guest library selection and executing compiler identity are separate. If a
manifest declaration or explicit root is selected, an opt-in persistent module
cache request rejects before cache fingerprint/publication I/O—even if the root
looks identical to the working-directory library. The host does not currently
supply proven immutable origin for the loaded compiler; hashing guest directory B
cannot identify executing compiler A. Normal uncached execution remains supported.

Legacy no-selection cache contexts and the standalone module-cache API retain
existing behavior. The default fingerprint list includes `import_roots.sapp`;
`default_dependency_paths()` returns a fresh array. Cache filesystem publication
mechanics and their explicit capability requirements are unchanged.

## Qualification

`tests/tooling/selfhost_sensory_parity_check.py` accepts explicit `--repo`,
`--runtime`, new `--report` directory, existing `--lock`, `--lock-dev` and
`--lock-inode`. Choose `--group original`, `focused`, `regression`, `resolver` or `cache`.
It snapshots source, recomputes only disposable lock metadata, and preserves all
four original fixture sources. Original acceptance covers external declared root
with no local library and a matching-layout control; native output is a separate
oracle. Resolver controls also load a byte-distinct disposable compiler A-prime against
the same guest root B and verify selected-root cache rejection before publication.
Cache-only checks use the existing adapter's explicit libc symbol grants;
sensory and ordinary VM checks deny FFI and process execution.

The harness acquires the existing lock for at most 45 seconds without replacing
it. Each phase has a 180-second execution budget, each case at most 30 seconds,
28 CPU seconds and 768 MiB address-space limit. A selector drain retains at most
65,536 combined output bytes and kills/reaps only its own process group on overflow
or timeout. Bubblewrap provides network isolation, read-only source mounts and a writable
disposable `/tmp`; the retained VM does not provide per-path filesystem grants.
The resolver control uses `--deny-fs` and asserts that a direct manifest read fails
while import-free guest execution succeeds. Before/after source hashes are recorded. These
process limits describe qualification, not all users' runtime isolation.

Receipts record actual expected markers/assertions, exit status and setup/runtime
failures separately. Exit zero alone does not establish semantic success. A
passing synthetic fixture does not establish installed, cross-platform, physical
sensor, universal performance or complete-security status.
