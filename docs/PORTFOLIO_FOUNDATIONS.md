# Office, organization and data foundations

Four PBLang-only libraries are shared by PBOffice, PBJobTracker, PBDataLab and PBStudy. These are implemented pure operations, not a declaration that all platform runtimes support every library.

| Library | Implemented behavior | Explicit limits |
|---|---|---|
| `text_document.sapp` | Detached paragraph/heading/bullet documents, insertion/deletion, find/replace, statistics, strict JSON and plain text conversion | 1–64 blocks; 1024 UTF-8 bytes/block; 256-byte title; 32768 total text bytes; 262144 serialized bytes |
| `civil_date.sapp` | Strict proleptic Gregorian YYYY-MM-DD, zero-based ordinal, date arithmetic | Years 0001–9999; no clock, timezone or locale; at most 14 year-search iterations |
| `table_profile.sapp` | Missing/text/numeric counts, distinct values, duplicate rows, sum/mean/min/max/population variance, stable literal filtering | Existing Sheet 32x8/8192-byte model; explicit header/delimiter; plain decimals within +/- 1e12 |
| `study_cards.sapp` | Writer heading/answer conversion, detached decks, due ordering, explicit review transitions and strict JSON | 1–64 cards; 1024-byte prompts; 4096-byte answers; 32768 text bytes; 1M reviews/card |

## Text document contract

A document is exactly `{format:"pblang.text-document.v1",title,blocks}`. Blocks are exactly `{kind,text}` with kind `paragraph`, `heading` or `bullet`. Edits return detached documents. Failed edits never mutate the original. Block indices are zero-based. Deletion explicitly permits deleting nonempty blocks; the calling editor offers undo.

Find/replace is case-sensitive, literal and body-only. Find returns one index per matching block. Replacement replaces non-overlapping occurrences and never rescans inserted text. Output byte budgets are checked before appending. Title text is not searched or replaced. Word statistics use ASCII space/tab/CR/LF boundaries; they are not a linguistic tokenizer. Plain-text conversion joins blocks with LF, loses kinds/title, and normalizes CRLF when importing. Native JSON preserves the supported model exactly, rejects duplicate/unknown fields, and is not DOCX/ODT/PDF.

## Table profiles

Empty means exactly the empty string; whitespace-only values are text. Distinct values are compared literally without normalization; empty values are excluded. Numeric classification trims surrounding whitespace and accepts one optional sign and decimal point, ASCII digits, and at least one digit. Exponents, formulas and nonfinite names remain text. An otherwise numeric value outside the supported range rejects the profile. No formula runs. The first row is excluded only with `header:true`; header-only input has zero observations and null mean/min/max/variance. Duplicate count is the number of data rows beyond the first identical occurrence, not the number of duplicate groups.

Welford accumulation calculates population variance in one pass per column; this is ordinary floating-point analysis, not exact financial arithmetic. The report omits headers and cell strings but aggregate counts/numbers can still disclose information, so `safe_to_log:false`. `select_rows` returns stable zero-based source indices, excludes an explicit header and does not copy or modify the table.

## Study policy

Each heading is a question; following non-heading blocks become its answer until the next heading. Nonempty text before the first heading, empty questions/answers, and limit violations reject the whole conversion. This is explicit rule-based conversion, not AI generation or training.

The caller supplies a valid civil date. New cards are due immediately at stage zero. `good` advances a stage, capped at five, with intervals 1, 3, 7, 14 and 30 days. `again` resets to stage zero and schedules tomorrow. A card cannot be reviewed before its due date or twice on the same review day. Date overflow rejects the entire operation. Due cards sort oldest first, preserving input order for equal dates. No scheduling service, notification, hidden clock or remote persistence is installed.

## Provenance and validation boundary

Implementation is newly authored PBLang. Direction comes from the existing `MIGRATION_INVENTORY_2026-08-21.md` document utilities/educational/data examples, `POLYGLOT_MIGRATION_CATALOG_2026-08-20.md` reporting ideas, and PBOffice's `docs/SUITE_ROADMAP.md`. PBStudy is an original product composition of Writer documents and explicit review scheduling. No private scanned content, datasets, vendor source or existing application database was copied. Scanner admission/security contracts are unchanged.

Tests cover Unicode, malformed structures, strict dates, leap years, roundtrips, retained input after failure, due transitions and aggregate arithmetic. Linux VM execution and generated Web/Android snapshots are separate evidence; these do not establish physical Windows/macOS/mobile operation or signed release readiness. No quantified performance gain or full-language capability coverage is claimed.
