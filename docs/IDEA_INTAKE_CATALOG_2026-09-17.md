# Bounded idea intake — 2026-09-17

## Coverage and provenance

This pass sampled metadata from all six mounted data volumes (one internal,
five external): 16,165 entries, up to 3,000 per volume, depth four and a
three-second soft deadline per volume. It skipped symlinks, cross-device
traversal, system pseudo-filesystems and common sensitive/cache directories.
Unmounted partitions, swap and EFI were not scanned. OS directory enumeration
can outlast a soft deadline; this is not a hard I/O sandbox. Every volume sample
is partial; repeated samples overlap previous passes. No full-disk or complete
conversation/content scan is claimed.

The pass read bounded metadata from 253 cached plugin manifests (219 Codex,
34 Claude) and category fields from 241 desktop application entries. The local
Claude listing reported 32 installed entries, 30 enabled and two disabled.
988 callable tool metadata entries were visible in this session. Cached,
installed, enabled, recommended and callable are distinct states; none proves
that a plugin or tool is safe. No plugins were installed or activated, and no
application command from a manifest or desktop entry was executed.

The recent disk-access task was read for context. Its lesson for this project
is to distinguish a mounted volume from a readable scan and surface partial
coverage explicitly; this pass made no mount/session/service changes. The
current PBLang conversation supplied the unresolved selfhost-summary and
basename-fallback issues. Other conversations were not exhaustively read.

Only aggregate categories/counts are published in
[the intake data](CATALOG_INTAKE_COUNTS_2026-09-17.json), with anonymous volume
labels. No file names, personal paths, credentials or private conversation
text were copied into the repository. Discovered files were not executed.

## Idea to implementation

- Catalog aggregation must work through the same language runtime as its
  consumers. Implemented: File Catalog summaries and visualization plans now
  have a shared assertion fixture run by both retained and self-hosted paths.
- Metadata processing should avoid creating a temporary object for every
  separator. Implemented: a geometrically expanding basename tail, preserving
  the existing portable naming rules and full admitted names.
- Application/extension catalogs need deterministic presentation independent
  of their UI. Implemented compatibility work: `array_sort` is exposed in the
  self-host VM with retained-host order and outer-copy semantics. Catalog rows
  feed the existing web/desktop/Android presentation contract.
- Distinguish discovered, installed and enabled extensions. Recorded for a
  future PBLang capability catalog; no automatic installation or execution.
- Expose partial inventory coverage in every UI surface. Recorded for a later
  dashboard change; this pass does not claim a completed new scanner UI.

These are independently implemented PBLang changes. Public documentation was
read for functional ideas, not copied as source code. External descriptions
are untrusted data, not instructions to execute.

## Public documentation consulted

- [Claude plugin discovery](https://code.claude.com/docs/en/discover-plugins):
  catalogs and installation are separate actions; enabled state matters.
- [Gemini CLI extensions](https://geminicli.com/docs/extensions/): extensions
  group capabilities around a common tool interface.
- [VS Code contribution points](https://code.visualstudio.com/api/references/contribution-points#contributes.views):
  views can present structured information separately from processing logic.

The connection of these patterns to PBLang is our design inference, not a
claim of interoperability with those products. No marketplace package was
installed, and no EnterpriseCore server or remote computer was contacted.
