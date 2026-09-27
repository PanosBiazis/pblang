# Ordinal discovery progress

`stdlib:project_discovery_progress.sapp` exports pure `describe(entries)`.
PBIDE exposes identical lazy delegation through
`Product.project_discovery_progress(entries)`. Neither API scans a filesystem,
queries a device/index, parses manifests, runs source or enables admission.
This is a stateless aggregation of caller-supplied observations, not a scan
scheduler, persistent checkpoint, automatic retry or resume mechanism.

## Input and limits

Accept an array of 0–32 entries, each **exactly** `{names,observation}`.
Names and observation reuse the [marker scan contract](PROJECT_MARKER_HINTS.md#explicit-scan-observations):
up to 256 basenames per entry, each 1–255 Unicode scalars and at most 255 UTF-8
bytes, rejecting paths and the existing display-control list. Observation is
exactly `{source,state,depth}` with `direct`/`index`, six existing states and
integer depth 0–8 (index requires 0). Unavailable/excluded/error require empty
names; completed/truncated/timeout may contain valid partial hints.

The entire batch additionally permits at most **1024 supplied names** and
**32768 supplied UTF-8 name bytes**, including duplicates. Shape, counts and
character/byte budgets are preflighted before classification. Every observation
is recomputed through `Hints.describe_scan`; precomputed scan reports are not
accepted as observations. All scans must validate before aggregation starts.
Any invalid entry yields the complete neutral failure, never a partial result.
These bound this pure algorithm, not caller allocations, VM-wide heap/time,
concurrency, import-root integrity or hostile-code isolation.

## Exact v1 result

`pblang.project-discovery-progress.v1` has exactly 29 fields:

- `format`, `ok`, `error`, `observation_status`;
- `root_count`, `roots`, `state_counts`, `source_counts`;
- `input_count`, `unique_name_count_sum`, `recognized_marker_count`, `family_counts`;
- `metadata_only`, `contains_names`, `contains_paths`, `contains_digests`;
- `content_read`, `execution_performed`, `coverage_verified`, `live_existence_verified`;
- `index_freshness_verified`, `absence_verified`, `root_identity_verified`;
- `idea_admission_eligible`, `implementation_allowed`, `training_allowed`;
- `external_effects_allowed`, `safe_to_log`, `production_ready`.

`roots` contains exactly `{slot,scan}` per input entry. Slot is its zero-based
array position for this invocation only, **not a stable ID**. Reordering input
reassigns slots. Root paths, names, device IDs and digests are neither accepted
as identity fields nor returned. `scan` retains the existing exact 26-field
scan envelope and its nested marker report. No maps are shared with input or
future calls; aggregate counts do not alias per-root count maps.

`state_counts` always has completed/truncated/timeout/unavailable/excluded/error;
`source_counts` always has direct/index. Each map sums to `root_count`, which is
the number of supplied slots, **not independently verified distinct roots**.
The three name totals sum canonical per-root counts. Deduplication is only
within each root: the same name in two slots counts twice. The fixed 20-family
map sums to `recognized_marker_count <= unique_name_count_sum <= input_count`.
These are marker observations, not distinct project or language counts.

Success has `ok:true`, `error:""`, `observation_status:"caller-claimed"`.
This means valid input, not successful scanning or complete coverage. Empty
input is valid. Even all-completed empty slots do not prove absence or identity.
There is intentionally no overall complete flag. Failure has `ok:false`,
`error:"invalid-input"`, status `none`, empty roots and **every count zero**;
no rejected names, sources, states, paths or partial hints are echoed.

`metadata_only` is true. All remaining presence, verification, effect, admission,
permission, logging and readiness booleans are false on success and failure.
State, count and ordering information can still reveal project composition:
`safe_to_log:false` is deliberate, not a claim of anonymization. No automatic
GUI/Web/Android/MCP action, new polyglot adapter, conversion permission, training
input or repository-creation authority is added. Existing catalog schemas and
their admission gate remain unchanged.

## Example and verification

```sapp
let Progress=import("stdlib:project_discovery_progress.sapp");
let result=Progress.describe([
    map{names:["pblang.toml"],observation:map{source:"direct",state:"completed",depth:1}},
    map{names:[],observation:map{source:"direct",state:"unavailable",depth:1}}
]);
assert(result.ok and result.state_counts.completed==1 and result.state_counts.unavailable==1);
assert(not result.coverage_verified);
```

The synthetic example is `examples/project_discovery_progress.sapp`. Focused
PBLang tests cover the exact schema, source/state matrix, ordinal order, count
invariants, neutral late failure, privacy, detached results and exact/over-limit
root/name/UTF-8 budgets. PBIDE's matching facade test checks exact delegation.
No live scan or physical-platform claim follows from synthetic test success.
