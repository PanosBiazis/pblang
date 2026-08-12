# PBLang-only migration program

The long-term objective is to make owned application and library code across
AAP, additional workspaces, and future products implementable in PBLang. This
is a migration program, not a bulk file conversion or permission to rewrite
operating-system, vendor, generated, or third-party code.

## Safety contract

1. Inventory disks read-only and record filesystem identity, project root,
   ownership, license, language, build entrypoints, tests, generated paths, and
   secrets that must never enter reports.
2. Classify each project as owned source, third-party dependency, generated
   artifact, system component, personal data, or unknown. Only explicitly owned
   source is a migration candidate by default.
3. Select one bounded product or library slice. Document its observable API,
   file formats, protocols, performance baseline, security boundaries, and
   platform support.
4. Implement parity in PBLang. Foreign/system APIs remain capability-gated
   bootstrap adapters only where the operating system requires them.
5. Run equivalent tests, differential fixtures, security checks, benchmarks,
   and platform verification. A translated file is not evidence of parity.
6. Keep the previous implementation until the PBLang version passes the parity
   gate. Removal requires an explicit reviewed migration record and rollback
   artifact.
7. Never modify mounted recovery media, OS installations, third-party package
   caches, or unrelated user data as part of an inventory or migration run.

## Canonical source policy

Every owned component has one canonical source tree. Product bundles, stdlib
copies, release packages, generated JVM/DEX/C99/native code, and compatibility
fixtures are generated outputs with hashes and provenance. Two independently
editable copies of the same product are forbidden because fixes can diverge.

New PBLang code must use bounded inputs, structured errors, capability checks,
deterministic output where possible, and tests scaled to risk. New Rust code is
not added. Existing Rust is retained as a bootstrap/reference implementation
until tested PBLang parity exists.

## Product incubation

Ideas such as `pboffice` begin with a product brief and a shared-library gap
analysis. A new project is justified only when it owns a distinct user workflow
and does not duplicate an existing library or product. `pboffice`, for example,
would first define document/spreadsheet/presentation formats, collaboration,
printing/export, accessibility, sandboxing, recovery, and cross-platform GUI
contracts. Reusable document, layout, formula, archive, font, and rendering
work belongs in coherent PBLang libraries rather than private product copies.

## Completion evidence

A migration slice is complete only when the PBLang implementation builds,
passes functional and negative tests, has no known regression, documents every
remaining platform exception, and meets measured performance/resource budgets.
Portfolio progress is reported from immutable per-project evidence; empty,
truncated, or timed-out scans fail closed.
