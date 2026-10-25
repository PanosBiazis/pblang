# Bounded Writer briefs and excerpts

`stdlib:document_brief.sapp` combines two to four already valid Writer documents
into one detached `pblang.text-document.v1` document. `create(documents, title,
selections=nil)` copies all blocks unless each source has a corresponding selection
of zero-based block indices. Every selection must be nonempty, strictly increasing,
unique, integer-valued and in bounds. Source order and block kinds are preserved.
A heading `Source N: <source title>` precedes each source. These labels are supplied
by the documents, not authenticated authorship.

The output title must be nonblank and at most 256 UTF-8 bytes. The shared Writer
limits include the new title and headings: 64 blocks, 1,024 bytes per block and
32,768 total UTF-8 text bytes. Invalid sources, invalid selections and oversized
results throw without returning a partial document or mutating any source.
Source documents are validated in full, including blocks not selected for output.
Output blocks are copied; later edits to either source or result are independent.
Text is copied literally. This API is not summarization, redaction or anonymization.

`stdlib:document_excerpt.sapp` exports `select(document, indices)`, with the same
selection rules, producing a detached Writer document. `parse_indices(text)` parses
one-based comma-separated decimal block numbers into zero-based indices. It bounds
input to 192 characters and each number to 1–64; it does not validate source bounds,
uniqueness or order until `select` is called. Whitespace and empty entries reject.

`stdlib:document_brief_files.sapp` exports:

- `compose(paths, title, selections=nil)` reads two to four `.pbtext` sources and
  returns `{ok:true, document:..., snapshot:true}` or a fixed, content-free error.
- `save_copy(path, document)` delegates to `text_document_files.save_copy`.
- `compose_copy(paths, title, output, selections=nil)` composes first, then saves
  to an exclusively created new output path. Existing sources/outputs are preserved.

The file adapter inherits platform availability and failure behavior from
`text_document_files` and `secure_file`. A failed write can leave an incomplete new
file. Atomic visibility, durability, universal platform support and multi-file
transactional reads are not promised. A returned snapshot does not track later
source changes. The in-memory functions perform no filesystem or network operations.

Focused regression suites are `tests/semantics/document_brief_test.sapp` and
`tests/semantics/document_excerpt_test.sapp`. PBBrief product tests separately check
its file workflow and application state; passing the pure suites does not establish
installed GUI or cross-platform acceptance.
