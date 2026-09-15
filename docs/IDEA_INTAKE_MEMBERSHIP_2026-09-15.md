# Membership continuation: ideas and evidence

This bounded continuation follows the structured-predicate slice at `41c0b86`.
It does not represent a full scan of all conversations, disks, marketplaces or
plugin implementation code. No plugins were installed or activated, and no
remote runtime or Specisoft server was used.

## Intake actually performed

- Re-read two recent turns from the technical conversation “Manual ESP32-DevKitC V4”.
  Transferable requirement: distinguish memory regions and physical capabilities
  from implemented compiler operations; device paths from an old conversation
  must not be treated as stable current identity. A second sampled conversation was unrelated to the product
  work and was excluded from intake.
- Re-inventoried 249 cached manifests (215 Codex, 34 Claude), yielding 161 distinct
  name/version pairs. There were **no new name/version pairs** compared with the
  previous predicate intake. Cache presence does not establish activation state.
  Only names, descriptions and versions were considered; credentials/configuration
  were not read. Selected descriptions: claude-obsidian, cockroachdb, data,
  mcp-server-dev and plugin-dev. These are capability descriptions, not verified
  implementation or security claims.
- Counted 986 callable-tool metadata entries in this session and sampled plugin
  discovery/management descriptions. The user-supplied uninstalled recommendation
  catalog was also available as metadata; no missing plugin was installed to scan it.
- Revisited the Claude marketplace and Continue provider documentation below;
  consulted SQLite and SQLAlchemy primary documentation for membership semantics.
- Inspected the owning predicate module, all its direct tests, benchmark, example
  and canonical contract. Selected embedded-target/device-evidence source sections
  were read for idea triage, not subjected to a complete device audit.

## Concrete conversion and pending ideas

| Input | PBLang application | State / next gate |
| --- | --- | --- |
| SQLAlchemy parameter-list and empty-set operators | Explicit `in` / `not_in` in the pure SQLite predicate helper; one bind per member; empty subquery preserves name resolution | Implemented and exercised against SQLite and self-hosted frontend |
| Data/database plugin descriptions | Reusable multi-value status/category selection for product queries | Library and ORM example implemented; no connector integration or remote DB use |
| Local knowledge/provenance plugin descriptions | Keep idea source, implementation state and test evidence separate | This intake and validation report; automatic trusted provenance remains pending |
| Claude marketplace metadata | Separate catalog availability, version and installed/active state in future PBLang package tooling | Design candidate; not an implemented marketplace or authenticity guarantee |
| Continue provider abstraction | Explicit provider configuration around a stable calling contract | Candidate for pbAI adapters; no provider configured or called |
| Embedded-device conversation and source | Per-target memory regions and current artifact/physical evidence instead of universal support claims | Existing descriptors partially represent this; native/backend and hardware verification remain pending |

The SQLAlchemy pattern was adapted to PBLang's flat contract; no dependency or
third-party implementation code was copied. New membership has construction cost.
There is no claim of general compiler/runtime acceleration or disk-size reduction.

## Primary sources

- [SQLite IN / NOT IN semantics](https://www.sqlite.org/lang_expr.html#the_in_and_not_in_operators):
  empty and NULL result matrix used for the tests.
- [SQLAlchemy operators](https://docs.sqlalchemy.org/en/20/core/operators.html#in-comparisons):
  expanded bound parameters and backend-specific empty-set subquery inspired the API.
- [Claude marketplace documentation](https://code.claude.com/docs/en/plugin-marketplaces):
  catalogs associate plugin names with sources and version metadata; discovery and
  installation are separate steps. The PBLang application above is an inference.
- [Continue provider configuration](https://docs.continue.dev/customize/model-providers/top-level/openai):
  explicit provider configuration is a future adapter-design reference, not a
  verified pbAI integration.

## 1.0.0 remains incomplete

Outstanding: composite/shared ownership and cleanup; native runtime/FFI and
isolation; backend compatibility and allocation correctness; audit/release
report authenticity; PBIDE Run/Designer; cross-product library adoption; real
guest-OS/device validation. Existing SQLite parameter/depth limits still apply.
Caller-side resource policy remains necessary for untrusted request collections.
