# Whole serialized-result UTF-8 cap

`stdlib:tool_result_cap.sapp` is a pure in-memory admission helper for
**caller-owned, already validated serialized JSON**. It never parses,
reserializes, slices or repairs that text. Its accepted output preserves every
byte represented by the original PBLang string, including whitespace, escapes,
large integer lexemes and exponent notation. Admission does not prove valid JSON:
even malformed text passes unchanged if the caller violates the precondition.

Call `bound_prevalidated_json(final_serialized_result, max_bytes = 65536)`.
Serialize the complete final wrapper first, then apply the cap. Sending only a
bounded body inside a later wrapper does not bound the final message. The return
map is a **local receipt**, not the wire envelope: transmit only `output` when
it is non-nil. The size of `json_stringify(receipt)`, an added newline, transport
framing or any later wrapper is not covered by this payload limit.

| Receipt field | Meaning |
| --- | --- |
| `ok` | Source type and byte limit are supported; not JSON validity or permission. |
| `accepted` | Entire input is returned unchanged. |
| `capped` | Input was withheld because of an output-byte or input-scalar limit. |
| `input_bytes` | Exact UTF-8 bytes when measured; nil when measurement did not occur. |
| `output` | Entire original text, entire fixed replacement, or nil for no output. |
| `output_bytes` | Exact UTF-8 bytes of `output`; zero for nil. |
| `reason` | Fixed code; never includes input text, keys, hashes or excerpts. |

When oversized, the only replacement is the complete JSON
`{"error":"tool_result_too_large"}`. Its entire UTF-8 size must fit. Otherwise
`output=nil` and `output_bytes=0`; the helper never emits an oversized error.
`replacement_json()` exposes that fixed string for callers/tests.

Caps must be numeric integers in **0..1,048,576**, inclusive. Zero does not disable
the cap. Negative/fractional/non-finite/coerced/unsupported limits return
`ok=false`, no payload and fixed `invalid_limit`. Non-string source values are
not converted and return `invalid_source_type`. `limits()` returns independent
metadata with the default and hard ceilings; changing it changes no policy.

Before encoding, source strings exceeding **1,048,576 Unicode scalars** are
withheld with `input_scalar_limit` and unknown `input_bytes=nil`. Otherwise UTF-8
encoding creates at most 4,194,304 temporary byte entries. This is an element
bound, not a 4 MiB process-memory guarantee: runtime representation and the
already allocated caller string cost additional memory. Counting scalars can
still scan the caller's full existing string. There is no stream ingestion,
producer-memory admission, external I/O, timeout or general sandbox claim.

This helper is independent of the pbAI broker's existing private character
limits and of `serialization.stringify_safe`'s value conversion. It does not
replace broker authorization, content inspection, provider limits or transport
policy. Accepted content is unchanged and may contain sensitive information;
only withheld content has the no-reflection behavior described above. Nothing
is automatically installed in a broker, MCP server or provider connector.

Run the focused test and example from the repository with a PBLang runtime:

```sh
pblang run tests/semantics/tool_result_cap_test.sapp
pblang run examples/tool_result_cap/main.sapp
```

Tests include ASCII/Greek/emoji exact byte boundaries, complete fallback at its
minimum size, zero/tiny/invalid limits, large numeric lexeme preservation, opaque
malformed-text behavior, hard input/output ceilings, final-wrapper accounting,
and a synthetic withheld marker checked against the entire receipt. This is a
new standalone capability; no comparative performance claim is made.
