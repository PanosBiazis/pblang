# Stored-date study agenda — wave 12

Study policy in PORTFOLIO_FOUNDATIONS and the progress-preserving revision work establish deterministic, explicit dates. This slice adds workload visibility while preserving that contract. PBStudyAgenda is a new read-only planner; PBStudy adds due-copy export and agenda CLI. No private-content intake, automatic learning or hidden scheduling changes.

`study_agenda.plan(deck,today,days=7)` validates the existing Study model and a 1–31 day horizon, builds one bucket per civil date and visits each card once to increment its stored-date bucket, overdue count or after-horizon count. The prefix sum starts with overdue cards. Counts are bounded by 64 and calendar overflow rejects. Complexity is O(cards + days) after validation; no measured speedup claim.

The report's `scheduled` values are exact saved dates and `due_by` assumes no completed reviews. It is not a recurrence forecast because ratings change future dates. Output contains no prompt/answer/title but is not declared safe to log. `select_due` reuses canonical `study_cards.due`, retaining stable date ordering and detached metadata. Empty selections reject rather than fabricate an invalid empty deck.

`study_files` extracts PBStudy's existing bounded deck open/new-copy-save routines into a shared module. PBStudy preserves its public store facade and local import/revision functions; the new planner reuses the same adapter. Source files and existing targets are preserved; failed writes may leave partial new files. No atomic write guarantee is introduced.

Tests cover leap days, earliest/latest calendar dates, overflow, invalid horizons, sparse IDs, reviewed progress, equal-date order, detached copies, a full 64-card/31-day independent oracle, input preservation, stale form settings, paged readers, partial-export dirty-state retention and new-file collisions. Native Linux, browser interaction and generated Android evidence are distinct from Windows/macOS or physical mobile validation.
