# Exact source filenames for `pblang check`

The PBLang driver `stdlib/check_cli.sapp` reads the actual process argument
array after the first `--`. It preserves each argument, including Unicode,
newlines, tabs, leading/trailing spaces and empty values. It no longer splits
`PBLANG_SCRIPT_ARGS` on newlines or trims filenames. This prevents checking a
different file, such as `bad.sapp` instead of a requested `bad.sapp `.

The native `check FILE...` command already passes absolute filenames to its
trusted driver as separate child-process arguments. The same contract applies
to explicit driver calls:

```sh
pblang check 'source with trailing space.sapp '
pblang run stdlib/check_cli.sapp -- 'source with trailing space.sapp '
```

## Compatibility

`PBLANG_SCRIPT_ARGS` is no longer a filename input for this driver. Env-only
callers must pass real arguments after `--`; a missing separator or empty tail
fails with the existing usage error. An empty argument remains an attempted
file read and fails rather than disappearing. Native `check` may first resolve
an empty argument to the caller directory, which likewise fails to read as a
source file. Runtime `run` already replaces the environment channel from its
real arguments; it is not a reliable independent argument API.

`PBLANG_CHECK_PROFILE`, the explicit `--retained-source` / `--import-origin`
driver flags, `check_files` and the existing diagnostic and exit contracts are
unchanged. A profile-looking literal filename uses an absolute path or `./`
prefix. A second `--` is an ordinary driver argument, not another separator.
Native `check` still filters flag-looking arguments before launching its
driver; this change does not modify that native behavior.

Valid sources return 0. Source diagnostics, missing files and driver usage
errors remain nonzero. Checking never executes the source. No new capability,
permission or environment grant is introduced.

## Qualification

`tests/tooling/check_cli_exact_argv.py` runs prepared old/new release copies
with source checkouts absent, networking isolated and synthetic writable
fixtures. It records the old wrong-file result, the reverse-content control,
exact newline/Unicode/space names, batches, source-effect nonexecution, profiles
and public/direct routes. Parent-environment cases demonstrate end-to-end routing
only: native `run` overwrites that channel. Static inspection separately verifies
that `_arg_files` does not read the filename environment. A historical attempt
to set the environment after startup was rejected by the existing deny-exec
profile; no grant was widened to bypass that check. Captured output is limited
while streaming;
child time, memory and process cleanup are bounded by the harness. The memory
limit is not a measured RSS result.

The caller provides a prepared candidate with matching libraries and dependency
lock and serializes heavy work under its existing test lock. The harness does
not install anything. Linux qualification is not Windows/macOS evidence.
Invalid UTF-8 OS argv, unbounded source/diagnostic-construction budgets, direct retained
self-host-check false success and nested `p:run` remain separate limitations.

## Bounded diagnostic output

The public `check` driver and its direct script route limit their own stdout
per invocation to 16,384 UTF-8 bytes. All files still undergo the original
analysis, and the final complete file/error/warning counts determine the same
exit status. Exhausting the detail budget never changes a later error into a
successful check. Exact filenames are still used for reads even when their
printed representation is shortened.

Details share 15,872 bytes, including every implicit print newline. A separate
512-byte reserve covers one fixed truncation notice and the final summary.
Counts must be finite nonnegative exact-safe integers; malformed or overflowing
counts fail rather than silently reporting success. The notice appears for any
field, entry-count or aggregate truncation, once per invocation.

`diagnostics.format_bounded(result, max_bytes=16384)` returns
`{text, bytes, truncated}`. The limit is an integer from 0 through 16384;
`bytes` counts UTF-8 bytes exactly. It visits at most 128 diagnostic entries,
and each string field contributes at most 1024 UTF-8 bytes. The additive
`format_text_bounded(parts, max_bytes=16384)` accepts at most 32 string
fragments, with the same per-fragment limit. These functions validate rendered
field types and numeric counts/locations; they do not validate an unvisited
trailing diagnostic list. Invalid schemas produce fixed errors rather than
stringifying arbitrary objects.

Output below all limits retains legacy formatting, including trailing
newlines. Clipping never splits a Unicode scalar. Later punctuation or other
fragments can still fit after a long field is clipped: the resulting text is
not necessarily one contiguous prefix of the unlimited output. Check
`truncated`, not the appearance of the text, when using this API directly.
Legacy `diagnostics.format` and `watch.format_diagnostics` are unchanged.

This is a rendering/output bound, not a source-admission, parser, typechecker,
diagnostic-construction, process-memory or CPU bound. The bootstrap's stderr
and unexpected internal exception rendering are outside this stdout contract.
The separate `--check --via-pblang` route and repeated external calls to
`check_files` do not share this aggregate budget. No runtime capability or
platform support changes are implied.
