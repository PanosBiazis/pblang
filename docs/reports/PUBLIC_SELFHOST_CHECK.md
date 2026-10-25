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
`spawn`, `gfx`, `env`, and `import`. Other options (including FFI profiles,
I/O-timeout and memory-report options) are not part of this checker contract
and fail admission. Exact `--ffi-lib`/`--ffi-symbol` grants are accepted only
with the bounded-source option described below. No permission is inferred from a filename or environment
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
on diagnosed source errors. By default, the adapter reads the entire file with the existing `fs_read`
contract before analysis; that route has no pre-read source-byte admission
limit. The opt-in bounded route is described below. Neither route adds a total
CPU/memory ceiling for analysis or establishes universal frontend/native
typechecker equivalence.

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

## Opt-in bounded source admission

```sh
# Qualified Linux x86-64 example. Select the actual trusted libc on your host.
pblang --check --via-pblang --bounded-source=65536 \
  --ffi-lib=/usr/lib/x86_64-linux-gnu/libc.so.6 \
  --ffi-symbol=free --ffi-symbol=close --ffi-symbol=dup --ffi-symbol=strdup --ffi-symbol=openat \
  --ffi-symbol=memfd_create --ffi-symbol=sendfile --ffi-symbol=lseek \
  --deny-exec --deny-net examples/bounded_source_input.sapp
```

`--bounded-source=BYTES` accepts one decimal integer from 1 to 1048576, after a
leading check selector and before `--`. Repeating the option or supplying an
invalid limit exits 2 before source access. Omitting it retains the original
whole-file read, which has **no pre-allocation source limit**.

The opt-in profile uses `compiler/bounded_source.sapp::read_source(path, max_bytes)`
and the existing `linux_dirfs.read_bytes_relative` primitive. It copies at most
`BYTES+1` bytes from a held no-follow source descriptor into an anonymous kernel
file. Overflow is rejected **before** the VM loads that snapshot. Accepted
source bytes then undergo strict UTF-8 round-trip validation and the existing
source analysis. The extra byte distinguishes an exact-size file from an
oversize file; an empty file reaches the normal empty-source diagnostic.

This profile requires Linux, procfs, the existing trusted libc FFI operations,
filesystem access and module import. `--allow-ffi` alone leaves the runtime's
FFI allowlist empty and is insufficient. The example supplies the exact existing
library/symbol grants; the runtime canonicalizes the library path and enforces
its own grant/deny and locked-sandbox rules. The option parser forwards these
grants unchanged and accepts them only with `--bounded-source`. FFI profiles
remain unsupported. It grants no permissions automatically, invokes no shell
command, and never falls back to unbounded reading. Missing permissions or
unsupported backends fail nonzero. `--deny-exec` remains supported. The existing
native runtime/FFI implementation remains unchanged.

Path semantics are deliberately stricter: symlink source files and symlink
parent components are rejected; parent traversal, backslashes, trailing slashes
and backend-invalid components are rejected. Spaces, Unicode, newlines, absolute
paths and a literal `root` filename are supported. Directories, FIFOs and Unix
sockets are rejected by the tested Linux backend. This is an opt-in compatibility
difference, not a change to ordinary checks.

The bound applies to source bytes admitted by this route, not total process
memory, module loading, token/AST expansion, CPU time or hostile-code isolation.
The reader closes the source before decoding the captured bytes; concurrent
file growth cannot increase the copy beyond `BYTES+1`. It does not promise an
atomic filesystem snapshot, detect every concurrent in-place edit, or certify
that a later execution sees the same file. Check again after modifying a source.

Run `tests/tooling/bounded_source_check.py` with `--candidate RELEASE --report
FILE --group source|argv|permissions|allocation|library`. The allocation group uses
`strace` and a sparse 4 GiB fixture to verify the bounded copy and absence of
source/snapshot reads on rejection; other groups run with the checkout hidden
inside the same minimal Linux sandbox as the ordinary checker contract tests.

## Explicit retained-source compatibility preflight

