# Package Version Inspector

This PBLang-only example migrates an earlier Python module-version prompt into
a bounded package metadata inspector. It does not import a requested module,
execute package initialization code or run arbitrary `--version` commands.

```text
pblang run tools/pblock_install.sapp --allow-all -- examples/package_version_inspector
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

## Compare releases (synthetic example)

The existing screen now compares `graphics-kit` 1.3.0 (stable) and 2.0.0-rc.1
(prerelease), with declared current version 1.0.0 and requirement `>=1.0.0`.
**Stable only** selects 1.3.0; **Include prerelease** selects 2.0.0-rc.1. Both
cards remain visible so their channels, compatibility and evidence can be
compared. Choosing a channel changes the candidate summary; it performs no
installation, download, package import, signature check or filesystem scan.
The candidate is the newest compatible metadata entry, not a recommendation.

`comparison.sapp` reuses `package_inspector.evaluate`, `release_channel.resolve`
and `product_release_evidence.evaluate`. It does not introduce another package
resolver, release channel parser or evidence validator. `model(mode, requirement)`
accepts only `stable` or `include-prerelease`; malformed requirements fail, and
an empty compatible set explicitly displays **No compatible release**.

The stable fixture has structurally complete synthetic declarations; the RC
fixture lacks several declarations. Both display **Verified artifact / installed
code: none; insufficient evidence**. Synthetic hash strings, test counts and
physical-device claims are not observations, valid signatures, artifact bytes
or proof of a real installation. Even the complete fixture remains
`trust_verified:false`, `production_ready:false`, `installation_authorized:false`.
The historical source digest above describes the original example inspiration,
not the identity of these new synthetic release fixtures.

Run the focused test from the repository root:

```text
pblang run examples/package_version_inspector/test.sapp
```

The test exercises stable/prerelease switching, restrictive and unsatisfied
requirements, malformed input, evidence boundaries, headless native callbacks
and generated Web/native/Android records. Generated Android records are not
a device execution test. A Web preview can be exported through the existing
`modern_ui.export_web` interface using `App.view(App.model())`.

The retained runtime resolves a nested dependency lock relative to the process
working directory. The first command above materializes only this example's
local lock using the existing PBLang helper. Before publishing or moving the
checkout, restore the portable lock with
`pblang run tools/pblock.sapp --allow-all -- examples/package_version_inspector`.
Do not commit a host-bound lock. See `docs/INSTALLED_DEPENDENCY_LOCKS.md` for this
existing runtime limitation. The Discovery kit runs from its shared root
dependency snapshot and does not include the nested manifest/lock.

## Review declaration gaps

Each release card has a **Review evidence** button. The shared details panel
names the reviewed version and shows every exact blocker already returned by
`product_release_evidence.evaluate`; it does not run a new verifier. For the
stable synthetic fixture, zero blockers is explicitly explained as complete
declarations without proof that an artifact, signature or installation was
checked. The RC fixture displays its eight missing declaration checks.

Reviewing another release replaces the whole title and detail text. Changing
the channel keeps the explicitly reviewed version unchanged. **Clear details**
resets the review panel without changing the channel or compatible candidate.
The persistent synthetic-data boundary remains visible in all states. The
example uses the existing portable `action_sequence` and `action_set_text`
actions, with ordinary escaped text; no installation or network action is added.

Keyboard order after the three package Inspect buttons is: Stable only, Include
prerelease, Review stable evidence, Review RC evidence, Clear details. Generated
Web controls use native buttons, so Tab and Enter exercise the same actions.
