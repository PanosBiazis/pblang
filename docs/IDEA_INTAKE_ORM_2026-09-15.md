# Incremental idea intake: query tools and reusable device components

This extends [the DB intake](IDEA_INTAKE_DB_2026-09-15.md). It is selected,
read-only discovery, not a full scan of every disk, conversation, AI product
or marketplace. No third-party plugin was installed, activated or executed.
No EnterpriseOrg/Hetzner server or remote workstation was accessed.

## Current sources

- The available conversation catalog and one retrieved turn from
  `Αναγνώριση εξαρτημάτων Arduino`. It describes analog/digital circuits,
  robotics and reuse of components in later projects. The practical adaptation
  is reusable sensor/actuator interfaces with simulation before live effects.
  Historical design discussion does not prove current hardware compatibility.
- Re-read metadata from 249 cached plugin manifests (215 Codex, 34 Claude),
  representing 161 distinct name/version pairs. Examined selected data,
  engineering, operations, plugin-development and MCP-development descriptions.
  Cache presence does not establish active/inactive settings. The supplied
  available-skills and recommended-but-not-installed lists remain discovery
  sources; account configuration and secrets were not read.
- Revisited the 986 callable tool descriptions, selecting database-query
  surfaces for comparison. Their distinction between metadata inspection,
  querying and effectful database access suggests separate PBLang contracts;
  none of those tools was executed against a database.
- Read seven selected PBLang source files for export/size/hash discovery:
  `embedded_controller`, `embedded_circuit`, `embedded_targets`,
  `embedded_protocol`, `device_emulator`, `device_evidence_contract` and
  `provider_registry`. This lexical inventory is not a full correctness review.
  The changed ORM owning module, its callers/tests and relevant native array
  behavior were reviewed separately.

## Official external references and inferred adaptations

| Source | Observed capability | Proposed PBLang adaptation |
| --- | --- | --- |
| [Continue MCP](https://docs.continue.dev/customize/deep-dives/mcp) | Workspace MCP configuration and import of JSON configurations from other hosts | A metadata-only compatibility preview that reports supported fields, transport and missing capabilities before any activation |
| [Cline MCP management](https://docs.cline.bot/mcp/mcp-overview) | Explicit enabled/disabled status, transport selection, restart and request timeouts | PBIDE capability lifecycle state with inspectable startup/stop results; preserve existing approval and process-boundary work |
| [Cline marketplace](https://github.com/cline/mcp-marketplace) | Search by name/category/tags and discoverable installation instructions | Local discovery records with provenance and an implementation queue; marketplace metadata must not become permission to execute README commands |

These are inferred design ideas, not newly implemented PBLang integrations or
certifications of third-party plugins. Existing Claude/Codex/Cursor/JetBrains,
Gemini and MCP Registry references remain in earlier intake documents.

## Conversion and next steps

**Implemented now:** the earlier generated-SQL optimization idea became wide
INSERT/UPSERT construction with exact SQL/parameter regression tests. The
ORDER grammar bug found during owning-module review is fixed before query
execution. The example composes these APIs with the prior automatic connection
cleanup helper. No changes were made to SQLite's native runtime.

**Next query work:** structured predicates and sorting objects for tools that
accept external requests; keep raw SQL an explicit developer boundary. Add
parameter/identifier contracts first, then measure their cost on small and
large queries. Do not infer safety from an interface's read-only label alone.

**Next device work:** reuse existing protocol, controller, target and emulator
APIs for sensor sampling and actuator lifetime/Stop behavior; pair every live
operation with a simulation case and exact device evidence. No pin was driven,
firmware flashed or physical compatibility claimed in this slice.

**Next extension work:** a local catalog with stable IDs, version/duplicate
handling, provenance, configuration previews and deferred tool schemas. No
catalog import is enabled by this document. Remaining 1.0.0 composite/shared
memory, runtime/FFI, backends, authentic audits/releases, PBIDE and guest-OS
work is still open.
