# Whole-unit allocation workflows

The existing Discovery Recipes proposed workshop seats, game resources and test budgets using `integer_allocation`. PBPortions turns that foundation into a local table workflow. PBLesson applies the same exact integer method to caller-specified minutes per lesson section. This is an independently authored product integration inspired by existing documentation, not a new scan or an import of private datasets/vendor implementations.

## Libraries and invariants

`allocation_table.integer` accepts bounded ASCII integer text, optional surrounding whitespace/leading zeros and values 0..1000000. No sign, exponent, decimal or locale syntax. `weights` accepts up to 31 comma-separated values in at most 2048 UTF-8 bytes; empty members reject.

`create(labels,weights,total,unit="Units")` requires 1..31 nonblank bounded labels, equal-length integer weights and a bounded one-line unit heading. It delegates to existing `integer_allocation.distribute`, builds a detached three-column Sheet and validates all output budgets. Reports expose format, document, total, row count, unit and the exact method identifier. Labels are not unique IDs and duplicates remain separate rows. `from_sheet` requires exact Label/Weight headers and normalizes numeric weight text without mutating input.

Largest remainders receive leftover units after floor quotas. Stable input order breaks ties; a zero weight gets zero. Shares conserve the exact total. No minimum, capacity, automatic weighting or total-monotonicity guarantee is added. Changing the total or row order can redistribute shares. Existing arithmetic limits remain: inputs up to one million and exact integer products at most 10^12. The table wrapper is limited to 31 recipients; the original integer library still supports 128 and is unchanged.

`allocation_files.open/save/run` composes bounded existing table I/O. Failed validation precedes writes; successful outputs require new filenames. Returns contain counts/method rather than repeating table text. A failed new-copy write can be incomplete; no new atomic/durable-save guarantee.

`lesson_allocation.create(bundle,minutes,weights)` derives titles in the existing lesson section order, requires exactly one weight per section, and returns the same table with a Minutes heading. Zero minutes are possible and no minimum teaching time is inferred. Notes, slides, cards, dates and study progress are unchanged. PBLesson's new allocate-minutes CLI saves a separate `.pbsheet`; its existing GUI remains a lesson converter/reader. PBPortions provides the native allocation workflow for Label/Weight tables.

## Bounds and evidence

Sheet limits remain 32 rows, 256 UTF-8 bytes/cell, 8192 total bytes and bounded file reads. An input within its two-column budget can exceed the output budget when shares are appended; reject rather than truncate. Results contain supplied labels. Portable exports embed one selected result row with all three cells; they do not recompute allocation.

Tests cover source detachment, canonical integer fields, zero/tied weights, maximum values, duplicate labels, malformed input, output-byte overflow, lesson minute conservation, stale UI settings and actual file roundtrips. Eighty fixed synthetic cases use independently computed integer quotients/remainders as an oracle, alongside the original allocation suite. Product builds/manifests, lint, Linux event-loop, browser actions and Android generation are separate checks. No measured speedup, installed Windows/macOS/Android parity or production readiness is claimed.
