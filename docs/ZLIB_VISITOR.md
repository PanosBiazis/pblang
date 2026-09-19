# Visiting decoded zlib output

`stdlib:flate.sapp` adds `zlib_visit(data, visitor, max_out, exact=false)`.
Existing `zlib_decode`, raw DEFLATE and gzip array APIs remain available.
The new API decodes stored, fixed-Huffman and dynamic-Huffman blocks while
retaining recent history instead of the complete decoded output.

The input is an existing byte array or a string converted to UTF-8 bytes. This
is incremental **output consumption**, not streaming input from a file/socket.
`visitor(chunk, offset)` receives detached byte arrays in order and a zero-based
absolute decoded offset. Chunks are nonempty and contain at most 65,793 bytes.
The decoder retains the last 32,768 bytes after each flush; a match can temporarily
extend the working array to 65,793 bytes. Copies for delivery/compaction and the
Huffman tables also consume memory. A visitor retaining all chunks can still
use unbounded memory, as can input storage and retained-runtime weak registries.
This is not a replacement for general runtime garbage collection.

The visitor must be a runtime callable. `max_out` is an explicit nonnegative
integral safe number, checked before growth; no language-wide output limit is
introduced. `exact` must be boolean. Return is
`{decoded_bytes: number, checksum_verified: true}` after trailer validation.
No callbacks run for a valid empty stream. Callback failure propagates without
retry or automatic fallback. Callback mutation cannot alter later history or
checksum calculations.

**Chunks are provisional until successful return.** A late checksum/trailer
failure may happen after callbacks. Consumers must keep changes local and avoid
publishing files, messages or other effects from the callback. Adler-32 detects
corruption, not source authenticity. This API performs no file/network/native
operations and does not grant any capability to its consumer.

The visiting decoder validates byte values, zlib header/window/dictionary rules,
stored block lengths, reserved symbols, dynamic code counts, missing end-of-block,
oversubscribed/incomplete trees, backreference history/window bounds, output
quota and Adler-32. `exact=true` rejects trailing data. Its stricter malformed
Huffman/header checks do not change the legacy decoder's acceptance policy.
The existing shared Huffman reader and table construction are reused.

The current self-hosted VM represents callables as maps and does not satisfy
the visitor's callable contract. Such calls are rejected. Consumers supporting
that VM must explicitly retain the array API; do not accept map-shaped claims
of callability. PB-MCP's PNG sanitizer does this before choosing a decoder.

Validation: existing flate and dynamic encoder/decoder suites, the new
`tests/semantics/flate_visit_test.sapp`, four independently generated Python-zlib
window-crossing streams, and PB-MCP's independent PNG pixel/error corpus.
Tests include overlapping matches across flushes, callback mutation/failure,
empty output, exactness, quotas, checksum failure and malformed dynamic trees.
No general arbitrary-size soak, all-backend or universal platform claim is made.

## Self-hosted module compatibility

The fixed-encoder cache now declares `map<string,any>` explicitly: its readiness
flag, initially empty slots and populated Huffman arrays are heterogeneous.
Without that declaration the self-hosted checker inferred optional booleans and
rejected the module, preventing even unrelated checksum/decoder exports from
loading. No general type-checking rule was relaxed and concrete boolean-map
assignments remain checked. Cache contents and initialization timing are unchanged.

`tests/semantics/selfhost_flate_test.sapp` verifies real module compilation,
checksums, repeated zlib encode/decode, gzip and malformed-trailer rejection.
PB-MCP additionally validates its small PNG corpus through the actual self-hosted
frontend. This fixes the module-loading failure; map-shaped visitor callables
remain unsupported, so self-hosted PNG still uses the full-array decoder.
