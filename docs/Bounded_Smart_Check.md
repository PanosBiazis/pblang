# Bounded source admission for smart check

Smart diagnostics can opt into a Linux descriptor-based source reader:

```sh
PBLANG_CHECK_PROFILE=bounded-source pblang check program.sapp
```

The trusted direct driver also accepts `--bounded-source` after its argument
separator. Run it from the prepared bundle root when using a portable lock:

```sh
cd "$PBLANG_ROOT"
bin/pblang run stdlib/check_cli.sapp --allow-all -- --bounded-source /path/to/program.sapp
```

`--allow-all` above is an explicit caller choice for an already trusted driver,
not a permission granted by this feature. The existing native `check` wrapper
already launches its trusted driver with that permission. Direct callers may
instead use the exact Linux descriptor-reader FFI grants documented in
[the bounded self-hosted check](PUBLIC_SELFHOST_CHECK.md). A denied or unavailable
capability fails closed. The new reader never invokes `fs_read` as a fallback.

This profile is separate from the `--check --via-pblang --bounded-source=N`
frontend route. That route's configurable byte limit and flags are unchanged.
The native `check` command does not forward driver flags: use the environment
profile for that command, and the explicit flag only with the direct driver.

## Limits and semantics

- At most **64 requested files**, checked before any requested source is opened.
  Trusted driver/library loading still precedes this gate.
- At most **1,048,576 source bytes per file** and **4,194,304 admitted source
  bytes per invocation**. UTF-8 bytes, not character counts, determine these
  limits. A file exactly at its allowed size is accepted for analysis.
- Each read's allowance is the smaller of the per-file ceiling and remaining
  aggregate budget. A successful read charges its actual bytes. A failed read
  charges its full allowance, even when it failed before copying any bytes.
  This intentionally conservative rule prevents repeated failed reads from
  refunding their possible allocation. A descriptor read can inspect one extra
  byte to detect overflow; at most 64 such probe bytes are additional to the
  aggregate admission accounting.
- When the aggregate budget is exhausted, later files are rejected without an
  open/read attempt, including empty files. Final file/error counts still
  include these rejected inputs. An exhausted detail-output budget does not
  stop source-error counting.
- Invalid UTF-8, final or parent symlinks, non-regular files, invalid paths and
  absent permissions are rejected by the existing `compiler/bounded_source`
  reader. Errors use a fixed admission diagnostic, without echoing rejected
  source contents or underlying system exception text.
- The reader closes its source handle and validates strict UTF-8 by round-trip
  comparison. It uses the descriptor-backed snapshot implementation, without
  claiming an atomic point-in-time view during arbitrary concurrent writes.
- Actual filename arguments are preserved. Prefix a filename equal to an option
  with `./`, or use its absolute path. Duplicate explicit `--bounded-source`
  options are rejected. Selecting the same profile by environment and one flag
  is allowed. Other opt-in source profiles can be combined using their direct
  driver flags. Empty/default and unknown-profile behavior remain unchanged.

`check_files(files, retained_source=false, import_origin=false,
bounded_source=false)` adds a backward-compatible fourth argument. Each call
has its own source and output budgets. No source file is executed by checking.
Default checking retains its previous whole-file reader for compatibility.

The existing **16,384-byte stdout diagnostic bound** is retained. It does not
cover native stderr or unexpected exception rendering. Source admission bounds
input bytes; it is **not a bound on parser/typechecker complexity, total process
memory, recursion, diagnostic construction, or running time**. UTF-8 validation
itself allocates representation copies. For hostile workloads use external
process/time/memory isolation as well. No throughput or RAM improvement is
claimed without measurements.

## Qualification and remaining platform limits

`tests/tooling/bounded_smart_check.py --release PREPARED_RELEASE --report NEW.json`
runs actual Linux driver and public-check processes. It covers valid/malformed
sources, strict UTF-8, byte/count/aggregate boundaries, symlink/FIFO refusal,
permissions, exact filenames, nonexecution and profile combinations. Trace
checks verify no source open after admission refusal and cap-plus-one copying
for oversized inputs. Synthetic files are immutable across each check.
The harness uses network/PID isolation, a 90-second per-child deadline, a
768 MiB address-space limit and streaming output caps. Limits are test
containment, not measurements of peak RAM or performance guarantees. An
existing report is refused; failures remain evidence.

Only Linux is qualified. Other platforms explicitly reject this opt-in profile.
The retained host's portable dependency lock resolution from an unrelated
working directory remains a separate limitation; direct qualification runs from
the prepared bundle root. Native `check` already changes to that root internally.
This change does not repair native lint's unary-negation traversal discrepancy,
install a runtime, or establish Windows/macOS/Android compatibility.
