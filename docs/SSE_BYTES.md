# Incremental UTF-8 bytes for SSE

`stdlib:sse_bytes.sapp` accepts consecutive arrays of integer bytes, including
chunks ending inside a UTF-8 character, and returns events from the existing
`stdlib:sse_reader.sapp`. It adds strict byte admission without changing that text
parser or adding network, filesystem, provider, reconnection, or timer operations.

```pblang
let S = import("stdlib:sse_bytes.sapp");
let reader = S.new(map{ chunk_bytes: 32, total_bytes: 128 });
reader.feed([100,97,116,97,58,32,240,159]); // "data: " + partial emoji -> []
let events = reader.feed([152,128,10,10]);
// [map{ event: "message", data: "😀", last_event_id: "" }]
reader.finish(); // []
```

## Interface and representation

`new(byte_options = nil, text_options = nil)` creates a reader with `feed(bytes)`,
`finish()`, and `status()` closures. A byte is a finite, integer-valued PBLang
number in 0..255. Strings, booleans, nil, fractions, nested arrays/maps, NaN and
infinities are rejected. This is the same byte-array representation accepted by
`str_to_bytes` / `bytes_to_str`, not a string containing byte-valued characters.
No binary value is passed through `str_from_char_code` by the implementation.

Byte options are copied and validated. Unknown keys, unsupported types, negative
values, fractions, nonfinite values, and values above the hard caps are rejected.
Zero is supported. `defaults()` and `limits()` return fresh byte-option maps.
`text_options` is passed to the existing SSE constructor, which validates and
copies it separately. See [SSE_READER.md](SSE_READER.md) for its text/event limits.

Each successful byte feed calls the text parser exactly once, including when
the decoded prefix is empty. Consequently empty feeds and feeds containing only
an incomplete scalar count against both the byte and text feed budgets. The
`text_options.chunk_chars` limit still applies to decoded text in one feed; raising
the byte chunk limit does not implicitly raise any text limit.

`status()` returns a detached map with `phase` (`open`, `closed`, `failed`),
`pending_bytes` (0..3), `last_event_id`, and `retry_ms`. No pending byte contents
or underlying text-reader handle is exposed. Use one reader from a single owner;
concurrent calls and serialization of its closures are unsupported.

## Strict decoding and composition

UTF-8 sequence validation follows the one-to-four-byte scalar ranges in
[RFC 3629](https://www.rfc-editor.org/rfc/rfc3629). Invalid leads and continuations,
overlong encodings, UTF-16 surrogate values, and values above U+10FFFF are rejected.
An incomplete but otherwise valid prefix retains at most three bytes between
feeds. A malformed prefix is rejected as soon as the available bytes prove it
invalid; it is not held until a future call. A valid incomplete sequence at EOF
is rejected by `finish()`. There is no replacement-character recovery.

The wrapper checks the entire current feed before forwarding text. It converts
one complete prefix with the existing strict `bytes_to_str` builtin, rather than
decoding each byte separately. The existing one-shot `unicode_encoding.utf8`
adapter is not used for chunks: it requires complete input and strips a leading
BOM on each invocation. Here all decoded BOM characters are passed to the text
parser, which ignores exactly one at the start of the complete stream and
preserves later BOM characters according to its existing field rules.

SSE CR, LF and split CRLF handling, multiline data, empty events, persistent IDs,
NUL-containing ID rejection, retry hints, event-name resets, and unfinished SSE
data at EOF retain the text parser's contract. `[DONE]` remains ordinary data.
This wrapper does not validate response status, MIME type, URL/origin, JSON,
provider usage or billing, and it does not turn whole-body HTTP APIs into live
streaming transports.

## Bounds and failure

| Byte option | Default | Hard cap | Meaning |
| --- | ---: | ---: | --- |
| `chunk_bytes` | 4,096 | 65,536 | Bytes in one supplied array |
| `total_bytes` | 4,194,304 | 67,108,864 | Lifetime admitted input bytes, including incomplete prefixes |
| `feeds` | 65,536 | 1,048,576 | Lifetime feed calls, including empty arrays |
| `decoded_chars` | 1,048,576 | 16,777,216 | Lifetime completed Unicode scalars, including BOMs and SSE framing |

Type, per-chunk, cumulative-byte and call-count admission precede the chunk copy.
The decoded-scalar budget is checked during validation before calling the decoder
or text parser. A working byte array contains at most `chunk_bytes + 3` numbers;
the only retained undecoded suffix has at most three. Conversion may create a
bounded prefix copy and decoded string. The text parser independently bounds
line/event state, total characters, emitted characters and event count. Every
limit must be satisfied; increasing one does not disable another.

These are logical limits on arrays, characters and calls. PBLang numbers and array
elements are not packed single-byte allocations. The bounds do not measure exact
allocator/RSS cost, impose wall-clock cancellation, limit an input array already
allocated by the caller, or limit results retained by the caller. No benchmark or
performance claim is made.

Any bad feed or inner parser failure makes the wrapper permanently `failed` and
throws the fixed message `sse_bytes: feed rejected; reader failed`. Truncated UTF-8
at EOF similarly throws `sse_bytes: finish rejected; reader failed`. Failure clears
decoder state and uses the text reader's existing failure path to clear its state,
then releases the inner reader reference. Failed status has zero pending bytes,
empty last ID and nil retry. Errors do not include supplied bytes or option values.
Logical clearing is not a guarantee of secure allocator-memory erasure or recovery
after process termination/allocation failure.

A failing feed returns **no partial event list**, even if its malformed suffix
follows a valid, complete SSE event in that same array. Events returned by earlier
successful feeds belong to the caller and cannot be retracted. In particular a
feed can emit a valid earlier event and retain a later incomplete UTF-8 suffix;
if `finish()` then rejects that suffix, the previously returned event remains
delivered. Successful finish preserves committed SSE ID/retry status, discards
unfinished SSE fields as before, and closes the wrapper. Aliases observe the same
closed/failed state; later feed/finish calls reject without reopening it.

## Verification

`tests/semantics/sse_bytes_test.sapp` covers every two-chunk byte split in synthetic
Unicode/SSE fixtures and every pair of split positions in a short non-BMP event,
plus one-byte feeds, exact scalar range edges, malformed and truncated sequences,
numeric/type validation, limits, alias isolation, atomic failure and lifecycle.
The runnable example splits both a BOM and emoji across real byte arrays.

```sh
pblang run tests/semantics/sse_bytes_test.sapp
pblang run examples/sse_bytes/main.sapp
```

These are local parser checks. They establish no deployed-network, browser,
device, hosted-CI or universal-platform result.
