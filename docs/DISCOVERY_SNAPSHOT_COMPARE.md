# Snapshot manifest comparison

`snapshot_compare.sapp` implements the recorded `source-copy-comparison` proposal as a pure library. A PBIDE interface and actual filesystem snapshots remain future integration work.

## Contract

`validate(manifest)` accepts exactly `{format:"pblang.snapshot-manifest.v1", entries:[...]}`. Each entry has exactly `id`, `digest`, `size`:

- `id`: ASCII logical identifier, 1–32 characters, following the existing `template_fields.valid_name` grammar. It is not a filesystem path.
- `digest`: exactly 64 lowercase hexadecimal characters, representing a caller-supplied SHA-256 claim. This module does not calculate or authenticate it.
- `size`: declared byte count, a numeric integer from 0 through 1,000,000,000,000 inclusive. Booleans, strings, fractions and non-finite values are rejected.
- At most 256 entries per snapshot; duplicate IDs and unknown fields are rejected. Empty snapshots are allowed.

`compare(before, after)` validates both inputs before returning a new report. Rows follow `before` order, followed by newly added IDs in `after` order. At most 512 rows are emitted. Lookup uses maps; validation inspects every bounded digest character.

| Status | Declared metadata |
| --- | --- |
| unchanged | Same ID, digest and size |
| modified | Same ID, different digest, regardless of size |
| inconsistent | Same ID and digest but different size |
| removed | ID occurs only in before |
| added | ID occurs only in after |

Rows contain ID, status, before/after size (`nil` for absent sides). The report includes counts and explicit evidence flags. It omits digests and source content, but identifiers and sizes may themselves be sensitive: choose synthetic or opaque IDs before sharing a report.

No filesystem, network, hashing, execution, merge, rename inference or deletion capability is used. Inputs remain unchanged. `unchanged` means agreement of declarations, not verified file equality or backup integrity. A wrong but identical digest can still produce `unchanged`; `contents_verified` therefore always remains false. JSON decoding is the caller's responsibility; validate decoded data before use.

## Run

```sh
pblang run tests/semantics/snapshot_compare_test.sapp
pblang run examples/discovery_snapshot_compare.sapp
```

The example has four demonstrated recipes: metadata agreement/change, addition/removal, inconsistent size claims, and deterministic non-mutating review. All inputs are synthetic. Tests cover malformed structures, types and limits, duplicate IDs, output privacy and detachment, empty snapshots and a 512-row union. These checks do not certify physical files, SHA-256 authenticity, PBIDE integration or platforms not executed.
