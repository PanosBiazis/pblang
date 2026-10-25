# Explicit workday planning scenarios

Existing discovery recipes identified workday follow-up, weekend workshops and reverse deadlines as reusable needs. The existing `work_calendar` library provides the date arithmetic. This integration composes it with `task_plan` in PBWorkdays and adds an explicit PBPlan CLI mode, without changing calendar-day scheduling. No new scan or private-data ingestion was performed.

## Shared contract

`task_workdays.schedule(plan,weekdays=nil,closed=nil)` validates and detaches the existing plan, preserves its topological order/status, normalizes a closed initial date forward, and places each task after its latest prerequisite. Inclusive duration 1 finishes on the starting workday; successors start on the next open day. Independent branches overlap. Numeric source durations are explicitly reinterpreted as workdays; the source file and `task_plan.schedule` remain unchanged.

The report uses `pblang.task-workdays.v1`: title, requested_start, actual start/due, dependency-path workdays, inclusive calendar_days span, completed/total, tasks with workdays/start/due/status, detached weekdays/closed lists, `source_days_interpreted_as_workdays:true`, fixed/unlimited-worker assumptions and `execution_performed:false`. It includes labels and supplied dates. Readiness remains based on completion of prerequisites, not today's date.

`writer` exports headings/paragraphs for every task and puts the complete supplied calendar and interpretation into the first task's paragraph. At most 64 blocks for 32 tasks. Existing Writer block/text budgets apply independently: excessive calendar metadata rejects the whole export. Nothing is silently omitted to force an export to fit.

`workday_fields.parse` centralizes strict human fields. Weekdays are 1–7 in text, zero-based in the returned library configuration. Comma-separated days and comma/newline-separated strict civil closure dates permit surrounding whitespace but reject duplicate/malformed/empty entries. Limits: 64 bytes for weekdays, 4096 bytes/366 dates for closures. An entirely blank closure field means an empty list. Defaults are Monday–Friday with no closures.

`task_plan_files` moves the existing PBPlan file adapter into the shared library. PBPlan retains forwarding functions with the same names and behavior. New-copy, bounded reads, portable formats and Writer export semantics remain unchanged. This introduces no replacement write or new persistence guarantee.

`work_schedule_view.create` generates four-task read-only pages with precomputed page buttons. It embeds user labels and calculated dates, the weekday list and closure count; it does not embed a full editable calendar. It performs no file operation or background scheduling.

## Bounds and evidence

Existing plan bounds remain 32 tasks, durations 1–365, at most 16 prerequisites per task, strict IDs/labels, completion consistency and Gregorian dates. No total 366-workday truncation: per-task calendar traversal supports longer dependency paths. Each existing `work_calendar.add` call retains its 6000-step bound. Overall work is bounded by task/dependency counts and calendar-list limits; no speedup is claimed.

Tests include a 32-task single-weekday chain (11680 workdays), exact Friday/weekend and holiday cases, leap dates, overflow, independent branches joining at the latest parent, detached state and unchanged ordinary PBPlan schedules. Twelve fixed synthetic DAG scenarios are checked against independently generated day-by-day reference dates. Product tests cover stale fields, retained results, real file roundtrips and no overwrite. Native, browser, Android generation and physical platform evidence are separate; see the accompanying run report.

PBPlan's new `workdays` CLI is additive. Its native editor remains a calendar-day editor. PBWorkdays provides the separate workday scenario UI. Neither product schedules background reminders or modifies the input plan from this workflow.
