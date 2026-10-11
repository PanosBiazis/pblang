# Retained runtime imports from another working directory

The retained runtime verifies a local dependency's locked path against the
process working directory. A portable lock containing `path = "stdlib"` can
therefore fail with `dependency path not found: stdlib` when an absolute installed
example is launched from another directory. A same-named caller directory can
also cause a hash mismatch. This is a retained package-manager limitation; its
Rust implementation is unchanged.

## Explicit installation step

Keep release/source lockfiles portable. After placing a reviewed installation
at its final location, materialize **that installation's local lock**:

```sh
cd "$installation"
./bin/pblang run tools/pblock_install.sapp --allow-all -- .
# The absolute example can now be launched from an unrelated working directory:
cd "$caller_directory"
"$installation/bin/pblang" run "$installation/examples/your_example.sapp" --allow-import
```

Replace the example path with an actual installed program. The maintenance
command needs the existing secure-file/FFI authority on trusted source; it does
not weaken import permissions for subsequent execution. Its success message
reports only a dependency count, not the local installation path. Normal runtime
errors may still include paths, as before.

The PBLang API is `portable_lock.install(project_dir)`. It securely reads the
manifest and lock, checks that local dependency paths agree, requires accessible
no-follow dependency directories, and atomically replaces the lock. It changes
only local `path` values to absolute paths rooted at the specified project.
Digests and all other metadata remain unchanged. Content hashes and the manifest
hash are still verified by the runtime on import; materialization itself does
not rehash or certify the dependency contents.

`portable_lock.installed_text(manifest, lock, absolute_project_root)` offers the
same bounded text transformation without filesystem writes. Its result format
is `pblang.installed-lock.v1`. It requires a valid manifest digest, current content
hash algorithm, valid dependency digests, and declared local dependencies only.
Registry/git lock sources are refused. The existing portable publisher retains
its own behavior for those dependencies.

## Portability and limits

- The installed lock is host-specific metadata. Do not commit it or include it
  in a portable release. No personal absolute path is added to this repository.
- Before moving an installation, restore its portable lock with
  `tools/pblock.sapp`, move it, then run `pblock_install.sapp` at the new location.
  Explicit materialization refuses a lock still bound to another root; it does
  not silently rebind it. Merely moving/copying the directory does not invoke
  that validation: the retained runtime may still use the old absolute dependency
  while it exists, or fail when it is absent. Publish before moving, then
  materialize at the destination before launching there.
- Run the maintenance tool from a working project/installation root so that its
  own imports work before it repairs the target. It cannot bootstrap its own
  missing dependencies from an arbitrary directory.
- Nested packages with their own locks need their own reviewed installation
  step. This does not repair every implicit relative filesystem path in an app,
  change caller `cwd()`, or qualify all compiler behavior.
- File publication currently uses the existing Linux secure-file/directory
  backend. Other platforms are not qualified by the Linux integration test.
- Inputs retain the existing bounded manifest/lock parser. Rewritten output is
  capped at 1 MiB UTF-8. Unsupported formatting fails before publication. This
  is a trusted installation tool, not an isolation mechanism for an adversary
  concurrently modifying an installation.

The common lock rewriter also resets dependency-section tracking at every TOML
section, preventing an unrelated later `path` field from being treated as a
local dependency. Duplicate recognized lock paths are refused.

## Regression coverage

`installed_lock_test.sapp` covers absolute-root admission, hash preservation,
portable round trips, idempotence, quoted roots, wrong roots/digests, remote
sources, undeclared dependencies, and unrelated TOML sections.

`installed_lock_live_test.sapp` creates an isolated minimal installed project,
reproduces foreign-cwd failure, invokes the actual maintenance command, executes
from a directory containing a decoy stdlib, checks tamper and sandbox refusal,
round-trips through portable relocation, and checks symlink refusal without
changing the old lock. Fixtures are removed on both success and failure.

Linux verification on 2026-10-03 passed both new suites and the existing
`portable_lock_test`, `portable_lock_current_test`, and
`portable_lock_quoted_test`. Four changed PBLang files passed diagnostics with
zero errors/warnings; build, portable-lock publication and manifest validation
passed. A temporary full copy of retained release `0.6.0-f0c0e37b19b5` reproduced
`dependency path not found: stdlib` for the absolute secure-archive example,
then ran that example and the calibration-sheet example successfully from an
unrelated directory after materialization. Manifest/dependency hashes stayed
unchanged and the active installation was not modified. This is local Linux
evidence, not activation or cross-platform qualification.

## Literal CLI directory arguments

Both maintenance scripts read actual arguments after `--`, without trimming or
splitting directory names. `PBLANG_SCRIPT_ARGS` does not select their target.
`pblock.sapp` accepts zero arguments (the current directory) or one nonempty
argument. `pblock_install.sapp` requires exactly one nonempty argument. Extra
arguments and an explicitly empty argument are refused before the lock writer
runs. No argument is different from an explicitly empty one.

Spaces in a supplied directory are significant, including leading/trailing
spaces. LF, CR and NUL are explicitly unsupported and refused; a newline never
selects its prefix as a separate directory. This does not relax the existing
no-follow, bounded parsing, digest-shape or atomic-publication constraints.
The installation command continues not to print the installation root in its
success message; native error rendering may still include script paths.

`tests/tooling/lock_cli_exact_argv.py --binary RETAINED_LAUNCHER --report NEW.json`
checks actual scripts in disposable Linux projects and preserves failure reports.
It compares complete fixture file contents before/after each invocation, so
successful operations must alter only the selected lock and rejected operations
must not alter any fixture. `--tools-dir` can select baseline scripts for a
negative control while keeping the same runtime and library. Synthetic fixture
digests validate metadata preservation only, not dependency-content integrity;
the existing live import/tamper tests cover that separate contract.
