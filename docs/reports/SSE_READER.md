# Incremental SSE text reader

`stdlib:sse_reader.sapp` parses consecutive **already-decoded Unicode strings**.
It is pure PBLang and imports only `string_builder.sapp`. It performs no network,
filesystem, environment, provider, timer, or reconnection operation. Existing HTTP
and AI modules are unchanged. It does not turn their whole-body helpers into
network streaming and is not a browser `EventSource` implementation.

```pblang
let SSE = import("stdlib:sse_reader.sapp");
let reader = SSE.new(map{ chunk_chars: 256, total_chars: 1024, events: 8 });
reader.feed("data: hel"); // []
let events = reader.feed("lo\r\n\r\n");
// [map{ event: "message", data: "hello", last_event_id: "" }]
reader.finish(); // [], drops an unfinished line/event and closes the reader
```

`new()` and `new(nil)` use defaults. A map overrides selected limits; unknown keys,
non-numbers, nonfinite numbers, fractions, negatives, and values above the hard caps
are rejected. Zero is supported. Options are copied, and `defaults()` and `limits()`
return fresh maps. The returned reader exposes `feed(text)`, `finish()`, and
`status()`. Status is a fresh map with `phase` (`open`, `closed`, or `failed`),
`last_event_id`, and `retry_ms` (initially nil). Use a reader from one owner;
concurrent calls and serialization of closures are unsupported.

## Parsing contract

The line and field rules follow the WHATWG HTML
[server-sent event parsing and interpretation algorithms](https://html.spec.whatwg.org/multipage/server-sent-events.html#parsing-an-event-stream):

- LF, CR, and CRLF terminate lines, including CRLF split across feeds. A CR can
  produce events immediately; a subsequent LF is consumed without another line.
- One initial U+FEFF is ignored; empty initial feeds do not consume that allowance.
- Comments and unknown fields are ignored. Field names are case-sensitive. The
  first colon separates the field; only one following ASCII space is removed.
  All other whitespace and colons are preserved.
- Each `data` field contributes its value and one LF. A blank line dispatches
  when at least one data field exists, removing the final LF. Empty data fields
  therefore produce empty events or intentional newlines. Event names default
  to `message` and reset on every blank line, including blocks without data.
- IDs persist across events, empty IDs reset them, and fields containing NUL are
  ignored. `last_event_id` in status commits on a blank line, including blocks
  without data; a pending ID does not change that committed status.
- Nonempty ASCII-digit-only `retry` updates `retry_ms` without creating an event.
  Other values are ignored. Digit-only values above the configured `retry_ms`
  limit fail the reader; this explicit bounded policy is narrower than unbounded
  decimal integers. Leading zeroes are accepted. No timer/reconnection is started.
- `finish()` discards pending data and any unterminated line. It never synthesizes
  a blank line or dispatches the unfinished event. Committed ID and processed
  retry remain available in status. `[DONE]` is ordinary data.

The caller must decode UTF-8 incrementally before calling `feed`; a byte chunk
ending inside a code point is not valid input to this interface. The parser cannot
validate or repair a previous decoder's decisions. Pass either decoded text with
its leading BOM or text whose decoder stripped exactly that BOM, consistently.
For byte chunks, the opt-in [strict UTF-8 wrapper](SSE_BYTES.md) performs bounded
incremental decoding before calling this unchanged text interface.
There is no origin, URL, MIME/status validation, dispatch queue, JSON decoding,
provider usage inference, or billing estimate.

## Bounds and failure

All sizes count Unicode scalar characters, not UTF-8 bytes or graphemes.

| Option | Default | Hard cap | Meaning |
| --- | ---: | ---: | --- |
| `chunk_chars` | 4,096 | 65,536 | Characters in one supplied string |
| `line_chars` | 4,096 | 16,384 | Incomplete line, excluding its terminator |
| `data_chars` | 65,536 | 262,144 | Current event data, including appended LF characters |
| `pending_chars` | 131,072 | 524,288 | Line + data + event name + ID buffer + committed ID |
| `total_chars` | 1,048,576 | 16,777,216 | Sum of all admitted input, including BOM and terminators |
| `output_chars` | 1,048,576 | 16,777,216 | Lifetime sum of emitted data + event name + ID lengths |
| `events` | 4,096 | 65,536 | Lifetime emitted event count, including empty data events |
| `feeds` | 65,536 | 1,048,576 | Lifetime feed count, including empty feeds |
| `retry_ms` | 2,147,483,647 | 2,147,483,647 | Largest supported reconnection hint |

Line admission also applies to ignored comments/fields. The pending budget
conservatively counts both ID slots even when they share a value. Line text may be
retained transiently during processing; these are logical parser limits, not an
exact allocator/RSS bound. The per-call result is bounded by the lifetime event
and output budgets, without a separate per-feed event threshold that would change
acceptance solely because valid text was split differently.

A bad chunk type, exceeded limit, or catchable parsing failure permanently marks
that reader `failed`, clears pending content, the committed ID, and retry, and
throws the fixed message `sse_reader: feed rejected; reader failed`. The failing
call returns **no partial event list**. Results from earlier successful calls
remain owned by the caller; they are not rolled back. There is no recovery/reset:
create a new reader and apply the application's explicit restart policy. Both
`feed` and `finish` reject after closing/failure; `status` remains readable.
Errors do not echo stream contents or options.

Bounds cap parser-managed text, arrays, numeric arithmetic, and total accepted
input/calls. They do not impose wall-clock cancellation, bound the caller's
already-allocated input, measure VM heap overhead, or bound application retention
of returned events. Counting an oversized supplied string requires the runtime's
length scan before rejection. Allocation failures and process termination are not
recoverable guarantees. Chunk arrays are created only after length admission;
line and data builders avoid repeated concatenation of accumulated prefixes.
No performance improvement or universal memory guarantee has been measured.

## Verification and example

`tests/semantics/sse_reader_test.sapp` checks expected events independently across
all two-chunk cuts and one-character feeds, including Greek and emoji. It covers
line endings, whitespace, BOM, comments, empty/multiline data, names, IDs/NUL,
retry, EOF, aliases, invalid types/options, cumulative limits, boundary values,
atomic failure, and closed/failed readers. Partition equivalence applies when all
chunks and calls fit the configured limits; exceeding a per-chunk/call budget is
an intentional admission failure.

Run from a repository with a current portable dependency lock:

```sh
pblang run tests/semantics/sse_reader_test.sapp
pblang run examples/sse_reader/main.sapp
```

The runnable example uses fixed synthetic text only. The focused suite is local
parser evidence, not network, deployed service, browser, or hosted CI evidence.
