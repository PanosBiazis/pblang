# JSON number decoding in PBLang

`stdlib:json.sapp.parse(text)` and the standalone
`stdlib:json_decode.sapp.parse(text)` preserve the ordinary finite binary64
conversion of the original numeric tokens. For example, `9007199254740991.0`,
`9007199254740991.00` and `90071992547409910e-1` produce the same number as
`parse_number("9007199254740991")`, rather than the retained JSON decoder's
incorrect `9007199254740990` result for the decimal spelling.

## Supported contract

- This is ordinary binary64, **not arbitrary-precision decimal JSON**. `0.1`
  receives the normal binary64 rounding. Integers beyond exact representation
  can round; use an explicitly agreed string/exact-value application schema
  when those mathematical values must be lossless.
- Original native JSON grammar, recursion/depth and range admission remain
  authoritative. The complete original document is validated first, including
  values later discarded by duplicate-key handling. This does not extend the
  native parser's accepted range or repair its own admission restrictions.
- Original invalid-input/type/overflow diagnostics and their original source
  positions are retained. No error uses positions in the intermediate encoding.
- Native duplicate object keys remain last-wins, including escaped equivalent
  names. Use `serialization.parse_safe` with its existing duplicate-key checks
  when duplicate rejection is required. No transport guard is replaced.
- Signed zero, negative zero underflow, positive underflow to zero, Unicode
  escapes, empty strings and string/key contents remain intact.
- A conversion which does not produce a finite binary64 value fails; there is
  no fallback that silently returns the retained decoder's damaged value.

The JSON facade's `try_parse`, `parse_or`, `is_valid`, `parse_lines` and
`parse_lines_limit` use this path. `serialization.parse_safe(...,"json")` and
`serialization.parse_format(...,"json")` also use it. The serialization scanner's
string-only key decoder remains native: it does not convert numeric values.
Its size/depth/item budgets, duplicate-key guard and structured error contracts
remain in place. Other serialization formats are unchanged.

## Explicit exactness

`parse_exact(text)` is a **separate opt-in API** on both modules. It returns
ordinary numbers only if each surviving numeric value is mathematically exactly
representable as binary64; otherwise it throws a fixed precision error. Thus
`0.5`, `1.25e2` and `9007199254740992` are exact, while `0.1`,
`9007199254740993` and nonzero values rounded down to zero are rejected.
It does not change ordinary `parse`, the serialization defaults or MCP behavior.

Exact checking is bounded before constructing exact rational intermediates:
1024 characters per numeric token and an exponent in -4096..4096. Exceeding either
bound is an explicit failure, including zero with an out-of-policy spelling.
The exactness walk follows native last-wins object construction: a valid rounded
value overwritten by another value is not checked, but syntax/overflow in any
original token still fails the initial native validation.

## Implementation and integration

The production implementation is PBLang, in one standalone module with **no
module imports and no I/O**. It uses the installed host's `json_parse`, fixed
`regex_replace_all`, `regex_match`, `parse_number` and ordinary type/string/array/map primitives.
The optional exact mode additionally uses existing `rational`, `bigint` and
`num_pow` primitives. No native binary or Rust code is changed.

After original validation, ordinary parsing removes complete quoted strings
from a temporary text copy and checks the remaining numeric spellings. If all
numbers are integer tokens of at most 15 digits (with negative zero excluded),
the original parsed tree is reused: those values are exactly representable and
the retained integer conversion is unaffected. Strings and escaped keys cannot
influence this classification. Decimals, exponents, >=16 digit integers and
negative zero retain the lexical-conversion path; strict mode also uses it.
This avoids building a second tree for the densest small-integer inputs.

