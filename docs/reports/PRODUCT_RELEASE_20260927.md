# Private product source release — 2026-09-27

This branch publishes the shared dependencies for PBDataLab, PBStudy and PBIDE,
including table transformations, review cards and pipeline timing. It starts
from upstream main and adds an explicit file list; unrelated local compiler,
AI, operating-system and other product work is not swept into this branch.
Existing upstream source/history remains part of the PBLang repository.

## Installation layout

Clone the private repositories as siblings named `pblang`, `pbdatalab`,
`pbstudy` and `pbide`. Use the matching PBLang commit listed in each product's
`docs/INSTALLATION.md`. The PBIDE additions are on `codex/product-release-20260927`
until the corresponding pull request is merged. The two new product repositories
start on `main`. These are source releases, not binary installers or APKs.

Build or supply a compatible PBLang runtime using the canonical README. The
retained Rust bootstrap is existing infrastructure; no new Rust source is part
of this release. Tests used the installed 0.6.0-line runtime; rebuilding that
runtime and platform packaging are separate checks, not claimed here.

The manifests resolve the common library using `../pblang/stdlib`. Build and
publish portable lock paths with the same runtime after an intentional library
change. Do not disable content-hash verification. Each product's installation
document includes startup and standalone test commands.

## Delivered behavior

- PBDataLab: bounded table inspection, transforms, explicit column selection,
  undo and new-copy CSV/TSV/Sheet output. A standalone file-roundtrip test needs
  only PBDataLab and PBLang; optional cross-product tests remain separate.
- PBStudy: explicit-date reviews, correction of card text preserving progress,
  eight-step undo and new-copy persistence. Draft edits block conflicting
  navigation/save/open operations. Native multiline fields remain basic.
- PBIDE: masked pipeline inspection, timing estimates, critical jobs/slack and
  a separate interactive native timing inspector. Web export includes all four
  required HTML/CSS/JavaScript bundle files.

All products remain alpha. Web/Android generated readers do not imply native
editing parity or device validation. The patch does not claim a general security
audit. The requested specialist pre-publication review was unavailable because
the configured model lacked access; file allowlisting and marker checks are
limited release checks, not a substitute for that review.

## Verification boundaries

Tests are rerun against this isolated publication tree, including declared
imports and portable locks. No private user documents, caches, local logs,
build output, runtime binaries or generated desktop screenshots are intentionally
added. Explicit synthetic fixtures and pre-change benchmark source are included.
The original workspaces and their unrelated dirty files remain untouched by
publication commits. Performance evidence from earlier cycles remains specific
to its exact tested snapshots; this release does not promise identical timings.
