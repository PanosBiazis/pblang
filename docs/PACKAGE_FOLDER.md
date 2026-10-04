# PBLang-owned source-folder packaging

`tools/pbpackage.sapp` builds a new source-folder package through
`stdlib/package_folder.sapp`. It never calls the destructive retained native
`pblang bundle` command. The shared `pblang` launcher and retained binary are not
replaced by a dispatcher. Use this explicit route for the supported profile.

```sh
mkdir -m 700 ../package-output
target/release/pblang run tools/pbpackage.sapp --allow-all -- \
  examples/package_folder_demo ../package-output/demo \
  --runtime "$PWD/target/release/pblang"
../package-output/demo/pblang run ../package-output/demo
```

For an installed release, use that release's absolute `tools/pbpackage.sapp` and
`bin/pblang` paths. Source and destination may be relative or absolute. The
specified source must contain its own manifest; ancestor-manifest search is not
performed. The parent of the output must already exist, belong to the current
user and have mode 0700. Source and output parent must be on the same kernel mount; cross-mount and bind-mount aliases are refused in this profile. The final output directory must be absent. The tool
never changes permissions of an existing caller directory.

The library entry point is `bundle(source_root, new_output, runtime_file)`. The
CLI prints its JSON result and exits unsuccessfully when `ok` is false. No
package source is executed or typechecked during packaging. The caller chooses
and trusts the runtime; an x86-64 ELF header check is not runtime authentication.

## Supported profile and limits

This initial profile is Linux x86-64 with the existing directory-handle,
sealed-memory, no-replace publication and required FFI/exec operations available.
There is no fallback to unchecked copies when those operations are denied.
It includes visible regular files, binary assets and empty directories; hidden
entries and `target` directories are omitted. Symlinks and special files fail
admission. The top-level names `pblang`, `package-receipt.json` and `root` are
reserved. File data is copied, but original executable bits are not propagated:
source/assets are private 0600 files and the explicit runtime is 0700.

Limits are 512 source entries, depth 24, 1 MiB per source file, 8 MiB total source
bytes, 64 local dependencies and a separate runtime file up to 64 MiB. This is a
bounded small-project surface, not a general replacement for every existing
source distribution. The limits cannot be silently disabled by CLI flags.

The root manifest requires package name, canonical version and a declared
relative `.sapp` entry. Dependencies use single-line `{path="in-package-directory"}` entries under
`[dependencies]`, with bare names and no trailing inline comment. The strict
serialization subset rejects unsupported TOML values and duplicate definitions.
External, registry and Git dependencies are rejected, as are nested `pblang.toml`/`pblang.lock` files. This initial profile has one root manifest. Dependencies require a
current lock. The manifest digest and dependency tree digests are verified over
the captured bytes using the actual `pblang-package-tree-v2` contract: entry
kind, big-endian lengths, UTF-8 relative path and content, with empty directory
entries included. Unsupported/excluded dependency contents produce a digest
mismatch. Hashing operates directly on the captured packaging entries. The
compiler snapshot follows the same v2 contract; see [locked snapshots](LOCKED_SOURCE_SNAPSHOT.md).

The root lock is normalized only in the output. Absolute local paths must match
the original manifest root before being rewritten to relative paths. A project
without dependencies may omit its lock; a minimal lock is created in the output,
without adding anything to the source. Assets/modules are copied without edits.

## Source and destination integrity

Source traversal uses held directory roots and no-follow reads. Captured file
bytes are retained privately and sealed before publication. Lexical overlap in
both directions is rejected, including an output equal to or nested beneath the
source. Device/inode strings are checked for source directories and output
handles so identity comparisons do not lose 64-bit precision.

Output initialization is exclusive. Each nonempty file is published from a
complete sealed snapshot through an anonymous inode, without replacing an
existing destination; empty files use the existing exclusive atomic writer.
The selected runtime is copied from a sealed snapshot in bounded native chunks.
The completion receipt is written last. The source manifest, lock, modules and
assets are never opened for writing, and the native bundle command is never
invoked. Existing outputs and competing files are not deleted or replaced.

This is not a filesystem-wide snapshot or an atomic whole-directory transaction.
Concurrent changes to original sources are not rolled back. Cooperating callers
must keep the source and output-parent namespace stable during the operation;
privileged mount changes and hostile same-user namespace replacement are not a
claimed sandbox boundary. A partial destination is deliberately retained on
failure. Inspect it before retrying with a fresh destination. No automatic
recursive cleanup runs on a caller pathname.

`ok` includes successful publication and confirmed closure of owned handles.
`complete` records payload completion separately. Results expose
`output_created`, `output_may_exist`, `partial_output_possible`,
`publication_attempted`, `files_written` and `owned_handles_closed`. The primary
operation error is retained if cleanup also fails, with a separate
`cleanup_error`. An ambiguous result must not be interpreted as proof that the
destination is absent. The receipt describes published content, not a guarantee
that all later handle cleanup or whole-bundle durability was confirmed.

## Validation

`tests/tooling/package_folder.py` creates only disposable projects, compares
complete source inventories/hashes, exercises the public CLI and executes the
finished artifacts with original sources and compiler checkouts unavailable.
It covers relative, absolute and normalized nested roots, portable local
imports, binary/empty assets, overlap, existing outputs, symlinks, special files,
malformed metadata, duplicate dependencies, stale/mismatched locks, denied FFI/exec,
bind-mount aliases, runtime admission and a competing
runtime publication. Exact execution results belong to the delivery report.

The retained native relative-root defect remains documented in
[NATIVE_BUNDLE_SOURCE_INTEGRITY.md](NATIVE_BUNDLE_SOURCE_INTEGRITY.md). This route
is a separate PBLang implementation, not a repaired native binary.
