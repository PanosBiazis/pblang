# pbAI curriculum contracts

Both standalone and embedded pbAI expose two explicit data-preparation operations:

- `neural_curriculum_split(vocab_size, context, groups)`: different declared
  task families across training, validation and final test (transfer diagnostic).
- `neural_task_curriculum_split(vocab_size, context, groups)`: the same declared
  families in all three splits, with different inputs (known-task learning).

Neither operation trains, persists, loads a model, selects a candidate or grants
activation. Both delegate to the shared PBLang `stdlib/pbai_curriculum.sapp`.

## Input and limits

Each group is exactly `{family, split, examples}`. `split` is `training`,
`validation` or `test`. A family identifier has 1..48 lowercase ASCII letters,
digits or hyphens. Each example is exactly `{x,y}`: 1..context integer token IDs
and an integer next-token label, all within the declared vocabulary.

Vocabulary is 2..256 and context is 1..16; numeric values must be finite integers.
There are 3..12 groups, 1..4 per split, and 1..32 examples per group with at most
32 examples per split (96 total). Unknown keys, absent fields, missing splits,
nonfinite values and out-of-range values throw before any result is returned.

Transfer mode accepts each family once globally. Known-task mode accepts a family
once per split and requires it in all three splits. A family may therefore have
only one group in each split; join its examples before calling rather than submitting
multiple groups with the same family/split.

Both modes reject a repeated normalized context anywhere, even with a different
label or family name. Signed zero is normalized before identity checks. Reusing
an input is invalid even within one split. Token-context disjointness alone does
not establish semantic or independently sourced dataset separation.

## Output and ownership

Results contain flattened `training` and `validation` arrays, grouped
`test_families`, and a `report`. Examples are detached from input arrays/maps.
Reports include declared family IDs, split/counts and per-group data digests, not
raw contexts or labels. Use nonsensitive identifiers; hashes of small datasets are
not a confidentiality mechanism. No data are sent externally or written to disk.

Transfer reports retain format `pbai.curriculum.split.v1` and their existing
ordered snapshot digest. Known-task reports use `pbai.curriculum.known-task-split.v1`,
set `same_declared_families_in_all_splits=true` and
`declared_family_disjointness_checked=false`, and include a `known-task` domain
marker in their snapshot digest. Both set `exact_context_disjointness_checked=true`.

`semantic_family_identity_verified`, `independent_provenance_verified`,
`training_performed`, `persists_data` and `authorizes_activation` remain false.
A caller could rename related tasks; this function does not discover semantic
families or authenticate data sources. Reports are not approval tokens.

Map width is checked before key enumeration; nested or cyclic values fail the
scalar schema without recursive serialization. These are local, already-materialized
value checks. A remote transport must separately bound the request before decoding.

Snapshot identity normalizes map-field order and signed zero but preserves group
and example order. Returned data remain caller-owned and mutable. Editing them
does not update the report: re-run preparation after edits. This is not a hidden-test
service. Keep final-test labels out of downstream training and selection.

## Example

```pblang
let AI=import("stdlib/pbai.sapp");
let groups=[
 map{family:"sum-mod-four",split:"training",examples:[map{x:[4,0,0],y:0}]},
 map{family:"sum-mod-four",split:"validation",examples:[map{x:[4,1,2],y:3}]},
 map{family:"sum-mod-four",split:"test",examples:[map{x:[4,3,2],y:1}]}
];
let data=AI.neural_task_curriculum_split(8,3,groups);
print json_stringify(data.report);
```

The full runnable example is `examples/pbai_task_curriculum.sapp` in PBLang and
`demo/pbai_task_curriculum.sapp` in standalone pbAI. It prepares data only; its
output is not a trained model or a model-quality score.

## Verification and scope

Run `pblang run tests/semantics/pbai_curriculum_contracts_test.sapp` for the shared
contract and `pblang run tests/semantics/pbai_curriculum_facade_test.sapp` for the
product entry points. Tests cover both modes, complete known-family coverage,
malformed inputs, repeated family/split groups, normalized input overlap, exact
96-example limits, snapshot sensitivity and detached ownership.

The known-task option addresses a methodological gap: learning a demonstrated
operation on new inputs must be measured separately from transfer to unknown
operations. It does not increase model accuracy by itself. Independent datasets,
learned neural representations, representative language evaluation and execution
on each target device remain separate work. No AGI/ASI or universal-platform
claim follows from these contract tests.

The [local verification record](benchmarks/pbai_curriculum_contracts-checks-2026-09-27.json) lists commands, results and review limits.
