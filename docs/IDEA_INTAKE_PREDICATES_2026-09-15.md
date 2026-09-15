# Incremental intake: reusable structured query construction

This continues [the ORM intake](IDEA_INTAKE_ORM_2026-09-15.md), with one concrete
library implementation rather than new product installations. No remote
workstation or EnterpriseOrg/Hetzner server was accessed. No third-party plugin,
hook, configuration, marketplace installation command or database connector
was executed.

## Bounded discovery

- One retrieved turn of `PBComms επιλογές ονομάτων` clarifies interest in
  distinct projects beyond PBMedia. Its search/file/sync proposals motivate
  shared filtering primitives across products; the suggestions are not
  evidence that those proposed products exist or are complete.
- Re-read 249 cached Codex/Claude plugin manifests (215/34), using only
  name/description/version metadata. Cache presence does not identify current
  activation state. The supplied active tool/skill catalog and recommended
  not-installed plugin list were considered as availability descriptions.
- Inspected selected query/filter tool descriptions from the current 986-tool
  metadata catalog. They suggest a separation between structured requests,
  validation, generated queries and effects. None was called against live data.
- Refreshed export/size/hash inventory for six PBLang modules: provider_registry,
  file_catalog, project_inventory, orm, db and db_owned. This overlaps earlier
  scans and is not a complete disk or content audit. The new module and its
  owning integration/tests were reviewed separately.

## Official references and adaptations

| Source | Observed capability | PBLang adaptation/status |
| --- | --- | --- |
| [SQLAlchemy](https://www.sqlalchemy.org/features.html) | Independently usable SQL expression layer, bind parameters and an optional ORM | Implemented a small optional pure predicate compiler feeding the existing ORM; this is an original restricted PBLang API, not a port of SQLAlchemy |
| [Continue MCP](https://docs.continue.dev/customize/deep-dives/mcp) | Workspace tool configuration and compatible JSON configuration imports | Proposed inspectable metadata adapters with a preview before activation; no configuration was imported |
| [Cline marketplace](https://github.com/cline/mcp-marketplace) | Searchable extension metadata and setup instructions, examined in the prior intake | Retain separate discovery and execution records; do not execute catalog instructions during scans |

These are inferred adaptations, not claims of upstream guarantees. The earlier
Claude/Codex, Cursor, JetBrains, Gemini and MCP Registry research remains in
linked intake records; no full marketplace or all-conversation coverage is
claimed and no external implementation source was copied.

## Conversion status and remaining work

**Implemented:** `sql_predicate.compile_all` compiles a nonempty flat list of
comparisons to quoted-column SQL plus parameters, including correct NULL
semantics. Tests and an ORM/automatic-cleanup example accompany it. This closes
the flat-AND portion of the structured-query idea, not every raw SQL boundary.

**Next:** explicit nested boolean expressions, IN lists, structured sort/schema
contracts, dialect adapters and request-specific resource profiles. Design
empty-list/NULL semantics before expansion; retain raw developer APIs and
measure the optional construction cost rather than claiming zero overhead.

**Other product ideas remain proposals:** local search/sync/file catalog
consumers, inspectable extension configuration previews, simulation-first
sensor/actuator components, and reusable transformations. Current code provides
building blocks; integration, review and device evidence are still necessary.

**1.0.0 still open:** composite/shared memory, runtime/FFI enforcement, broader
compiler/backend compatibility, audit/release authenticity, PBIDE Run/Designer
and guest operating systems. Earlier small-row ORM timing overhead is not
claimed fixed by this separate optional feature.
