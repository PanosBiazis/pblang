# Focused task plans — portfolio cycle 15

Extends the office/organization direction in PORTFOLIO_PLANNING and the existing dependency-task model. `task_scope.extract(doc,targets)` reuses `devops_pipeline.select_jobs`, then selects detached task records in source order. The full source is validated first. All ancestors, including completed prerequisites, remain. Labels, IDs, days, needs, completion, title and start date are unchanged. No new graph algorithm or task execution layer.

`task_scope.parse_ids` accepts bounded comma-separated unique ASCII IDs. Existing 32-task, 16-prerequisite, 48-character-ID, 32768-byte document limits remain. Selector text is capped at 1600 UTF-8 bytes. A subset has the same scheduled dates/status for retained tasks under the fixed calendar-day/unlimited-worker assumptions, while its total finish may shorten after unrelated branches are removed.

`task_scope_view` powers PBTaskScope with native open/apply/export and one-task portable snapshots. Changed source paths must be opened, and changed selection text must be applied. Partial export in PBPlan preserves saved baseline/history and the dirty-close guard. Existing file adapters create new copies without overwriting; failed writes may leave partial new files. No automatic cleanup, external transfer, telemetry, AI processing or progress synchronization.

Exports/previews carry plan title and task text, including automatically selected ancestors. Hide changes display only. Treat artifacts like the source when deciding where to share them. Source-only privacy checks and synthetic file/GUI tests are not a full platform or dependency audit.

Tests cover diamonds, shared/completed ancestors, non-topological source order, independent branches, 32-node chain closure oracle, schedule equality, detached dependency arrays, malformed/duplicate/unknown IDs, excluded-invalid-branch rejection, native stale-input guards and no-overwrite exports. Platform evidence is reported separately from generated previews. No performance speedup is claimed.
