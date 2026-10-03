# Retained import-origin checking

The retained executable can lose the defining source origin of an exported
function. A lazy import in that function can then resolve against the caller's
working directory/manifest. An installed dependency lock alone cannot restore
that missing context. Direct retained execution is **not repaired** by this check.

`stdlib:compiler/import_origin_check.sapp` provides `check(source)`. It inspects
source without resolving, loading or executing dependencies. It allows direct
literal imports at module initialization and rejects:

- imports inside function declarations, closures, class methods, expression-block
  closures or either side of `try/catch`, including unreachable branches;
- escaped import handles (aliases, returned functions, callback values);
- computed, empty, NUL-containing, over-4096-character or non-string paths, and
  calls with other than one argument;
- invalid/recovered syntax, exceeded source/analysis limits, or import keywords
  not accounted for by the parsed AST.

A function may use an already bound provider without importing it again. This
preserves explicit application-controlled loading: the checker never hoists an
optional import, changes cwd, rewrites source, or silently reroutes execution.
Top-level conditional imports remain conditional. Passing source can still fail
at runtime because a declared module is missing, its lock is invalid, or it needs
permissions. Only the submitted source is inspected; inspect every dependency
source separately. It is not an authenticated origin or capability certificate,
not a sandbox, and not a complete retained-host compatibility checker.

## CLI and installed use

From a trusted project directory, the existing native `check` command accepts
the profile through its environment (no Rust CLI changes):

```sh
PBLANG_CHECK_PROFILE=import-origin ./target/release/pblang check program.sapp
```

The explicit PBLang driver also accepts a flag, with multiple source files:

```sh
./target/release/pblang run stdlib/check_cli.sapp --allow-all -- --import-origin app.sapp provider.sapp
```

For a reviewed installed package with its own materialized dependency lock, use
its explicit driver path. For example, from a foreign cwd:

```sh
"$PBLANG_ROOT/bin/pblang" run "$PBLANG_ROOT/stdlib/check_cli.sapp" \
  --deny-net --deny-http --deny-exec --deny-ffi --deny-spawn --deny-sqlite \
  -- --import-origin /absolute/path/to/app.sapp
```

`PBLANG_ROOT` is the reviewed package root, not a caller-supplied module locator.
Both origin and retained-source checkers are bound at the driver's top level,
so its nested `check_files` function does not import a checker from caller cwd.
Normal source/type diagnostics still run after admission. Any failed source
makes the batch exit unsuccessfully; duplicate flags and unknown profiles fail.
Default checking remains unchanged. `--retained-source` can be combined with
`--import-origin`; its stricter rejection of **all imports** remains in force.
That separate profile still diagnoses nested `p:run`/try limitations. Neither
profile changes `pblang run` automatically.

## Result and limits

The result contains `ok`, `code`, `message`, and counts of `direct`, `deferred`,
`dynamic` and `escaped` imports. Counts can overlap: a deferred call can also be
dynamic. Failed admission/analysis may leave partial counts; do not treat them
as a complete inventory. `dependencies_checked`, `execution_performed`, and
`runtime_origin_fixed` are always false. No source text, import path, identifier
or exception content appears in this library's result. The ordinary CLI still
prints explicitly supplied file names and its existing source diagnostics.

Codes have prefix `import-origin-`: `invalid-source`, `scan-limit`, `escaped`,
`deferred`, `dynamic`, `unclassified`. Fixed priority makes the result independent
of AST map traversal order. Every call returns fresh metadata.

The shared checked parser limits input to 1 MiB UTF-8 and 65,536 tokens; the
iterative AST walk adds depth 128 and 262,144 admitted map/array nodes. The CLI's
existing whole-file read happens **before** these limits; this is not a bounded
filesystem ingestion service. Parsing retains its existing recursion guards.
The checker lexes a second time for keyword accounting. No speedup is claimed.

This is a conservative accepted subset, not a general proof. Keyword accounting
can reject benign `import` member/map-key spellings or lowering which duplicates
an import expression. Parser-unsupported constructs also fail. Unknown syntax
must not be accepted merely because no import was found in a recovered AST.

## Example and evidence

`examples/import_origin/main.sapp` binds a required SemVer provider at application
initialization and uses it from an exported function. Run the example normally;
check it with the profile separately. Its output is `true`.

`tests/semantics/import_origin_check_test.sapp` covers supported shapes, nested
and escaped imports, malformed and oversized source, scan bounds, fixed-message
privacy and independent results.

`tests/semantics/import_origin_cli_live_test.sapp` requires Linux, `bwrap`, and
standard local file/process tools. It copies the retained executable and stdlib
into a private temporary package, builds local fixture locks, then hides the
source checkout and runs the installed checker from a foreign cwd. Caller-owned
locked decoy checker/provider modules must never execute. Fourteen subprocess
checks cover both profiles, mixed batches, ordinary diagnostics, unknown and
duplicate options, missing sources, native check dispatch and a manifestless
caller, plus actual execution of the bound-provider example from foreign cwd. Fixtures are removed afterward. This is Linux installed-fixture
qualification; it changes no global installation and makes no macOS/Windows or
all-project claim.