On the lexical-conversion path, the first tree is released, and one fixed regular expression matches complete JSON
strings or number tokens. It encodes strings as `"payload,"` and numbers as
`",lexeme"`; a JSON number cannot end with a comma. Both object keys and string
values receive the reversible string encoding, so user strings, commas, numeric
text, escape sequences and operator-shaped keys cannot collide with markers.
The host parses this numeric-free document, and a PBLang walk restores strings,
keys and numbers. Arrays are private to this parse and are restored in place;
maps are rebuilt to avoid key-renaming collisions. Within an array, tagged string
and number leaves are restored directly, without a recursive call for each leaf.
An immediately repeated tagged scalar reuses the preceding restored immutable
value. The comparison uses the exact tagged string, never numeric equality: `0`
and `-0`, or distinct decimal spellings, cannot be conflated. No arrays or objects
are reused. A non-string tagged value breaks the run, so a nested value cannot
change strict-error ordering through a stale reused result. All tokens were
already admitted by the complete original validation before this optimization.

This is a one-token reference per active array traversal, released with that
traversal; it is not a growing dictionary, global cache or cross-request state.
Strings are immutable, and numeric values are immutable binary64 values. The
first occurrence still performs all finite-number and optional exactness checks;
their failure flags remain sticky for the parse. Caller objects are not used
or mutated. Original host validation rejects numeric object keys before quoting
could admit them, and rejects overflowing overwritten duplicate values.

The ordinary path adds a fixed classification pass. The lexical-conversion
path additionally performs a regex pass, a second structural decode and a value
walk. Temporary memory is proportional to input/tree size; dense arrays can
cost more than equally sized strings. These are not VM instruction-budgeted
steps. **The standalone parser adds no input-byte, item or execution deadline
policy**: the embedding caller must enforce those, or use the existing bounded
serialization API. See the source-hidden test driver's `scale` group for 2 MiB
acceptance timings; these are measured samples, not universal latency promises.

For a bundled runtime, copy and pin both `stdlib/json.sapp` and
`stdlib/json_decode.sapp`, refresh the stdlib dependency lock and release hashes,
and call the facade explicitly. Direct retained `json_parse(...)`, internal
native JSON consumers, and already installed product copies remain unchanged.
This does not add JSON builtins to the self-host VM registry or change the known
retained nested `p:run()` behavior. MCP transport integration belongs to its
separate owner and requires its own wire tests and installation qualification.

## Verification

- `tests/semantics/json_numbers_test.sapp`: spellings, adjacent values, strict
  mode, escaped/duplicate/operator-shaped keys, JSON Lines and bounded facade.
- `tests/tooling/json_number_roundtrip.py --group numbers`: 512 deterministic
  binary64 byte oracles (Python `float` + `struct.pack`) and 512 roundtrips.
- `--group diagnostics --start N --count M`: exact original error-message parity
  for malformed input, overflow, invalid numeric keys and duplicate overflow.
- `--group scale --start N --count 1`: individual source-hidden workloads near
  or at 2 MiB, each with a 30-second process deadline. The process report records
  actual byte counts, elapsed time and failures.
- `--group arrays`: 512 independent numeric spellings, each checked six times
  across repeated entries and nested array boundaries (3,072 byte comparisons).
- `--group exact-diagnostics`: repeated strict failures and diagnostic ordering
  across intervening strings, nulls, booleans, arrays and objects.
- `tests/tooling/json_number_cost.py --baseline OLD_RELEASE --candidate NEW_RELEASE
  --report cost.json --start N --count 1`: five alternating paired samples. Each
  fresh isolated process warms up once, times one decode and then validates the
  complete output against Python's binary64 conversion, including signed zero.
  The first fixture is the unchanged cycle11 65,536-cell fractional MCP request
  (plus its 256 query values). This harness measures the parser only, not MCP
  transport. Other fixtures include unique/alternating decimals, repeated boundary
  numbers, strict mode, dense integers and a small mixed object. Timings and RSS
  are separate: RSS includes warmup and output serialization; results are samples,
  not universal speed or memory guarantees.
- Existing JSON and serialization regression suites remain required.

Run the executable `examples/json_numbers/main.sapp` for the corrected facade
and explicit strict-mode distinction. Release qualification reports state the
actual passed checks, timings, supported host and remaining native limitations.
