# Column-reuse continuation: ideas and boundaries

This slice follows `99ef7a4`. It focuses on measured filter-construction cost,
while retaining the wider 1.0.0 backlog. No EnterpriseOrg server, remote workstation,
cloud runtime, plugin installation or activation was used.

## Evidence actually sampled

The conversation catalog was refreshed (14 pinned entries and 30 recent
entries). One returned turn from “Διπλότυπα στα Scheduled” contained multiple
historical automation responses; selected technical excerpts were read. This
is not a complete reading of that conversation or all historical chats. News,
version, vulnerability and model-availability claims in those responses were
not used as current facts. Reusable engineering themes were tracking repeated
work, explicit optimization objectives and distinguishing capability from access.

Local cache metadata was refreshed: 249 manifests (215 Codex, 34 Claude), 161
unique name/version pairs, no new name/version pairs relative to the membership
intake. Selected descriptions included productivity, data-analytics,
mcp-server-dev and plugin-dev. This describes advertised capabilities, not a
code audit or proof of activation. A separate read-only `claude plugin list
--json` inventory reported 32 installed entries: 30 configured enabled and two
configured disabled. Only aggregate counts and metadata were retained; install
paths/account settings were not published. This is configuration evidence,
not a test that every plugin loads or works. The session exposed 986 callable-tool metadata
entries; selected automation and task-progress descriptions were reviewed. The
user-provided uninstalled recommendation catalog was available as metadata.
No external account data, credentials or private plugin configuration was copied.

The owning predicate module, its three tests, example, benchmark and canonical
contract were read in full. This was a focused source review, not a new full-disk
scan. Existing runtime and product blockers are not reclassified as fixed.

## Conversion and next actions

| Source idea | Adaptation | Evidence / status |
| --- | --- | --- |
| SQLAlchemy avoids repeated SQL compilation only when the cache key represents the relevant state | Cache only the immediately preceding validated column spelling in one PBLang call | Implemented; values/operators still validated per term, no result or global cache |
| Repeated-task/resource discussions in the sampled conversation | Benchmark repeated, alternating, unique and membership inputs rather than only favorable repetition | Benchmark modes and paired output hashes implemented |
| Claude installed/disabled plugin states and Gemini extension status | Future product catalogs should distinguish available, installed, active and currently usable capabilities | Intake/design candidate; cache inventory alone remains insufficient |
| Productivity/plugin-development descriptions and task-progress tools | Future local workflow deduplication keyed by explicit operation identity with recoverable progress | Candidate only; no recurring task or background agent created |

PBLang-specific review found that map equality is overloadable. A cache lookup
must reject nonstring column names **before** equality to preserve the pure
helper contract. Callback-counter regression tests cover this in the retained
and self-hosted routes. This was caught during design review before publishing.

## Primary sources consulted

- [SQLAlchemy performance FAQ](https://docs.sqlalchemy.org/en/20/faq/performance.html):
  compilation caching depends on a key representing relevant state; its profiling
  discussion separates construction from database execution. The one-entry
  PBLang cache is an independent adaptation, not a port or dependency.
- [Claude plugin discovery and management](https://code.claude.com/docs/en/discover-plugins):
  installed plugins can be enabled or disabled; catalog presence and running
  capability are distinct. No plugin was enabled by this work.
- [Gemini CLI extensions](https://geminicli.com/docs/extensions/): extensions bundle
  capabilities and have separate discovery, installation and status inspection.
  No Gemini extension or model was executed.

## Remaining work

Composite/shared memory and automatic cleanup; native runtime/FFI and isolation;
backend allocation and compatibility; audit/release authenticity; PBIDE
Run/Designer; cross-product adoption; real guest-OS and device validation.
The column cache is not a memory-safety or production-readiness guarantee.
Performance conclusions apply only to the measured workloads and local runtime.
