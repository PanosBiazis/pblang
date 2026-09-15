# ORM query construction and ORDER BY contract

`stdlib:orm.sapp` keeps the existing insert, upsert and find APIs. This update
improves construction of wide INSERT/UPSERT statements and enforces the
already documented restricted ordering grammar before dispatching a query.
See `examples/orm_owned.sapp` for composition with optional SQLite cleanup.

## Ordering

`find(conn, table, opts)` accepts `opts.order` as a comma-separated list of
ASCII identifiers, optionally followed by one ASCII space and `ASC` or `DESC`
(case-insensitive). Each identifier uses the existing `[A-Za-z_][A-Za-z0-9_]*`
grammar and existing 64-character bound. Outer whitespace around each comma
part is trimmed for validation; the original accepted SQL text is preserved.
Examples: `name`, `name DESC`, `name asc, id DESC`.

Each part must have exactly one or two tokens. Empty parts, repeated internal
spaces, internal tabs, expressions, comments, semicolons, extra keywords,
`COLLATE`, `NULLS FIRST/LAST`, extra LIMIT/OFFSET text and non-string values
are rejected with the fixed `orm: invalid order clause` error. An absent/nil
or empty-string order option still means no ORDER BY clause.

This intentionally tightens a security failure: previously `id ASC --` passed
validation and could comment out a subsequently appended LIMIT; `id ASC LIMIT
0` and arbitrary extra tokens after ASC/DESC were also passed through. Invalid
ordering now fails before `conn.query` is called, without reflecting the input
in the ordering error. No database call is needed to validate the grammar.

This change does not make arbitrary SQL safe. `where_clause`/`opts.where`,
`define_model` column definitions, and `raw_query`/`raw_exec` are raw,
developer-authored SQL boundaries. Bind all external *values* through `params`.
For supported flat AND filters, use the optional
[structured predicate compiler](SQL_PREDICATES.md). More complex external
requests still need a structured API; never pass arbitrary external SQL
fragments. Placeholders cannot protect identifiers, clauses or SQL keywords.

## Wide insert/upsert construction

Both methods still validate the table and all column identifiers first and
retain the same exact SQL spacing, value order, row-ID result and single exec
call. The zero-column case retains its old generated SQL; this is compatibility,
not a claim that SQLite accepts an empty INSERT column list.

For up to eight keys the existing simple concatenation loop remains. Above
eight, column names are joined once; values are read through the same key
snapshot, and that private array is reused as placeholder markers. A second
join produces `?, ?, ?`. The input map is never changed. Eight is an internal
algorithm-selection threshold, not a limit on row width or project size.

The array returned by `map_keys` is independent of the map on the examined
retained, self-host and C99 routes. This avoids a second marker array and
repeatedly copying every growing SQL prefix. No `str_repeat` or
`str_replace_all` cap is introduced. All output is still materialized in RAM;
untrusted row widths require a caller resource policy. Existing C99 unchecked
allocation/overflow handling remains a backend limitation. Engine-specific
column/parameter limits are independent of the builder.

Benchmarks and validation are documented in
[ORM validation](ORM_VALIDATION_2026-09-15.md). No zero-overhead, universal
backend safety or whole-project speedup claim is made.
