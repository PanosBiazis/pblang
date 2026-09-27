# Study card corrections and encoding

`study_cards.edit(deck, id, prompt, answer)` returns a detached validated deck.
It replaces both content fields of one existing integer ID, preserves card order
and all scheduling metadata, and rejects unknown IDs, empty content, invalid
UTF-8 byte budgets and aggregate deck overflow. The input remains unchanged on
success or failure. Limits remain 64 cards, 1024 question bytes, 4096 answer
bytes and 32768 total title/question/answer bytes. The persisted v1 format and
review rules are unchanged. A correction is not a new review or a schedule reset.

PBStudy exposes this through `edit` and a separate `edit-gui` command. The native
editor separates its per-card draft from the accepted deck, keeps eight undo
states, and blocks navigation/save/open/undo while a draft differs. It uses the
existing new-copy store. The ordinary review GUI still opens and rates saved
decks. Native multiline controls currently only append and backspace; Clear and
Revert make replacement explicit. Full cursor/selection/clipboard/IME/scrolling
and persistent Web/Android editing remain future work.

Encoding now validates the deck then passes it directly to the existing read-only
bounded JSON serializer, removing reconstruction of all card maps. It still
performs validation and all serialization limits. Exact old/new bytes, detached
edit output, source preservation, Unicode and maximal bounds are checked by
`tests/semantics/study_edit_test.sapp`. The exact pre-change source is retained in
`bench/fixtures/study_cards_before_edit.sapp`.

Run `pblang run bench/study_encode.sapp --allow-all` for the same 1/16/64-card
workloads: three warmups, nine alternating samples, four calls per sample.
Output is wall-clock milliseconds per encode, excluding disk I/O. With nine
samples nearest-rank p95 is the maximum. Concurrent load matters; removal of a
copy does not guarantee a particular end-to-end save-time reduction.

Product tests cover draft guards, no-op edits, malformed edits, undo limits,
future-card selection, close retention and new-copy disk roundtrips. Existing
review, lesson and portable-reader suites remain relevant. Generated readers
are distinct from live Android execution and persistent mobile editing.
