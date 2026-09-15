# Structured SQLite predicates

`stdlib:sql_predicate.sapp` provides the optional pure function
`compile_all(terms)`. It constructs a WHERE expression and positional parameters;
it does not open a database, execute SQL, install extensions or authorize access.
Existing ORM/raw-SQL APIs are unchanged and do not import the helper implicitly.

```sapp
let P = import("stdlib:sql_predicate.sapp");
let p = P.compile_all([["age", "ge", 18], ["note", "eq", nil]]);
// p.sql: ([age] >= ?) AND ([note] IS NULL)
// p.params: [18]
// ORM.find(conn, "items", map{where:p.sql, params:p.params});
```

See `examples/sql_predicate.sapp` for a working in-memory example that combines
this helper with ORM and automatic SQLite connection cleanup.

## Supported input

The outer value must be a nonempty array of exact three-element arrays:
`[column, operator, value]`. Every term is parenthesized and joined with AND.
There is no nesting, OR, general NOT, raw expression, qualified column or
execution helper. Membership uses the explicit `in` and `not_in` operators. Empty filters are rejected instead of silently
producing an unconditional query. Intentional unfiltered queries remain an
explicit choice through existing APIs.

Columns use the existing ORM-compatible ASCII identifier syntax
`[A-Za-z_][A-Za-z0-9_]*`, 1–64 characters, then are bracket-quoted. Quoting also
allows SQL keywords as column names. Brackets avoid SQLite's legacy double-quoted
string fallback, so an unknown column fails instead of becoming constant text. The identifier bound is part of this
optional identifier contract, not a compiler/source/project-size limit.

| Exact lowercase operator | SQL | Value |
| --- | --- | --- |
| `eq`, `ne` | `=`, `<>` | finite number, string, bool; nil uses IS NULL / IS NOT NULL |
| `lt`, `le`, `gt`, `ge` | `<`, `<=`, `>`, `>=` | finite number, string or bool |
| `like` | LIKE | string, retaining SQL `%` and `_` wildcard semantics |
| `in`, `not_in` | IN, NOT IN | flat array of finite numbers, strings, bools or nil; may be empty |

For scalar comparisons, every non-nil value is appended to a fresh parameter array in input order and
never interpolated into SQL. Nil adds no parameter. NaN and both infinities,
compound/function values, malformed terms, unknown operators and identifiers
outside the grammar are rejected. Finite REAL values are not restricted to
the exact-integer range; no arbitrary magnitude cap is added. Existing SQLite
binding represents bool as 1/0. Comparison coercion/collation follows SQLite.

Failures use only fixed messages: `sql predicate: invalid terms`, `invalid term`,
`invalid column`, `invalid operator`, or `invalid value` (each with the same
`sql predicate: ` prefix). Invalid input is not embedded in errors. Parameters
can contain private values: callers should not log them indiscriminately.

## Membership, empty lists and NULL

```sapp
let p = P.compile_all([["status", "in", ["ready", "queued"]], ["owner", "not_in", []]]);
// p.sql: ([status] IN (?, ?)) AND ([owner] NOT IN (SELECT 1 WHERE 0))
// p.params: ["ready", "queued"]
```

Each member, including nil, gets one positional parameter in term/member order.
The caller's array is copied into the result's parameter array; duplicate values
are preserved. Lists never accept raw SQL, nested arrays, maps or functions.
Markers are joined once; no database module is imported by the pure helper.

Empty membership is written as `IN (SELECT 1 WHERE 0)` or its `NOT IN`
counterpart. This retains identifier resolution: an unknown column still fails.
**IN empty is false; NOT IN empty is true even for a NULL column**, so an empty
exclusion list excludes no rows. This does not change rejection of an empty
outer filter. Callers needing different product behavior must choose it explicitly.

A nonempty list containing nil follows SQL three-valued logic, unlike `eq nil`:
`x IN (1, NULL)` matches x=1, but yields NULL for a nonmatch. `NOT IN (1, NULL)`
yields NULL for a nonmatch too, so it does not select those rows. A NULL left
operand with a nonempty list yields NULL; WHERE does not select NULL results.
Matching NULL requires `eq nil`; combining alternatives with OR is not supported.
These semantics follow [SQLite's IN/NOT IN matrix](https://www.sqlite.org/lang_expr.html#the_in_and_not_in_operators).

## Lifetime, resources and trust

Input arrays are not changed. Successful results contain a newly allocated
map and parameter array; primitive values are copied by their existing language
semantics. Each call is independent. No mutable compound values are accepted.
A late invalid member or term discards locally assembled output and returns no result;
there are no external effects to roll back. A membership array is inspected one level only. Cycles/nested member arrays
fail the flat shape/type checks without recursive traversal.

There is no new term-count, membership-count or string-size cap. Pieces are joined once, avoiding
repeatedly copying a growing SQL prefix. Work and output memory grow with the
terms and membership values; a host accepting untrusted requests must set its own resource policy.
This does not enforce CPU/heap limits, protect against arbitrary OOM, or repair
existing C99 allocation/overflow behavior. SQLite also has independent
expression-depth and parameter-count limits: compiling 4,096 terms does not
mean an engine accepts that query.

The mutable `{sql, params}` result is ordinary data, not an authorization,
row-level-access or tenant-isolation token. Callers can still bypass it through
raw ORM APIs or alter returned fields. Only use trusted code for assembly of
the final query, including table, order, limit and raw clauses. The `?` dialect
and execution tests apply to SQLite/current ORM; other SQL dialects, backends
and guest OSes are not certified by these tests.

Measured construction costs and test coverage are in
[validation](SQL_PREDICATE_VALIDATION_2026-09-15.md).

Membership validation and same-run scalar overhead measurements are in
[membership validation](SQL_MEMBERSHIP_VALIDATION_2026-09-15.md).

## Adjacent column reuse

Within one `compile_all` call, consecutive terms naming the same column reuse
its validated bracket-quoted spelling. This is a single-entry local cache;
alternating or unique columns are validated normally. No values, query results,
errors or cross-call state are cached, and term/operator/value validation still
runs for each term. SQL and parameter order are unchanged.

A type check precedes the name comparison: PBLang maps may overload equality,
so comparing an unvalidated compound name could invoke caller code. Nonstrings
always take the existing fixed-error validation path. The cache is assigned
only after successful identifier validation. It cannot certify a database
schema; unknown columns still fail when SQLite resolves the query.

There is no new source/project/filter limit. The cache retains at most one
validated name and quoted spelling per call, independent of distinct-column
count. It helps repeated columns but adds checking work to misses; measured
tradeoffs and reproducibility are in [column reuse validation](SQL_COLUMN_REUSE_VALIDATION_2026-09-15.md).
