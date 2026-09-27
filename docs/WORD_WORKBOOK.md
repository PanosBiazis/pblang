# Complete vocabulary workbooks

`stdlib:word_workbook.sapp` reuses `word_profile.analyze` once per report or
encoding. It removes screen-page and Sheet-row constraints from **complete
CSV/JSON exports**, without changing tokenization, validation or page exports.

`report(doc)` returns `format: "pblang.word-workbook.v1"`, `total`, `unique`,
and all `entries`. Each entry contains one-based `rank`, literal `token`,
`count` and unique **one-based** `blocks`. This differs deliberately from the
existing profile's zero-based JSON locations. Frequency descending and
first-appearance tie order are unchanged. The title is excluded. Results are
detached from the source; there is no filesystem, network or persistent cache.

`encode(doc, "json")` preserves exact tokens, including leading apostrophes,
formula-like strings, Unicode and punctuation. `encode(doc, "csv")` emits
Rank, Token (prefixed with apostrophe), Count and Writer blocks (1-based).
**Every CSV token receives exactly one additional leading apostrophe** as an
explicit text marker, even if it already starts with one. General CSV readers
may show the marker. Remove exactly that first marker to recover the token,
or use JSON for exact interchange. This is not a universal guarantee about
how spreadsheet programs interpret imports. Existing eight-entry page CSVs
retain their literal behavior and do not receive this new marker.

Writer document limits and the profile's 256 distinct forms / 256 UTF-8 bytes
per token apply before serialization. Both encoders check a 262144-byte output
cap. CSV has at most 257 records including the header; it is not a PBOffice
Sheet and can exceed that format's 32-row limit. Empty bodies export a CSV
header or JSON with an empty entries array. Invalid input is rejected as a
whole; output is never truncated. CSV quoting uses the existing CSV library.

PBWords `export-all input.pbtext csv new.csv` and the native **All CSV** button
use the same module. **All exact JSON** and the `json` command option select
lossless output. Neither depends on the currently displayed page or Find query.
Only the loaded document is exported; changing the input path does not open it.
Choose Open first. Exports require new matching .csv/.json filenames in an
existing directory, use secure_file's exclusive write, and never overwrite.
Write failure may leave an incomplete new file; atomic visibility and crash
durability are not promised. Source document and current page are preserved.

Exports intentionally contain vocabulary and source-block locations; treat them
as document-derived data. No titles, source paths, timestamps or source bodies
are added. User-selected exports remain until manually deleted. Omitting titles
does not anonymize vocabulary. Generated demo documents contain synthetic text.

CLI/native export uses the same portable PBLang module. Existing web/Android
readers remain fixed-page readers, without filesystem export. Linux runtime
tests and native/web/Android view compilation are distinct from physical
Windows/macOS/Android testing, which has not been performed for this feature.

Tests: `tests/semantics/word_workbook_test.sapp`, existing word_profile and
word_locate suites; PBWords app/find/store/workbook suites. The workbook tests
cover the maximum entry count, all 64 block locations, stable ties, Unicode,
CSV markers/quoting, JSON preservation, malformed input, non-overwrite, absent
directories, unchanged source/view and native control bounds.
