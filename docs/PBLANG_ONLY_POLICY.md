# PBLang-only implementation and migration policy

New language functionality, libraries, tools, and product logic must be written
in PBLang when the current runtime can express the required behavior. Existing
Rust or other host-language code is a legacy reference and bootstrap boundary,
not the default place for new implementation work.

This policy does **not** claim that the repository is already free of legacy
Rust. Removal is deliberately slower than replacement: the old implementation
stays available until the PBLang port has behavior tests, regression coverage,
recorded parity evidence, and a separately reviewed retirement step.

## Executable gate

`stdlib:migration_policy.sapp` evaluates a bounded JSON change manifest without
running `git`, a shell, or another language. `tools/pbmigrate.sapp` is the
PBLang-only CLI:

```sh
./target/release/pblang run tools/pbmigrate.sapp examples/migration_policy_manifest.json
./target/release/pblang run tools/pbmigrate.sapp --json examples/migration_policy_manifest.json
```

The manifest format is `pblang.migration.change_manifest.v1`. Each change has
an exact relative `path`, `change` (`added`, `modified`, or `deleted`), and
bounded `added_lines` / `removed_lines` counts. A repository hook or CI job can
produce these records from its trusted source-control API. The policy engine
intentionally does not grant itself process execution merely to inspect git.

The gate fails closed for malformed or excessive input, unsafe paths, duplicate
records, invalid migration evidence, and expired exceptions. Paths are capped
at 512 bytes; manifests are capped at 10,000 changes, 1,000 exceptions, and
2,000 migration records.

## Rules

- New `.rs` files are always blocked.
- Added lines in an existing `.rs` file are always blocked, even if an
  exception is supplied.
- Rust deletion-only changes are allowed so verified migrations can reduce the
  legacy implementation.
- New PBLang source (`.sapp`, `.sap`, or `.bpx`) is allowed.
- Documentation, configuration, and static data are not implementation code.
- Other implementation languages, shell scripts, HTML, and CSS require a
  valid exact-path host-boundary exception when lines are added.
- Unknown file types fail closed as implementation boundaries when lines are
  added.

The gate evaluates the supplied manifest. It does not infer line counts or file
contents, so the caller must derive the manifest from a trusted diff and must
not let an untrusted contributor self-approve it.

## Temporary host-boundary exceptions

An exception is permitted only for a boundary that PBLang cannot yet express:
`bootstrap`, `system-api`, `abi-shim`, or `generated-artifact`. It must include:

- a unique `id` and exact relative `path` (no wildcard),
- `status: "active"`,
- a concrete `reason` and accountable `owner`,
- a bounded `max_added_lines`,
- a `review_by` date,
- one or more verification tests,
- a concrete `removal_plan` describing the PBLang replacement.

Exceptions cannot authorize Rust additions. Expired, malformed, duplicate, or
over-limit exceptions block the gate. An unused valid exception is reported as
a review warning so temporary permissions do not silently become permanent.

Current boundary categories include the legacy compiler/runtime bootstrap,
operating-system and device APIs that are not yet exposed by the PBLang
runtime, ABI shims, and generated portable C99 artifacts. These categories are
not blanket approvals: every new non-PBLang change still needs an exact record
with tests and a removal plan.

## Migration lifecycle

Migration records use `pblang.migration.record.v1` and advance in this order:

1. `reference`: the host implementation remains available while the PBLang
   implementation is introduced.
2. `parity`: focused behavior tests exist, but the old fallback is retained.
3. `verified`: behavior tests, complete regression tests, and parity evidence
   pass; the old fallback is still retained.
4. `retired`: a separate record confirms prior verification and removal of the
   legacy path.

`retirement_ready(record)` is true only for a valid `verified` record that
still retains the fallback. It is a readiness signal, not automatic deletion.
The removal itself must be reviewed and followed by the same focused and full
regression suites.

The passing example is `examples/migration_policy_manifest.json`. Safety,
classification, exception, line-limit, duplicate, traversal, expiry, and
lifecycle behavior are covered by
`tests/semantics/migration_policy_test.sapp`; the shell-free CLI contract is
covered by `tests/semantics/pbmigrate_cli_test.sapp`.

