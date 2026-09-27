# Declarative recipe manifests

`stdlib/recipe_manifest.sapp` describes bounded examples independently of a
runner. It uses the existing `template_fields.valid_name` identifier contract.
It performs no file access, process launch, code evaluation, device discovery,
installation, capability grant, or network request.

The manifest is an exact object with `format: "pblang.recipe-manifest.v1"` and
`recipes`, an array of at most 32 records. Each recipe has exactly five fields:

- `id`: unique ASCII identifier, at most 32 characters.
- `platforms`: 1–6 unique values from linux, windows, macos, android, ios, web.
- `requires`: up to 16 unique capability identifiers.
- `inputs` and `expected`: flat maps with up to 16 identifier keys each.

Scalar values may be null, boolean, a finite number between -1e12 and 1e12,
or a string of at most 256 characters AND 256 UTF-8 bytes, without NUL. Arrays,
nested objects, functions, missing fields, extra fields and duplicate recipe
IDs are rejected. An empty collection is valid and makes no success claim.

`validate(manifest)` checks the in-memory object. `plan(manifest, platform,
available)` accepts at most 64 unique available capability IDs, and returns
detached inputs and expected outputs. A recipe is `ready` only when its declared
platform and capabilities match; otherwise it is `unsupported`, with missing
capabilities and platform support reported independently. Availability is a
caller-supplied claim, not evidence of permission, installation or device access.

`assess(manifest, platform, available, observations)` accepts at most 32 exact
`{id, actual}` records, where actual has the same scalar-map limits. Unknown,
duplicate, malformed or unsupported observations throw before any assessment.
The resulting states are:

| State | Meaning |
| --- | --- |
| matched | Supplied values equal expected values, including types and field sets. |
| mismatch | Supplied values differ; for example number 7 is not string "007". |
| not_run | Recipe was ready but no observation was supplied. |
| unsupported | Declared platform or capability requirements were not met. |

Assessment preserves manifest order and omits raw inputs, expectations and
observed values. It retains recipe and capability IDs. It explicitly reports
`execution_verified: false`: comparing supplied observations does not prove
where or how a recipe ran. The library is not a sandbox or authorization gate.
A runner must establish its own permissions, run provenance and platform evidence.

The example calls the existing PBLang literal-template library itself, then
submits its actual result for comparison. It also demonstrates a missing physical
device capability, a typed identifier mismatch, and a ready recipe with no result.
No physical device or second operating system is exercised by that example.

Manifests are JSON-compatible. The example roundtrips a trusted generated JSON
string using the existing runtime. This module accepts objects, not serialized
input: callers reading untrusted JSON must separately enforce byte/depth bounds
and their duplicate-key policy before invoking `validate`.

```text
pblang run tests/semantics/recipe_manifest_test.sapp
pblang run examples/discovery_manifest_recipes.sapp
```

Tests cover detached plans, exact typed equality, unsupported and missing
observations, duplicate IDs, malformed records, scalar limits, and the maximum
32 recipes. This completes the catalogue's portable-recipe-manifest library
proposal; a runner UI and physical multi-platform verification remain separate.
