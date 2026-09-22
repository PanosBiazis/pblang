# Self-hosted PBCS container and call contracts

The PBLang codec is `stdlib/selfhost_pbck.sapp`. PBCS holds self-hosted compiler
chunk maps; it is **not** the retained native PBCK format, despite both sometimes
using a `.pbck` filename. `selfhost_to_pbck.sapp` translates between those IRs.

## Version 3

The writer emits magic `PBCS` followed by version byte `3`. The reader accepts
versions 1, 2 and 3, and rejects unknown versions and trailing bytes.
Version 2 introduced tagged operands for local superinstructions. Version 3
retains those encodings and changes only function constants (value tag `4`):

| Field | Encoding |
| --- | --- |
| Total arity | little-endian u32 |
| Minimum-arity presence | u8, exactly 0 or 1 |
| Minimum arity | little-endian u32, only when presence is 1 |
| Upvalue count | little-endian u32 |
| Upvalues | repeated u32 index, u8 local flag |
| Body | recursively encoded chunk |

An explicit minimum must be an integer in `0..arity`. Zero is a real minimum,
not absence. Invalid values are rejected before writing and after reading.
The presence field adds one byte per function without metadata, or five bytes
per function with metadata. Functions inside closures and try/catch subchunks
use the same recursive format. Non-function payloads are unchanged apart from
the version byte. VM instructions and per-instruction accounting are unchanged.

## Why the metadata matters

For `fn add(a, b=2) { return a+b; } print add(3);`, compilation records total
arity 2 and minimum arity 1. Versions 1/2 discarded the minimum. Native PBCK
translation then treated the reloaded function as requiring both arguments and
rejected the valid call. Some self-hosted VM call paths instead lost their
minimum-argument check. Version 3 preserves the compiler's original contract.

## Migration

Upgrade readers/bundled loaders before distributing v3 artifacts. Old readers
reject v3; renaming a file or changing its version byte is not a migration.
Do not mix a new writer with an old packaged loader. Existing source APIs retain
their signatures. Native PBCK format/version is unchanged.

Old v1/v2 artifacts remain readable with absent `min_arity`; v3 roundtrips
preserve that absence instead of fabricating a contract. Recompile source to
recover lost required/default-argument metadata. Re-encoding an old artifact
alone cannot repair it. The module cache's default toolchain fingerprint includes
`selfhost_pbck.sapp`, so changed codec source changes that fingerprint. This does
not authenticate cache contents or custom dependency lists.

## Scope and validation

`tests/semantics/selfhost_pbck_arity_test.sapp` covers plain/optimized compilation,
recursive metadata, exact native translation bytes, VM/loader execution,
missing-argument rejection, malformed metadata, truncation and legacy absence.
`tests/fixtures/selfhost_pbcs_defaults_emit.sapp` emits original and roundtrip
native artifacts in a disposable working directory for execution comparison.
`bench/selfhost_pbcs_arity.sapp` measures identical codec/run workloads on both
revisions. See [current evidence](PBCS_ARITY_VALIDATION_2026-09-22.md).

This change does not verify all bytecode stack/control-flow invariants, fix raw
VM/method entry-point parity, change scalar closure sharing, provide hard memory
isolation, or reduce GC pauses. Existing container admission limits and whole-file
reads are unchanged. A bounded container reader is not a sandbox for arbitrary
code. No new source-size or general application limit is introduced.