`pblang --check --via-pblang --retained-source FILE` adds the existing
`compiler/retained_source_check.sapp` gate before ordinary analysis. It rejects
known/conservative nested-try capture hazards and unqualified imports without
executing the source. It can be combined with `--bounded-source=BYTES` and that
profile's explicit grants. Repeated/misplaced profile selectors fail with usage
status 2. `--retained-source` after `--` is a literal filename.

This public argv-based profile does not consult `PBLANG_CHECK_PROFILE`; that
environment variable continues to belong to `pblang check` / `check_cli.sapp`.
Default checking and `run` are unchanged. See [the retained source contract and
PBCK example](PUBLIC_RETAINED_SOURCE_CHECK.md) for limitations.

## Bounded snapshot cleanup failures

The Linux `read_bytes` and `read_bytes_relative` snapshot operation returns no
bytes when its owned parent, source or snapshot descriptor cleanup is unconfirmed.
Each descriptor number is consumed before one close attempt and is never retried;
a reported close error does not prove whether the kernel released it. Remaining
owned descriptors are still attempted. Existing process cleanup poison rejects
later bounded reads in that module instance. Caller-owned root cleanup remains
the caller's responsibility. Resources not handed back by underlying helpers are
outside this operation's ownership guarantee.

Invalid copy progress (negative, nonintegral, nonfinite, or larger than the
remaining request), invalid snapshot size, and a nonzero rewind result reject
the snapshot before publication. The `max_bytes+1` copy limit does not bound
parser/AST expansion, diagnostic construction, or whole-process CPU/RAM.

A known operation error remains the primary backend error when cleanup also
fails; `cleanup_failed`, `invalidate_handle`, `process_admission_poisoned`, and
`error_code: "bounded-read-cleanup-poisoned"` describe that uncertainty. The
compiler `bounded_source.read_source` facade retains its existing behavior:
it discards extra backend metadata and gives its own root-close failure
precedence. Both paths reject success publication on read failure. These rules
do not change other directory operations, caller permissions, the eight explicit
libc symbols, or the legacy direct native `--check --via-pblang` exit limitation.

`tests/tooling/bounded_read_cleanup_check.py` compares an unchanged baseline with
a candidate using whole copied modules and deterministic native-effect boundaries.
These are simulated failures, not claims of induced kernel close faults. Existing
`bounded_source_check.py` groups separately exercise real Linux admission, exact
argv, permission denial, source-size enforcement and library contracts.

### Shared bounded-reader implementation

The source-admission facade imports the internal
`linux_dirfs_read_core.sapp` module directly. Full `linux_dirfs.sapp` uses the
same fixed-signature native dispatcher, component traversal and bounded-read
implementation. Source admission therefore does not import the full directory
API or its publication, crypto and enumeration dependencies.

The two paths share one private, monotonic cleanup-poison state per imported
core module instance. A failed copy or unavailable memfd with confirmed cleanup
is an ordinary refusal; it does not alone poison admission. Ambiguous cleanup
continues to block later bounded reads through either path. No temporary-file
fallback, new native symbol, signature, permission grant or cleanup retry is
introduced. Full root closure retains its child, lock and enumeration cleanup.
The internal read-only root closure refuses full roots and roots carrying
children, locks or enumeration state; it must not be used as a replacement for
the full directory owner's `close` operation.

The existing full-directory public exports/defaults and compiler
`read_source(path, max_bytes)` contract are unchanged. The shared module's
underscored helpers and policy callbacks are internal implementation boundaries;
creation/protection callbacks are bound by the full directory wrappers, never
selected from file input. Factoring import dependencies alone does not establish
a startup or memory improvement; compare identical workloads and runtimes.

The copied-module cleanup qualification additionally checks direct shared-core
reads, poison propagation in both directions, read-only ownership refusal and
full child/lock/root close ordering. `--candidate-only` explicitly skips the
historical broken-reader comparison; it must not be reported as a differential
proof. The exclusive-write harness also supports the factored facade without
substituting a separate reader implementation.
