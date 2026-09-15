# Incremental idea intake: resource lifetimes and extension discovery

This continues [the earlier intake](IDEA_INTAKE_2026-09-15.md). It is a bounded
read-only discovery record, not complete access to all conversations, disks,
installed AI products or plugin activation states. No third-party plugin,
hook, extension or marketplace command was installed or executed. No remote
workstation or EnterpriseOrg/Hetzner server was accessed or used for testing.

## What was inspected this time

- Recent history from `PBLang production blockers και σαρώσεις`: its completed
  preflight-gate work is a useful precedent for rejecting invalid work before
  acquiring a resource and for publishing measured overhead honestly. This is
  retrieved historical context, not fresh validation of that older patch.
- Three recent turns from `Continue PBLang and PBRemote`: these failed due to
  model capacity. They preserve the user's request for idea conversion and
  runtime/release work, but provide no newly completed implementation evidence.
- Re-read name/description/version metadata from 249 cached plugin manifests:
  215 Codex and 34 Claude, covering 161 distinct name/version pairs. Comparison
  with the prior snapshot found zero added or removed cache/name/version keys.
  Version duplicates are counted explicitly; activation was not audited.
- Revisited 986 callable tool metadata entries and the user-provided available
  skills and recommended-but-not-installed catalog. These are availability
  descriptions, not authorization to execute every effectful tool.
- Read eight selected local source files for an export/size/hash inventory:
  `resources.sapp`, `db.sapp`, `orm.sapp`, `db_owned.sapp`, `file_catalog.sapp`,
  `project_inventory.sapp`, `ui_form_interchange.sapp`, `performance_budget.sapp`.
  This includes overlap with earlier scans and is not eight new discoveries.
  Owning DB modules, callers and tests were reviewed separately for changes.
  No private source bodies, credentials, account configuration or personal
  paths are copied into the published intake.

## New official sources and proposed adaptations

The right-hand column contains our inferred PBLang design ideas, not features
already implemented by PBLang.

| Official source and observed feature | PBLang/product adaptation | Acceptance evidence still needed |
| --- | --- | --- |
| [Cursor MCP](https://cursor.com/docs/mcp): tools/resources/prompts, multiple transports and MCP Apps with ordinary-response fallback | PB-MCP responses usable by both a plain CLI and optional PBIDE views | Same result semantics without UI; negotiated capabilities; explicit lifecycle and Stop tests |
| [JetBrains MCP](https://www.jetbrains.com/help/ai-assistant/mcp.html): project/global configuration, per-tool exposure and router-only tools | A small searchable tool index, loading full schemas only when selected; scope tools to the selected project | Measure context/RAM savings; ensure hidden schemas do not imply hidden authorization checks; preserve deterministic direct calls |
| [Anthropic official plugin directory](https://github.com/anthropics/claude-plugins-official): separate internal/external packages, immutable names and rename mappings | Stable extension IDs, distinct display names and explicit migration aliases in a metadata-only catalog | Duplicate/version/alias-cycle fixtures, revision provenance, existing package/release validation before activation |

Marketplaces offer product ideas, not automatic trust or permission to run
third-party code. Existing catalogs for documents, data, media, project tracking
and scientific tools remain candidates for thin PBLang adapters. Their domain
capabilities must be validated individually, with platform and resource costs.

## Conversion status

1. **Implemented in this slice:** optional SQLite callback cleanup with explicit
   close-call status; fixed non-sensitive errors; faster SQL placeholders.
   This advances the earlier generic-resource-cleanup idea for one concrete
   resource. It does not implement universal cleanup or runtime isolation.
2. **Next library candidates:** files/streams with captured immutable resource
   identity; transaction-aware callback composition without the old nil/error
   ambiguity; generated SQL builders that avoid repeated prefix copying.
   Each requires owning-module review, lifetime/error tests and measurements.
3. **Discovery backlog:** stable local extension metadata, duplicate/version
   handling, deferred schema loading, conversion previews, provenance and
   revocable capabilities. No configuration is imported or activated by this
   record; future security changes require separate review of actual boundaries.
4. **Still open for 1.0.0:** composite/shared ownership, complete native runtime
   and FFI/backend contracts, authentic audits/releases, PBIDE Run/Designer,
   library compatibility and tests inside guest operating systems. Emulators
   exercise selected behavior; they do not certify all hardware/operating systems.
