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
Invalid UTF-8 OS argv, unbounded source/diagnostic budgets, direct retained
self-host-check false success and nested `p:run` remain separate limitations.
