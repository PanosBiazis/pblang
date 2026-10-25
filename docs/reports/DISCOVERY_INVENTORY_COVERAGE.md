# Inventory coverage example

Run `pblang run examples/discovery_inventory_coverage.sapp` from a prepared PBLang checkout.
This self-checking example composes the existing `project_discovery_progress` and
`project_marker_hints` libraries; it adds no scanner, device access or new dependency.

Six synthetic root claims demonstrate completed, truncated, timeout, unavailable,
excluded and error. Five input names become four per-root unique observations:
the duplicate Cargo marker in one root is counted once, while the same marker in
another root remains another observation. This does not count distinct projects.

The direct/index distinction remains visible. A caller's completed claim does not
verify drive coverage, root identity, live existence, absence or index freshness.
Excluded, unavailable and error claims must contain no names. A contradictory later
claim rejects the entire batch with neutral counts, including earlier valid input.

The example checks preservation of the original input and name/path-free output.
Aggregates can still be sensitive; `safe_to_log` remains false. Source diagnostics
may expose literals in a caller's script, so synthetic fixtures are used throughout.

The existing library accepts at most 32 roots, 256 basenames per root, 1024 names
and 32768 UTF-8 bytes per batch. Root slots are ordinal, not durable identities.
There is no persistent checkpoint, resumable traversal, storage, authorization or
training admission. A real collector must track its own exclusions, failures,
depth/entry/time limits and source provenance independently.
