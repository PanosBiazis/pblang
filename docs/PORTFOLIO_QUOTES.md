# Estimates and customer document interchange

This slice adds PBQuote 0.1.0-alpha and PBOffer one-unit estimate export. It follows
`OFFER_PLAN.md`, the migration catalog budgeting/reporting ideas and the existing
PBOffice document workflow. Code is newly authored; no new private-file scan,
training, vendor-code ingestion or payment collection occurred. The existing
quote/offer modules are now included in the private PBLang source repository
to support a reproducible standalone PBQuote checkout.

## Canonical model

`quote_document.sapp` owns `pblang.quote-document.v1`. Exactly five root fields:
format, title, currency, notes, items. Each item has exactly description, quantity,
unit_cents. Title and description are nonblank single-line text up to 256 UTF-8
bytes each. Notes are up to 2048 bytes and 64 LF-separated lines. C0 controls
except LF in notes and DEL are rejected; normalize CRLF externally if needed.
All text remains literal. Denomination is exactly EUR/USD/GBP; there is no FX.

One to sixteen rows, quantity 1..100000, unit cents 0..100000000. Numeric values
must be finite ordinary integers; booleans and numeric strings are rejected.
The maximum subtotal is 160000000000000 cents, below binary64's exact-integer
limit. Calculation visits items once and never loops over quantities.

Native JSON is at most 64 KiB with depth 5 and item budget 256. Duplicate/unknown
keys, trailing data, wrong types and unsupported versions are rejected. `copy`
returns detached line items. CSV uses the existing bounded three-column literal
Sheet reader/writer; no header and no spreadsheet formula evaluation. Prices
reuse offer_plan's strict unsigned decimal parser and money's exact formatting.

`to_document` produces a supported PBOffice Writer document: explicit estimate
notice, line calculations, subtotal and notes. Notes split into <=1024-byte
paragraphs at character boundaries; native quote notes are unchanged. Output
contains no tax amount, customer identity inference or payment instructions.
`from_offer` validates the plan but copies only a selected price into one unit
with explicit description/title. Forecast units and all internal assumptions
remain in PBOffer.

## Files and views

`quote_files.sapp` composes existing secure_file new-exclusive operations with
strict extensions, bounded decode and prevalidated encoding. Native quote,
Writer and plain-text copies never overwrite an existing path. Failed writes
may leave partial new copies; atomic visibility is explicitly false. Fixed
errors do not echo raw document contents or file paths.

`quote_view.sapp` wraps into 28-column, ten-line pages. A page updates one body
and its position using three action nodes including the sequence wrapper; this
keeps admitted maximum documents within Modern UI's 128-action limit. The body
is readonly and disabled so Android's existing enabled contract also prevents
editing. Page buttons remain enabled. Display normalization and the existing
text-layout cluster model do not change persisted text; full UAX29, bidirectional
shaping and complex-script glyph parity are not claimed.

Native/CLI support editing and files. Portable readers support page selection,
with no browser/mobile persistence. Generated Android records are not a tested
APK. Native Windows/macOS, physical Android and packaging remain unverified.

## Verification

Focused tests cover exact fractional sums, maxima, malformed input, UTF-8 notes,
strict round trips, one-unit export, detached copies, portable generation and
native reader actions. Product tests cover layout bounds, pending typing, failed
Apply/Open preservation, Undo/Redo, close guards, baseline tracking, exclusive copies
and PBOffer -> PBQuote -> PBOffice interchange. Validation uses an isolated source
snapshot because other active tasks also change the shared stdlib; it does not
bypass dependency hashes. The retained runtime executes the tests. The known
self-hosted checker range-arity issue is not repaired or claimed passed here.
