# Public self-hosted check exit status (Linux launcher)

The qualified Linux launcher routes the explicit self-hosted legacy check to a
PBLang adapter. It bases the process result on
`compiler/source_analysis.sapp::analyze(source, false).ok`, never on diagnostic
text. The retained native executable is unchanged. Its direct
`pblang-native --check --via-pblang` can still print failure and exit 0.

```sh
pblang --check --via-pblang 'source with spaces.sapp'
pblang --self-host --check program.sapp
pblang --check program.sapp --via-pblang
pblang --check --via-pblang --deny-exec --deny-net program.sapp
pblang --check --via-pblang -- --filename-starting-with-dashes
```

## Selected command and argument contract

A leading `--check`, `--via-pblang`, or `--self-host` selects this family;
capability options may precede it. A self-hosted selector and `--check` must both
be present. The accepted command has exactly one `--check`, one or more
self-hosted selectors, and exactly one nonempty source filename. The two
self-hosted selectors are aliases. Options can precede or follow the filename
until `--`; all subsequent arguments are filenames, not options. Spaces,
Unicode, newlines, and literal option names after `--` preserve their argv
boundaries. Unknown options and extra filenames fail with usage status 2.

The retained handler scans selectors even after `--`. The launcher intercepts
that ambiguous legacy shape too, then applies the strict boundary above. Thus
`--check -- file.sapp --via-pblang` is rejected, not silently passed back to a
native checker that can report false success.

`run SCRIPT -- --check --via-pblang`, bare scripts, `check FILE`, plain
`--check FILE`, `fmt`, `test`, and unrelated native commands keep their existing
routes. Leading alias permutations are an explicit launcher extension; the
retained legacy command does not implement all those permutations.

Supported capability options are `--allow-all`, `--sandbox`, and
`--allow-NAME`/`--deny-NAME` for `exec`, `ffi`, `net`, `fs`, `http`, `sqlite`,
`spawn`, `gfx`, `env`, and `import`. Other options (including FFI profile/grant,
I/O-timeout and memory-report options) are not part of this checker contract
and fail admission. No permission is inferred from a filename or environment
argument string.

The adapter is run with the exact original argv through the native `run`
handler. That handler enforces capability ordering, environment defaults, the
`--` separator and the locked sandbox. In particular, later `--allow-all` does
not unlock `--sandbox`. The adapter needs source-module import and source-file
read access; denying either produces a nonzero status. It does not grant
`--allow-all`, read `PBLANG_SCRIPT_ARGS`, or override caller permissions.

## Result and execution boundary

- Successful analysis exits 0 and prints the existing statement/token summary.
- Structured syntax/type/empty-source diagnostics exit 1.
- An unreadable or missing source exits 1.
- Invalid supported-command arguments exit 2 before reading a source.
- Host, import, permission or unexpected analyzer failures remain nonzero native
  runtime failures; they are not converted into successful checks.

The adapter parses and type-checks one source file; it does not execute that
source, load its declared imports, or certify transitive dependencies. It uses
the same tokenizer/parser/type checker as `frontend_analyze`, with the token
payload omitted. Existing structured negative results remain ordinary return
values for library callers: this change does not make `frontend_analyze` throw
on diagnosed source errors. The adapter reads the entire file with the existing `fs_read` contract before
analysis. There is no pre-read source-byte/memory admission limit or new
CPU/memory ceiling for analysis, and
no universal frontend/native-typechecker equivalence claim.

`tools/native_bundle_guard.sapp` contains one argv parser reused in two phases.
The admission phase runs under `--sandbox`, performs no imports or file reads,
and returns one of three exact protocol markers: native dispatch, self-hosted
check dispatch, or check usage error. The execution phase performs the admitted
analysis with the caller's permissions. `tools/pblang-launch.sh` only bridges
those decisions to exec/status; empty, corrupt or unknown admission output
still fails closed. Native `bundle` remains disabled by the same guard.

## Qualification and installation

Use the versioned candidate layout in
[NATIVE_BUNDLE_SOURCE_INTEGRITY.md](NATIVE_BUNDLE_SOURCE_INTEGRITY.md), including
its installed manifest, full matching stdlib, and materialized dependency lock.
The standalone fixture used only to test bundle rejection needs no stdlib;
self-hosted checking does. Source lookup remains relative to the caller cwd,
while trusted compiler modules resolve from the installed guard's origin.

`tests/tooling/public_selfhost_check.py` has four short test groups (`source`,
`argv`, `permissions`, `scope`). Each runs a prepared candidate in a network-
isolated minimal Linux filesystem with synthetic inputs and compares source
hashes. Run existing `native_bundle_guard.py` too, plus structured-analysis
negative/parity regressions. Candidate success does not activate the public
installation: activation is a separate, coordinated operation with rollback.

This only qualifies Linux launcher behavior. Direct retained-native checks,
nested `p:run`, other check profiles, Windows/macOS, and full compiler correctness
remain separate. No Rust compiler change is included.
