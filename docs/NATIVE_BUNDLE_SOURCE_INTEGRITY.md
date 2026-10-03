# Retained native bundle source-integrity limitation

The retained PBLang 0.6.0 executable can truncate the source entry while reporting
success for `pblang bundle . OUT`. Do not use this invocation on an original
project. Updating PBLang-written compiler libraries does **not** fix this native
command.

## Reproduced scope

An isolated Linux x86-64 fixture on 2026-10-04 used this manifest:

```toml
[package]
name = "bundle-integrity-fixture"
version = "0.1.0"
entry = "main.sapp"
```

`main.sapp` contained `print "bundle-integrity-canary";` and a newline (33 bytes).
The working directory was the fixture source directory. The command was
`pblang bundle . /tmp/FIXTURE/output`, with source and output in distinct sibling
directories. It exited 0, left the original entry at 0 bytes, omitted the entry
from the output, and printed an absolute source entry in its success/run hint.
The output held only the executable, manifest and lock. No dependencies were
needed to reproduce the failure. The shadow validator's unavailable warning did
not stop the native command.

The same fixture with a canonical absolute package-root argument, and with an
absolute package-root argument ending in `/.`, preserved the 33-byte source and
included `main.sapp` in the output. Those are bounded controls, not a general
packaging qualification: overlapping output, symlinks, local dependency locks,
complete module copying and portability need separate verification.

The reproduced executable SHA-256 was
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.
Source SHA-256 before the relative-root call was
`087b7e4346dd1efb2a9558f1286e4be7a6ea1bb4b7183c09eeafe590dc338845`;
afterwards it was the empty-file hash
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
Only disposable fixtures were writable in the test sandbox; real source checkouts
were absent, the executable was read-only, networking was isolated, and temporary
fixtures were removed afterwards.

## Cause and boundary

`find_manifest_dir` preserves a relative package root such as `.`.
`resolve_package_entry` returns a canonical absolute entry path. In
`src/cli/bundle.rs`, stripping the relative root from that absolute entry fails;
`unwrap_or(&entry_path)` keeps the absolute path. Joining an absolute entry to
`out_dir` discards the intended output directory. The subsequent native copy is
therefore a copy of the entry onto itself and truncates it.

The retained command needs a host fix: normalize the package root consistently,
reject a failed relative-path derivation, and reject source/destination identity
before writes, including manifest/lock/runtime copies. No such host change is
included in the PBLang VM argument-count fix. There is no PBLang wrapper on the
direct executable path that could reliably intercept this operation; a warning
from the optional manifest validator is not a fail-closed guard.

## Alternative routes

The separate PBLang-owned [`tools/pbpackage.sapp`](PACKAGE_FOLDER.md) packages
small Linux x64 source projects with local assets and in-project path dependencies.
It reads sources without writing them, refuses existing destinations, verifies
dependency content hashes, and publishes a completion receipt. Its documented
limits apply; it does not change or certify the retained native command.

For the compiler's supported static module subset, `tools/pbc.sapp` with
`--bundle --target pbck --out DISTINCT_FILE.pbck` is a separate PBLang-owned
compiler route. It produces compiled bytecode, not the native command's portable
source folder. The source-hidden compiler portability suite covers this route;
see [compiler CLI portability](COMPILER_CLI_PORTABILITY.md). Use a fresh output
path separate from every input. It is not a replacement for arbitrary source
packages, dynamic imports, runtime assets or every host builtin.

For a source-folder distribution, stage an independent copy of the package and
its required files, compare original hashes, regenerate/verify dependencies in
that copy, and qualify it with original sources unavailable. Do not run the
broken native relative-root command on the original package. This document
records the defect and alternatives; it does not certify general bundle safety
or repair a source file already truncated by an earlier invocation.
