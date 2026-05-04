# Stability policy (v0.x)

For v0.x we allow **breaking changes** when needed, but we track them and aim to keep the core predictable.

## What we consider breaking

- **Syntax**: Changing grammar or keyword meaning so that existing valid source stops parsing or changes behavior.
- **Stdlib builtins**: Removing or renaming a builtin, or changing its arity/semantics in a way that breaks existing scripts.
- **Module system**: Changing how `import` resolves paths, how exports work, or the semantics of the module cache.
- **Bytecode / VM**: Changing instruction set or execution semantics so that previously compiled chunks behave differently.
- **Error semantics**: Changing when runtime errors are produced (e.g. truthiness, indexing, arity) in a way that breaks existing assumptions.

## What we try to preserve

- **Language semantics** as documented in `Language_Semantics.md` (truthiness, indexing, error model, arity).
- **VmHost and embedding API**: `err`, `import_module`, `gc_collect_cycles`, and the capability flags (`allow_fs`, etc.) are part of the embedding contract; we avoid breaking them without a clear deprecation path.
- **Lockfile format**: The structure of `pblang.lock` (manifest_sha256, dependencies with path/sha256) is kept stable so that tools and CI can rely on it.

## Versioning and changelog

- **Versions**: We use semantic versioning (e.g. 0.2.0). For v0.x, the minor version may bump for breaking changes; patch for fixes and non-breaking additions.
- **Changelog**: Notable breaking changes and additions should be listed (e.g. in `CHANGELOG.md` or release notes) so that embedders and script authors can adapt.

## Sandbox and limits

- **StdlibConfig** and **VmLimits** are part of the embedding API. New options (e.g. new capability flags or limit types) will be additive where possible; changing the meaning of existing flags is breaking.
