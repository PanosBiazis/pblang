# Resource capacity and translation review

Two pure PBLang libraries extend existing interval and literal-template contracts.
They do not access files, servers, calendars, translators or physical devices.

## Weighted resource capacity

`resource_capacity.analyze(resources, bookings, window)` accepts:

- 1–32 exact `{id, capacity}` records. IDs are unique nonempty strings, at most
  64 characters and 128 UTF-8 bytes, without NUL.
- At most 128 exact `{id, resource, start_ms, end_ms, units}` bookings. Booking
  IDs are globally unique, resource references must exist, and each interval
  must fit wholly inside the explicit window.
- An exact `{start_ms, end_ms}` window, validated by `interval_analysis`.
  Times are integer milliseconds from 0 through 359999999; start is less than end.
- Capacities and requested units are integers from 1 through 10000. Larger
  requests than capacity are valid inputs: they produce shortage diagnostics.

Intervals are half-open: an ending booking does not overlap a booking starting
at the same instant. Every resource has a complete partition of the window,
including idle intervals, with load, excess, and active booking IDs. Reports
include peak units, available and used unit-milliseconds, overload milliseconds,
and shortfall unit-milliseconds. Spare capacity of another resource is not
substituted. Limits keep the maximum integral below the exact-integer range of
the numeric runtime. This is capacity accounting, not optimization, automatic
scheduling, timezone handling or a prediction of real resource availability.

## Translation contracts

`translation_review.review(source, translated)` accepts arrays of exact
`{id, text}` records. Each side permits at most 128 messages and 131072 UTF-8
bytes of text. IDs follow `template_fields.valid_name`: ASCII letter or underscore
followed by letters, digits or underscores, at most 32 characters; duplicates fail.

Messages reuse `template_fields.compile`: text up to 4096 bytes, at most 64
literal `{{name}}` occurrences, without template expressions, dotted fields,
filters, directives or comments. This is not a gettext/ICU/printf parser.

Reports distinguish missing and extra IDs, missing and extra field names,
changed occurrence counts, and empty translations. Field order may change.
Repeated occurrences must retain their count to pass this strict review; this
does not establish linguistic quality, and a translator may intentionally choose
a different wording that needs human review. `ok` is a contract result, not a
translation-quality score. Source ordering is preserved in matched-message
diagnostics. Unknown fields, malformed records and unsupported grammar throw
before any report is returned. No text is rendered or evaluated by this library.

Both libraries return detached results and preserve their inputs. Reports retain
caller-provided identifiers; translation reports also retain field names but omit
message text. Callers remain responsible for identifier privacy and output escaping.

## Examples and checks

```text
pblang run examples/discovery_capacity_recipes.sapp
pblang run examples/discovery_translation_recipes.sapp
pblang run tests/semantics/resource_capacity_test.sapp
pblang run tests/semantics/translation_review_test.sapp
```

The capacity tests include an independent discrete-time oracle, weighted/nested
intervals, simultaneous starts and ends, idle time, malformed inputs and maximum
counts. Translation tests cover Greek messages, reordered fields, repetition
counts, missing/extra IDs, privacy of message text, malformed syntax, and exact
record/byte/field limits. Examples use synthetic data only. No native/web/mobile
product editor or physical cross-platform parity is claimed.

Ideas were informed by the local discovery catalogue and selected existing
gettext, Symfony translation, and scheduling documentation. Their code was not
copied and no third-party dependency was introduced. Existing PBLang
`interval_analysis` and `template_fields` implementations are reused.
