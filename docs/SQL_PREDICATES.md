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
There is no nesting, OR, IN, NOT, raw expression, qualified column or execution
helper in this first contract. Empty filters are rejected instead of silently
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

Every non-nil value is appended to a fresh parameter array in input order and
never interpolated into SQL. Nil adds no parameter. NaN and both infinities,
compound/function values, malformed terms, unknown operators and identifiers
outside the grammar are rejected. Finite REAL values are not restricted to
the exact-integer range; no arbitrary magnitude cap is added. Existing SQLite
binding represents bool as 1/0. Comparison coercion/collation follows SQLite.

Failures use only fixed messages: `sql predicate: invalid terms`, `invalid term`,
`invalid column`, `invalid operator`, or `invalid value` (each with the same
`sql predicate: ` prefix). Invalid input is not embedded in errors. Parameters
can contain private values: callers should not log them indiscriminately.

## Lifetime, resources and trust

Input arrays are not changed. Successful results contain a newly allocated
map and parameter array; primitive values are copied by their existing language
semantics. Each call is independent. No mutable compound values are accepted.
A late invalid term discards locally assembled output and returns no result;
there are no external effects to roll back. Cycles/nested arrays fail the flat
shape/type checks without recursive traversal.

There is no new term-count or string-size cap. Pieces are joined once, avoiding
repeatedly copying a growing SQL prefix. Work and output memory grow with the
terms; a host accepting untrusted requests must set its own resource policy.
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
