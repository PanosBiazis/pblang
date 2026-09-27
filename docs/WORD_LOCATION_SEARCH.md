# Exact vocabulary search and native location display

`word_profile.locate(doc, token)` searches the same literal forms and stable
frequency order as `analyze`. A query is a nonempty string of at most 256 UTF-8
bytes, with no NUL. There is no trimming, case folding, normalization, substring
search or linguistic segmentation. A valid query containing separators simply
cannot equal a stored token. The document is validated using the existing limits.

A hit returns `{found:true,position,page}`: zero-based position in the complete
frequency report, plus the existing complete page shape
`{total,unique,pages,index,entries}`. A miss returns `{found:false}`. It analyzes
once and slices that report through a private helper also used by the existing
`page` function. Returned data is detached from the source document. No global
index, file scan, document cache or persistence is added.

PBWords adds Find exact form. A hit opens the correct eight-entry page and the
text screen containing the entry heading. Invalid queries and misses preserve
page, text-screen position, loaded source and result lines. Its result text now
shows actual one-based Writer block numbers for each form, wrapped without
truncation. CLI `find <input.pbtext> <exact form>` returns structured status and
the same lookup result; quote queries as required by your shell.

Each rendered native page records its known total page count. Next uses that
count to clamp navigation before refreshing the selected page. Previously it
performed one full analysis to obtain that count and another to render the page.
The loaded source changes only through Open, which refreshes page zero; explicit
programmatic state replacement must likewise reset the page and call `refresh`.
No token report is cached. Failed Open leaves the previous page count intact.
Generated ASCII block-number lines already within 72 columns are emitted directly;
longer lists use the existing wrapping routine. All 1–64 block counts are compared
with the full wrapper to preserve identical text.

Web/Android keep the existing generated reader for an explicitly chosen page;
use the lookup's page index when exporting. They do not gain live document search
or file workflows. JSON position/page/block indices are zero-based; native block
numbers and CSV/Sheets block numbers remain one-based.

Tests cover all 256 distinct forms and every page, exact case/punctuation/Unicode,
misses, empty bodies, bounded and malformed queries, source independence,
portable selection of the found page, full 64-block location display, failed-open
retention, reduced page count after replacing a source, and native layout bounds.

PBWords `bench/page_navigation.sapp` compares the exact prior app module retained
under `bench/fixtures/app_before_find.sapp` (with its unchanged local store import).
Both versions use the same current shared libraries and loaded document. It times
Next from page zero including synchronous text preparation/refresh for 9/64/256
unique forms. The new UI additionally renders actual block numbers; displays are
intentionally not byte-identical. Report/page/source equivalence is asserted.
Three warmup batches, nine alternating-order samples, five calls per sample;
wall-clock ms per Next action, excluding native event-loop presentation and file
I/O. Median and nearest-rank p95 (max of nine) describe only that run.
