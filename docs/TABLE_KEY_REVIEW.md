# Literal selected-key review

Motivation: PORTFOLIO_TABLE_QUALITY documents whole-row review before cleaning.
A whole-row duplicate check does not detect repeated identifiers with different
values in other columns. This extension adds an explicit single-column key check,
using existing bounded Sheet and table_row_selection contracts. No new broad disk
scan or customer data was used to derive the three original Greek scenarios.

`table_key_quality.inspect(doc,column,header=false)` validates the table and a
zero-based integer column, excludes a selected header, and returns missing keys
(trim is empty) and later exact repeats with the first source-row reference.
Missing keys do not form duplicate groups. Nonblank keys keep case/whitespace.
A bounded index locates the first occurrence; a separate source-order traversal
collects the review selection. No benchmark speedup is claimed.

The report schema pblang.table-key-quality.v1 includes flagged_rows, missing_keys,
duplicate_keys, rows and review_indices/review_rows. It contains no cell/header
text but is not safe to log because counts/positions may be sensitive.
`select_review` recomputes from source, adds the first occurrence of every repeated
key, and delegates detached copying to table_row_selection. Empty selections
reject. Existing shape/byte limits apply unchanged. Caller report mutations have
no effect on later exports. describe_row uses one-based display positions.

PBTableCheck 0.2 integrates the same logic into CLI, native controls and paginated
Web/Android readers. Pending choices block export; failed checks/opens retain
working state. General quality checks remain available and unchanged. Examples
cover product codes, work-item identifiers and learning-section codes.

Privacy review: no new network, authentication, telemetry or third-party package.
Diagnostics omit source strings; exported review tables intentionally retain them.
User-created copies have no automatic expiry. Existing table_files/secure_file
boundaries are retained, including exclusive new output and no atomicity claim.
Focused symlink/source-preservation tests do not certify the entire filesystem
backend. Full runtime/CVE audits and physical-platform validation are out of this
slice. Current reference execution uses the retained Linux PBLang 0.6.0 runtime.
