# Package Version Inspector

This PBLang-only example migrates an earlier Python module-version prompt into
a bounded package metadata inspector. It does not import a requested module,
execute package initialization code or run arbitrary `--version` commands.

```text
pblang run examples/package_version_inspector/main.sapp
pblang run examples/package_version_inspector/benchmark.sapp
```

`stdlib:package_inspector.sapp` validates PBLang manifest text, summarizes
dependency kinds, normalizes inert version catalogs, resolves stable or
explicitly requested prerelease versions and evaluates exact, caret, tilde and
comparison requirements. Results distinguish missing, current, outdated,
incompatible, invalid and ahead installations.

The inspector uses the additive `semver.parse_strict` API. Legacy permissive
SemVer parsing remains available for compatibility, while package catalogs
reject leading `v`, leading-zero core numbers, empty identifiers, malformed
metadata and values outside PBLang's exact numeric comparison range.

Catalogs are capped at 1,024 records, bulk queries at 128 and manifest text at
262,144 characters. Inputs are copied, exact duplicate releases collapse, and
dependency names remain hidden unless explicitly requested. One shared UI tree
compiles to responsive Web, native desktop and Android records.

This example inspects supplied metadata only. It does not enumerate Python,
operating-system or registry packages, access arbitrary paths, contact a live
registry, verify artifact signatures or prove that installed code matches a
declared version. Those operations require separate permission-bound adapters.

The original source is represented only by SHA-256
`bd9a8a41cdd89325386449d4a7dc1e5b57edddc11c74ecf6b09c7fa6aa27c240`.
